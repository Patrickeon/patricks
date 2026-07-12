---
doc: DS-40_API명세서
version: v0.10
last_updated: 2026-07-12
status: draft
author: Architect
---

# DS-40 API 명세서 - AI Provider 및 Redmine 연동

## 개정이력

| 버전 | 일자 | 작성자 | 내용 |
|------|------|--------|------|
| v0.1 | 2026-06-23 | Architect | Claude/OpenAI/Gemini/Redmine API 연동 구조 및 Rust reqwest 기반 HTTP/SSE 명세 최초 작성 |
| v0.2 | 2026-07-02 | Architect | Browser command API 명세 추가 |
| v0.3 | 2026-07-02 | Architect | 시스템 브라우저 열기 command 삭제 반영, 임베디드 브라우저 command 4종 기준으로 현행화 |
| v0.4 | 2026-07-02 | Architect | `browser_back`, `browser_forward`, `browser:navigation` 이벤트와 Redmine 역할별 API key 선택 규격 반영 |
| v0.5 | 2026-07-04 | Architect | `fetch_url_content` command, `FetchedPage` 스키마, fetch 오류 코드 추가 |
| v0.6 | 2026-07-08 | Architect | Redmine #21 채팅 입력 이미지 첨부·문서 붙여넣기 API 규격 초안 추가 |
| v0.7 | 2026-07-09 | Architect | Redmine #21 backend 구현 완료 반영: §7 첨부 전처리 규격 현행 상태 명시, §3.2 provider.rs `attachments`/`content_blocks`/`ProviderContentBlock`/`ProviderAttachment` 현행 반영 기술로 갱신 (DS-60 v0.9와 교차 정합) |
| v0.8 | 2026-07-11 | Architect | Redmine #17 설계전환: 임베디드 브라우저를 main 콘텐츠 영역 강제 핀 고정(child+좌표추종) 방식에서 **독립 이동 가능 창**으로 전환. §9 개요·URL규칙·`browser_open`(geometry 선택적 초기값화)·`browser:navigation`·정합성 항목 개정, `browser_resize` 폐기예정 표기. 구현 대기(전환상태 §9.1 명시), DS-60 v0.10과 교차 정합 |
| v0.9 | 2026-07-11 | Architect | Redmine #17 **구현 완료 현행화**(BE/FE 구현 종료): §9.1 전환상태 주석 → 전환 완료(2026-07-11)·command 표에서 `browser_resize` 삭제, `browser_open` parent 제거·decorations/resizable true·geometry 전 인자 optional(기본 960×720) 확정, §9.8 `browser_resize` 상세절 삭제(command 완전 제거), §9.10 프론트 `ResizeObserver`/`resizeBrowser` 제거 확정, §9.11 목표/현행 구분 문단을 전환 완료 현행 기준으로 재작성. DS-60 v0.11과 교차 정합 |
| v0.10 | 2026-07-12 | Architect | Redmine #23 **Claude OAuth/CLI 이미지 vision 지원 구현 완료 현행화**: §7.3 provider capability 표의 `Claude CLI/OAuth shell provider` 행을 'MVP 이미지 미지원 → `ATTACHMENT_UNSUPPORTED`'에서 'stream-json content 배열로 이미지 base64 전달(Anthropic Messages 스키마, #23), vision 미지원 모델만 `ATTACHMENT_UNSUPPORTED`'로 갱신. `claude_cli` `supports_vision=true` 반영 |

---

## 1. 문서 개요

### 1.1 목적

본 문서는 AgiTeamBuilder GUI의 Rust 백엔드가 외부 API 및 Tauri command API와 연동하는 방식을 정의한다. 대상 API는 Claude API, OpenAI API, Gemini API, Redmine API, Browser command API이며, 외부 HTTP 호출과 OS/WebView 연동은 Tauri Rust backend에서 수행한다.

### 1.2 입력 산출물

| 산출물 | 참조 내용 |
|--------|-----------|
| DS-20 아키텍처설계서 | AI Provider Adapter Layer, CredentialStoreService, HealthCheckService, AgentSessionService |

### 1.3 공통 원칙

- Vue 프론트엔드는 외부 API를 직접 호출하지 않는다.
- API key/token은 OS Credential Vault에서 Rust backend만 읽는다.
- Provider별 요청/응답 차이는 Adapter 내부에서 표준 모델로 변환한다.
- Streaming 응답은 Rust에서 수신 후 Tauri event로 frontend에 전달한다.
- Provider 오류는 `AppError`로 정규화한다.
- 브라우저/OS 연동은 Tauri command로만 호출하고, 프론트엔드는 `invoke` 요청과 UI 상태 관리만 수행한다.

---

## 2. 공통 HTTP 클라이언트 설계

### 2.1 Rust 모듈 구조

```text
src-tauri/src/
  http/
    client.rs
    retry.rs
    sse.rs
  providers/
    mod.rs
    claude.rs
    openai.rs
    gemini.rs
    redmine.rs
  models/
    provider.rs
    message.rs
    error.rs
```

### 2.2 reqwest Client 기본 설정

