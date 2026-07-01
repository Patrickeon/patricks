// commands/agent.rs — Agent Session 관련 Tauri commands
// DS-60 §3.2: boot_team, boot_role, stop_role, send_agent_message 등

use std::sync::Arc;
use tauri::State;

use crate::{
    app_state::AppState,
    models::{
        error::AppError,
        session::{
            AgentLifecycleState, AgentSessionDetail, BootTeamResult, AgentSessionSummary,
            MessageAck, CommandResult,
        },
        message::MessagePage,
    },
};

const DEFAULT_MODEL: &str = "claude-sonnet-4-5-20250929";

/// 전체 역할 agent session booting을 시작한다 (DS-60 §3.2)
/// 부팅 완료 후 PM startup message를 자동 전송한다 (agiteam.json pm.startupMessage).
#[tauri::command]
pub async fn boot_team(
    workspace_id: String,
    state: State<'_, AppState>,
) -> Result<BootTeamResult, AppError> {
    let config = state.get_agiteam_config(&workspace_id)?;
    let svc = state.session_service(&workspace_id)?;
    let result = svc.boot_team(&workspace_id, &config).await?;

    // PM startupMessage 자동 전송 (비동기, 논블로킹)
    if !config.pm.startup_message.is_empty() {
        if let Some(pm) = result.sessions.iter().find(|s| s.role == "PM" && s.state == AgentLifecycleState::Ready) {
            let svc_clone = Arc::clone(&svc);
            let pm_session_id = pm.session_id.clone();
            let startup_msg = config.pm.startup_message.clone();
            let config_clone = config.clone();

            tokio::spawn(async move {
                // 프론트엔드 이벤트 리스너 등록 대기 (500ms)
                tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
                if let Err(e) = svc_clone.send_message(
                    &pm_session_id,
                    &startup_msg,
                    &config_clone,
                    DEFAULT_MODEL,
                ).await {
                    log::warn!("PM startup message 전송 실패: {}", e.message);
                }
            });
        }
    }

    Ok(result)
}

/// 단일 역할 agent session booting을 시작한다 (DS-60 §3.2)
#[tauri::command]
pub async fn boot_role(
    workspace_id: String,
    role: String,
    state: State<'_, AppState>,
) -> Result<AgentSessionSummary, AppError> {
    let config = state.get_agiteam_config(&workspace_id)?;
    let svc = state.session_service(&workspace_id)?;
    svc.boot_role(&workspace_id, &role, &config).await
}

/// 실행 중인 역할 세션을 중지한다 (DS-60 §3.2)
#[tauri::command]
pub async fn stop_role(
    session_id: String,
    state: State<'_, AppState>,
) -> Result<CommandResult, AppError> {
    // session에서 workspace_id 조회
    let workspace_id = state.get_workspace_id_for_session(&session_id)?;
    let svc = state.session_service(&workspace_id)?;
    match svc.stop_session(&session_id) {
        Ok(_) => Ok(CommandResult::ok()),
        Err(e) => Ok(CommandResult::err(e)),
    }
}

/// ready 상태의 agent session에 사용자 메시지를 전송한다 (DS-60 §3.2)
#[tauri::command]
pub async fn send_agent_message(
    session_id: String,
    content: String,
    state: State<'_, AppState>,
) -> Result<MessageAck, AppError> {
    let workspace_id = state.get_workspace_id_for_session(&session_id)?;
    let config = state.get_agiteam_config(&workspace_id)?;
    let svc = state.session_service(&workspace_id)?;

    // OAuth 계정에서 사용 가능한 모델 (2025-07 기준)
    let default_model = "claude-sonnet-4-5-20250929";

    svc.send_message(&session_id, &content, &config, default_model).await
}

/// 세션 상세 상태를 조회한다 (DS-60 §3.2)
#[tauri::command]
pub async fn get_agent_session(
    session_id: String,
    state: State<'_, AppState>,
) -> Result<AgentSessionDetail, AppError> {
    let workspace_id = state.get_workspace_id_for_session(&session_id)?;
    let svc = state.session_service(&workspace_id)?;
    svc.get_session_detail(&session_id)
}

/// 세션 메시지 로그를 페이지 단위로 조회한다 (DS-60 §3.2)
#[tauri::command]
pub async fn list_agent_messages(
    session_id: String,
    cursor: Option<String>,
    limit: Option<u32>,
    state: State<'_, AppState>,
) -> Result<MessagePage, AppError> {
    let workspace_id = state.get_workspace_id_for_session(&session_id)?;
    let svc = state.session_service(&workspace_id)?;
    let limit = limit.unwrap_or(50) as usize;
    svc.list_messages(&session_id, cursor.as_deref(), limit)
}
