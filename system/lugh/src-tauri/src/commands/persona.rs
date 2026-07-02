// commands/persona.rs — Persona Bundle Tauri command
// DS-60 §3.3: build_persona_bundle

use tauri::State;

use crate::{
    app_state::AppState,
    models::{error::AppError, persona::PersonaBundlePreview},
    persona_bundle::PersonaBundleService,
};

/// Shared persona와 역할 persona를 조합해 bundle 미리보기를 반환한다 (DS-60 §3.3)
#[tauri::command]
pub async fn build_persona_bundle(
    workspace_id: String,
    role: String,
    state: State<'_, AppState>,
) -> Result<PersonaBundlePreview, AppError> {
    let workspace_path = state.get_workspace_path(&workspace_id)?;
    let config = state.get_agiteam_config(&workspace_id)?;
    let svc = PersonaBundleService::new(&workspace_path);
    svc.build_preview(&config, &role)
}
