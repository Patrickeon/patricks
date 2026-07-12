// provider/mod.rs — AI Provider Adapter trait 및 팩토리
// DS-20 §6.2, DS-40 §3

pub mod claude;
pub mod claude_cli;
pub mod gemini;
pub mod openai;

use async_trait::async_trait;

use crate::models::{
    error::AppError,
    provider::{
        AiProviderKind, CredentialRef, ProviderEvent, ProviderHealth,
        ProviderMessageRequest, ProviderMessageResult, ProviderSessionRef,
    },
};

/// provider 이벤트를 frontend로 emit하는 싱크 타입
pub type ProviderEventSink = tokio::sync::mpsc::Sender<ProviderEvent>;

/// AI Provider Adapter trait (DS-20 §6.2, DS-40 §3.1)
/// 모든 provider adapter는 이 trait을 구현한다.
#[async_trait]
pub trait AiProvider: Send + Sync {
    /// 이미지(vision) content block 입력 지원 여부 (Redmine #21, DS-40 §7.3)
    /// 미지원 provider는 이미지 첨부 시 전송 전 ATTACHMENT_UNSUPPORTED로 거절된다.
    fn supports_vision(&self) -> bool {
        true
    }

    /// provider credential이 유효한지 검증한다.
    async fn validate_credential(&self, credential: CredentialRef) -> Result<ProviderHealth, AppError>;

    /// provider와 세션을 시작한다 (stateless provider는 참조만 반환).
    async fn start_session(
        &self,
        request: &ProviderMessageRequest,
    ) -> Result<ProviderSessionRef, AppError>;

    /// 메시지를 전송하고 streaming response를 sink로 emit한다.
    /// 각 chunk는 ProviderEvent로 변환되어 sink로 전달된다.
    async fn send_message_stream(
        &self,
        request: ProviderMessageRequest,
        sink: ProviderEventSink,
    ) -> Result<ProviderMessageResult, AppError>;
}

/// provider 종류에 따라 adapter 인스턴스를 반환하는 팩토리
/// Redmine은 AI provider가 아니므로 이 함수를 호출하지 말 것
pub fn create_provider(
    kind: &AiProviderKind,
    api_key: String,
) -> Box<dyn AiProvider> {
    match kind {
        AiProviderKind::Claude => Box::new(claude::ClaudeApiAdapter::new(api_key)),
        AiProviderKind::OpenAi => Box::new(openai::OpenAiApiAdapter::new(api_key)),
        AiProviderKind::Gemini => Box::new(gemini::GeminiApiAdapter::new(api_key)),
        AiProviderKind::Redmine => {
            // Redmine은 AI provider가 아닙니다.
            // validate_credential에서 먼저 분기하므로 여기에 도달하면 프로그래밍 오류입니다.
            panic!("Redmine은 AI provider가 아닙니다 — validate_credential에서 먼저 분기해야 합니다")
        }
    }
}

/// SSE 라인 파싱 유틸리티 (claude, openai 공용)
pub(crate) fn parse_sse_data(line: &str) -> Option<&str> {
    line.strip_prefix("data: ")
}

/// 공통 reqwest Client 설정
pub(crate) fn build_http_client() -> reqwest::Client {
    reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(60))
        .user_agent(format!("AgiTeamBuilder/{}", env!("CARGO_PKG_VERSION")))
        .build()
        .unwrap_or_default()
}
