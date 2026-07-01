// persona_bundle.rs — PersonaBundleService
// Shared persona + 역할 persona + 부팅 대기 규칙 번들링
// 메모리 내 결합 → 임시 파일 불필요 (DS-20 §3.1, DS-60 §3.3)

use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

use crate::models::{
    error::{AppError, AppResult},
    persona::{PersonaBundle, PersonaBundlePreview},
    workspace::AgiteamConfig,
};

/// PersonaBundleService — persona 파일 읽기 + 번들 생성
pub struct PersonaBundleService {
    workspace_root: PathBuf,
}

impl PersonaBundleService {
    pub fn new(workspace_root: impl Into<PathBuf>) -> Self {
        Self {
            workspace_root: workspace_root.into(),
        }
    }

    /// 역할 persona bundle을 생성한다.
    /// 구성: Shared/persona.md + brain/<role>/persona.md + startupFiles(PM 전용) + 부팅 대기 규칙
    pub fn build(&self, config: &AgiteamConfig, role: &str) -> AppResult<PersonaBundle> {
        let persona_dir = self.workspace_root.join(&config.persona.dir);
        let common_file = self.workspace_root.join(&config.persona.common_file);

        let mut source_files: Vec<String> = Vec::new();
        let mut parts: Vec<String> = Vec::new();
        // 중복 포함 방지를 위한 절대경로 집합
        let mut included_paths = std::collections::HashSet::new();

        // 1. Shared persona (있으면 포함)
        if common_file.exists() {
            let shared = self.read_file(&common_file)?;
            parts.push(shared);
            parts.push("\n---\n".to_string());
            let rel = common_file
                .strip_prefix(&self.workspace_root)
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_else(|_| config.persona.common_file.clone());
            source_files.push(rel);
            included_paths.insert(common_file.canonicalize().unwrap_or_else(|_| common_file.clone()));
        }

        // 2. 역할별 persona
        let role_file = persona_dir.join(role).join("persona.md");
        if !role_file.exists() {
            return Err(AppError::persona_not_found(role));
        }
        let role_content = self.read_file(&role_file)?;
        parts.push(role_content);
        let role_rel = role_file
            .strip_prefix(&self.workspace_root)
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|_| format!("{}/{}/persona.md", config.persona.dir, role));
        source_files.push(role_rel);
        included_paths.insert(role_file.canonicalize().unwrap_or_else(|_| role_file.clone()));

        // 3. PM 전용: startupFiles 추가 (이미 포함된 파일 자동 스킵)
        if role == "PM" && !config.pm.startup_files.is_empty() {
            for file_rel in &config.pm.startup_files {
                let file_abs = self.workspace_root.join(file_rel);
                let canonical = file_abs.canonicalize().unwrap_or_else(|_| file_abs.clone());

                // 중복 스킵
                if included_paths.contains(&canonical) {
                    continue;
                }

                if !file_abs.exists() {
                    log::warn!("startupFile 없음 (스킵): {}", file_rel);
                    continue;
                }

                match self.read_file(&file_abs) {
                    Ok(content) => {
                        parts.push(format!("\n---\n<!-- 참조 문서: {} -->\n{}", file_rel, content));
                        source_files.push(file_rel.clone());
                        included_paths.insert(canonical);
                    }
                    Err(e) => {
                        log::warn!("startupFile 읽기 실패 (스킵): {} — {}", file_rel, e.message);
                    }
                }
            }
        }

        // 4. 부팅 대기 규칙 (비-PM 역할)
        if role != "PM" {
            let pm_name = &config.pm.name;
            parts.push(Self::build_ready_rule(role, pm_name));
        }

        let content = parts.join("\n");
        let content_hash = Self::sha256_hex(&content);