| 항목 | 값 |
|------|----|
| HTTP client | `reqwest::Client` |
| Timeout | 기본 60초. Streaming 요청은 read timeout 별도 확장 |
| Redirect | 기본 정책 허용. Redmine 내부망 호출은 동일 host만 허용 |
| TLS | OS 기본 trust store |
| Proxy | OS/env proxy 정책 검토. MVP는 기본 reqwest 정책 |
| User-Agent | `AgiTeamBuilder/<app_version>` |

### 2.3 공통 Header

| Header | 값 | 적용 |
|--------|----|------|
| `User-Agent` | `AgiTeamBuilder/<version>` | 전체 |
| `Content-Type` | `application/json` | JSON 요청 |
| `Accept` | `application/json` | 일반 JSON 응답 |
| `Accept` | `text/event-stream` | SSE streaming |

### 2.4 공통 오류 모델

```rust
pub struct AppError {
    pub code: String,
    pub message: String,
    pub detail: Option<serde_json::Value>,
    pub recoverable: bool,
}
```

| 코드 | 의미 | 복구 가능 |
|------|------|-----------|
| `CREDENTIAL_MISSING` | provider credential 없음 | true |
| `AUTH_FAILED` | API key/token 인증 실패 | true |
| `PROVIDER_UNREACHABLE` | endpoint 도달 불가 | true |
| `RATE_LIMITED` | rate limit 초과 | true |
| `REQUEST_INVALID` | 요청 schema 오류 | false |
| `STREAM_INTERRUPTED` | SSE/stream 중단 | true |
| `PROVIDER_ERROR` | provider가 5xx 또는 비표준 오류 반환 | true |
| `REDMINE_ERROR` | Redmine API 오류 | true |
| `ATTACHMENT_UNSUPPORTED` | 선택 provider/model이 첨부 유형을 지원하지 않음 | true |
| `ATTACHMENT_TOO_LARGE` | 첨부 원본 또는 추출 결과가 허용 한도를 초과함 | true |
| `ATTACHMENT_EXTRACT_FAILED` | 문서 내용 추출 실패 | true |
| `ATTACHMENT_INVALID_TYPE` | 허용되지 않은 확장자 또는 MIME type | false |

### 2.5 재시도 정책

| 상황 | 정책 |
|------|------|
| 네트워크 일시 실패 | exponential backoff, 최대 3회 |
| HTTP 429 | provider `Retry-After` 우선, 없으면 backoff |
| HTTP 5xx | 최대 2회 재시도 |
| HTTP 4xx 인증/권한 | 재시도하지 않음 |
| Streaming 중단 | 세션 상태 `failed`, 사용자 재시도 필요 |

---

## 3. 내부 표준 Provider API

### 3.1 Provider Trait

```rust
#[async_trait::async_trait]
pub trait AiProvider: Send + Sync {
    async fn validate_credential(&self, credential: CredentialRef) -> Result<ProviderHealth, AppError>;
    async fn send_message_stream(
        &self,
        request: ProviderMessageRequest,
        sink: ProviderEventSink,
    ) -> Result<ProviderMessageResult, AppError>;
}
```

### 3.2 표준 요청 모델

```rust
pub struct ProviderMessageRequest {
    pub session_id: String,
    pub provider: AiProviderKind,
    pub model: String,
    pub system_prompt: String,
    pub messages: Vec<ProviderMessage>,
    pub attachments: Vec<ProviderAttachment>,
    pub temperature: Option<f32>,
    pub max_tokens: Option<u32>,
    pub tools: Vec<ToolDefinition>,
}
```

`attachments`는 Redmine #21 첨부 메타/본문 목록이다. 2026-07-09 현재 `src-tauri/src/models/provider.rs`에 `ProviderMessageRequest.attachments: Vec<ProviderAttachment>`, `ProviderMessage.content_blocks`, `ProviderContentBlock`, `ProviderAttachment`가 현행 반영되어 있으며, `attachments` 기본값을 빈 배열로 두어 기존 텍스트 전용 호출과 하위 호환을 유지한다.

```rust
pub struct ProviderMessage {
    pub role: MessageRole,
    pub content: String,
    pub content_blocks: Option<Vec<ProviderContentBlock>>,
}

pub enum ProviderContentBlock {
    Text {
        text: String,
    },
    Image {
        attachment_id: String,
        media_type: String,
        base64_data: String,
    },
    DocumentText {
        attachment_id: String,
        filename: String,
        media_type: String,
        extracted_text: String,
        truncated: bool,
    },
}
```

`content`는 기존 텍스트 전용 호출과 로그 표시를 위한 하위 호환 필드이다. 첨부가 포함된 신규 호출은 `content_blocks`를 provider 변환의 정본으로 사용하고, `content`에는 사용자가 입력한 순수 텍스트만 유지한다.

### 3.2.1 첨부 표준 모델

```rust
pub struct ProviderAttachment {
    pub id: String,
    pub kind: AttachmentKind,
    pub filename: String,
    pub media_type: String,
    pub size_bytes: u64,
    pub source: AttachmentSource,
    pub content_base64: Option<String>,
    pub extracted_text: Option<String>,
    pub extraction_status: AttachmentExtractionStatus,
    pub truncated: bool,
}
```

