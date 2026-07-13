// models/error.rs — AppError 표준 오류 모델 (DS-20 §3.3, DS-40 §2.4)

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Tauri IPC 및 내부 서비스 공통 오류 구조체
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppError {
    /// 오류 코드 (DS-60 §8 참조)
    pub code: String,
    /// 사람이 읽을 수 있는 오류 메시지
    pub message: String,
    /// 상세 정보 (선택적, API 응답 등)
    pub detail: Option<serde_json::Value>,
    /// 재시도/복구 가능 여부
    pub recoverable: bool,
}

impl AppError {
    pub fn new(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
            detail: None,
            recoverable: false,
        }
    }

    pub fn recoverable(mut self) -> Self {
        self.recoverable = true;
        self
    }

    pub fn with_detail(mut self, detail: serde_json::Value) -> Self {
        self.detail = Some(detail);
        self
    }

    // ---- 자주 쓰는 생성자 ----

    pub fn workspace_not_found(path: impl std::fmt::Display) -> Self {
        Self::new("WORKSPACE_NOT_FOUND", format!("workspace 경로를 찾을 수 없습니다: {}", path))
    }

    pub fn config_invalid(msg: impl Into<String>) -> Self {
        Self::new("CONFIG_INVALID", msg)
    }

    pub fn persona_not_found(role: impl std::fmt::Display) -> Self {
        Self::new("PERSONA_NOT_FOUND", format!("역할 {} persona 파일이 없습니다", role))
    }

    pub fn credential_missing(provider: impl std::fmt::Display) -> Self {
        Self::new("CREDENTIAL_MISSING", format!("{} credential이 없습니다", provider))
            .recoverable()
    }

    pub fn auth_failed(provider: impl std::fmt::Display) -> Self {
        Self::new("AUTH_FAILED", format!("{} 인증에 실패했습니다", provider))
            .recoverable()
    }

    pub fn provider_unreachable(provider: impl std::fmt::Display) -> Self {
        Self::new("PROVIDER_UNREACHABLE", format!("{} endpoint에 도달할 수 없습니다", provider))
            .recoverable()
    }

    pub fn rate_limited() -> Self {
        Self::new("RATE_LIMITED", "rate limit이 초과되었습니다").recoverable()
    }

    pub fn session_not_ready(session_id: impl std::fmt::Display) -> Self {
        Self::new("SESSION_NOT_READY", format!("세션 {}이 ready 상태가 아닙니다", session_id))
            .recoverable()
    }

    /// 세션이 running/booting 중일 때 대화 초기화(clear_session_messages) 시도 시 반환
    /// (DS-60 §3.2, §9, Redmine #24)
    pub fn session_busy(session_id: impl std::fmt::Display) -> Self {
        Self::new(
            "SESSION_BUSY",
            format!("세션 {}이(가) 처리 중입니다(running/booting) — 응답 완료·중지 후 재시도하세요", session_id),
        )
        .recoverable()
    }

    pub fn stream_interrupted() -> Self {
        Self::new("STREAM_INTERRUPTED", "streaming이 중단되었습니다").recoverable()
    }

    pub fn document_write_failed(msg: impl Into<String>) -> Self {
        Self::new("DOCUMENT_WRITE_FAILED", msg)
    }
}

impl std::fmt::Display for AppError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[{}] {}", self.code, self.message)
    }
}

impl std::error::Error for AppError {}

/// thiserror 기반 내부 서비스 오류 → AppError 변환
#[derive(Debug, Error)]
pub enum InternalError {
    #[error("IO 오류: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON 파싱 오류: {0}")]
    Json(#[from] serde_json::Error),

    #[error("YAML 파싱 오류: {0}")]
    Yaml(#[from] serde_yaml::Error),

    #[error("HTTP 오류: {0}")]
    Http(#[from] reqwest::Error),

    #[error("앱 오류: {0}")]
    App(AppError),
}

impl From<InternalError> for AppError {
    fn from(e: InternalError) -> Self {
        match e {
            InternalError::Io(e) => AppError::new("IO_ERROR", e.to_string()),
            InternalError::Json(e) => AppError::new("JSON_ERROR", e.to_string()),
            InternalError::Yaml(e) => AppError::new("YAML_ERROR", e.to_string()),
            InternalError::Http(e) => AppError::new("HTTP_ERROR", e.to_string()).recoverable(),
            InternalError::App(e) => e,
        }
    }
}

/// Tauri command 반환을 위한 Result alias
pub type AppResult<T> = Result<T, AppError>;

impl serde::Serialize for InternalError {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        AppError::from(InternalError::App(AppError::new("INTERNAL", self.to_string())))
            .serialize(s)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_app_error_creation() {
        let e = AppError::credential_missing("claude");
        assert_eq!(e.code, "CREDENTIAL_MISSING");
        assert!(e.recoverable);
    }

    #[test]
    fn test_app_error_display() {
        let e = AppError::workspace_not_found("/tmp/test");
        assert!(e.to_string().contains("WORKSPACE_NOT_FOUND"));
    }

    #[test]
    fn test_app_error_with_detail() {
        let e = AppError::new("TEST", "test error")
            .with_detail(serde_json::json!({"key": "value"}));
        assert!(e.detail.is_some());
    }
}