        Ok(PersonaBundle {
            role: role.to_string(),
            content,
            content_hash,
            source_files,
        })
    }

    /// build_persona_bundle command 응답용 preview 생성
    pub fn build_preview(
        &self,
        config: &AgiteamConfig,
        role: &str,
    ) -> AppResult<PersonaBundlePreview> {
        let bundle = self.build(config, role)?;
        Ok(bundle.into())
    }

    fn read_file(&self, path: &Path) -> AppResult<String> {
        std::fs::read_to_string(path).map_err(|e| {
            AppError::new(
                "PERSONA_READ_FAILED",
                format!("파일 읽기 실패 {}: {}", path.display(), e),
            )
        })
    }

    fn build_ready_rule(role: &str, pm_name: &str) -> String {
        format!(
            r#"
---

## 부팅 직후 공통 대기 규칙 (최우선)

이 규칙은 다른 초기 행동 지침보다 우선합니다.

1. PM({pm_name})의 명시적 작업 지시가 내려오기 전까지 어떤 작업도 시작하지 마세요.
2. 지시 전에는 파일 읽기, 문서 탐색, 레드마인 조회, 코드 작성, 시안 작성, 명령 실행, 계획 수립을 하지 마세요.
3. 도구/CLI가 시작 과정에서 확인, 승인, Continue, Action Required 같은 상호작용을 요구하면 부팅 완료를 위해 필요한 최소 입력만 처리하세요.
4. 부팅이 완료되고 추가 상호작용이 더 이상 필요 없으면 정확히 한 줄만 출력하세요.

READY: {role}

5. READY 출력 후에는 PM의 다음 지시가 있을 때까지 추가 행동이나 추가 출력 없이 대기하세요.
"#,
            pm_name = pm_name,
            role = role
        )
    }

    fn sha256_hex(input: &str) -> String {
        let mut hasher = Sha256::new();
        hasher.update(input.as_bytes());
        hex::encode(hasher.finalize())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::workspace::{
        AgiteamConfig, AgiteamSettings, PersonaPathConfig, PmConfig, ProjectMeta, TeamMemberConfig,
        WorkspaceMeta,
    };
    use std::fs;
    use tempfile::TempDir;

    fn make_workspace(dir: &TempDir) -> (AgiteamConfig, PathBuf) {
        let root = dir.path().to_path_buf();
        // brain/Shared/persona.md
        fs::create_dir_all(root.join("brain/Shared")).unwrap();
        fs::write(root.join("brain/Shared/persona.md"), "# Shared Persona\n\n공용 내용").unwrap();
        // brain/DeveloperBE/persona.md
        fs::create_dir_all(root.join("brain/DeveloperBE")).unwrap();
        fs::write(
            root.join("brain/DeveloperBE/persona.md"),
            "# DeveloperBE\n\n역할 내용",
        )
        .unwrap();

        let config = AgiteamConfig {
            project: ProjectMeta {
                name: "test".into(),
                display_name: None,
                workspace: WorkspaceMeta {
                    name: "테스트".into(),
                    color: None,
                },
            },
            persona: PersonaPathConfig {
                dir: "brain".into(),
                common_file: "brain/Shared/persona.md".into(),
            },
            team: vec![TeamMemberConfig {
                role: "DeveloperBE".into(),
                name: "박개발".into(),
                agent: "claude".into(),
                command: "claude --dangerously-skip-permissions".into(),
                layout: "middle_top".into(),
            }],
            pm: PmConfig {
                name: "박피엠".into(),
                agent: "claude".into(),
                command: "claude --dangerously-skip-permissions".into(),
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
        (config, root)
    }

    #[test]
    fn test_build_persona_bundle_includes_shared() {
        let dir = TempDir::new().unwrap();
        let (config, root) = make_workspace(&dir);
        let svc = PersonaBundleService::new(&root);
        let bundle = svc.build(&config, "DeveloperBE").unwrap();
        assert!(bundle.content.contains("Shared Persona"), "Shared persona가 포함되어야 함");
        assert!(bundle.content.contains("DeveloperBE"), "역할 persona가 포함되어야 함");
        assert!(bundle.content.contains("READY: DeveloperBE"), "READY 지시문이 포함되어야 함");
    }

    #[test]
    fn test_build_pm_bundle_no_ready_rule() {
        let dir = TempDir::new().unwrap();
        let (config, root) = make_workspace(&dir);
        // PM persona 생성
        std::fs::create_dir_all(root.join("brain/PM")).unwrap();
        std::fs::write(root.join("brain/PM/persona.md"), "# PM\n\nPM 내용").unwrap();

        let svc = PersonaBundleService::new(&root);
        let bundle = svc.build(&config, "PM").unwrap();
        assert!(!bundle.content.contains("READY: PM"), "PM bundle에는 READY 규칙이 없어야 함");
    }

    #[test]
    fn test_content_hash_is_deterministic() {
        let dir = TempDir::new().unwrap();
        let (config, root) = make_workspace(&dir);
        let svc = PersonaBundleService::new(&root);
        let b1 = svc.build(&config, "DeveloperBE").unwrap();
        let b2 = svc.build(&config, "DeveloperBE").unwrap();
        assert_eq!(b1.content_hash, b2.content_hash, "같은 입력이면 hash가 동일해야 함");
    }

    #[test]
    fn test_persona_not_found_error() {
        let dir = TempDir::new().unwrap();
        let (config, root) = make_workspace(&dir);
        let svc = PersonaBundleService::new(&root);
        let result = svc.build(&config, "NonExistentRole");
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().code, "PERSONA_NOT_FOUND");
    }

    #[test]
    fn test_source_files_listed() {
        let dir = TempDir::new().unwrap();
        let (config, root) = make_workspace(&dir);
        let svc = PersonaBundleService::new(&root);
        let bundle = svc.build(&config, "DeveloperBE").unwrap();
        assert!(bundle.source_files.len() >= 2);
        assert!(bundle.source_files.iter().any(|f| f.contains("Shared")));
        assert!(bundle.source_files.iter().any(|f| f.contains("DeveloperBE")));
    }
}
