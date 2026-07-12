---
doc: DV-20_백엔드코드_INDEX
version: v0.1
last_updated: 2026-06-23
author: DeveloperBE
---

# DV-20 백엔드 코드 INDEX — AgiTeamBuilder Rust 백엔드

## 개정이력

| 버전 | 일자 | 작성자 | 내용 |
|------|------|--------|------|
| v0.1 | 2026-06-23 | DeveloperBE | Tauri v2 + Rust 백엔드 최초 구현 및 INDEX 작성 |

---

## 1. 프로젝트 위치

```
system/tos/                      ← Tauri v2 + Vue 3 + TypeScript 앱 루트
├── src/                         ← Vue 3 프론트엔드 (DV-30 FE 대상)
├── src-tauri/
│   ├── Cargo.toml               ← Rust 의존성
│   ├── tauri.conf.json          ← Tauri 앱 설정
│   ├── capabilities/
│   │   └── default.json         ← Tauri capability 설정
│   └── src/                     ← Rust 백엔드 소스 (DV-20 구현 대상)
│       ├── main.rs              ← 바이너리 진입점
│       ├── lib.rs               ← 모듈 등록 + Tauri Builder + invoke_handler
│       ├── app_state.rs         ← AppState (workspace/session 레지스트리)
│       ├── credential.rs        ← CredentialStoreService (OS Keychain/Credential Manager)
│       ├── file_document.rs     ← FileDocumentService (.latest.md + _archive)
│       ├── persona_bundle.rs    ← PersonaBundleService (persona 파일 번들링)
│       ├── health_check.rs      ← HealthCheckService (workspace + provider 점검)
│       ├── agent_session.rs     ← AgentSessionService (생명주기 + streaming)
│       ├── models/              ← 공통 데이터 모델
│       │   ├── mod.rs
│       │   ├── error.rs         ← AppError, AppResult
│       │   ├── provider.rs      ← AiProviderKind, ProviderHealth, ProviderEvent 등
│       │   ├── workspace.rs     ← Workspace, AgiteamConfig, ProjectState 등
│       │   ├── session.rs       ← AgentSession, AgentLifecycleState 등
│       │   ├── message.rs       ← AgentMessage, event payload 등
│       │   ├── persona.rs       ← PersonaBundle, PersonaBundlePreview
│       │   └── role.rs          ← RoleConfig
│       ├── commands/            ← Tauri invoke command 핸들러
│       │   ├── mod.rs
│       │   ├── workspace.rs     ← open_workspace, load_workspace_config, validate_workspace
│       │   ├── agent.rs         ← boot_team, boot_role, stop_role, send_agent_message 등
│       │   ├── persona.rs       ← build_persona_bundle
│       │   ├── document.rs      ← list_documents, read_document, write_latest_document
│       │   ├── credential.rs    ← save_credential, delete_credential, validate_credential
│       │   └── health.rs        ← run_health_check
│       └── provider/            ← AI Provider Adapter
│           ├── mod.rs           ← AiProvider trait + 팩토리 + 공통 유틸
│           ├── claude.rs        ← ClaudeApiAdapter (Anthropic Messages API SSE)
│           ├── openai.rs        ← OpenAiApiAdapter (OpenAI Responses API SSE)
│           └── gemini.rs        ← GeminiApiAdapter (Gemini streamGenerateContent)
```

---

## 2. 모듈별 설명

### 2.1 credential.rs — CredentialStoreService

OS Credential Vault(macOS Keychain / Windows Credential Manager)에 API key를 저장·조회·삭제한다.

| 메서드 | 설명 |
|--------|------|
| `save(provider, account, secret)` | OS vault에 credential 저장 → CredentialRef 반환 |
| `get_secret(provider, account)` | OS vault에서 API key 조회 (pub(crate), secret 노출 최소화) |
| `exists(provider, account)` | credential 존재 여부 확인 (secret 미반환) |
| `delete(provider, account)` | credential 삭제 |
| `check_existence(provider, account)` | ProviderHealth 형태로 존재 여부 반환 |

