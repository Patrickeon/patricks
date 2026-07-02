// models/message.rs — AgentMessage, MessagePage (DS-20 §6.4, DS-60 §3.2)

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use super::provider::{MessageRole, TokenUsage};

/// 저장된 에이전트 메시지 (세션 로그)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentMessage {
    pub id: String,
    pub session_id: String,
    pub role: MessageRole,
    pub content: String,
    pub created_at: DateTime<Utc>,
    pub usage: Option<TokenUsage>,
    /// streaming 중인지 여부
    pub is_streaming: bool,
}

/// list_agent_messages 응답 (DS-60 §3.2)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessagePage {
    pub session_id: String,
    pub messages: Vec<AgentMessage>,
    pub next_cursor: Option<String>,
    pub total: u32,
}

/// agent:message_started event payload (DS-60 §4.2)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentMessageStarted {
    pub session_id: String,
    pub message_id: String,
    pub started_at: DateTime<Utc>,
}

/// agent:message_delta event payload (DS-60 §4.2)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentMessageDelta {
    pub session_id: String,
    pub message_id: String,
    pub delta: String,
    pub sequence: u32,
}

/// agent:message_completed event payload (DS-60 §4.2)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentMessageCompleted {
    pub session_id: String,
    pub message_id: String,
    pub usage: Option<TokenUsage>,
    pub completed_at: DateTime<Utc>,
}

/// agent:message_failed event payload (DS-60 §4.2)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentMessageFailed {
    pub session_id: String,
    pub message_id: Option<String>,
    pub error: super::error::AppError,
}

/// agent:tool_requested event payload (DS-60 §4.1)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentToolRequested {
    pub session_id: String,
    pub tool_name: String,
    pub arguments: serde_json::Value,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::provider::MessageRole;

    #[test]
    fn test_agent_message_serialization() {
        let msg = AgentMessage {
            id: "msg-001".into(),
            session_id: "sess-001".into(),
            role: MessageRole::User,
            content: "테스트 메시지".into(),
            created_at: Utc::now(),
            usage: None,
            is_streaming: false,
        };
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("msg-001"));
        assert!(json.contains("user"));
    }

    #[test]
    fn test_message_delta_serialization() {
        let delta = AgentMessageDelta {
            session_id: "sess-001".into(),
            message_id: "msg-002".into(),
            delta: "안녕".into(),
            sequence: 1,
        };
        let json = serde_json::to_string(&delta).unwrap();
        assert!(json.contains("sequence"));
    }
}
