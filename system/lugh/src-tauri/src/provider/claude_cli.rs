// provider/claude_cli.rs — ClaudeCliAdapter
// `claude --print` CLI 서브프로세스를 통해 메시지를 전송한다.
// OAuth 직접 API rate_limit 우회용 폴백 어댑터.
// CLI 응답 전체를 받은 뒤 청크 단위 emit으로 타이핑 효과를 재현한다.
// DS-40 §4 (CLI 경로) — #23: stream-json content 배열을 통한 이미지(vision) 지원 포함

use async_trait::async_trait;
use serde_json::Value;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::Command;
use uuid::Uuid;
use std::process::Stdio;

use crate::chat_attachment::document_block_text;
use crate::models::{
    error::AppError,
    provider::{
        AiProviderKind, CredentialRef, HealthStatus, ProviderContentBlock, ProviderEvent,
        ProviderHealth, ProviderMessage, ProviderMessageRequest, ProviderMessageResult,
        ProviderSessionRef, MessageRole,
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

    /// CLI 전송용 메시지 content 필드 구성 (#21, #23 DS-40 §7.3)
    /// content_blocks가 있으면 claude.rs::message_content와 동일한 Claude Messages API
    /// content block 배열 스키마로 직렬화한다 — 텍스트 → {type:text}, 이미지 →
    /// {type:image, source:{type:base64, media_type, data}}, 문서 첨부는 추출 텍스트를
    /// text block으로 병합. 이 스키마는 #23 1단계 실증(claude --print --input-format
    /// stream-json)에서 이미지 인식이 정상 동작함을 확인한 형식이다.
    fn message_content(msg: &ProviderMessage) -> Value {
        match &msg.content_blocks {
            Some(blocks) if !blocks.is_empty() => {
                let arr: Vec<Value> = blocks
                    .iter()
                    .map(|block| match block {
                        ProviderContentBlock::Text { text } => {
                            serde_json::json!({ "type": "text", "text": text })
                        }
                        ProviderContentBlock::Image { media_type, base64_data, .. } => {
                            serde_json::json!({
                                "type": "image",
                                "source": {
                                    "type": "base64",
                                    "media_type": media_type,
                                    "data": base64_data,
                                }
                            })
                        }
                        ProviderContentBlock::DocumentText {
                            filename, media_type, extracted_text, ..
                        } => serde_json::json!({
                            "type": "text",
                            "text": document_block_text(filename, media_type, extracted_text),
                        }),
                    })
                    .collect();
                Value::Array(arr)
            }
            _ => Value::String(msg.content.clone()),
        }
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
                    "content": Self::message_content(msg)
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
    /// Claude CLI shell provider도 stream-json content 배열로 이미지 전달 지원 (#23)
    /// 1단계 실증에서 `claude --print --input-format stream-json` stdin에 Anthropic
    /// Messages API와 동일한 image content block을 태우면 CLI가 이를 파싱해 모델에
    /// 전달하고, 실제 이미지 내용을 인식한 응답이 돌아옴을 확인했다 (DS-40 §7.3 갱신 필요).
    fn supports_vision(&self) -> bool {
        true
    }

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::provider::AiProviderKind;

    fn base_request(messages: Vec<ProviderMessage>) -> ProviderMessageRequest {
        ProviderMessageRequest {
            session_id: "s1".into(),
            provider: AiProviderKind::Claude,
            model: "claude-sonnet-4-5-20250929".into(),
            system_prompt: "system".into(),
            messages,
            attachments: vec![],
            temperature: None,
            max_tokens: None,
            tools: vec![],
        }
    }

    #[test]
    fn test_supports_vision_true() {
        // #23: CLI shell provider도 stream-json content 배열로 이미지 지원 (1단계 실증 확인)
        let adapter = ClaudeCliAdapter::new();
        assert!(adapter.supports_vision());
    }

    #[test]
    fn test_build_stdin_plain_text_backward_compat() {
        let req = base_request(vec![ProviderMessage {
            role: MessageRole::User,
            content: "안녕".into(),
            content_blocks: None,
        }]);
        let stdin = ClaudeCliAdapter::build_stdin(&req);
        assert!(stdin.contains("안녕"));
        assert!(stdin.contains("\"role\":\"user\""));
    }

    #[test]
    fn test_build_stdin_merges_document_text_into_prompt() {
        // #21: 문서 첨부는 추출 텍스트만 프롬프트에 병합 (DS-40 §7.3)
        let req = base_request(vec![ProviderMessage {
            role: MessageRole::User,
            content: "검토 부탁".into(),
            content_blocks: Some(vec![
                ProviderContentBlock::Text { text: "검토 부탁".into() },
                ProviderContentBlock::DocumentText {
                    attachment_id: "att-1".into(),
                    filename: "spec.md".into(),
                    media_type: "text/markdown".into(),
                    extracted_text: "# 명세 내용".into(),
                    truncated: false,
                },
            ]),
        }]);
        let stdin = ClaudeCliAdapter::build_stdin(&req);
        assert!(stdin.contains("검토 부탁"));
        assert!(stdin.contains("[첨부 문서: spec.md, text/markdown]"));
        assert!(stdin.contains("# 명세 내용"));
    }

    #[test]
    fn test_build_stdin_plain_text_content_is_json_string() {
        // 하위 호환: content_blocks 없으면 content 필드는 여전히 순수 문자열이어야 한다
        let req = base_request(vec![ProviderMessage {
            role: MessageRole::User,
            content: "안녕".into(),
            content_blocks: None,
        }]);
        let stdin = ClaudeCliAdapter::build_stdin(&req);
        let line = stdin.lines().next().expect("stdin에 최소 한 줄 있어야 함");
        let val: Value = serde_json::from_str(line).expect("유효한 JSON 라인이어야 함");
        assert!(val["message"]["content"].is_string());
        assert_eq!(val["message"]["content"], "안녕");
    }

    #[test]
    fn test_build_stdin_serializes_image_content_block() {
        // #23: content_blocks에 Image가 있으면 Claude Messages API와 동일한
        // {type:image, source:{type:base64, media_type, data}} 스키마로 직렬화된다.
        // 이 정확한 스키마로 1단계 실증(claude --print --input-format stream-json)에서
        // CLI가 이미지를 실제로 인식함을 확인했다.
        let req = base_request(vec![ProviderMessage {
            role: MessageRole::User,
            content: "이 이미지 봐줘".into(),
            content_blocks: Some(vec![
                ProviderContentBlock::Text { text: "이 이미지 봐줘".into() },
                ProviderContentBlock::Image {
                    attachment_id: "att-img-1".into(),
                    media_type: "image/png".into(),
                    base64_data: "iVBORw0KGgo=".into(),
                },
            ]),
        }]);
        let stdin = ClaudeCliAdapter::build_stdin(&req);
        let line = stdin.lines().next().expect("stdin에 최소 한 줄 있어야 함");
        let val: Value = serde_json::from_str(line).expect("유효한 JSON 라인이어야 함");
        let content = val["message"]["content"]
            .as_array()
            .expect("content_blocks 있으면 content는 배열이어야 함");
        assert_eq!(content.len(), 2);
        assert_eq!(content[0]["type"], "text");
        assert_eq!(content[0]["text"], "이 이미지 봐줘");
        assert_eq!(content[1]["type"], "image");
        assert_eq!(content[1]["source"]["type"], "base64");
        assert_eq!(content[1]["source"]["media_type"], "image/png");
        assert_eq!(content[1]["source"]["data"], "iVBORw0KGgo=");
    }

    #[test]
    fn test_build_stdin_multiturn_history_with_image_in_last_turn() {
        // 멀티턴 히스토리 + 마지막 turn에만 이미지 첨부가 있는 케이스 하위호환 확인
        let req = base_request(vec![
            ProviderMessage {
                role: MessageRole::User,
                content: "안녕".into(),
                content_blocks: None,
            },
            ProviderMessage {
                role: MessageRole::Assistant,
                content: "안녕하세요".into(),
                content_blocks: None,
            },
            ProviderMessage {
                role: MessageRole::User,
                content: "이거 봐줘".into(),
                content_blocks: Some(vec![
                    ProviderContentBlock::Text { text: "이거 봐줘".into() },
                    ProviderContentBlock::Image {
                        attachment_id: "att-img-2".into(),
                        media_type: "image/jpeg".into(),
                        base64_data: "/9j/4AAQ".into(),
                    },
                ]),
            },
        ]);
        let stdin = ClaudeCliAdapter::build_stdin(&req);
        let lines: Vec<&str> = stdin.lines().collect();
        assert_eq!(lines.len(), 3, "system 없는 3턴 모두 stdin에 포함되어야 함");

        let turn1: Value = serde_json::from_str(lines[0]).unwrap();
        assert!(turn1["message"]["content"].is_string());
        assert_eq!(turn1["message"]["role"], "user");

        let turn3: Value = serde_json::from_str(lines[2]).unwrap();
        let content3 = turn3["message"]["content"].as_array().unwrap();
        assert_eq!(content3[1]["source"]["media_type"], "image/jpeg");
    }
}
