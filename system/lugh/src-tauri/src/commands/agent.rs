// commands/agent.rs — Agent Session 관련 Tauri commands
// DS-60 §3.2: boot_team, boot_role, stop_role, send_agent_message 등

use std::sync::Arc;
use tauri::{AppHandle, Emitter, State};

use crate::{
    app_state::AppState,
    chat_attachment,
    models::{
        attachment::{
            ChatAttachmentFailed, ChatAttachmentInput, ChatAttachmentPrepared,
            PreparedChatAttachment,
        },
        error::AppError,
        session::{
            AgentLifecycleState, AgentMessagesCleared, AgentSessionDetail, BootTeamResult,
            AgentSessionSummary, ClearSessionResult, MessageAck, CommandResult,
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
                    vec![],
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
/// `attachments`가 없거나 빈 배열이면 기존 텍스트 전용 동작과 동일하다 (#21).
#[tauri::command]
pub async fn send_agent_message(
    session_id: String,
    content: String,
    attachments: Option<Vec<PreparedChatAttachment>>,
    state: State<'_, AppState>,
) -> Result<MessageAck, AppError> {
    let workspace_id = state.get_workspace_id_for_session(&session_id)?;
    let config = state.get_agiteam_config(&workspace_id)?;
    let svc = state.session_service(&workspace_id)?;

    // OAuth 계정에서 사용 가능한 모델 (2025-07 기준)
    let default_model = "claude-sonnet-4-5-20250929";

    svc.send_message(
        &session_id,
        &content,
        attachments.unwrap_or_default(),
        &config,
        default_model,
    ).await
}

/// 채팅 입력창 첨부 1건을 검증하고 이미지 base64 또는 문서 추출 텍스트로 정규화한다
/// (DS-60 §3.2 prepare_chat_attachment, Redmine #21)
/// 성공 시 chat:attachment_prepared, 실패 시 chat:attachment_failed를 emit한다.
#[tauri::command]
pub async fn prepare_chat_attachment(
    session_id: String,
    attachment: ChatAttachmentInput,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<PreparedChatAttachment, AppError> {
    // 세션 존재 검증 (미존재 세션이면 SESSION_NOT_FOUND)
    let _workspace_id = state.get_workspace_id_for_session(&session_id)?;

    let attachment_id = attachment.id.clone();
    let filename = attachment.filename.clone();

    // base64 decode·sha256·PDF 추출은 CPU 작업 → blocking pool로 위임
    let result = tokio::task::spawn_blocking(move || chat_attachment::prepare_attachment(attachment))
        .await
        .map_err(|e| AppError::new("INTERNAL", format!("첨부 전처리 태스크 실패: {}", e)))?;

    match result {
        Ok(prepared) => {
            let _ = app.emit(
                "chat:attachment_prepared",
                ChatAttachmentPrepared {
                    session_id,
                    attachment: prepared.clone(),
                },
            );
            Ok(prepared)
        }
        Err(error) => {
            let _ = app.emit(
                "chat:attachment_failed",
                ChatAttachmentFailed {
                    session_id,
                    attachment_id,
                    filename,
                    error: error.clone(),
                },
            );
            Err(error)
        }
    }
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

/// 지정 세션의 대화 히스토리를 전부 삭제한다 (역할별 대화 초기화, DS-60 §3.2, Redmine #24)
/// 세션 자체(lifecycle 상태·provider·persona_hash 등)와 페르소나 주입은 유지된다 —
/// 초기화 직후 첫 전송부터 send_message가 페르소나를 재주입하므로 별도 재부팅이 필요 없다.
/// 세션이 running/booting 중이면 SESSION_BUSY로 거절된다.
/// 성공 시 agent:messages_cleared 이벤트를 emit한다.
#[tauri::command]
pub async fn clear_session_messages(
    session_id: String,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<ClearSessionResult, AppError> {
    let workspace_id = state.get_workspace_id_for_session(&session_id)?;
    let svc = state.session_service(&workspace_id)?;
    let result = svc.clear_session_messages(&session_id)?;

    let _ = app.emit(
        "agent:messages_cleared",
        AgentMessagesCleared {
            session_id: result.session_id.clone(),
            cleared_count: result.cleared_count,
            cleared_at: result.cleared_at,
        },
    );

    Ok(result)
}