| 필드 | 규칙 |
|------|------|
| `id` | frontend가 UUID로 생성하고 Rust에서 중복 검증 |
| `kind` | `image` 또는 `document` |
| `source` | `file_picker`, `clipboard`, `drag_drop` |
| `content_base64` | 이미지 원본에만 필수. 문서 추출 후 provider 전송에는 원칙적으로 제외 |
| `extracted_text` | md/txt/pdf 등 문서에서 추출한 텍스트. provider에는 text block으로 전달 |
| `extraction_status` | `not_required`, `pending`, `completed`, `failed` |

첨부 한도 기본값은 이미지 10MB/건, 문서 20MB/건, 메시지당 첨부 10건, 문서 추출 텍스트 100KB/건, 전체 provider 입력 text budget 300KB이다. 실제 token 한도 초과 가능성이 있으면 Rust에서 앞부분 100KB와 파일 메타데이터를 유지하고 `truncated=true`를 설정한다.

### 3.3 표준 Streaming Event

| Event | 설명 |
|-------|------|
| `MessageStarted` | provider 응답 시작 |
| `MessageDelta` | text chunk 수신 |
| `ToolRequested` | tool/function call 요청 |
| `MessageCompleted` | 응답 완료 및 usage 기록 |
| `MessageFailed` | 응답 실패 |

---

## 4. Claude API 명세

### 4.1 Endpoint

| 항목 | 값 |
|------|----|
| Base URL | `https://api.anthropic.com` |
| Message | `POST /v1/messages` |
| Health | 최소 권한 검증용 경량 요청 또는 모델 목록 endpoint 사용 가능 여부 검토 |

### 4.2 인증

| Header | 값 |
|--------|----|
| `x-api-key` | OS Credential Vault의 Claude API key |
| `anthropic-version` | 고정 버전 문자열. 구현 시 configuration으로 관리 |
| `Content-Type` | `application/json` |

### 4.3 요청 변환

| 내부 필드 | Claude 요청 필드 |
|----------|------------------|
| `model` | `model` |
| `system_prompt` | `system` |
| `messages[]` | `messages[]` |
| `max_tokens` | `max_tokens` |
| `temperature` | `temperature` |
| `tools` | `tools` |
| streaming | `stream: true` |

`content_blocks`가 있는 사용자 메시지는 Claude Messages API의 content block 배열로 변환한다. 텍스트는 `{ "type": "text", "text": "..." }`, 이미지는 `{ "type": "image", "source": { "type": "base64", "media_type": "...", "data": "..." } }`로 전달한다. 문서 첨부는 원본 binary를 Claude에 업로드하지 않고 추출 텍스트를 파일명/미디어 타입 헤더와 함께 text block으로 전달한다.

### 4.4 요청 예시

```json
{
  "model": "claude-3-5-sonnet-latest",
  "system": "역할 persona bundle",
  "messages": [
    {
      "role": "user",
      "content": [
        { "type": "text", "text": "이미지를 검토해 주세요." },
        {
          "type": "image",
          "source": {
            "type": "base64",
            "media_type": "image/png",
            "data": "iVBORw0KGgo..."
          }
        },
        {
          "type": "text",
          "text": "[첨부 문서: 요구사항.md, text/markdown]\n..."
        }
      ]
    }
  ],
  "max_tokens": 4096,
  "stream": true
}
```

### 4.5 Streaming 처리

Claude streaming 응답은 SSE로 수신한다. Adapter는 provider event를 다음 내부 event로 변환한다.

| Claude event | 내부 event |
|--------------|------------|
| message start 계열 | `MessageStarted` |
| content block delta 계열 | `MessageDelta` |
| tool use block | `ToolRequested` |
| message delta usage | usage 누적 |
| message stop | `MessageCompleted` |
| error | `MessageFailed` |

---

## 5. OpenAI API 명세

### 5.1 Endpoint

| 항목 | 값 |
|------|----|
| Base URL | `https://api.openai.com` |
| Responses | `POST /v1/responses` |
| Health | 경량 models 조회 또는 최소 검증 요청 |

### 5.2 인증

| Header | 값 |
|--------|----|
| `Authorization` | `Bearer <OpenAI API key>` |
| `Content-Type` | `application/json` |

### 5.3 요청 변환

| 내부 필드 | OpenAI 요청 필드 |
|----------|------------------|
| `model` | `model` |
| `system_prompt` | `instructions` 또는 input system message |
| `messages[]` | `input` |
| `tools` | `tools` |
| streaming | `stream: true` |

OpenAI Responses API 연동은 provider/model capability가 vision을 지원할 때 `content_blocks`를 `input[].content[]`로 변환한다. 텍스트와 문서 추출 결과는 `input_text`, 이미지는 `input_image`로 매핑한다. 이미지 data URL 형식은 `data:<media_type>;base64,<base64_data>`를 사용한다. 선택 모델이 이미지 입력을 지원하지 않으면 자동 텍스트 대체를 하지 않고 `ATTACHMENT_UNSUPPORTED`를 반환한다.

