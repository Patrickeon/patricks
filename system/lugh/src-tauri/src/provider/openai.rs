// provider/openai.rs — OpenAiApiAdapter
// OpenAI Responses API (SSE streaming) 연동
// DS-40 §5

use async_trait::async_trait;
use chrono::Utc;
use futures_util::StreamExt;
use reqwest::header::{AUTHORIZATION, CONTENT_TYPE, HeaderMap, HeaderValue};
use serde_json::Value;
use uuid::Uuid;

use crate::models::{
    error::AppError,
    provider::{
        AiProviderKind, CredentialRef, HealthStatus, MessageRole, ProviderEvent, ProviderHealth,
        ProviderMessage, ProviderMessageRequest, ProviderMessageResult, ProviderSessionRef,
        TokenUsage,
    },
};
use super::{AiProvider, ProviderEventSink, build_http_client, parse_sse_data};

const OPENAI_RESPONSES_URL: &str = "https://api.openai.com/v1/responses";

pub struct OpenAiApiAdapter {
    api_key: String,
    http: reqwest::Client,
}

impl OpenAiApiAdapter {
    pub fn new(api_key: String) -> Self {
        Self {
            api_key,
            http: build_http_client(),
        }
    }

    fn make_headers(&self) -> HeaderMap {
        let mut headers = HeaderMap::new();
        headers.insert(
            AUTHORIZATION,
            HeaderValue::from_str(&format!("Bearer {}", self.api_key))
                .unwrap_or(HeaderValue::from_static("")),
        );
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
        headers
    }

