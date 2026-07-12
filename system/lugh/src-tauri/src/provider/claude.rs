// provider/claude.rs — ClaudeApiAdapter
// Anthropic Messages API (SSE streaming) 연동
// DS-40 §4

use async_trait::async_trait;
use chrono::Utc;
use futures_util::StreamExt;
use reqwest::header::{HeaderMap, HeaderValue, CONTENT_TYPE};
use serde_json::Value;
use uuid::Uuid;

use crate::models::{
    error::AppError,
    provider::{
        AiProviderKind, CredentialRef, HealthStatus, MessageRole, ProviderContentBlock,
        ProviderEvent, ProviderHealth, ProviderMessage, ProviderMessageRequest,
        ProviderMessageResult, ProviderSessionRef, TokenUsage,
    },
};
use crate::chat_attachment::document_block_text;
use super::{AiProvider, ProviderEventSink, build_http_client, parse_sse_data};

const ANTHROPIC_API_URL: &str = "https://api.anthropic.com/v1/messages";
const ANTHROPIC_VERSION: &str = "2023-06-01";
/// 인증 확인용 경량 요청 모델 (cost 최소화)
const HEALTH_CHECK_MODEL: &str = "claude-3-haiku-20240307";

enum ClaudeAuth {
    ApiKey(String),
    OAuthBearer(String),
}

pub struct ClaudeApiAdapter {
    auth: ClaudeAuth,
    http: reqwest::Client,
}

impl ClaudeApiAdapter {
    pub fn new(api_key: String) -> Self {
        Self {
            auth: ClaudeAuth::ApiKey(api_key),
            http: build_http_client(),
        }
    }

    pub fn new_with_oauth(token: String) -> Self {
        Self {
            auth: ClaudeAuth::OAuthBearer(token),
            http: build_http_client(),
        }
    }

    fn make_headers(&self) -> HeaderMap {
        let mut headers = HeaderMap::new();
        match &self.auth {
            ClaudeAuth::ApiKey(key) => {
                headers.insert(
                    "x-api-key",
                    HeaderValue::from_str(key).unwrap_or(HeaderValue::from_static("")),
                );
            }
            ClaudeAuth::OAuthBearer(token) => {
                let bearer = format!("Bearer {}", token);
                headers.insert(
                    reqwest::header::AUTHORIZATION,
                    HeaderValue::from_str(&bearer).unwrap_or(HeaderValue::from_static("")),
                );
            }
        }
        headers.insert(
            "anthropic-version",
            HeaderValue::from_static(ANTHROPIC_VERSION),
        );
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
        headers
    }

    /// content_blocks가 있으면 Claude Messages API content block 배열로 변환 (DS-40 §4.3, #21)
    /// 텍스트 → {type:text}, 이미지 → {type:image, source:{type:base64,...}},
    /// 문서 → 추출 텍스트를 파일명/미디어 타입 헤더와 함께 text block으로 전달
    fn message_content(m: &ProviderMessage) -> serde_json::Value {
        match &m.content_blocks {
            Some(blocks) if !blocks.is_empty() => {
                let arr: Vec<serde_json::Value> = blocks
                    .iter()
                    .map(|b| match b {
                        ProviderContentBlock::Text { text } => {
                            serde_json::json!({ "type": "text", "text": text })
                        }
                        ProviderContentBlock::Image { media_type, base64_data, .. } => {
                            serde_json::json!({
                                "type": "image",
                                "source": {
                                    "type": "base64",
                                    "media_type": media_type,
                                    "data": base64_data,
                                }
                            })
                        }
                        ProviderContentBlock::DocumentText {
                            filename, media_type, extracted_text, ..
                        } => serde_json::json!({
                            "type": "text",
                            "text": document_block_text(filename, media_type, extracted_text),
                        }),
                    })
                    .collect();
                serde_json::Value::Array(arr)
            }
            _ => serde_json::Value::String(m.content.clone()),
        }
    }