### 5.4 요청 예시

```json
{
  "model": "gpt-4.1",
  "instructions": "역할 persona bundle",
  "input": [
    {
      "role": "user",
      "content": [
        { "type": "input_text", "text": "이미지를 검토해 주세요." },
        {
          "type": "input_image",
          "image_url": "data:image/png;base64,iVBORw0KGgo..."
        }
      ]
    }
  ],
  "stream": true
}
```

### 5.5 Streaming 처리

| OpenAI event | 내부 event |
|--------------|------------|
| response.created | `MessageStarted` |
| response.output_text.delta | `MessageDelta` |
| response.output_item.added tool call | `ToolRequested` |
| response.completed | `MessageCompleted` |
| response.failed / error | `MessageFailed` |

---

## 6. Gemini API 명세

### 6.1 Endpoint

| 항목 | 값 |
|------|----|
| Base URL | `https://generativelanguage.googleapis.com` |
| Generate | `POST /v1beta/models/{model}:generateContent` |
| Streaming | `POST /v1beta/models/{model}:streamGenerateContent` |
| Health | 경량 모델 조회 또는 최소 검증 요청 |

### 6.2 인증

| 방식 | 값 |
|------|----|
| API key query | `?key=<Gemini API key>` |
| 또는 Header | provider 정책에 따라 `x-goog-api-key` 사용 가능 |

### 6.3 요청 변환

| 내부 필드 | Gemini 요청 필드 |
|----------|------------------|
| `system_prompt` | `systemInstruction` |
| `messages[]` | `contents[]` |
| `temperature`, `max_tokens` | `generationConfig` |
| `tools` | `tools` |

Gemini 연동은 provider/model capability가 vision을 지원할 때 `content_blocks`를 `contents[].parts[]`로 변환한다. 텍스트와 문서 추출 결과는 `{ "text": "..." }`, 이미지는 `{ "inlineData": { "mimeType": "...", "data": "..." } }`로 전달한다. 선택 모델이 이미지 입력을 지원하지 않으면 `ATTACHMENT_UNSUPPORTED`를 반환한다.

### 6.4 요청 예시

```json
{
  "systemInstruction": {
    "parts": [{ "text": "역할 persona bundle" }]
  },
  "contents": [
    {
      "role": "user",
      "parts": [
        { "text": "이미지를 검토해 주세요." },
        {
          "inlineData": {
            "mimeType": "image/png",
            "data": "iVBORw0KGgo..."
          }
        }
      ]
    }
  ],
  "generationConfig": {
    "temperature": 0.2,
    "maxOutputTokens": 4096
  }
}
```

### 6.5 Streaming 처리

| Gemini 응답 요소 | 내부 event |
|------------------|------------|
| 첫 candidate chunk | `MessageStarted` |
| `candidates[].content.parts[].text` | `MessageDelta` |
| functionCall part | `ToolRequested` |
| finishReason | `MessageCompleted` |
| error | `MessageFailed` |

---

## 7. 첨부 전처리 및 문서 추출 API 규격

> 상태: 현행(등록 완료). 본 절의 첨부 전처리·문서 추출 규격은 `prepare_chat_attachment` command(DS-60 §3.2)로 구현되어 있다. 2026-07-09 현재 `src-tauri/src/chat_attachment.rs`에 허용 MIME 4종, 확장자+MIME 교차 검증, 추출 텍스트 100KB 한도, sha256/extracted_text/truncated, content_block 변환이 반영되어 단위테스트 85/85 PASS 상태다.

### 7.1 입력 파일 분류

| 분류 | 허용 MIME/확장자 | 처리 |
|------|------------------|------|
| 이미지 | `image/png`, `image/jpeg`, `image/webp`, `image/gif` | base64로 인코딩 후 vision 지원 provider에 content block 전달 |
| 텍스트 문서 | `.md`, `.markdown`, `.txt`, `.csv`, `.json`, `.yaml`, `.yml`, `.log` | UTF-8 우선, 실패 시 BOM/인코딩 감지 후 텍스트 추출 |
| PDF | `.pdf`, `application/pdf` | Rust backend에서 텍스트 레이어 추출. 이미지 기반 PDF는 MVP에서 OCR 미지원 |
| 기타 | docx, hwp, 압축 파일 등 | MVP 미지원. `ATTACHMENT_INVALID_TYPE` 반환 |

### 7.2 문서 추출 방식

문서 추출은 frontend가 파일 binary를 직접 provider로 보내지 않고 Rust command로 위임한다. Rust는 확장자와 MIME type을 함께 검증하고, workspace 밖 임시 파일 경로를 영구 저장하지 않는다.

| 형식 | 추출 방식 | 실패 처리 |
|------|-----------|-----------|
| md/txt/log/csv/json/yaml | byte 읽기 후 UTF-8 decode, CRLF 정규화, NUL 제거 | decode 실패 시 `ATTACHMENT_EXTRACT_FAILED` |
| pdf | `pdf-extract` 또는 동등 Rust crate로 text layer 추출 | 암호화/스캔 PDF/추출 불가 시 `ATTACHMENT_EXTRACT_FAILED` |

