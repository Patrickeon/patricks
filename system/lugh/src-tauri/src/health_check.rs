// health_check.rs — HealthCheckService
// workspace 구조 + provider API 도달성 + credential 검증
// DS-20 §10.2, DS-40 §9, DS-60 §3.6

use chrono::Utc;
use reqwest::StatusCode;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::time::Instant;

use crate::credential::CredentialStoreService;
use crate::models::{
    error::{AppError, AppResult},
    provider::{AiProviderKind, HealthStatus, ProviderHealth},
    workspace::AgiteamConfig,
};

/// health:completed event / run_health_check 응답 (DS-60 §3.6, §4.1)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthCheckReport {
    pub workspace_id: String,
    pub ok: bool,
    pub checks: Vec<HealthCheckItem>,
    pub blocking_issues: Vec<String>,
    pub run_at: chrono::DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthCheckItem {
    pub category: HealthCategory,
    pub name: String,
    pub status: HealthStatus,
    pub message: Option<String>,
    pub latency_ms: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum HealthCategory {
    Workspace,
    Config,
    Credential,
    Network,
    Document,
}

/// HealthCheckService
pub struct HealthCheckService {
    workspace_root: PathBuf,
    http: reqwest::Client,
}

impl HealthCheckService {
    pub fn new(workspace_root: impl Into<PathBuf>) -> Self {
        let http = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(10))
            .user_agent(format!("AgiTeamBuilder/{}", env!("CARGO_PKG_VERSION")))
            .build()
            .unwrap_or_default();

        Self {
            workspace_root: workspace_root.into(),
            http,
        }
    }

    /// workspace 구조 + provider 상태 전체 점검
    pub async fn run(
        &self,
        workspace_id: &str,
        config: &AgiteamConfig,
    ) -> AppResult<HealthCheckReport> {
        let mut checks = Vec::new();
        let mut blocking = Vec::new();

        // 1. Workspace 구조 검증
        self.check_workspace_structure(config, &mut checks, &mut blocking);

        // 2. Credential 존재 확인
        self.check_credentials(config, &mut checks);

        // 3. Network / Provider endpoint 도달성
        self.check_provider_network(config, &mut checks).await;

        // 4. Documents 쓰기 가능 여부
        self.check_documents_writable(&mut checks);

        let ok = checks.iter().all(|c| c.status == HealthStatus::Ok || c.status == HealthStatus::Unknown);

        Ok(HealthCheckReport {
            workspace_id: workspace_id.to_string(),
            ok,
            checks,
            blocking_issues: blocking,
            run_at: Utc::now(),
        })
    }

    // ---- Workspace 구조 ----

    fn check_workspace_structure(
        &self,
        config: &AgiteamConfig,
        checks: &mut Vec<HealthCheckItem>,
        blocking: &mut Vec<String>,
    ) {
        // agiteam.json 존재
        let agiteam_path = self.workspace_root.join("agiteam.json");
        if agiteam_path.exists() {
            checks.push(HealthCheckItem {
                category: HealthCategory::Workspace,
                name: "agiteam.json".into(),
                status: HealthStatus::Ok,
                message: None,
                latency_ms: None,
            });
        } else {
            let msg = "agiteam.json이 없습니다".to_string();
            blocking.push(msg.clone());
            checks.push(HealthCheckItem {
                category: HealthCategory::Workspace,
                name: "agiteam.json".into(),
                status: HealthStatus::Unreachable,
                message: Some(msg),
                latency_ms: None,
            });
        }

        // project_state.yaml 존재
        let state_path = self.workspace_root.join("project_state.yaml");
        checks.push(HealthCheckItem {
            category: HealthCategory::Workspace,
            name: "project_state.yaml".into(),
            status: if state_path.exists() { HealthStatus::Ok } else { HealthStatus::Degraded },
            message: if !state_path.exists() {
                Some("project_state.yaml이 없습니다 (선택 항목)".into())
            } else {
                None
            },
            latency_ms: None,
        });

        // 역할별 persona.md 존재
        let persona_dir = self.workspace_root.join(&config.persona.dir);
        for member in &config.team {
            let persona_path = persona_dir.join(&member.role).join("persona.md");
            if persona_path.exists() {
                checks.push(HealthCheckItem {
                    category: HealthCategory::Workspace,
                    name: format!("brain/{}/persona.md", member.role),
                    status: HealthStatus::Ok,
                    message: None,
                    latency_ms: None,
                });
            } else {
                let msg = format!("{} persona 파일이 없습니다", member.role);
                blocking.push(msg.clone());
                checks.push(HealthCheckItem {
                    category: HealthCategory::Workspace,
                    name: format!("brain/{}/persona.md", member.role),
                    status: HealthStatus::Unreachable,
                    message: Some(msg),
                    latency_ms: None,
                });
            }
        }
    }

    // ---- Credential ----

    fn check_credentials(&self, config: &AgiteamConfig, checks: &mut Vec<HealthCheckItem>) {
        let providers: Vec<AiProviderKind> = config
            .team
            .iter()
            .filter_map(|m| {
                m.agent.parse::<AiProviderKind>().ok()
            })
            .collect::<std::collections::HashSet<_>>()
            .into_iter()
            .collect();

        for provider in providers {
            let account = format!("default-{}", provider);
            let health = CredentialStoreService::check_existence(&provider, &account);
            checks.push(HealthCheckItem {
                category: HealthCategory::Credential,
                name: format!("{} credential", provider),
                status: health.status,
                message: health.error.map(|e| e.message),
                latency_ms: None,
            });
        }
    }

    // ---- Network ----

    async fn check_provider_network(
        &self,
        config: &AgiteamConfig,
        checks: &mut Vec<HealthCheckItem>,
    ) {
        // Redmine은 AI provider가 아니므로 AI endpoint 체크에서 제외하고 별도 처리
        let providers: Vec<AiProviderKind> = config
            .team
            .iter()
            .filter_map(|m| m.agent.parse::<AiProviderKind>().ok())
            .filter(|p| *p != AiProviderKind::Redmine)
            .collect::<std::collections::HashSet<_>>()
            .into_iter()
            .collect();

        for provider in providers {
            let health = self.check_provider_endpoint(&provider).await;
            checks.push(HealthCheckItem {
                category: HealthCategory::Network,
                name: format!("{} endpoint", provider),
                status: health.status,
                message: health.error.map(|e| e.message),
                latency_ms: health.latency_ms,
            });
        }

        // Redmine 내부망 확인 (별도 체크)
        let redmine_health = self.check_redmine_endpoint().await;
        checks.push(HealthCheckItem {
            category: HealthCategory::Network,
            name: "Redmine endpoint".into(),
            status: redmine_health.status,
            message: redmine_health.error.map(|e| e.message),
            latency_ms: redmine_health.latency_ms,
        });
    }

    /// Provider endpoint 도달성 확인 (HEAD 요청, DS-40 §9.2)
    pub async fn check_provider_endpoint(&self, provider: &AiProviderKind) -> ProviderHealth {
        let url = Self::provider_health_url(provider);
        let start = Instant::now();
        let result = self.http.head(url).send().await;
        let latency_ms = start.elapsed().as_millis() as u64;

        match result {
            Ok(resp) if resp.status().is_success() => ProviderHealth {
                provider: provider.clone(),
                status: HealthStatus::Ok,
                latency_ms: Some(latency_ms),
                checked_at: Utc::now(),
                error: None,
            },
            Ok(_) | Err(_) => ProviderHealth {
                provider: provider.clone(),
                status: HealthStatus::Unreachable,
                latency_ms: Some(latency_ms),
                checked_at: Utc::now(),
                error: Some(AppError::provider_unreachable(provider)),
            },
        }
    }

    async fn check_redmine_endpoint(&self) -> ProviderHealth {
        let url = "http://211.117.60.5:8080/";
        let start = Instant::now();
        let result = self.http.head(url).send().await;
        let latency_ms = start.elapsed().as_millis() as u64;

        let (status, error) = match result {
            Ok(_) => (HealthStatus::Ok, None),
            Err(e) => (
                HealthStatus::Degraded,
                Some(AppError::new("REDMINE_ERROR", e.to_string()).recoverable()),
            ),
        };
        ProviderHealth {
            provider: AiProviderKind::Redmine,
            status,
            latency_ms: Some(latency_ms),
            checked_at: Utc::now(),
            error,
        }
    }

    fn provider_health_url(provider: &AiProviderKind) -> &'static str {
        match provider {
            AiProviderKind::Claude => "https://api.anthropic.com/",
            AiProviderKind::OpenAi => "https://api.openai.com/",
            AiProviderKind::Gemini => "https://generativelanguage.googleapis.com/",
            // Redmine은 AI provider가 아니므로 check_provider_endpoint가 아닌
            // check_redmine_endpoint로 별도 처리된다. 여기에 도달하면 프로그래밍 오류.
            AiProviderKind::Redmine => "http://211.117.60.5:8080/",
        }
    }

    // ---- Documents ----

    fn check_documents_writable(&self, checks: &mut Vec<HealthCheckItem>) {
        let docs_dir = self.workspace_root.join("documents");
        if !docs_dir.exists() {
            checks.push(HealthCheckItem {
                category: HealthCategory::Document,
                name: "documents 폴더".into(),
                status: HealthStatus::Degraded,
                message: Some("documents 폴더가 없습니다".into()),
                latency_ms: None,
            });
            return;
        }

        // 임시 파일 쓰기로 쓰기 권한 확인
        let probe = docs_dir.join(".write_probe");
        let status = match std::fs::write(&probe, b"probe") {
            Ok(_) => {
                let _ = std::fs::remove_file(&probe);
                HealthStatus::Ok
            }
            Err(_) => HealthStatus::Degraded,
        };
        checks.push(HealthCheckItem {
            category: HealthCategory::Document,
            name: "documents 쓰기 권한".into(),
            status: status.clone(),
            message: if status == HealthStatus::Degraded {
                Some("documents 폴더에 쓰기 권한이 없습니다".into())
            } else {
                None
            },
            latency_ms: None,
        });
    }
}

