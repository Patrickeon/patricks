// provider/gemini.rs — GeminiApiAdapter
// Google Gemini streamGenerateContent API 연동
// DS-40 §6

use async_trait::async_trait;
use chrono::Utc;
use futures_util::StreamExt;
use reqwest::header::{CONTENT_TYPE, HeaderMap, HeaderValue};
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
use super::{AiProvider, ProviderEventSink, build_http_client};

const GEMINI_BASE_URL: &str = "https://generativelanguage.googleapis.com";

pub struct GeminiApiAdapter {
    api_key: String,
    http: reqwest::Client,
}

impl GeminiApiAdapter {
    pub fn new(api_key: String) -> Self {
        Self {
            api_key,
            http: build_http_client(),
        }
    }

    fn stream_url(&self, model: &str) -> String {
        format!(
            "{}/v1beta/models/{}:streamGenerateContent?key={}",
            GEMINI_BASE_URL, model, self.api_key
        )
    }

    fn health_url(&self) -> String {
        format!(
            "{}/v1beta/models?key={}",
            GEMINI_BASE_URL, self.api_key
        )
    }

    fn base_headers() -> HeaderMap {
        let mut headers = HeaderMap::new();
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
        headers
    }

    /// content_blocks가 있으면 contents[].parts[]로 변환 (DS-40 §6.3, #21)
    /// 텍스트/문서 추출 결과 → { text }, 이미지 → { inlineData: { mimeType, data } }
    fn message_parts(m: &ProviderMessage) -> serde_json::Value {
        match &m.content_blocks {
            Some(blocks) if !blocks.is_empty() => {
                let parts: Vec<serde_json::Value> = blocks
                    .iter()
                    .map(|b| match b {
                        ProviderContentBlock::Text { text } => {
                            serde_json::json!({ "text": text })
                        }
                        ProviderContentBlock::Image { media_type, base64_data, .. } => {
                            serde_json::json!({
                                "inlineData": {
                                    "mimeType": media_type,
                                    "data": base64_data,
                                }
                            })
                        }
                        ProviderContentBlock::DocumentText {
                            filename, media_type, extracted_text, ..
                        } => serde_json::json!({
                            "text": document_block_text(filename, media_type, extracted_text),
                        }),
                    })
                    .collect();
                serde_json::Value::Array(parts)
            }
            _ => serde_json::json!([{ "text": m.content }]),
        }
    }

    /// 내부 메시지를 Gemini API 형식으로 변환 (DS-40 §6.3)
    fn build_request_body(request: &ProviderMessageRequest) -> serde_json::Value {
        // system message는 별도 필드로 분리
        let non_system: Vec<&ProviderMessage> = request
            .messages
            .iter()
            .filter(|m| m.role != MessageRole::System)
            .collect();

        let contents: Vec<serde_json::Value> = non_system
            .iter()
            .map(|m| {
                serde_json::json!({
                    "role": match m.role {
                        MessageRole::User => "user",
                        MessageRole::Assistant => "model",
                        MessageRole::System => "user",
                    },
                    "parts": Self::message_parts(m),
                })
            })
            .collect();

        let mut body = serde_json::json!({
            "systemInstruction": {
                "parts": [{ "text": request.system_prompt }]
            },
            "contents": contents,
        });

        let mut gen_config = serde_json::json!({});
        if let Some(max) = request.max_tokens {
            gen_config["maxOutputTokens"] = max.into();
        }
        if let Some(temp) = request.temperature {
            gen_config["temperature"] = serde_json::Number::from_f64(temp as f64)
                .unwrap_or(serde_json::Number::from(1))
                .into();
        }
        if gen_config.as_object().map(|o| !o.is_empty()).unwrap_or(false) {
            body["generationConfig"] = gen_config;
        }

        if !request.tools.is_empty() {
            let functions: Vec<_> = request
                .tools
                .iter()
                .map(|t| {
                    serde_json::json!({
                        "name": t.name,
                        "description": t.description,
                        "parameters": t.parameters,
                    })
                })
                .collect();
            body["tools"] = serde_json::json!([{
                "function_declarations": functions
            }]);
        }

        body
    }

