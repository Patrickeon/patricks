// models/session.rs — AgentSession, AgentLifecycleState (DS-20 §3.3, §7)

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use super::provider::AiProviderKind;

/// 에이전트 생명주기 상태 (DS-20 §7.1)
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AgentLifecycleState {
    /// 세션 미생성 또는 정지 상태
    Idle,
    /// persona bundle 생성, credential 확인, provider 세션 준비 중
    Booting,
    /// PM 지시 수신 가능 상태
    Ready,
    /// provider 요청 처리 또는 응답 streaming 중
    Running,
    /// 복구 가능한/불가능한 오류 발생
    Failed,
}

impl std::fmt::Display for AgentLifecycleState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AgentLifecycleState::Idle => write!(f, "idle"),
            AgentLifecycleState::Booting => write!(f, "booting"),
            AgentLifecycleState::Ready => write!(f, "ready"),
            AgentLifecycleState::Running => write!(f, "running"),
            AgentLifecycleState::Failed => write!(f, "failed"),
        }
    }
}

/// AgentSession 전체 상태 (DS-20 §3.3)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentSession {
    pub id: String,
    pub workspace_id: String,
    pub role: String,
    pub display_name: String,
    pub provider: AiProviderKind,
    pub state: AgentLifecycleState,
    pub persona_hash: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub failure_reason: Option<String>,
}

/// boot_team/boot_role command 응답에서 사용하는 요약
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentSessionSummary {
    pub session_id: String,
    pub role: String,
    pub display_name: String,
    pub provider: AiProviderKind,
    pub state: AgentLifecycleState,
}

/// get_agent_session command 응답
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentSessionDetail {
    pub session_id: String,
    pub workspace_id: String,
    pub role: String,
    pub display_name: String,
    pub provider: AiProviderKind,
    pub state: AgentLifecycleState,
    pub persona_hash: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub failure_reason: Option<String>,
    pub message_count: u32,
}

/// boot_team command 응답
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BootTeamResult {
    pub workspace_id: String,
    pub sessions: Vec<AgentSessionSummary>,
}

/// send_agent_message command 응답 (DS-60 §3.2)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageAck {
    pub session_id: String,
    pub user_message_id: String,
    pub accepted_at: DateTime<Utc>,
}

/// agent:status_changed event payload (DS-60 §4.2)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentStatusChanged {
    pub session_id: String,
    pub role: String,
    pub from: AgentLifecycleState,
    pub to: AgentLifecycleState,
    pub reason: Option<String>,
    pub changed_at: DateTime<Utc>,
}

/// CommandResult — 단순 성공/실패 (DS-60 §2.2)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandResult {
    pub ok: bool,
    pub error: Option<super::error::AppError>,
}

impl CommandResult {
    pub fn ok() -> Self { Self { ok: true, error: None } }
    pub fn err(error: super::error::AppError) -> Self { Self { ok: false, error: Some(error) } }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lifecycle_state_display() {
        assert_eq!(AgentLifecycleState::Ready.to_string(), "ready");
        assert_eq!(AgentLifecycleState::Booting.to_string(), "booting");
    }

    #[test]
    fn test_lifecycle_state_serde() {
        let json = serde_json::to_string(&AgentLifecycleState::Running).unwrap();
        assert_eq!(json, "\"running\"");
        let state: AgentLifecycleState = serde_json::from_str("\"failed\"").unwrap();
        assert_eq!(state, AgentLifecycleState::Failed);
    }

    #[test]
    fn test_command_result() {
        let ok = CommandResult::ok();
        assert!(ok.ok);
        assert!(ok.error.is_none());
    }
}
