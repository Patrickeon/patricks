// commands/credential.rs — Credential 관련 Tauri commands
// DS-60 §3.5: save_credential, delete_credential, validate_credential

use tauri::{Emitter, State};
use std::str::FromStr;

use crate::{
    app_state::AppState,
    credential::{CredentialStoreService, MaskedCredential},
    models::{
        error::AppError,
        provider::{AiProviderKind, CredentialRef, ProviderHealth},
        session::CommandResult,
    },
    provider,
};

/// credential:validated event payload (DS-60 §4.1)
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CredentialValidated {
    pub provider: AiProviderKind,
    pub account: String,
    pub status: crate::models::provider::HealthStatus,
}

/// provider credential을 OS vault에 저장한다 (DS-60 §3.5)
#[tauri::command]
pub async fn save_credential(
    provider: String,
    account: String,
    secret: String,
    _state: State<'_, AppState>,
) -> Result<CredentialRef, AppError> {
    let provider_kind = AiProviderKind::from_str(&provider)?;
    CredentialStoreService::save(&provider_kind, &account, &secret)
}

/// credential을 삭제한다 (DS-60 §3.5)
#[tauri::command]
pub async fn delete_credential(
    provider: String,
    account: String,
    _state: State<'_, AppState>,
) -> Result<CommandResult, AppError> {
    let provider_kind = AiProviderKind::from_str(&provider)?;
    match CredentialStoreService::delete(&provider_kind, &account) {
        Ok(_) => Ok(CommandResult::ok()),
        Err(e) => Ok(CommandResult::err(e)),
    }
}

/// provider credential 유효성을 검증한다 (DS-60 §3.5)
#[tauri::command]
pub async fn validate_credential(
    provider: String,
    account: String,
    app: tauri::AppHandle,
    _state: State<'_, AppState>,
) -> Result<ProviderHealth, AppError> {
    let provider_kind = AiProviderKind::from_str(&provider)?;

    // Redmine은 AI provider가 아니므로 단순 존재 확인만 수행
    if provider_kind == AiProviderKind::Redmine {
        let health = CredentialStoreService::check_existence(&provider_kind, &account);
        return Ok(health);
    }

    // OS vault에서 secret 조회
    let api_key = CredentialStoreService::get_secret(&provider_kind, &account)?;

    // provider adapter로 실제 API 검증
    let adapter = provider::create_provider(&provider_kind, api_key);
    let cref = CredentialRef {
        provider: provider_kind.clone(),
        account: account.clone(),
    };
    let health = adapter.validate_credential(cref).await?;

    // credential:validated event emit (DS-60 §4.1)
    let event_payload = CredentialValidated {
        provider: provider_kind,
        account,
        status: health.status.clone(),
    };
    let _ = app.emit("credential:validated", event_payload);

    Ok(health)
}

/// Claude Code CLI OAuth 토큰 존재 여부 확인 (FE 설정 화면용)
#[tauri::command]
pub async fn check_claude_oauth() -> Result<CommandResult, AppError> {
    match crate::credential::get_claude_oauth_token() {
        Ok(_) => Ok(CommandResult::ok()),
        Err(e) => Ok(CommandResult::err(e)),
    }
}

/// masked credential 정보를 조회한다 (secret 미포함)
#[tauri::command]
pub async fn get_masked_credential(
    provider: String,
    account: String,
    _state: State<'_, AppState>,
) -> Result<MaskedCredential, AppError> {
    let provider_kind = AiProviderKind::from_str(&provider)?;
    let cref = CredentialRef {
        provider: provider_kind,
        account,
    };
    Ok(MaskedCredential::from_ref(&cref))
}