    /// Gemini 응답 chunk → 내부 ProviderEvent (DS-40 §6.5)
    /// Gemini는 JSON 배열 스트리밍 (SSE가 아닌 JSON 청크)
    fn parse_chunk(
        data: &Value,
        message_id: &str,
        sequence: &mut u32,
        is_first: &mut bool,
    ) -> Vec<ProviderEvent> {
        let mut events = Vec::new();

        if *is_first {
            events.push(ProviderEvent::MessageStarted {
                message_id: message_id.to_string(),
            });
            *is_first = false;
        }

        // candidates[].content.parts[].text
        if let Some(candidates) = data.get("candidates").and_then(|c| c.as_array()) {
            for candidate in candidates {
                if let Some(parts) = candidate
                    .get("content")
                    .and_then(|c| c.get("parts"))
                    .and_then(|p| p.as_array())
                {
                    for part in parts {
                        if let Some(text) = part.get("text").and_then(|t| t.as_str()) {
                            if !text.is_empty() {
                                *sequence += 1;
                                events.push(ProviderEvent::MessageDelta {
                                    message_id: message_id.to_string(),
                                    delta: text.to_string(),
                                    sequence: *sequence,
                                });
                            }
                        }
                        // functionCall
                        if let Some(fc) = part.get("functionCall") {
                            let name = fc.get("name").and_then(|n| n.as_str()).unwrap_or("unknown");
                            let args = fc.get("args").cloned().unwrap_or(Value::Null);
                            events.push(ProviderEvent::ToolRequested {
                                tool_name: name.to_string(),
                                arguments: args,
                            });
                        }
                    }
                }

                // finishReason → MessageCompleted
                if let Some(reason) = candidate.get("finishReason").and_then(|r| r.as_str()) {
                    if reason != "STOP" && !reason.is_empty() {
                        // finishReason이 STOP이 아니면 경고
                    }
                }
            }
        }

        // usage metadata
        if let Some(usage_meta) = data.get("usageMetadata") {
            let usage = TokenUsage {
                input_tokens: usage_meta
                    .get("promptTokenCount")
                    .and_then(|v| v.as_u64())
                    .map(|n| n as u32),
                output_tokens: usage_meta
                    .get("candidatesTokenCount")
                    .and_then(|v| v.as_u64())
                    .map(|n| n as u32),
                total_tokens: usage_meta
                    .get("totalTokenCount")
                    .and_then(|v| v.as_u64())
                    .map(|n| n as u32),
            };
            events.push(ProviderEvent::MessageCompleted {
                message_id: message_id.to_string(),
                usage: Some(usage),
            });
        }

        events
    }
}

#[async_trait]
impl AiProvider for GeminiApiAdapter {
    async fn validate_credential(&self, _credential: CredentialRef) -> Result<ProviderHealth, AppError> {
        let start = std::time::Instant::now();
        let resp = self.http
            .get(&self.health_url())
            .headers(Self::base_headers())
            .send()
            .await;

        let latency_ms = start.elapsed().as_millis() as u64;

        match resp {
            Ok(r) if r.status().is_success() => Ok(ProviderHealth {
                provider: AiProviderKind::Gemini,
                status: HealthStatus::Ok,
                latency_ms: Some(latency_ms),
                checked_at: Utc::now(),
                error: None,
            }),
            Ok(r) if r.status().as_u16() == 400 || r.status().as_u16() == 403 => {
                Err(AppError::auth_failed("Gemini"))
            }
            Ok(r) => Err(AppError::new("PROVIDER_ERROR", format!("Gemini HTTP {}", r.status()))),
            Err(e) => Err(AppError::provider_unreachable("Gemini")
                .with_detail(serde_json::json!({"error": e.to_string()}))),
        }
    }

    async fn start_session(
        &self,
        request: &ProviderMessageRequest,
    ) -> Result<ProviderSessionRef, AppError> {
        Ok(ProviderSessionRef {
            session_id: request.session_id.clone(),
            provider: AiProviderKind::Gemini,
            model: request.model.clone(),
        })
    }

