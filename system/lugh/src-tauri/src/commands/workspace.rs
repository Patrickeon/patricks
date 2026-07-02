// commands/workspace.rs — Workspace 관련 Tauri commands
// DS-60 §3.1: open_workspace, load_workspace_config, validate_workspace

use std::path::PathBuf;
use tauri::State;

use crate::{
    app_state::AppState,
    models::{
        error::AppError,
        workspace::{
            AgiteamConfig, IssueSeverity, ProjectState, ValidationIssue, ValidationReport,
            WorkspaceConfig, WorkspaceSummary,
        },
    },
};

/// workspace를 열고 기본 정보를 반환한다 (DS-60 §3.1)
#[tauri::command]
pub async fn open_workspace(
    path: String,
    state: State<'_, AppState>,
) -> Result<WorkspaceSummary, AppError> {
    let workspace_path = PathBuf::from(&path);
    if !workspace_path.exists() {
        return Err(AppError::workspace_not_found(&path));
    }

    let workspace_id = uuid::Uuid::new_v4().to_string();

    // agiteam.json 로드
    let config_path = workspace_path.join("agiteam.json");
    let (name, display_name) = if config_path.exists() {
        let content = std::fs::read_to_string(&config_path).map_err(|e| {
            AppError::new("CONFIG_READ_FAILED", e.to_string())
        })?;
        let config: AgiteamConfig = serde_json::from_str(&content)
            .map_err(|e| AppError::config_invalid(e.to_string()))?;
        (
            config.project.name.clone(),
            config.project.display_name.clone(),
        )
    } else {
        (
            workspace_path
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| "workspace".to_string()),
            None,
        )
    };

    let summary = WorkspaceSummary {
        workspace_id: workspace_id.clone(),
        path: path.clone(),
        name: name.clone(),
        display_name: display_name.clone(),
    };

    // AppState에 workspace 등록
    state.set_workspace(workspace_id.clone(), path);

    Ok(summary)
}

/// agiteam.json과 project_state.yaml을 로드한다 (DS-60 §3.1)
#[tauri::command]
pub async fn load_workspace_config(
    workspace_id: String,
    state: State<'_, AppState>,
) -> Result<WorkspaceConfig, AppError> {
    let workspace_path = state.get_workspace_path(&workspace_id)?;
    let path = PathBuf::from(&workspace_path);

    // agiteam.json
    let config_path = path.join("agiteam.json");
    let agiteam: AgiteamConfig = {
        let content = std::fs::read_to_string(&config_path).map_err(|e| {
            AppError::workspace_not_found(&config_path.display().to_string())
                .with_detail(serde_json::json!({"error": e.to_string()}))
        })?;
        serde_json::from_str(&content).map_err(|e| AppError::config_invalid(e.to_string()))?
    };

    // project_state.yaml (없으면 기본값)
    let state_path = path.join("project_state.yaml");
    let project_state: ProjectState = if state_path.exists() {
        let content = std::fs::read_to_string(&state_path).map_err(|e| {
            AppError::new("STATE_READ_FAILED", e.to_string())
        })?;
        serde_yaml::from_str(&content).map_err(|e| {
            AppError::new("STATE_PARSE_FAILED", e.to_string())
        })?
    } else {
        ProjectState {
            business_type: None,
            current_mode: None,
            milestone: None,
            wbs_track: None,
            milestones: vec![],
        }
    };

    Ok(WorkspaceConfig {
        workspace_id,
        agiteam,
        project_state,
    })
}

