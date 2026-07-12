// models/provider.rs — AI Provider 관련 모델 (DS-20 §6, DS-40 §3)

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use super::attachment::{AttachmentExtractionStatus, AttachmentKind, AttachmentSource};
use super::error::AppError;

/// 지원 AI Provider 종류 (Redmine은 자격증명 저장 전용)
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AiProviderKind {
    Claude,
    OpenAi,
    Gemini,
    /// Redmine HTTP 클라이언트용 credential 저장 전용 (AI provider 아님)
    Redmine,
}

impl std::fmt::Display for AiProviderKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AiProviderKind::Claude => write!(f, "claude"),
            AiProviderKind::OpenAi => write!(f, "openai"),
            AiProviderKind::Gemini => write!(f, "gemini"),
            AiProviderKind::Redmine => write!(f, "redmine"),
        }
    }
}

impl std::str::FromStr for AiProviderKind {
    type Err = AppError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "claude" => Ok(AiProviderKind::Claude),
            "openai" | "codex" => Ok(AiProviderKind::OpenAi),
            "gemini" => Ok(AiProviderKind::Gemini),
            "redmine" => Ok(AiProviderKind::Redmine),
            _ => Err(AppError::new("UNKNOWN_PROVIDER", format!("알 수 없는 provider: {}", s))),
        }
    }
}

/// Provider Health 상태
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum HealthStatus {
    Ok,
    Degraded,
    Unreachable,
    AuthFailed,
    Unknown,
}

/// Provider 건강 상태 결과
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderHealth {
    pub provider: AiProviderKind,
    pub status: HealthStatus,
    pub latency_ms: Option<u64>,
    pub checked_at: DateTime<Utc>,
    pub error: Option<AppError>,
}

/// OS Credential Vault에서 꺼낸 credential 참조 (secret 미포함)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CredentialRef {
    pub provider: AiProviderKind,
    pub account: String,
    // secret은 포함하지 않음 — Rust backend와 OS vault 사이에서만 이동
}

/// Provider에게 메시지를 보내기 위한 표준 요청 모델
/// `attachments`는 Redmine #21 첨부 메타/본문 목록 — 기본값 빈 배열로 기존 호출 유지 (DS-40 §3.2)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderMessageRequest {
    pub session_id: String,
    pub provider: AiProviderKind,
    pub model: String,
    pub system_prompt: String,
    pub messages: Vec<ProviderMessage>,
    #[serde(default)]
    pub attachments: Vec<ProviderAttachment>,
    pub temperature: Option<f32>,
    pub max_tokens: Option<u32>,
    pub tools: Vec<ToolDefinition>,
}

/// Provider 메시지 단건 (role + content)
/// `content`는 하위 호환/로그 표시용 순수 텍스트, 첨부 포함 신규 호출은
/// `content_blocks`가 provider 변환의 정본 (DS-40 §3.2)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderMessage {
    pub role: MessageRole,
    pub content: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content_blocks: Option<Vec<ProviderContentBlock>>,
}

/// Provider 전송용 content block (DS-40 §3.2 ProviderContentBlock)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ProviderContentBlock {
    Text {
        text: String,
    },
    Image {
        attachment_id: String,
        media_type: String,
        base64_data: String,
    },
    DocumentText {
        attachment_id: String,
        filename: String,
        media_type: String,
        extracted_text: String,
        truncated: bool,
    },
}

/// 첨부 표준 모델 (DS-40 §3.2.1 ProviderAttachment)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderAttachment {
    pub id: String,
    pub kind: AttachmentKind,
    pub filename: String,
    pub media_type: String,
    pub size_bytes: u64,
    pub source: AttachmentSource,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content_base64: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub extracted_text: Option<String>,
    pub extraction_status: AttachmentExtractionStatus,
    pub truncated: bool,
}

/// 메시지 역할
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MessageRole {
    User,
    Assistant,
    System,
}

/// Tool/Function call 정의
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    pub parameters: serde_json::Value,
}

/// Provider 응답의 token 사용량
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenUsage {
    pub input_tokens: Option<u32>,
    pub output_tokens: Option<u32>,
    pub total_tokens: Option<u32>,
}

/// Provider 스트리밍 이벤트 (내부 표준 모델, DS-40 §3.3)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ProviderEvent {
    MessageStarted { message_id: String },
    MessageDelta { message_id: String, delta: String, sequence: u32 },
    ToolRequested { tool_name: String, arguments: serde_json::Value },
    MessageCompleted { message_id: String, usage: Option<TokenUsage> },
    MessageFailed { message_id: String, error: AppError },
}

/// Provider 세션 참조 (provider별 내부 식별자)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderSessionRef {
    pub session_id: String,
    pub provider: AiProviderKind,
    pub model: String,
}

/// send_message_stream 결과
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderMessageResult {
    pub message_id: String,
    pub content: String,
    pub usage: Option<TokenUsage>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_provider_kind_display() {
        assert_eq!(AiProviderKind::Claude.to_string(), "claude");
        assert_eq!(AiProviderKind::OpenAi.to_string(), "openai");
        assert_eq!(AiProviderKind::Gemini.to_string(), "gemini");
    }

    #[test]
    fn test_provider_kind_from_str() {
        use std::str::FromStr;
        assert_eq!(AiProviderKind::from_str("claude").unwrap(), AiProviderKind::Claude);
        assert_eq!(AiProviderKind::from_str("OPENAI").unwrap(), AiProviderKind::OpenAi);
        assert!(AiProviderKind::from_str("unknown").is_err());
    }

    #[test]
    fn test_provider_health_serialization() {
        let health = ProviderHealth {
            provider: AiProviderKind::Claude,
            status: HealthStatus::Ok,
            latency_ms: Some(120),
            checked_at: Utc::now(),
            error: None,
        };
        let json = serde_json::to_string(&health).unwrap();
        assert!(json.contains("claude"));
    }
}