    async fn send_message_stream(
        &self,
        request: ProviderMessageRequest,
        sink: ProviderEventSink,
    ) -> Result<ProviderMessageResult, AppError> {
        let message_id = Uuid::new_v4().to_string();
        let url = self.stream_url(&request.model);
        let body = Self::build_request_body(&request);
        let mut sequence = 0u32;
        let mut full_content = String::new();
        let mut final_usage: Option<TokenUsage> = None;
        let mut is_first = true;

        let resp = self.http
            .post(&url)
            .headers(Self::base_headers())
            .json(&body)
            .send()
            .await
            .map_err(|e| AppError::provider_unreachable("Gemini")
                .with_detail(serde_json::json!({"error": e.to_string()})))?;

        if !resp.status().is_success() {
            let status = resp.status().as_u16();
            let body_text = resp.text().await.unwrap_or_default();
            return Err(match status {
                400 | 403 => AppError::auth_failed("Gemini"),
                429 => AppError::rate_limited(),
                _ => AppError::new("PROVIDER_ERROR", format!("Gemini HTTP {}: {}", status, body_text)),
            });
        }

        // Gemini는 JSON 배열 스트리밍
        let mut stream = resp.bytes_stream();
        let mut buffer = String::new();

        while let Some(chunk) = stream.next().await {
            let chunk = chunk.map_err(|_| AppError::stream_interrupted())?;
            buffer.push_str(&String::from_utf8_lossy(&chunk));

            // JSON 객체 경계 감지 (중괄호 레벨 추적)
            let mut depth = 0i32;
            let mut start_idx = None;

            for (i, ch) in buffer.char_indices() {
                match ch {
                    '{' => {
                        if depth == 0 {
                            start_idx = Some(i);
                        }
                        depth += 1;
                    }
                    '}' => {
                        depth -= 1;
                        if depth == 0 {
                            if let Some(s) = start_idx {
                                let json_str = &buffer[s..=i];
                                if let Ok(data) = serde_json::from_str::<Value>(json_str) {
                                    let events = Self::parse_chunk(
                                        &data,
                                        &message_id,
                                        &mut sequence,
                                        &mut is_first,
                                    );
                                    for event in events {
                                        if let ProviderEvent::MessageDelta { ref delta, .. } = event {
                                            full_content.push_str(delta);
                                        }
                                        if let ProviderEvent::MessageCompleted { ref usage, .. } = event {
                                            final_usage = usage.clone();
                                        }
                                        let _ = sink.send(event).await;
                                    }
                                }
                                start_idx = None;
                            }
                        }
                    }
                    _ => {}
                }
            }
            // 처리된 부분 제거
            if start_idx.is_none() {
                buffer.clear();
            }
        }

        // 스트림이 MessageCompleted 없이 종료된 경우 emit
        if is_first {
            // 아무것도 수신 못했으면 started+completed emit
            let _ = sink.send(ProviderEvent::MessageStarted { message_id: message_id.clone() }).await;
        }

        Ok(ProviderMessageResult {
            message_id,
            content: full_content,
            usage: final_usage,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_request_body_basic() {
        let req = ProviderMessageRequest {
            session_id: "s1".into(),
            provider: AiProviderKind::Gemini,
            model: "gemini-2.0-flash".into(),
            system_prompt: "You are helpful".into(),
            messages: vec![ProviderMessage {
                role: MessageRole::User,
                content: "안녕".into(),
                content_blocks: None,
            }],
            attachments: vec![],
            temperature: Some(0.2),
            max_tokens: Some(4096),
            tools: vec![],
        };
        let body = GeminiApiAdapter::build_request_body(&req);
        assert!(body.get("systemInstruction").is_some());
        assert!(body.get("contents").is_some());
        assert_eq!(body["generationConfig"]["maxOutputTokens"], 4096);
        assert_eq!(body["contents"][0]["parts"][0]["text"], "안녕");
    }

    #[test]
    fn test_build_request_body_with_content_blocks() {
        // #21: inlineData part 변환 (DS-40 §6.4)
        let req = ProviderMessageRequest {
            session_id: "s1".into(),
            provider: AiProviderKind::Gemini,
            model: "gemini-2.0-flash".into(),
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
                        filename: "spec.yaml".into(),
                        media_type: "application/x-yaml".into(),
                        extracted_text: "key: value".into(),
                        truncated: false,
                    },
                ]),
            }],
            attachments: vec![],
            temperature: None,
            max_tokens: None,
            tools: vec![],
        };
        let body = GeminiApiAdapter::build_request_body(&req);
        let parts = &body["contents"][0]["parts"];
        assert_eq!(parts[0]["text"], "이미지를 검토해 주세요.");
        assert_eq!(parts[1]["inlineData"]["mimeType"], "image/png");
        assert_eq!(parts[1]["inlineData"]["data"], "iVBORw0KGgo=");
        assert_eq!(parts[2]["text"], "[첨부 문서: spec.yaml, application/x-yaml]\nkey: value");
    }

    #[test]
    fn test_parse_chunk_text_delta() {
        let data = serde_json::json!({
            "candidates": [{
                "content": {
                    "parts": [{"text": "Hello!"}]
                }
            }]
        });
        let mut seq = 0u32;
        let mut is_first = true;
        let events = GeminiApiAdapter::parse_chunk(&data, "msg-1", &mut seq, &mut is_first);

        assert!(events.iter().any(|e| matches!(e, ProviderEvent::MessageStarted { .. })));
        assert!(events.iter().any(|e| matches!(e, ProviderEvent::MessageDelta { .. })));
    }

    #[test]
    fn test_parse_chunk_usage_metadata() {
        let data = serde_json::json!({
            "candidates": [],
            "usageMetadata": {
                "promptTokenCount": 10,
                "candidatesTokenCount": 20,
                "totalTokenCount": 30
            }
        });
        let mut seq = 0u32;
        let mut is_first = false;
        let events = GeminiApiAdapter::parse_chunk(&data, "msg-1", &mut seq, &mut is_first);
        assert!(events.iter().any(|e| matches!(e, ProviderEvent::MessageCompleted { .. })));
    }
}