**보안 원칙**: `get_secret`은 `pub(crate)`. secret 원문은 Renderer에 반환하지 않는다.

### 2.2 file_document.rs — FileDocumentService

`.latest.md` 파일 읽기/쓰기와 `_archive` 자동 백업을 처리한다. (Shared persona §6 버전관리 정책 SSOT 구현)

| 메서드 | 설명 |
|--------|------|
| `read_document(rel_path)` | workspace root 하위 문서 읽기 (path traversal 방어) |
| `write_latest_document(rel_path, content)` | 기존 파일 → `_archive/<이름>_YYYYMMDDhhmmss.md` 백업 후 `.latest.md` 갱신 |
| `list_documents(workspace_id)` | `documents/` 하위 `.md`/`.json` 트리 조회 (`_archive`, 숨김 제외) |

### 2.3 persona_bundle.rs — PersonaBundleService

Shared persona.md + 역할별 persona.md + 부팅 대기 규칙을 메모리 내에서 결합한다. 임시 파일 불필요.

| 메서드 | 설명 |
|--------|------|
| `build(config, role)` | PersonaBundle 생성 (content + SHA-256 hash + source_files) |
| `build_preview(config, role)` | build_persona_bundle command 응답용 PersonaBundlePreview 생성 |

**해시**: SHA-256(전체 content) → hex. 같은 입력이면 동일 hash (결정론적).

### 2.4 health_check.rs — HealthCheckService

workspace 구조, credential 존재, provider endpoint 도달성, 문서 쓰기 권한을 점검한다.

| 메서드 | 설명 |
|--------|------|
| `run(workspace_id, config)` | 전체 점검 실행 → HealthCheckReport 반환 |
| `check_provider_endpoint(provider)` | HEAD 요청으로 endpoint 도달성 + 응답 시간 측정 |

### 2.5 agent_session.rs — AgentSessionService

에이전트 세션 생명주기를 관리하고 provider streaming 응답을 Tauri event로 emit한다.

| 메서드 | 설명 |
|--------|------|
| `boot_team(workspace_id, config)` | 전체 팀 booting → BootTeamResult |
| `boot_role(workspace_id, role, config)` | 단일 역할 booting → AgentSessionSummary |
| `send_message(session_id, content, config, model)` | 메시지 전송 + streaming → MessageAck |
| `stop_session(session_id)` | 세션 idle 전이 |
| `get_session_detail(session_id)` | AgentSessionDetail 조회 |
| `list_messages(session_id, cursor, limit)` | MessagePage 조회 |

**Tauri Events 발생:**

| Event | 발생 시점 |
|-------|-----------|
| `agent:status_changed` | 생명주기 상태 전이마다 |
| `agent:message_started` | provider 응답 시작 |
| `agent:message_delta` | streaming chunk 수신 |
| `agent:message_completed` | 응답 완료 |
| `agent:message_failed` | 응답 실패 |
| `agent:tool_requested` | tool call 요청 |

### 2.6 provider/ — AI Provider Adapter

| 파일 | Adapter | 연동 방식 |
|------|---------|-----------|
| `provider/mod.rs` | AiProvider trait | `async_trait` 기반 trait + 팩토리 |
| `provider/claude.rs` | ClaudeApiAdapter | POST /v1/messages + SSE streaming |
| `provider/openai.rs` | OpenAiApiAdapter | POST /v1/responses + SSE streaming |
| `provider/gemini.rs` | GeminiApiAdapter | POST :streamGenerateContent + JSON chunk streaming |

---

## 3. 등록된 Tauri Invoke Commands

DS-60 §3에 정의된 모든 command가 `lib.rs` `invoke_handler`에 등록되어 있다.