    /// 내부 메시지를 OpenAI Responses API 형식으로 변환 (DS-40 §5.3)
    fn build_request_body(request: &ProviderMessageRequest, streaming: bool) -> serde_json::Value {
        let input: Vec<serde_json::Value> = request
            .messages
            .iter()
            .map(|m| {
                serde_json::json!({
                    "role": match m.role {
                        MessageRole::User => "user",
                        MessageRole::Assistant => "assistant",
                        MessageRole::System => "system",
                    },
                    "content": m.content,
                })
            })
            .collect();

        let mut body = serde_json::json!({
            "model": request.model,
            "instructions": request.system_prompt,
            "input": input,
            "stream": streaming,
        });

        if let Some(max) = request.max_tokens {
            body["max_output_tokens"] = serde_json::Value::Number(max.into());
        }
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
                        "type": "function",
                        "name": t.name,
                        "description": t.description,
                        "parameters": t.parameters,
                    })
                })
                .collect();
            body["tools"] = serde_json::Value::Array(tools);
        }

        body
    }

    /// OpenAI SSE event → 내부 ProviderEvent (DS-40 §5.5)
    fn parse_sse_event(
        event_type: &str,
        data: &Value,
        message_id: &str,
        sequence: &mut u32,
    ) -> Option<ProviderEvent> {
        match event_type {
            "response.created" => Some(ProviderEvent::MessageStarted {
                message_id: message_id.to_string(),
            }),
            "response.output_text.delta" => {
                let delta = data
                    .get("delta")
                    .and_then(|d| d.as_str())
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
            "response.completed" => {
                let usage = data.get("response").and_then(|r| r.get("usage")).map(|u| {
                    TokenUsage {
                        input_tokens: u.get("input_tokens").and_then(|v| v.as_u64()).map(|n| n as u32),
                        output_tokens: u.get("output_tokens").and_then(|v| v.as_u64()).map(|n| n as u32),
                        total_tokens: u.get("total_tokens").and_then(|v| v.as_u64()).map(|n| n as u32),
                    }
                });
                Some(ProviderEvent::MessageCompleted {
                    message_id: message_id.to_string(),
                    usage,
                })
            }
            "response.output_item.added" => {
                // tool call이 있으면 ToolRequested emit
                let item = data.get("item")?;
                if item.get("type").and_then(|t| t.as_str()) == Some("function_call") {
                    let name = item.get("name").and_then(|n| n.as_str()).unwrap_or("unknown");
                    let arguments = item.get("arguments").cloned().unwrap_or(Value::Null);
                    let args: Value = serde_json::from_str(
                        arguments.as_str().unwrap_or("{}"),
                    )
                    .unwrap_or(Value::Null);
                    return Some(ProviderEvent::ToolRequested {
                        tool_name: name.to_string(),
                        arguments: args,
                    });
                }
                None
            }
            "response.failed" | "error" => {
                let msg = data
                    .get("error")
                    .and_then(|e| e.get("message"))
                    .and_then(|m| m.as_str())
                    .unwrap_or("Unknown OpenAI error");
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
impl AiProvider for OpenAiApiAdapter {
    async fn validate_credential(&self, _credential: CredentialRef) -> Result<ProviderHealth, AppError> {
        let start = std::time::Instant::now();
        // 경량 요청으로 인증 확인
        let resp = self.http
            .get("https://api.openai.com/v1/models")
            .headers(self.make_headers())
            .send()
            .await;

        let latency_ms = start.elapsed().as_millis() as u64;

        match resp {
            Ok(r) if r.status().is_success() => Ok(ProviderHealth {
                provider: AiProviderKind::OpenAi,
                status: HealthStatus::Ok,
                latency_ms: Some(latency_ms),
                checked_at: Utc::now(),
                error: None,
            }),
            Ok(r) if r.status().as_u16() == 401 => Err(AppError::auth_failed("OpenAI")),
            Ok(r) => Err(AppError::new("PROVIDER_ERROR", format!("OpenAI HTTP {}", r.status()))),
            Err(e) => Err(AppError::provider_unreachable("OpenAI")
                .with_detail(serde_json::json!({"error": e.to_string()}))),
        }
    }

    async fn start_session(
        &self,
        request: &ProviderMessageRequest,
    ) -> Result<ProviderSessionRef, AppError> {
        Ok(ProviderSessionRef {
            session_id: request.session_id.clone(),
            provider: AiProviderKind::OpenAi,
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
            .post(OPENAI_RESPONSES_URL)
            .headers(self.make_headers())
            .json(&body)
            .send()
            .await
            .map_err(|e| AppError::provider_unreachable("OpenAI")
                .with_detail(serde_json::json!({"error": e.to_string()})))?;

        if !resp.status().is_success() {
            let status = resp.status().as_u16();
            let body_text = resp.text().await.unwrap_or_default();
            return Err(match status {
                401 => AppError::auth_failed("OpenAI"),
                429 => AppError::rate_limited(),
                _ => AppError::new("PROVIDER_ERROR", format!("OpenAI HTTP {}: {}", status, body_text)),
            });
        }

        let _ = sink.send(ProviderEvent::MessageStarted { message_id: message_id.clone() }).await;

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
                        if let Some(event) = Self::parse_sse_event(
                            &current_event_type,
                            &data,
                            &message_id,
                            &mut sequence,
                        ) {
                            if let ProviderEvent::MessageDelta { ref delta, .. } = event {
                                full_content.push_str(delta);
                            }
                            if let ProviderEvent::MessageCompleted { usage: ref u, .. } = event {
                                usage = u.clone();
                            }
                            let _ = sink.send(event).await;
                        }
                    }
                }
            }
        }

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

    #[test]
    fn test_build_request_body() {
        let req = ProviderMessageRequest {
            session_id: "s1".into(),
            provider: AiProviderKind::OpenAi,
            model: "gpt-4.1".into(),
            system_prompt: "instructions".into(),
            messages: vec![ProviderMessage {
                role: MessageRole::User,
                content: "Hello".into(),
            }],
            temperature: Some(0.7),
            max_tokens: Some(2048),
            tools: vec![],
        };
        let body = OpenAiApiAdapter::build_request_body(&req, true);
        assert_eq!(body["model"], "gpt-4.1");
        assert_eq!(body["instructions"], "instructions");
        assert_eq!(body["stream"], true);
    }

    #[test]
    fn test_sse_text_delta() {
        let mut seq = 0u32;
        let data = serde_json::json!({ "delta": "Hello, World!" });
        let event = OpenAiApiAdapter::parse_sse_event(
            "response.output_text.delta",
            &data,
            "msg-1",
            &mut seq,
        );
        assert!(matches!(event, Some(ProviderEvent::MessageDelta { .. })));
    }

    #[test]
    fn test_sse_response_created() {
        let mut seq = 0u32;
        let event = OpenAiApiAdapter::parse_sse_event(
            "response.created",
            &Value::Null,
            "msg-1",
            &mut seq,
        );
        assert!(matches!(event, Some(ProviderEvent::MessageStarted { .. })));
    }
}
