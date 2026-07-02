---
doc: DS-40_API명세서
version: v0.4
last_updated: 2026-07-02
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
    pub temperature: Option<f32>,
    pub max_tokens: Option<u32>,
    pub tools: Vec<ToolDefinition>,
}
```

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

### 4.4 요청 예시

```json
{
  "model": "claude-3-5-sonnet-latest",
  "system": "역할 persona bundle",
  "messages": [
    { "role": "user", "content": "작업 지시" }
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

### 5.4 요청 예시

```json
{
  "model": "gpt-4.1",
  "instructions": "역할 persona bundle",
  "input": [
    { "role": "user", "content": "작업 지시" }
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

### 6.4 요청 예시

```json
{
  "systemInstruction": {
    "parts": [{ "text": "역할 persona bundle" }]
  },
  "contents": [
    {
      "role": "user",
      "parts": [{ "text": "작업 지시" }]
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

## 7. Redmine API 명세

### 7.1 Endpoint

| 항목 | 값 |
|------|----|
| Base URL | `http://211.117.60.5:8080` |
| Protocol | HTTP 내부망 |
| Issues | `/issues.json`, `/issues/{id}.json` |

### 7.2 인증

| Header | 값 |
|--------|----|
| `X-Redmine-API-Key` | 역할별 Redmine API key |
| `Content-Type` | `application/json` |

API key는 OS Credential Vault에 저장한다. `role`이 지정된 Redmine command는 `api_key_${role}` 계정을 먼저 조회하고, 역할별 key가 없거나 `role`이 생략되면 기존 단일 계정 `api_key`를 fallback으로 조회한다. 구현 시 git 공유 문서에는 key를 저장하지 않는다.

### 7.3 이슈 생성

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

### 7.4 이슈 조회

| 항목 | 값 |
|------|----|
| Method | `GET` |
| URL | `/issues/{id}.json` |

프로젝트 열린 이슈 목록은 `GET /issues.json?project_id={project_id}&status_id=open`을 사용한다.

### 7.5 이슈 업데이트

| 작업 | Method | URL | Body |
|------|--------|-----|------|
| 해결 보고 | `PUT` | `/issues/{id}.json` | `{ "issue": { "done_ratio": 100, "status_id": 3 } }` |
| PM 완료 처리 | `PUT` | `/issues/{id}.json` | `{ "issue": { "status_id": 5 } }` |
| 코멘트 추가 | `PUT` | `/issues/{id}.json` | `{ "issue": { "notes": "코멘트 내용" } }` |

### 7.6 Redmine 상태/트래커

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

### 7.7 Tauri Redmine Command 파라미터

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

## 8. Browser Command API

### 8.1 개요

Browser command API는 프론트엔드 사이드바/브라우저 패널 영역에 임베디드 WebviewWindow를 띄우고 위치와 크기를 동기화하기 위한 Tauri invoke command이다. 현행 구현은 `src-tauri/src/commands/browser.rs`의 `embedded-browser` WebviewWindow 방식만 사용한다.

| Command | 구현 상태 | 용도 |
|---------|-----------|------|
| `browser_open` | Rust 등록됨 | `embedded-browser` WebviewWindow 생성 |
| `browser_navigate` | Rust 등록됨 | 열린 `embedded-browser`를 새 URL로 이동 |
| `browser_back` | Rust 등록됨 | 열린 `embedded-browser`에서 `history.back()` 실행 |
| `browser_forward` | Rust 등록됨 | 열린 `embedded-browser`에서 `history.forward()` 실행 |
| `browser_close` | Rust 등록됨 | `embedded-browser` 닫기 |
| `browser_resize` | Rust 등록됨 | `embedded-browser` 위치/크기 조정 |

### 8.2 공통 URL 규칙

| 항목 | 규칙 |
|------|------|
| 허용 스킴 | `http://`, `https://` |
| 스킴 누락 | `browser_open`, `browser_navigate`는 `https://`를 자동 부여 |
| URL 파싱 실패 | `INVALID_URL` 반환 |
| 창 라벨 | 임베디드 창은 `embedded-browser` 고정 라벨 사용 |
| 좌표계 | `x`, `y`는 main window 기준 논리 좌표 |
| 네비게이션 이벤트 | WebView 내부 이동 발생 시 main 창으로 `browser:navigation` emit |

### 8.3 `browser_open`

임베디드 브라우저 WebviewWindow를 생성한다. 기존 `embedded-browser` 창이 있으면 닫고 새 창을 만든다.

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
  x: number
  y: number
  width: number
  height: number
}
```

예시:

```ts
await invoke('browser_open', {
  url: 'https://docs.anthropic.com/',
  x: 840,
  y: 96,
  width: 520,
  height: 720,
})
```

오류:

| 코드 | 의미 |
|------|------|
| `NO_MAIN` | main window 조회 실패 |
| `INVALID_URL` | URL 파싱 실패 |
| `WIN_POS` | main window 위치 조회 실패 |
| `BROWSER_OPEN` | WebviewWindow 생성 실패 |

### 8.4 `browser_navigate`

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

### 8.5 `browser_back`

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

### 8.6 `browser_forward`

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

### 8.7 `browser_close`

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

### 8.8 `browser_resize`

열린 `embedded-browser` 창의 위치와 크기를 main window 기준 논리 좌표로 재조정한다. 창이 없으면 오류 없이 성공 처리한다.

| 항목 | 값 |
|------|----|
| Command | `browser_resize` |
| Rust 함수 | `commands::browser::browser_resize` |
| Method | Tauri `invoke` |
| 응답 | `void` |

요청:

```ts
type BrowserResizeRequest = {
  x: number
  y: number
  width: number
  height: number
}
```

예시:

```ts
await invoke('browser_resize', {
  x: 840,
  y: 96,
  width: 520,
  height: 720,
})
```

### 8.9 `browser:navigation` Event

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

### 8.10 Frontend 연계

| Frontend | 연계 내용 |
|----------|-----------|
| `src/components/BrowserPanel.vue` | 주소 입력, 빠른 링크, 최근 방문, 뒤로/앞으로, `browser:navigation` URL 동기화 |
| `src/stores/browser.ts` | `currentUrl`, `history`, `historyIndex`, `addressBarValue`, back/forward 가능 여부 관리 |
| `src/router/index.ts` | `/browser` route에서 `BrowserPanel.vue` 로드 |

### 8.11 구현 정합성 확인 사항

- `browser_open`, `browser_navigate`, `browser_back`, `browser_forward`, `browser_close`, `browser_resize`는 `lib.rs` invoke handler에 등록되어 있다.
- 삭제된 시스템 브라우저 열기 command는 더 이상 API 명세 대상이 아니다.
- 임베디드 창은 main window 기준 좌표를 스크린 논리 좌표로 변환해 생성/이동한다.

---

## 9. Credential 및 보안 처리

### 9.1 저장 대상

| Provider | Secret | 저장소 |
|----------|--------|--------|
| Claude | API key | OS Credential Vault |
| OpenAI | API key | OS Credential Vault |
| Gemini | API key | OS Credential Vault |
| Redmine | 역할별 API key | OS Credential Vault 또는 git 비공유 persona 인스턴스 |

### 9.2 로그 마스킹

다음 값은 로그, event payload, error detail에 원문 기록하지 않는다.

- `Authorization`
- `x-api-key`
- `X-Redmine-API-Key`
- query string의 `key`
- provider token
- Redmine API key

---

## 10. Health Check API

### 10.1 Provider Health

```rust
pub struct ProviderHealth {
    pub provider: AiProviderKind,
    pub status: HealthStatus,
    pub latency_ms: Option<u64>,
    pub checked_at: DateTime<Utc>,
    pub error: Option<AppError>,
}
```

### 10.2 검사 항목

| Provider | 검사 |
|----------|------|
| Claude | credential 존재, endpoint 도달성, 최소 인증 검증 |
| OpenAI | credential 존재, endpoint 도달성, 최소 인증 검증 |
| Gemini | credential 존재, endpoint 도달성, 최소 인증 검증 |
| Redmine | API key 존재, `/issues.json` 접근 가능성, HTTP status |

---

## 11. 후속 문서 연계

| 산출물 | 반영 필요 |
|--------|-----------|
| DS-60 연동규격서 | Provider 호출 결과를 frontend로 전달하는 IPC event payload |
| DS-30 DB설계서 | Provider credential ref, session, message, usage 저장 모델 |
| TS-05 시험계획서 | API 인증 실패, rate limit, stream 중단, Redmine 상태 전이 테스트 |
