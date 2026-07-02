// commands/health.rs — Health Check Tauri command
// DS-60 §3.6: run_health_check

use tauri::{Emitter, State};

use crate::{
    app_state::AppState,
    health_check::{HealthCheckReport, HealthCheckService},
    models::error::AppError,
};

/// workspace 구조, provider credential, network 상태를 점검한다 (DS-60 §3.6)
#[tauri::command]
pub async fn run_health_check(
    workspace_id: String,
    app: tauri::AppHandle,
    state: State<'_, AppState>,
) -> Result<HealthCheckReport, AppError> {
    let workspace_path = state.get_workspace_path(&workspace_id)?;
    let config = state.get_agiteam_config(&workspace_id)?;

    let svc = HealthCheckService::new(&workspace_path);
    let report = svc.run(&workspace_id, &config).await?;

    // health:completed event emit (DS-60 §4.1)
    let _ = app.emit("health:completed", &report);

    Ok(report)
}