추출 결과는 `filename`, `media_type`, `size_bytes`, `sha256`, `extracted_text`, `truncated`를 포함한다. `extracted_text`는 provider별 token 한도와 무관하게 DS-60 IPC 기본 한도 100KB를 먼저 적용한다.

### 7.3 Provider capability 및 미지원 처리

| Provider | 이미지 전달 | 문서 전달 | 미지원 규칙 |
|----------|-------------|-----------|-------------|
| Claude | Messages API image base64 content block | 추출 텍스트 content block | 모델 capability가 vision false이면 `ATTACHMENT_UNSUPPORTED` |
| OpenAI | Responses API `input_image` data URL | 추출 텍스트 `input_text` | 모델 capability가 vision false이면 `ATTACHMENT_UNSUPPORTED` |
| Gemini | `inlineData` part | 추출 텍스트 part | 모델 capability가 vision false이면 `ATTACHMENT_UNSUPPORTED` |
| Claude CLI/OAuth shell provider | stream-json content 배열로 이미지 base64 전달(Anthropic Messages 스키마 image block, #23) | 추출 텍스트 content 블록/프롬프트 병합 | 모델 capability가 vision false이면 `ATTACHMENT_UNSUPPORTED` (`claude_cli`는 `supports_vision=true`) |

Provider Adapter는 첨부를 조용히 누락하지 않는다. 하나 이상의 첨부가 미지원이면 메시지 전송 전체를 거절하고 frontend가 첨부 제거 또는 지원 provider 선택을 안내한다.

## 8. Redmine API 명세

### 8.1 Endpoint

| 항목 | 값 |
|------|----|
| Base URL | `http://211.117.60.5:8080` |
| Protocol | HTTP 내부망 |
| Issues | `/issues.json`, `/issues/{id}.json` |

### 8.2 인증

| Header | 값 |
|--------|----|
| `X-Redmine-API-Key` | 역할별 Redmine API key |
| `Content-Type` | `application/json` |

API key는 OS Credential Vault에 저장한다. `role`이 지정된 Redmine command는 `api_key_${role}` 계정을 먼저 조회하고, 역할별 key가 없거나 `role`이 생략되면 기존 단일 계정 `api_key`를 fallback으로 조회한다. 구현 시 git 공유 문서에는 key를 저장하지 않는다.

### 8.3 이슈 생성

| 항목 | 값 |
|------|----|
| Method | `POST` |
| URL | `/issues.json` |
| 용도 | 새기능/결함/지원 이슈 생성 |

```json
{
  "issue": {
    "project_id": "PROJECT_ID",
    "tracker_id": 2,
    "subject": "제목",
    "description": "내용",
    "assigned_to_id": 123
  }
}
```

### 8.4 이슈 조회

| 항목 | 값 |
|------|----|
| Method | `GET` |
| URL | `/issues/{id}.json` |

프로젝트 열린 이슈 목록은 `GET /issues.json?project_id={project_id}&status_id=open`을 사용한다.

### 8.5 이슈 업데이트

| 작업 | Method | URL | Body |
|------|--------|-----|------|
| 해결 보고 | `PUT` | `/issues/{id}.json` | `{ "issue": { "done_ratio": 100, "status_id": 3 } }` |
| PM 완료 처리 | `PUT` | `/issues/{id}.json` | `{ "issue": { "status_id": 5 } }` |
| 코멘트 추가 | `PUT` | `/issues/{id}.json` | `{ "issue": { "notes": "코멘트 내용" } }` |

### 8.6 Redmine 상태/트래커

| 구분 | ID | 의미 |
|------|----|------|
| tracker | 1 | 결함 |
| tracker | 2 | 새기능 |
| tracker | 3 | 지원 |
| status | 1 | 신규 |
| status | 2 | 진행 |
| status | 3 | 해결 |
| status | 5 | 완료 |
| status | 6 | 거절 |

작업자 완료 보고는 `status_id=3`까지만 수행한다. 최종 종결은 PM 검토 후 `status_id=5` 또는 `6`으로 처리한다.

### 8.7 Tauri Redmine Command 파라미터

Redmine command 4종은 모두 선택 필드 `role?: string`을 지원한다. 역할별 API key 저장 규칙은 `redmine/api_key_${role}`이며, fallback 계정은 `redmine/api_key`이다.

| Command | Request | Response |
|---------|---------|----------|
| `redmine_list_issues` | `{ workspace_id: string, project_id?: string, status_id?: string, role?: string }` | `RedmineIssueItem[]` |
| `redmine_get_issue` | `{ workspace_id: string, issue_id: number, role?: string }` | `RedmineIssueItem` |
| `redmine_create_issue` | `{ workspace_id: string, project_id: string, tracker_id: number, subject: string, description?: string, assigned_to_id?: number, role?: string }` | `RedmineIssueItem` |
| `redmine_update_issue` | `{ workspace_id: string, issue_id: number, status_id?: number, done_ratio?: number, notes?: string, role?: string }` | `void` |

API key 선택 순서:

1. `role`이 있으면 OS Credential Vault에서 `provider=redmine`, `account=api_key_${role}` 조회
2. 역할별 key가 없거나 `role`이 없으면 `provider=redmine`, `account=api_key` 조회
3. 둘 다 없으면 `REDMINE_API_KEY_NOT_SET` 반환

---

## 9. Browser Command API

### 9.1 개요

Browser command API는 앱 내에서 웹페이지(Redmine·GitHub·문서 등)를 여는 `embedded-browser` WebviewWindow를 제어하는 Tauri invoke command이다. 구현 파일은 `src-tauri/src/commands/browser.rs`이다.

**설계 모델: 독립 이동 가능 창 (Independent Movable Window)** — Redmine #17 전환.
`embedded-browser` 창은 main 창 콘텐츠 영역에 강제로 겹쳐 붙지 않고, 사용자가 **OS 창처럼 자유롭게 이동·리사이즈**할 수 있는 독립 최상위 창이다. main 창의 이동/리사이즈/최소화를 좌표 강제 추종하지 않는다.

- 창은 parent 지정 없이(완전 독립) `decorations(true)`(타이틀바로 이동/닫기)·`resizable(true)`로 생성한다.
- 위치/크기는 생성 후 OS와 사용자가 소유한다. 백엔드는 좌표를 강제 재배치하지 않는다.
- main 창과의 z-order 강제(always_on_top)·포커스 강탈·`Moved`/`Resized` 추종 로직은 두지 않는다.
- 앱(main 창) 종료 시 `embedded-browser` 창도 함께 정리된다.

> **전환 완료(Transition Complete, 2026-07-11)**: 본 §9는 Redmine #17 독립 이동 가능 창 전환의 **현행 규격**이다. BE/FE 구현이 종료되어 코드는 더 이상 핀 고정(child `parent(&main)` + 좌표 추종) 방식이 아니다. main창 `Moved`/`Resized` 추종 핸들러·`sync_embedded_browser_position`·throttle 로직과 `browser_resize` command가 모두 제거되었고, `browser_resize`는 현행 command 대조 대상에서 빠졌다(DS-60 §8.2와 교차 정합).

| Command | 구현 상태 | 용도 |
|---------|-----------|------|
| `browser_open` | 현행 | `embedded-browser` 독립 창 생성 |
| `browser_navigate` | 현행 | 열린 `embedded-browser`를 새 URL로 이동 |
| `browser_back` | 현행 | 열린 `embedded-browser`에서 `history.back()` 실행 |
| `browser_forward` | 현행 | 열린 `embedded-browser`에서 `history.forward()` 실행 |
| `browser_close` | 현행 | `embedded-browser` 닫기 |

### 9.2 공통 URL 규칙

| 항목 | 규칙 |
|------|------|
| 허용 스킴 | `http://`, `https://` |
| 스킴 누락 | `browser_open`, `browser_navigate`는 `https://`를 자동 부여 |
| URL 파싱 실패 | `INVALID_URL` 반환 |
| 창 라벨 | 임베디드 창은 `embedded-browser` 고정 라벨 사용 |
| 초기 배치 | `browser_open`의 위치/크기 인자는 **선택적 초기값**이다. 생략 시 백엔드 기본값으로 연다. 생성 후 위치/크기는 OS·사용자 소유 |
| 네비게이션 이벤트 | WebView 내부 이동 발생 시 main 창으로 `browser:navigation` emit |

### 9.3 `browser_open`

`embedded-browser` 독립 창을 생성한다. 기존 `embedded-browser` 창이 있으면 닫고 새 창을 만든다. 위치/크기 인자는 **선택적 초기값**이며, 생성 후에는 사용자가 OS 창처럼 이동·리사이즈한다.

| 항목 | 값 |
|------|----|
| Command | `browser_open` |
| Rust 함수 | `commands::browser::browser_open` |
| Method | Tauri `invoke` |
| 응답 | `void` |

요청:

```ts
type BrowserOpenRequest = {
  url: string
  // 선택적 초기 배치. 전부 생략 가능. 생략 시 백엔드 기본값 적용.
  x?: number       // 초기 좌표 (screen 논리 좌표). 생략 시 기본 위치
  y?: number
  width?: number   // 초기 크기. 생략 시 기본 크기(예: 960×720)
  height?: number
}
```

예시:

```ts
// 기본 배치로 열기
await invoke('browser_open', { url: 'https://docs.anthropic.com/' })

// 초기 위치/크기 지정 후 열기 (이후 이동/리사이즈는 사용자 소유)
await invoke('browser_open', {
  url: 'https://docs.anthropic.com/',
  x: 200, y: 120, width: 960, height: 720,
})
```

오류:

| 코드 | 의미 |
|------|------|
| `INVALID_URL` | URL 파싱 실패 |
| `BROWSER_OPEN` | WebviewWindow 생성 실패 |

> 독립 창 전환으로 좌표를 main 기준으로 변환하지 않으므로 `NO_MAIN`·`WIN_POS` 오류는 `browser_open` 경로에서 발생하지 않는다(구 핀 고정 방식 잔재).

### 9.4 `browser_navigate`

이미 열린 `embedded-browser` 창을 새 URL로 이동한다. 창이 없으면 오류 없이 성공 처리한다.

| 항목 | 값 |
|------|----|
| Command | `browser_navigate` |
| Rust 함수 | `commands::browser::browser_navigate` |
| Method | Tauri `invoke` |
| 응답 | `void` |

요청:

```ts
type BrowserNavigateRequest = {
  url: string
}
```

오류:

| 코드 | 의미 |
|------|------|
| `INVALID_URL` | URL 파싱 실패 |
| `BROWSER_NAV` | WebviewWindow navigation 실패 |

### 9.5 `browser_back`

열린 `embedded-browser` 창에서 JavaScript `history.back()`을 실행한다. 창이 없으면 오류 없이 성공 처리한다.

| 항목 | 값 |
|------|----|
| Command | `browser_back` |
| Rust 함수 | `commands::browser::browser_back` |
| Method | Tauri `invoke` |
| 응답 | `void` |

요청:

```ts
type BrowserBackRequest = {}
```

오류:

| 코드 | 의미 |
|------|------|
| `BROWSER_BACK` | WebviewWindow script evaluation 실패 |

### 9.6 `browser_forward`

열린 `embedded-browser` 창에서 JavaScript `history.forward()`를 실행한다. 창이 없으면 오류 없이 성공 처리한다.

| 항목 | 값 |
|------|----|
| Command | `browser_forward` |
| Rust 함수 | `commands::browser::browser_forward` |
| Method | Tauri `invoke` |
| 응답 | `void` |

요청:

```ts
type BrowserForwardRequest = {}
```

오류:

| 코드 | 의미 |
|------|------|
| `BROWSER_FORWARD` | WebviewWindow script evaluation 실패 |

### 9.7 `browser_close`

열린 `embedded-browser` 창을 닫는다. 창이 없으면 오류 없이 성공 처리한다.

| 항목 | 값 |
|------|----|
| Command | `browser_close` |
| Rust 함수 | `commands::browser::browser_close` |
| Method | Tauri `invoke` |
| 응답 | `void` |

요청:

```ts
type BrowserCloseRequest = {}
```

오류:

| 코드 | 의미 |
|------|------|
| `BROWSER_CLOSE` | WebviewWindow close 실패 |

### 9.8 `browser_resize` (삭제됨)

> **삭제됨 — Redmine #17 독립 창 전환 완료(2026-07-11)**: 이 command는 사이드바 rect에 창을 강제 추종시키기 위한 좌표 동기화 용도였다. 독립 이동 가능 창 모델에서는 창 위치/크기를 사용자가 직접 조정하므로 불필요하여, Rust command(`commands::browser::browser_resize`)·`generate_handler!` 등록·`src/ipc/browser.ts` `resizeBrowser` wrapper·프론트 `ResizeObserver`/`getBoundingClientRect` 기반 호출이 모두 제거되었다. 좌표 동기화가 필요했던 위치·크기 조정은 이제 OS 창 조작으로 대체된다.

### 9.9 `browser:navigation` Event

임베디드 WebView 안에서 페이지 이동이 발생하면 Rust backend가 main window로 현재 URL 문자열을 emit한다.

| 항목 | 값 |
|------|----|
| Event | `browser:navigation` |
| Emit 대상 | `main` window |
| Payload | `string` |
| 발생 시점 | `browser_open`으로 생성된 WebviewWindow의 `on_navigation` callback |

Payload 예시:

```ts
type BrowserNavigationPayload = string
```

### 9.10 Frontend 연계

| Frontend | 연계 내용 |
|----------|-----------|
| `src/components/BrowserPanel.vue` | **브라우저 컨트롤 바**(주소 입력, 빠른 링크, 최근 방문, 뒤로/앞으로, 열기/닫기, `browser:navigation` URL 동기화). 독립 창 전환 완료로 WebView를 겹쳐 그릴 rect 예약 영역(`webviewRef` 플레이스홀더)·`ResizeObserver`·`resizeBrowser` 호출이 제거되었다. 창 열림 여부(`isOpen`)는 `workspaceStore`로 이관되어 탭을 전환해도 창이 유지된다(자동 close 제거) |
| `src/stores/browser.ts` | `currentUrl`, `history`, `historyIndex`, `addressBarValue`, back/forward 가능 여부 관리 |
| `src/router/index.ts` | `/browser` route에서 `BrowserPanel.vue` 로드 |

### 9.11 구현 정합성 확인 사항

- **현행(전환 완료, 2026-07-11)**: `browser_open`, `browser_navigate`, `browser_back`, `browser_forward`, `browser_close` 5종만 `lib.rs` invoke handler에 등록되어 있다. `browser_resize`는 등록·구현이 모두 제거되었다.
- `on_window_event`의 `Moved`/`Resized` 좌표 추종 핸들러와 `sync_embedded_browser_position`·throttle 로직은 제거되었다. 백엔드는 창 위치/크기를 강제 추종·재배치하지 않는다.
- 삭제된 시스템 브라우저 열기 command는 더 이상 API 명세 대상이 아니다.
- `embedded-browser`는 생성 시 초기 배치(선택적 geometry, 기본 960×720)만 적용하고, 이후 위치/크기는 OS·사용자가 소유한다(백엔드 강제 재배치 없음).
- 프론트 `BrowserPanel.vue`의 `ResizeObserver`·`getBoundingClientRect` 기반 rect 예약 영역 및 `resizeBrowser` 호출은 제거되었다(§9.10과 교차). 창 열림 상태(`isOpen`)는 `workspaceStore`로 이관되어 탭 전환 시에도 창이 유지된다.

---

## 10. Web Content Fetch Command API

### 10.1 개요

`fetch_url_content`는 AI 에이전트 웹검색 연동 1단계 command이다. 지정 URL의 HTML을 가져와 `<script>`, `<style>`, 주석, 태그를 제거하고 본문 텍스트를 최대 50KB로 정리해 반환한다.

| 항목 | 값 |
|------|----|
| Command | `fetch_url_content` |
| Rust 함수 | `commands::web::fetch_url_content` |
| Method | Tauri `invoke` |
| Timeout | 15초 |
| User-Agent | `AgiTeamBuilder/0.1` 포함 브라우저형 UA |
| 응답 | `FetchedPage` |

### 10.2 요청

```ts
type FetchUrlContentRequest = {
  url: string
}
```

URL에 `http://` 또는 `https://`가 없으면 backend가 `https://`를 자동 부여한다.

### 10.3 응답 스키마

```ts
type FetchedPage = {
  url: string
  title?: string
  text: string
  fetched_at: string
}
```

| 필드 | 타입 | 설명 |
|------|------|------|
| `url` | string | 정규화 후 실제 요청한 URL |
| `title` | string \| undefined | `<title>` 태그에서 추출한 제목. 없으면 생략 |
| `text` | string | 태그 제거와 공백 정리를 거친 본문 텍스트. 최대 50KB |
| `fetched_at` | string | 조회 시각. RFC3339 UTC 문자열 |

### 10.4 오류 코드

| 코드 | 의미 | 복구 가능성 |
|------|------|-------------|
| `INVALID_URL` | URL 파싱 실패 | 입력 수정 후 재시도 가능 |
| `FETCH_TIMEOUT` | 15초 내 응답 없음 | 네트워크 상태 확인 후 재시도 가능 |
| `FETCH_FAILED` | HTTP client 생성, 요청, 비정상 HTTP status, 본문 읽기 실패 | 원인별 확인 후 재시도 가능 |

### 10.5 제약

- HTML 파싱은 외부 크레이트 없이 수동 strip 방식으로 수행하므로 복잡한 SPA/동적 렌더링 페이지의 본문 품질은 제한될 수 있다.
- JavaScript 실행 결과를 수집하지 않는다.
- binary/PDF 등 HTML이 아닌 응답은 텍스트 품질이 보장되지 않는다.
- 인증이 필요한 페이지는 public HTML 응답 범위까지만 처리된다.

---

## 11. Credential 및 보안 처리

### 11.1 저장 대상

| Provider | Secret | 저장소 |
|----------|--------|--------|
| Claude | API key | OS Credential Vault |
| OpenAI | API key | OS Credential Vault |
| Gemini | API key | OS Credential Vault |
| Redmine | 역할별 API key | OS Credential Vault 또는 git 비공유 persona 인스턴스 |

### 11.2 로그 마스킹

다음 값은 로그, event payload, error detail에 원문 기록하지 않는다.

- `Authorization`
- `x-api-key`
- `X-Redmine-API-Key`
- query string의 `key`
- provider token
- Redmine API key

---

## 12. Health Check API

### 12.1 Provider Health

```rust
pub struct ProviderHealth {
    pub provider: AiProviderKind,
    pub status: HealthStatus,
    pub latency_ms: Option<u64>,
    pub checked_at: DateTime<Utc>,
    pub error: Option<AppError>,
}
```

### 12.2 검사 항목

| Provider | 검사 |
|----------|------|
| Claude | credential 존재, endpoint 도달성, 최소 인증 검증 |
| OpenAI | credential 존재, endpoint 도달성, 최소 인증 검증 |
| Gemini | credential 존재, endpoint 도달성, 최소 인증 검증 |
| Redmine | API key 존재, `/issues.json` 접근 가능성, HTTP status |

---

## 13. 후속 문서 연계

| 산출물 | 반영 필요 |
|--------|-----------|
| DS-60 연동규격서 | Provider 호출 결과를 frontend로 전달하는 IPC event payload |
| DS-30 DB설계서 | Provider credential ref, session, message, usage 저장 모델 |
| TS-05 시험계획서 | API 인증 실패, rate limit, stream 중단, Redmine 상태 전이 테스트 |