| Command | 파일 | 설명 |
|---------|------|------|
| `open_workspace` | commands/workspace.rs | workspace 열기 |
| `load_workspace_config` | commands/workspace.rs | agiteam.json + project_state.yaml 로드 |
| `validate_workspace` | commands/workspace.rs | 필수 파일 구조 검증 |
| `boot_team` | commands/agent.rs | 전체 팀 booting |
| `boot_role` | commands/agent.rs | 단일 역할 booting |
| `stop_role` | commands/agent.rs | 세션 중지 |
| `send_agent_message` | commands/agent.rs | 메시지 전송 |
| `get_agent_session` | commands/agent.rs | 세션 상세 조회 |
| `list_agent_messages` | commands/agent.rs | 메시지 로그 페이지 |
| `build_persona_bundle` | commands/persona.rs | persona bundle 미리보기 |
| `list_documents` | commands/document.rs | 문서 트리 조회 |
| `read_document` | commands/document.rs | 문서 읽기 |
| `write_latest_document` | commands/document.rs | _archive 백업 + latest 갱신 |
| `save_credential` | commands/credential.rs | OS vault credential 저장 |
| `delete_credential` | commands/credential.rs | credential 삭제 |
| `validate_credential` | commands/credential.rs | credential 유효성 검증 |
| `get_masked_credential` | commands/credential.rs | 마스킹된 credential 정보 조회 |
| `run_health_check` | commands/health.rs | 전체 상태 점검 |

---

## 4. 주요 Rust 의존성

| Crate | 버전 | 용도 |
|-------|------|------|
| `tauri` | 2 | Tauri v2 앱 프레임워크 |
| `keyring` | 3 | OS Keychain / Credential Manager |
| `reqwest` | 0.12 | HTTP client (provider API 호출) |
| `tokio` | 1 | async runtime |
| `serde` + `serde_json` | 1 | JSON 직렬화 |
| `serde_yaml` | 0.9 | project_state.yaml 파싱 |
| `async-trait` | 0.1 | async trait 지원 |
| `uuid` | 1 | 세션/메시지 ID 생성 |
| `chrono` | 0.4 | DateTime + serde |
| `sha2` + `hex` | 0.10 / 0.4 | persona content SHA-256 해시 |
| `futures-util` | 0.3 | SSE 스트림 처리 |
| `thiserror` | 1 | error 타입 정의 |

---

## 5. 단위 테스트 목록

각 모듈에 `#[cfg(test)]` 모듈로 단위 테스트가 포함되어 있다.

