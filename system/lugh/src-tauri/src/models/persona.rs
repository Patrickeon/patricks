// models/persona.rs — PersonaBundle, PersonaBundlePreview (DS-20 §3.1, DS-60 §3.3)

use serde::{Deserialize, Serialize};

/// 생성된 persona bundle (메모리 내 표현, 임시 파일 불필요)
#[derive(Debug, Clone)]
pub struct PersonaBundle {
    pub role: String,
    /// Shared persona + 역할 persona + 부팅 대기 규칙 결합 내용
    pub content: String,
    /// SHA-256(content) hex
    pub content_hash: String,
    /// 참조된 소스 파일 경로 목록
    pub source_files: Vec<String>,
}

/// build_persona_bundle command 응답 (DS-60 §3.3)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersonaBundlePreview {
    pub role: String,
    pub content_hash: String,
    pub content: String,
    pub source_files: Vec<String>,
}

impl From<PersonaBundle> for PersonaBundlePreview {
    fn from(b: PersonaBundle) -> Self {
        Self {
            role: b.role,
            content_hash: b.content_hash,
            content: b.content,
            source_files: b.source_files,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_persona_bundle_preview_from() {
        let bundle = PersonaBundle {
            role: "DeveloperBE".into(),
            content: "test content".into(),
            content_hash: "abc123".into(),
            source_files: vec!["brain/Shared/persona.md".into()],
        };
        let preview: PersonaBundlePreview = bundle.into();
        assert_eq!(preview.role, "DeveloperBE");
        assert_eq!(preview.content_hash, "abc123");
    }
}
