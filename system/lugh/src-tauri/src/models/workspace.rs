// models/workspace.rs — Workspace, WorkspaceConfig, ValidationReport (DS-20 §3.3, DS-60 §3.1)

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Workspace 핵심 식별 정보
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Workspace {
    pub id: String,
    pub path: PathBuf,
    pub name: String,
    pub display_name: Option<String>,
    pub current_mode: Option<String>,
    pub milestone: Option<String>,
    pub wbs_track: Option<String>,
    pub business_type: Option<String>,
}

/// open_workspace command 응답 (DS-60 §3.1)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceSummary {
    pub workspace_id: String,
    pub path: String,
    pub name: String,
    pub display_name: Option<String>,
}

/// agiteam.json + project_state.yaml 로드 결과
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceConfig {
    pub workspace_id: String,
    pub agiteam: AgiteamConfig,
    pub project_state: ProjectState,
}

/// agiteam.json 전체 구조 (DS-40 §4)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgiteamConfig {
    pub project: ProjectMeta,
    pub persona: PersonaPathConfig,
    pub team: Vec<TeamMemberConfig>,
    pub pm: PmConfig,
    pub settings: AgiteamSettings,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectMeta {
    pub name: String,
    #[serde(rename = "displayName")]
    pub display_name: Option<String>,
    pub workspace: WorkspaceMeta,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceMeta {
    pub name: String,
    pub color: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersonaPathConfig {
    pub dir: String,
    #[serde(rename = "commonFile")]
    pub common_file: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TeamMemberConfig {
    pub role: String,
    pub name: String,
    pub agent: String,
    pub command: String,
    pub layout: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PmConfig {
    pub name: String,
    pub agent: String,
    pub command: String,
    #[serde(rename = "startupFiles", default)]
    pub startup_files: Vec<String>,
    #[serde(rename = "startupMessage", default)]
    pub startup_message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgiteamSettings {
    #[serde(rename = "readyTimeout", default = "default_ready_timeout")]
    pub ready_timeout: u32,
    #[serde(rename = "postLaunchDelay", default = "default_post_launch_delay")]
    pub post_launch_delay: u32,
    #[serde(rename = "readySignalTimeout", default = "default_ready_signal_timeout")]
    pub ready_signal_timeout: u32,
    #[serde(rename = "maxAutoSubmits", default = "default_max_auto_submits")]
    pub max_auto_submits: u32,
}

fn default_ready_timeout() -> u32 { 30 }
fn default_post_launch_delay() -> u32 { 3 }
fn default_ready_signal_timeout() -> u32 { 60 }
fn default_max_auto_submits() -> u32 { 5 }

/// project_state.yaml 스칼라 값 (DS-20 §2.2)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectState {
    pub business_type: Option<String>,
    pub current_mode: Option<String>,
    pub milestone: Option<String>,
    pub wbs_track: Option<String>,
    #[serde(default)]
    pub milestones: Vec<MilestoneEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MilestoneEntry {
    pub code: String,
    pub status: String,
    pub evidence: Option<String>,
}

/// validate_workspace command 응답
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationReport {
    pub workspace_id: String,
    pub valid: bool,
    pub issues: Vec<ValidationIssue>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationIssue {
    pub severity: IssueSeverity,
    pub code: String,
    pub message: String,
    pub path: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum IssueSeverity {
    Error,
    Warning,
    Info,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validation_report_serialization() {
        let report = ValidationReport {
            workspace_id: "ws-001".into(),
            valid: false,
            issues: vec![ValidationIssue {
                severity: IssueSeverity::Error,
                code: "PERSONA_NOT_FOUND".into(),
                message: "DeveloperBE persona 없음".into(),
                path: Some("brain/DeveloperBE/persona.md".into()),
            }],
        };
        let json = serde_json::to_string(&report).unwrap();
        assert!(json.contains("PERSONA_NOT_FOUND"));
    }

    #[test]
    fn test_agiteam_settings_defaults() {
        let json = r#"{"readyTimeout": 30, "postLaunchDelay": 3, "readySignalTimeout": 60, "maxAutoSubmits": 5}"#;
        let settings: AgiteamSettings = serde_json::from_str(json).unwrap();
        assert_eq!(settings.ready_timeout, 30);
    }
}