/// workspace 필수 구조와 persona 파일을 검증한다 (DS-60 §3.1)
#[tauri::command]
pub async fn validate_workspace(
    workspace_id: String,
    state: State<'_, AppState>,
) -> Result<ValidationReport, AppError> {
    let workspace_path = state.get_workspace_path(&workspace_id)?;
    let path = PathBuf::from(&workspace_path);
    let mut issues = Vec::new();

    // agiteam.json 존재 확인
    let config_path = path.join("agiteam.json");
    if !config_path.exists() {
        issues.push(ValidationIssue {
            severity: IssueSeverity::Error,
            code: "CONFIG_MISSING".into(),
            message: "agiteam.json이 없습니다".into(),
            path: Some("agiteam.json".into()),
        });
        return Ok(ValidationReport {
            workspace_id: workspace_id.clone(),
            valid: false,
            issues,
        });
    }

    // agiteam.json 파싱
    let content = std::fs::read_to_string(&config_path).map_err(|e| {
        AppError::config_invalid(e.to_string())
    })?;
    let config: AgiteamConfig = match serde_json::from_str(&content) {
        Ok(c) => c,
        Err(e) => {
            issues.push(ValidationIssue {
                severity: IssueSeverity::Error,
                code: "CONFIG_INVALID".into(),
                message: format!("agiteam.json 파싱 오류: {}", e),
                path: Some("agiteam.json".into()),
            });
            return Ok(ValidationReport {
                workspace_id,
                valid: false,
                issues,
            });
        }
    };

    // persona.dir 존재 확인
    let persona_dir = path.join(&config.persona.dir);
    if !persona_dir.exists() {
        issues.push(ValidationIssue {
            severity: IssueSeverity::Error,
            code: "PERSONA_DIR_MISSING".into(),
            message: format!("persona 디렉토리가 없습니다: {}", config.persona.dir),
            path: Some(config.persona.dir.clone()),
        });
    }

    // 역할별 persona.md 확인
    for member in &config.team {
        let persona_path = persona_dir.join(&member.role).join("persona.md");
        if !persona_path.exists() {
            issues.push(ValidationIssue {
                severity: IssueSeverity::Error,
                code: "PERSONA_NOT_FOUND".into(),
                message: format!("{} persona 파일이 없습니다", member.role),
                path: Some(format!("{}/{}/persona.md", config.persona.dir, member.role)),
            });
        }
    }

    // PM persona 확인
    let pm_persona = persona_dir.join("PM").join("persona.md");
    if !pm_persona.exists() {
        issues.push(ValidationIssue {
            severity: IssueSeverity::Error,
            code: "PERSONA_NOT_FOUND".into(),
            message: "PM persona 파일이 없습니다".into(),
            path: Some(format!("{}/PM/persona.md", config.persona.dir)),
        });
    }

    // Shared persona 경고 (없어도 동작하지만 권장)
    let shared_path = path.join(&config.persona.common_file);
    if !shared_path.exists() {
        issues.push(ValidationIssue {
            severity: IssueSeverity::Warning,
            code: "SHARED_PERSONA_MISSING".into(),
            message: "Shared persona 파일이 없습니다 (권장)".into(),
            path: Some(config.persona.common_file.clone()),
        });
    }

    let valid = !issues.iter().any(|i| i.severity == IssueSeverity::Error);
    Ok(ValidationReport {
        workspace_id,
        valid,
        issues,
    })
}

/// project_state.yaml을 디스크에 저장한다
#[tauri::command]
pub async fn write_project_state(
    workspace_id: String,
    state: serde_json::Value,
    app_state: State<'_, AppState>,
) -> Result<(), AppError> {
    let workspace_path = app_state.get_workspace_path(&workspace_id)?;
    let state_path = std::path::PathBuf::from(&workspace_path).join("project_state.yaml");

    // JSON Value를 ProjectState로 역직렬화
    let project_state: crate::models::workspace::ProjectState =
        serde_json::from_value(state)
            .map_err(|e| AppError::new("PARSE_FAILED", e.to_string()))?;

    // YAML로 직렬화 후 저장
    let yaml = serde_yaml::to_string(&project_state)
        .map_err(|e| AppError::new("SERIALIZE_FAILED", e.to_string()))?;

    std::fs::write(&state_path, yaml)
        .map_err(|e| AppError::new("WRITE_FAILED", e.to_string()))?;

    Ok(())
}

/// agiteam.json을 디스크에 저장한다 (DV60-005)
#[tauri::command]
pub async fn save_workspace_config(
    workspace_id: String,
    config: AgiteamConfig,
    state: State<'_, AppState>,
) -> Result<(), AppError> {
    let workspace_path = state.get_workspace_path(&workspace_id)?;
    let config_path = PathBuf::from(&workspace_path).join("agiteam.json");

    let json = serde_json::to_string_pretty(&config)
        .map_err(|e| AppError::new("SERIALIZE_FAILED", format!("agiteam.json 직렬화 오류: {}", e)))?;

    // 백업: agiteam.json.bak 생성
    if config_path.exists() {
        let bak_path = PathBuf::from(&workspace_path).join("agiteam.json.bak");
        std::fs::copy(&config_path, &bak_path).map_err(|e| {
            AppError::new("BACKUP_FAILED", format!("백업 생성 오류: {}", e))
        })?;
    }

    std::fs::write(&config_path, json)
        .map_err(|e| AppError::new("WRITE_FAILED", format!("agiteam.json 저장 오류: {}", e)))?;

    Ok(())
}