// AiProviderKind Hash 구현 (HashSet 사용, Eq는 derive로 이미 구현됨)
impl std::hash::Hash for AiProviderKind {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.to_string().hash(state);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_health_check_report_serialization() {
        let report = HealthCheckReport {
            workspace_id: "ws-001".into(),
            ok: true,
            checks: vec![HealthCheckItem {
                category: HealthCategory::Workspace,
                name: "agiteam.json".into(),
                status: HealthStatus::Ok,
                message: None,
                latency_ms: Some(0),
            }],
            blocking_issues: vec![],
            run_at: Utc::now(),
        };
        let json = serde_json::to_string(&report).unwrap();
        assert!(json.contains("workspace_id"));
        assert!(json.contains("ws-001"));
    }

    #[test]
    fn test_provider_health_url() {
        assert!(HealthCheckService::provider_health_url(&AiProviderKind::Claude)
            .contains("anthropic"));
        assert!(HealthCheckService::provider_health_url(&AiProviderKind::OpenAi)
            .contains("openai"));
        assert!(HealthCheckService::provider_health_url(&AiProviderKind::Gemini)
            .contains("googleapis"));
    }

    #[tokio::test]
    async fn test_workspace_check_missing_agiteam_json() {
        use crate::models::workspace::*;
        use tempfile::TempDir;

        let dir = TempDir::new().unwrap();
        let svc = HealthCheckService::new(dir.path());
        let config = AgiteamConfig {
            project: ProjectMeta {
                name: "test".into(),
                display_name: None,
                workspace: WorkspaceMeta { name: "T".into(), color: None },
            },
            persona: PersonaPathConfig {
                dir: "brain".into(),
                common_file: "brain/Shared/persona.md".into(),
            },
            team: vec![],
            pm: PmConfig {
                name: "PM".into(),
                agent: "claude".into(),
                command: "claude".into(),
                startup_files: vec![],
                startup_message: String::new(),
            },
            settings: AgiteamSettings {
                ready_timeout: 30,
                post_launch_delay: 3,
                ready_signal_timeout: 60,
                max_auto_submits: 5,
            },
        };

        let report = svc.run("ws-001", &config).await.unwrap();
        assert!(report.checks.iter().any(|c| c.name == "agiteam.json"
            && c.status == HealthStatus::Unreachable));
    }
}