    /// 내부 메시지를 Claude API 형식으로 변환 (DS-40 §4.3)
    fn build_request_body(request: &ProviderMessageRequest, streaming: bool) -> serde_json::Value {
        let messages: Vec<serde_json::Value> = request
            .messages
            .iter()
            .map(|m| {
                serde_json::json!({
                    "role": match m.role {
                        MessageRole::User => "user",
                        MessageRole::Assistant => "assistant",
                        MessageRole::System => "user", // Claude는 system 메시지를 top-level로 처리
                    },
                    "content": Self::message_content(m),
                })
            })
            .collect();

        let mut body = serde_json::json!({
            "model": request.model,
            "system": request.system_prompt,
            "messages": messages,
            "max_tokens": request.max_tokens.unwrap_or(4096),
            "stream": streaming,
        });

        if let Some(temp) = request.temperature {
            body["temperature"] = serde_json::Value::Number(
                serde_json::Number::from_f64(temp as f64).unwrap_or(serde_json::Number::from(1)),
            );
        }

        if !request.tools.is_empty() {
            let tools: Vec<_> = request
                .tools
                .iter()
                .map(|t| {
                    serde_json::json!({
                        "name": t.name,
                        "description": t.description,
                        "input_schema": t.parameters,
                    })
                })
                .collect();
            body["tools"] = serde_json::Value::Array(tools);
        }

        body
    }

    /// SSE event를 내부 ProviderEvent로 변환 (DS-40 §4.5)
    fn parse_sse_event(
        event_type: &str,
        data: &Value,
        message_id: &str,
        sequence: &mut u32,
    ) -> Option<ProviderEvent> {
        match event_type {
            "message_start" => Some(ProviderEvent::MessageStarted {
                message_id: message_id.to_string(),
            }),
            "content_block_delta" => {
                let delta = data
                    .get("delta")
                    .and_then(|d| d.get("text"))
                    .and_then(|t| t.as_str())
                    .unwrap_or("")
                    .to_string();
                if delta.is_empty() {
                    return None;
                }
                *sequence += 1;
                Some(ProviderEvent::MessageDelta {
                    message_id: message_id.to_string(),
                    delta,
                    sequence: *sequence,
                })
            }
            "message_delta" => {
                // usage 정보가 있으면 누적용으로 사용 (MessageCompleted에서 처리)
                None
            }
            "message_stop" => Some(ProviderEvent::MessageCompleted {
                message_id: message_id.to_string(),
                usage: None, // usage는 message_delta에서 별도 추출
            }),
            "tool_use" => {
                let name = data.get("name").and_then(|n| n.as_str()).unwrap_or("unknown");
                let input = data.get("input").cloned().unwrap_or(serde_json::Value::Null);
                Some(ProviderEvent::ToolRequested {
                    tool_name: name.to_string(),
                    arguments: input,
                })
            }
            "error" => {
                let msg = data
                    .get("error")
                    .and_then(|e| e.get("message"))
                    .and_then(|m| m.as_str())
                    .unwrap_or("Unknown Claude error");
                Some(ProviderEvent::MessageFailed {
                    message_id: message_id.to_string(),
                    error: AppError::new("PROVIDER_ERROR", msg).recoverable(),
                })
            }
            _ => None,
        }
    }
}

#[async_trait]
impl AiProvider for ClaudeApiAdapter {
    async fn validate_credential(&self, credential: CredentialRef) -> Result<ProviderHealth, AppError> {
        let start = std::time::Instant::now();
        // 최소 비용 요청으로 인증 검증
        let body = serde_json::json!({
            "model": HEALTH_CHECK_MODEL,
            "max_tokens": 1,
            "messages": [{"role": "user", "content": "ping"}],
        });

        let resp = self.http
            .post(ANTHROPIC_API_URL)
            .headers(self.make_headers())
            .json(&body)
            .send()
            .await;

        let latency_ms = start.elapsed().as_millis() as u64;

        match resp {
            Ok(r) if r.status().is_success() || r.status().as_u16() == 400 => {
                // 400은 bad request지만 인증은 통과된 상태
                Ok(ProviderHealth {
                    provider: AiProviderKind::Claude,
                    status: HealthStatus::Ok,
                    latency_ms: Some(latency_ms),
                    checked_at: Utc::now(),
                    error: None,
                })
            }
            Ok(r) if r.status().as_u16() == 401 => {
                Err(AppError::auth_failed("Claude"))
            }
            Ok(r) if r.status().as_u16() == 529 || r.status().as_u16() == 503 => {
                Ok(ProviderHealth {
                    provider: AiProviderKind::Claude,
                    status: HealthStatus::Degraded,
                    latency_ms: Some(latency_ms),
                    checked_at: Utc::now(),
                    error: None,
                })
            }
            Ok(r) => Err(AppError::new(
                "PROVIDER_ERROR",
                format!("Claude 예상치 못한 상태: {}", r.status()),
            )),
            Err(e) => Err(AppError::provider_unreachable("Claude")
                .with_detail(serde_json::json!({"error": e.to_string()}))),
        }
    }

