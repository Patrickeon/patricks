// commands/redmine.rs — Redmine Tauri 커맨드 레이어
// DV60-003: 실 API 연동 (RedmineClient 사용)
// Redmine API: http://211.117.60.5:8080/ (Shared persona §1)

use tauri::State;

use crate::{
    app_state::AppState,
    credential::CredentialStoreService,
    models::{error::AppError, provider::AiProviderKind},
    redmine_client::{RedmineClient, RedmineIssueItem},
};

// ── Helper ────────────────────────────────────────────────────────────────────

/// OS vault에서 Redmine API 키 조회 (#15 fix: 역할별 키 우선)
///
/// - role 지정 시: `api_key_${role}` 조회 (FE SettingsView 역할별 저장 키와 일치)
/// - role 키 없거나 role 미지정 시: 기존 단일 `api_key` fallback (하위 호환)
fn get_redmine_api_key(role: Option<&str>) -> Result<String, AppError> {
    if let Some(role) = role {
        let account = format!("api_key_{}", role);
        if let Ok(key) = CredentialStoreService::get_secret(&AiProviderKind::Redmine, &account) {
            return Ok(key);
        }
        // 역할별 키 미저장 → 단일 키 fallback으로 계속
    }
    CredentialStoreService::get_secret(&AiProviderKind::Redmine, "api_key")
        .map_err(|_| AppError::new(
            "REDMINE_API_KEY_NOT_SET",
            "레드마인 API 키가 설정되지 않았습니다. 설정 > API 키에서 저장해 주세요.",
        ))
}

/// OS vault에서 Redmine URL 조회 (폴백: Shared persona 기본값)
fn get_redmine_url() -> String {
    CredentialStoreService::get_secret(&AiProviderKind::Redmine, "url")
        .unwrap_or_else(|_| "http://211.117.60.5:8080".to_string())
}

/// api_key + base_url로 RedmineClient 생성 (#15 fix: role 지정 시 역할별 키 사용)
fn make_client(role: Option<&str>) -> Result<RedmineClient, AppError> {
    let api_key = get_redmine_api_key(role)?;
    let base_url = get_redmine_url();
    Ok(RedmineClient::new(base_url, api_key))
}

// ── Tauri Commands ────────────────────────────────────────────────────────────

/// 레드마인 이슈 목록 조회 (DS-40 §레드마인 API)
/// GET /issues.json?project_id=<id>&status_id=<open|all|id>&limit=100
#[tauri::command]
pub async fn redmine_list_issues(
    _workspace_id: String,
    project_id: Option<String>,
    status_id: Option<String>,
    role: Option<String>,
    _state: State<'_, AppState>,
) -> Result<Vec<RedmineIssueItem>, AppError> {
    let client = make_client(role.as_deref())?;
    client
        .list_issues(project_id.as_deref(), status_id.as_deref())
        .await
}

/// 레드마인 이슈 단건 조회 (DS-40 §레드마인 API)
/// GET /issues/<id>.json
#[tauri::command]
pub async fn redmine_get_issue(
    _workspace_id: String,
    issue_id: u32,
    role: Option<String>,
    _state: State<'_, AppState>,
) -> Result<RedmineIssueItem, AppError> {
    let client = make_client(role.as_deref())?;
    client.get_issue(issue_id).await
}

/// 레드마인 이슈 생성 (DS-40 §레드마인 API)
/// POST /issues.json
#[tauri::command]
pub async fn redmine_create_issue(
    _workspace_id: String,
    project_id: String,
    tracker_id: u32,
    subject: String,
    description: Option<String>,
    assigned_to_id: Option<u32>,
    role: Option<String>,
    _state: State<'_, AppState>,
) -> Result<RedmineIssueItem, AppError> {
    let client = make_client(role.as_deref())?;
    client
        .create_issue(
            &project_id,
            tracker_id,
            &subject,
            description.as_deref(),
            assigned_to_id,
        )
        .await
}

/// 레드마인 이슈 갱신 (DS-40 §레드마인 API)
/// PUT /issues/<id>.json
/// 204 No Content → ()
#[tauri::command]
pub async fn redmine_update_issue(
    _workspace_id: String,
    issue_id: u32,
    status_id: Option<u32>,
    done_ratio: Option<u32>,
    notes: Option<String>,
    role: Option<String>,
    _state: State<'_, AppState>,
) -> Result<(), AppError> {
    let client = make_client(role.as_deref())?;
    client
        .update_issue(issue_id, status_id, done_ratio, notes.as_deref())
        .await
}
