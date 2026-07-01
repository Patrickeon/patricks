// provider/claude_cli.rs — ClaudeCliAdapter
// `claude --print` CLI 서브프로세스를 통해 메시지를 전송한다.
// OAuth 직접 API rate_limit 우회용 폴백 어댑터.
// CLI 응답 전체를 받은 뒤 청크 단위 emit으로 타이핑 효과를 재현한다.
// DS-40 §4 (CLI 경로)

use async_trait::async_trait;
use serde_json::Value;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::Command;
use uuid::Uuid;
use std::process::Stdio;

use crate::models::{
    error::AppError,
    provider::{
        AiProviderKind, CredentialRef, HealthStatus, ProviderEvent, ProviderHealth,
        ProviderMessageRequest, ProviderMessageResult, ProviderSessionRef, MessageRole,
    },
};
use super::{AiProvider, ProviderEventSink};

/// 타이핑 효과 파라미터
/// 3글자씩 18ms 간격 → 약 167 chars/sec (자연스러운 AI 스트리밍 속도)
const CHUNK_SIZE: usize = 3;
const CHUNK_DELAY_MS: u64 = 18;

/// Claude CLI 서브프로세스 어댑터
/// `claude --print --input-format stream-json --output-format stream-json --verbose` 를 사용한다.
pub struct ClaudeCliAdapter;

impl ClaudeCliAdapter {
    pub fn new() -> Self {
        Self
    }

    /// stream-json stdin 입력 문자열 생성 (멀티턴 대화 히스토리 포함)
    fn build_stdin(request: &ProviderMessageRequest) -> String {
        let mut buf = String::new();
        for msg in &request.messages {
            let role_str = match msg.role {
                MessageRole::User      => "user",
                MessageRole::Assistant => "assistant",
                MessageRole::System    => continue, // system은 --system-prompt 플래그로 처리
            };
            let line = serde_json::json!({
                "type": role_str,
                "message": {
                    "role": role_str,
                    "content": msg.content
                }
            });
            buf.push_str(&line.to_string());
            buf.push('\n');
        }
        buf
    }
}

#[async_trait]
impl AiProvider for ClaudeCliAdapter {
    async fn validate_credential(&self, _credential: CredentialRef) -> Result<ProviderHealth, AppError> {
        // CLI 경로에서는 credential 검증 불필요 — claude CLI 자체 auth 사용
        Ok(ProviderHealth {
            provider: AiProviderKind::Claude,
            status: HealthStatus::Ok,
            latency_ms: None,
            checked_at: chrono::Utc::now(),
            error: None,
        })
    }

    async fn start_session(
        &self,
        request: &ProviderMessageRequest,
    ) -> Result<ProviderSessionRef, AppError> {
        Ok(ProviderSessionRef {
            session_id: request.session_id.clone(),
            provider: AiProviderKind::Claude,
            model: request.model.clone(),
        })
    }

    async fn send_message_stream(
        &self,
        request: ProviderMessageRequest,
        sink: ProviderEventSink,
    ) -> Result<ProviderMessageResult, AppError> {
        let message_id = Uuid::new_v4().to_string();

        // ── Phase 1: CLI 실행 및 전체 응답 수집 ──────────────────

        let stdin_input = Self::build_stdin(&request);

        let mut cmd = Command::new("claude");
        cmd.args([
            "--print",
            "--input-format",  "stream-json",
            "--output-format", "stream-json",
            "--verbose",
            "--model", &request.model,
            "--dangerously-skip-permissions",
        ]);

        if !request.system_prompt.is_empty() {
            cmd.args(["--system-prompt", &request.system_prompt]);
        }

        cmd.stdin(Stdio::piped())
           .stdout(Stdio::piped())
           .stderr(Stdio::null());

        let mut child = cmd.spawn().map_err(|e| {
            AppError::new("CLI_SPAWN_ERROR", format!("claude CLI 실행 실패: {}", e))
        })?;

        // stdin 쓰기 후 닫기 (EOF)
        if let Some(mut stdin) = child.stdin.take() {
            stdin.write_all(stdin_input.as_bytes()).await.map_err(|e| {
                AppError::new("CLI_STDIN_ERROR", format!("stdin 쓰기 실패: {}", e))
            })?;
        }

        let stdout = child.stdout.take().ok_or_else(|| {
            AppError::new("CLI_STDOUT_ERROR", "stdout 핸들 없음".to_string())
        })?;

        let mut reader = BufReader::new(stdout).lines();
        let mut full_content = String::new();

        // stdout JSON 라인 순회 — result 이벤트에서 최종 텍스트 추출
        while let Ok(Some(line)) = reader.next_line().await {
            let line = line.trim().to_string();
            if line.is_empty() { continue; }

            let Ok(val): Result<Value, _> = serde_json::from_str(&line) else { continue; };

            let event_type = val.get("type").and_then(|t| t.as_str()).unwrap_or("");
            let subtype    = val.get("subtype").and_then(|s| s.as_str()).unwrap_or("");

            match (event_type, subtype) {
                ("result", "success") => {
                    // 최종 응답 텍스트
                    full_content = val.get("result")
                        .and_then(|r| r.as_str())
                        .unwrap_or("")
                        .to_string();
                }
                ("result", _) => {
                    // error subtype 또는 기타 실패
                    let err_msg = val.get("result")
                        .and_then(|r| r.as_str())
                        .unwrap_or("CLI 오류")
                        .to_string();
                    let _ = child.kill().await;
                    return Err(AppError::new("CLI_RESULT_ERROR", err_msg));
                }
                _ => {} // system, thinking_tokens, assistant 중간 이벤트 무시
            }
        }

        // 프로세스 종료 대기
        let exit_status = child.wait().await.map_err(|e| {
            AppError::new("CLI_WAIT_ERROR", format!("프로세스 대기 실패: {}", e))
        })?;

        if full_content.is_empty() {
            let code = exit_status.code().unwrap_or(-1);
            return Err(AppError::new(
                "CLI_EMPTY_RESPONSE",
                format!("claude CLI 빈 응답 (exit code: {})", code),
            ));
        }

        // ── Phase 2: 타이핑 효과 스트리밍 ────────────────────────
        // MessageStarted → MessageDelta (청크 단위) → MessageCompleted

        let _ = sink.send(ProviderEvent::MessageStarted {
            message_id: message_id.clone(),
        }).await;

        let chars: Vec<char> = full_content.chars().collect();
        let mut idx = 0usize;
        let mut seq = 0u32;

        while idx < chars.len() {
            let end = (idx + CHUNK_SIZE).min(chars.len());
            let chunk: String = chars[idx..end].iter().collect();

            let _ = sink.send(ProviderEvent::MessageDelta {
                message_id: message_id.clone(),
                delta: chunk,
                sequence: seq,
            }).await;

            seq += 1;
            idx = end;

            tokio::time::sleep(tokio::time::Duration::from_millis(CHUNK_DELAY_MS)).await;
        }

        let _ = sink.send(ProviderEvent::MessageCompleted {
            message_id: message_id.clone(),
            usage: None,
        }).await;

        Ok(ProviderMessageResult {
            message_id,
            content: full_content,
            usage: None,
        })
    }
}