    async fn start_session(
        &self,
        request: &ProviderMessageRequest,
    ) -> Result<ProviderSessionRef, AppError> {
        // Claude는 stateless — 세션 참조만 반환
        Ok(ProviderSessionRef {
            session_id: request.session_id.clone(),
            provider: AiProviderKind::Claude,
            model: request.model.clone(),
        })
    }

    async fn send_message_stream(
        &self,
        request: ProviderMessageRequest,
        sink: ProviderEventSink,
    ) -> Result<ProviderMessageResult, AppError> {
        let message_id = Uuid::new_v4().to_string();
        let body = Self::build_request_body(&request, true);
        let mut sequence = 0u32;
        let mut full_content = String::new();
        let mut usage: Option<TokenUsage> = None;

        let resp = self.http
            .post(ANTHROPIC_API_URL)
            .headers(self.make_headers())
            .json(&body)
            .send()
            .await
            .map_err(|e| AppError::provider_unreachable("Claude")
                .with_detail(serde_json::json!({"error": e.to_string()})))?;

        if !resp.status().is_success() {
            let status = resp.status().as_u16();
            let body_text = resp.text().await.unwrap_or_default();
            return Err(match status {
                401 => AppError::auth_failed("Claude"),
                429 => AppError::rate_limited(),
                _ => AppError::new("PROVIDER_ERROR", format!("Claude HTTP {}: {}", status, body_text)),
            });
        }

        // MessageStarted 이벤트 emit
        let _ = sink.send(ProviderEvent::MessageStarted { message_id: message_id.clone() }).await;

        // SSE 스트림 처리
        let mut stream = resp.bytes_stream();
        let mut buffer = String::new();
        let mut current_event_type = String::new();

        while let Some(chunk) = stream.next().await {
            let chunk = chunk.map_err(|_| AppError::stream_interrupted())?;
            buffer.push_str(&String::from_utf8_lossy(&chunk));

            while let Some(newline_pos) = buffer.find('\n') {
                let line = buffer[..newline_pos].trim().to_string();
                buffer = buffer[newline_pos + 1..].to_string();

                if line.starts_with("event: ") {
                    current_event_type = line.trim_start_matches("event: ").to_string();
                } else if let Some(data_str) = parse_sse_data(&line) {
                    if data_str == "[DONE]" {
                        break;
                    }
                    if let Ok(data) = serde_json::from_str::<Value>(data_str) {
                        // usage 추출 (message_delta 이벤트)
                        if current_event_type == "message_delta" {
                            if let Some(u) = data.get("usage") {
                                usage = Some(TokenUsage {
                                    input_tokens: u.get("input_tokens").and_then(|v| v.as_u64()).map(|n| n as u32),
                                    output_tokens: u.get("output_tokens").and_then(|v| v.as_u64()).map(|n| n as u32),
                                    total_tokens: None,
                                });
                            }
                        }

                        if let Some(event) = Self::parse_sse_event(
                            &current_event_type,
                            &data,
                            &message_id,
                            &mut sequence,
                        ) {
                            // delta는 full_content에 누적
                            if let ProviderEvent::MessageDelta { ref delta, .. } = event {
                                full_content.push_str(delta);
                            }
                            let _ = sink.send(event).await;
                        }
                    }
                }
            }
        }

        // MessageCompleted emit
        let _ = sink.send(ProviderEvent::MessageCompleted {
            message_id: message_id.clone(),
            usage: usage.clone(),
        }).await;

        Ok(ProviderMessageResult {
            message_id,
            content: full_content,
            usage,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::provider::ToolDefinition;

    #[test]
    fn test_build_request_body_basic() {
        let req = ProviderMessageRequest {
            session_id: "s1".into(),
            provider: AiProviderKind::Claude,
            model: "claude-3-5-sonnet-latest".into(),
            system_prompt: "You are a helpful assistant".into(),
            messages: vec![ProviderMessage {
                role: MessageRole::User,
                content: "Hello".into(),
                content_blocks: None,
            }],
            attachments: vec![],
            temperature: None,
            max_tokens: Some(1024),
            tools: vec![],
        };
        let body = ClaudeApiAdapter::build_request_body(&req, true);
        assert_eq!(body["model"], "claude-3-5-sonnet-latest");
        assert_eq!(body["stream"], true);
        assert_eq!(body["max_tokens"], 1024);
        // content_blocks 없으면 기존 문자열 content 유지 (하위 호환)
        assert_eq!(body["messages"][0]["content"], "Hello");
    }

    #[test]
    fn test_build_request_body_with_content_blocks() {
        // #21: 이미지 base64 content block + 문서 추출 텍스트 변환 (DS-40 §4.4)
        let req = ProviderMessageRequest {
            session_id: "s1".into(),
            provider: AiProviderKind::Claude,
            model: "claude-3-5-sonnet-latest".into(),
            system_prompt: "system".into(),
            messages: vec![ProviderMessage {
                role: MessageRole::User,
                content: "이미지를 검토해 주세요.".into(),
                content_blocks: Some(vec![
                    ProviderContentBlock::Text { text: "이미지를 검토해 주세요.".into() },
                    ProviderContentBlock::Image {
                        attachment_id: "att-1".into(),
                        media_type: "image/png".into(),
                        base64_data: "iVBORw0KGgo=".into(),
                    },
                    ProviderContentBlock::DocumentText {
                        attachment_id: "att-2".into(),
                        filename: "요구사항.md".into(),
                        media_type: "text/markdown".into(),
                        extracted_text: "본문".into(),
                        truncated: false,
                    },
                ]),
            }],
            attachments: vec![],
            temperature: None,
            max_tokens: Some(1024),
            tools: vec![],
        };
        let body = ClaudeApiAdapter::build_request_body(&req, true);
        let content = &body["messages"][0]["content"];
        assert!(content.is_array());
        assert_eq!(content[0]["type"], "text");
        assert_eq!(content[1]["type"], "image");
        assert_eq!(content[1]["source"]["type"], "base64");
        assert_eq!(content[1]["source"]["media_type"], "image/png");
        assert_eq!(content[1]["source"]["data"], "iVBORw0KGgo=");
        assert_eq!(content[2]["type"], "text");
        assert_eq!(
            content[2]["text"],
            "[첨부 문서: 요구사항.md, text/markdown]\n본문"
        );
    }

    #[test]
    fn test_build_request_body_with_tools() {
        let req = ProviderMessageRequest {
            session_id: "s1".into(),
            provider: AiProviderKind::Claude,
            model: "claude-3-5-sonnet-latest".into(),
            system_prompt: "system".into(),
            messages: vec![],
            attachments: vec![],
            temperature: Some(0.5),
            max_tokens: None,
            tools: vec![ToolDefinition {
                name: "bash".into(),
                description: "run bash".into(),
                parameters: serde_json::json!({"type": "object"}),
            }],
        };
        let body = ClaudeApiAdapter::build_request_body(&req, true);
        assert!(body.get("tools").is_some());
        assert_eq!(body["tools"][0]["name"], "bash");
    }

    #[test]
    fn test_sse_event_message_started() {
        let mut seq = 0u32;
        let event = ClaudeApiAdapter::parse_sse_event(
            "message_start",
            &serde_json::Value::Null,
            "msg-1",
            &mut seq,
        );
        assert!(matches!(event, Some(ProviderEvent::MessageStarted { .. })));
    }

    #[test]
    fn test_sse_event_content_delta() {
        let mut seq = 0u32;
        let data = serde_json::json!({
            "delta": { "type": "text_delta", "text": "Hello" }
        });
        let event = ClaudeApiAdapter::parse_sse_event("content_block_delta", &data, "msg-1", &mut seq);
        assert!(matches!(event, Some(ProviderEvent::MessageDelta { ref delta, .. }) if delta == "Hello"));
        assert_eq!(seq, 1);
    }
}