| 파일 | 테스트 | 설명 |
|------|--------|------|
| `models/error.rs` | `test_app_error_creation` | AppError 생성 및 code 확인 |
| `models/error.rs` | `test_app_error_display` | Display trait |
| `models/error.rs` | `test_app_error_with_detail` | detail 첨부 |
| `models/provider.rs` | `test_provider_kind_display` | AiProviderKind 출력 |
| `models/provider.rs` | `test_provider_kind_from_str` | 문자열→종류 변환 |
| `models/provider.rs` | `test_provider_health_serialization` | JSON 직렬화 |
| `models/session.rs` | `test_lifecycle_state_display` | 상태 문자열 |
| `models/session.rs` | `test_lifecycle_state_serde` | JSON serde |
| `models/session.rs` | `test_command_result` | CommandResult 생성 |
| `models/message.rs` | `test_agent_message_serialization` | 메시지 JSON |
| `models/message.rs` | `test_message_delta_serialization` | delta JSON |
| `models/persona.rs` | `test_persona_bundle_preview_from` | From 변환 |
| `credential.rs` | `test_credential_save_get_delete` | OS keyring 통합 (keyring 없는 환경 skip) |
| `credential.rs` | `test_credential_missing_error` | 없는 credential 조회 오류 |
| `credential.rs` | `test_masked_credential` | MaskedCredential 생성 |
| `file_document.rs` | `test_write_and_read_document` | 쓰기+읽기 정합성 |
| `file_document.rs` | `test_write_creates_archive` | _archive 백업 생성 확인 |
| `file_document.rs` | `test_path_traversal_blocked` | path traversal 방어 |
| `file_document.rs` | `test_list_documents_empty` | 빈 workspace 트리 |
| `persona_bundle.rs` | `test_build_persona_bundle_includes_shared` | Shared persona 포함 확인 |
| `persona_bundle.rs` | `test_build_pm_bundle_no_ready_rule` | PM bundle READY 규칙 미포함 |
| `persona_bundle.rs` | `test_content_hash_is_deterministic` | 해시 결정론적 |
| `persona_bundle.rs` | `test_persona_not_found_error` | 없는 역할 오류 |
| `persona_bundle.rs` | `test_source_files_listed` | 소스 파일 목록 |
| `health_check.rs` | `test_health_check_report_serialization` | 보고서 JSON |
| `health_check.rs` | `test_provider_health_url` | URL 확인 |
| `health_check.rs` | `test_workspace_check_missing_agiteam_json` | agiteam.json 없음 감지 |
| `provider/claude.rs` | `test_build_request_body_basic` | Claude 요청 생성 |
| `provider/claude.rs` | `test_build_request_body_with_tools` | Claude tool 포함 요청 |
| `provider/claude.rs` | `test_sse_event_message_started` | SSE message_start 이벤트 |
| `provider/claude.rs` | `test_sse_event_content_delta` | SSE content_block_delta |
| `provider/openai.rs` | `test_build_request_body` | OpenAI 요청 생성 |
| `provider/openai.rs` | `test_sse_text_delta` | OpenAI SSE delta |
| `provider/openai.rs` | `test_sse_response_created` | OpenAI SSE created |
| `provider/gemini.rs` | `test_build_request_body_basic` | Gemini 요청 생성 |
| `provider/gemini.rs` | `test_parse_chunk_text_delta` | Gemini chunk 파싱 |
| `provider/gemini.rs` | `test_parse_chunk_usage_metadata` | Gemini usage 파싱 |
| `agent_session.rs` | `test_agent_lifecycle_state_transitions` | 상태 불일치 검증 |
| `agent_session.rs` | `test_message_ack_fields` | MessageAck 필드 |

**총 39개 단위 테스트** (파일시스템/keyring 기반 테스트는 tempfile/CI-skip 처리)

---

## 6. 빌드 실행 방법

### 필수 환경 (DV-10 환경구성서 참조)

```bash
# macOS
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source "$HOME/.cargo/env"
cargo install tauri-cli --locked
```

### 개발 실행

```bash
cd system/tos
pnpm install
cargo tauri dev
```

### 테스트 실행

```bash
cd system/tos/src-tauri
cargo test
```

### 운영 빌드

```bash
cd system/tos
cargo tauri build
```

---

## 7. 현재 제약 사항

| 항목 | 상태 | 비고 |
|------|------|------|
| Rust 미설치 | ⚠️ 빌드 불가 | DV-10 환경구성서 기준 rustup 설치 필요 |
| AgentSessionService 테스트 | ⚠️ AppHandle 의존 | 통합 테스트는 실 Tauri 환경 필요 |
| 메시지 저장 | 메모리 내 | 향후 SQLite(DS-30)로 이관 예정 |
| Provider 기본 모델 | 하드코딩 | `claude-3-5-sonnet-latest` 고정, 향후 설정화 |
| capabilities/default.json | 기본 템플릿 | command allowlist 추가 필요 (DS-60 §7.2) |

---

## 8. 영향받는 설계 문서 (Architect 갱신 필요)

| 문서 | 변경 이유 |
|------|-----------|
| DS-20 아키텍처설계서 | 실제 모듈명이 services/ 대신 루트 레벨 파일로 구현됨 (PM 지시 기준 반영) |
| DS-60 연동규격서 | `get_masked_credential` command 추가 (DS-60에 미정의) |
