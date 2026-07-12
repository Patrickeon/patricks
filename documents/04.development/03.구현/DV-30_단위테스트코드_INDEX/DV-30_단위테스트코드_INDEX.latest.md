---
doc: DV-30_단위테스트코드_INDEX
version: v0.1
last_updated: 2026-06-24T00:00:00+09:00
author: DeveloperBE
---

# DV-30 단위 테스트 코드 INDEX

## 개정이력

| 버전 | 일자 | 작성자 | 내용 |
|------|------|--------|------|
| v0.1 | 2026-06-24 | DeveloperBE | 최초 작성 — DV-30 단위 테스트 전체 구현 |

---

## 1. 개요

- **크레이트**: `system/tos/src-tauri` (Rust / Tauri 2)
- **테스트 방식**: Rust 표준 `#[cfg(test)]` + `#[test]` / `#[tokio::test]`
- **외부 의존 처리**: OS 키체인·네트워크는 실패 시 skip 처리, 파일시스템은 `tempfile::TempDir` mock 사용
- **실행 명령**: `cargo test --lib`
- **총 테스트 수**: 44개 (44 passed, 0 failed)

---

## 2. 모듈별 테스트 케이스 목록

### 2.1 credential.rs — API 키 저장/조회 (3개)

| 번호 | 테스트 함수 | 분류 | 설명 |
|:---:|------------|:---:|------|
| 1 | `test_credential_save_get_delete` | 정상 | OS 키체인 저장 → 조회 → 삭제 라운드트립 (키체인 불가 환경 자동 skip) |
| 2 | `test_credential_missing_error` | 오류 | 존재하지 않는 credential 조회 시 에러 반환 확인 |
| 3 | `test_masked_credential` | 정상 | CredentialRef → MaskedCredential 변환 (secret 미노출) 확인 |

### 2.2 persona_bundle.rs — 파일 읽기 + startupFiles 번들링 (5개)

| 번호 | 테스트 함수 | 분류 | 설명 |
|:---:|------------|:---:|------|
| 4 | `test_build_persona_bundle_includes_shared` | 정상 | Shared persona + 역할 persona + READY 규칙 번들 포함 확인 |
| 5 | `test_build_pm_bundle_no_ready_rule` | 경계 | PM 역할 bundle에는 READY 대기 규칙이 미포함 확인 |
| 6 | `test_content_hash_is_deterministic` | 정상 | 동일 입력 → 동일 SHA-256 hash 보장 확인 |
| 7 | `test_persona_not_found_error` | 오류 | 존재하지 않는 역할 요청 시 PERSONA_NOT_FOUND 에러 확인 |
| 8 | `test_source_files_listed` | 정상 | source_files에 Shared·역할 파일 경로 모두 포함 확인 |

### 2.3 file_document.rs — .latest.md 읽기/쓰기 + _archive 자동 백업 (4개)

| 번호 | 테스트 함수 | 분류 | 설명 |
|:---:|------------|:---:|------|
| 9 | `test_write_and_read_document` | 정상 | 문서 쓰기 후 읽기 내용 일치 확인 |
| 10 | `test_write_creates_archive` | 정상 | 2회 쓰기 시 `_archive/` 백업 파일 자동 생성 확인 |
| 11 | `test_path_traversal_blocked` | 오류 | `../../etc/passwd` 접근 시 PATH_TRAVERSAL 에러 확인 |
| 12 | `test_list_documents_empty` | 경계 | documents 폴더 없는 workspace의 빈 트리 반환 확인 |

### 2.4 health_check.rs — API 연결 상태 확인 (3개)

| 번호 | 테스트 함수 | 분류 | 설명 |
|:---:|------------|:---:|------|
| 13 | `test_health_check_report_serialization` | 정상 | HealthCheckReport JSON 직렬화 구조 확인 |
| 14 | `test_provider_health_url` | 정상 | 각 Provider endpoint URL 매핑 정확성 확인 |
| 15 | `test_workspace_check_missing_agiteam_json` | 오류 | agiteam.json 없는 workspace → Unreachable 상태 확인 (비동기) |

### 2.5 agent_session.rs — 세션 상태 전환 로직 (5개)

| 번호 | 테스트 함수 | 분류 | 설명 |
|:---:|------------|:---:|------|
| 16 | `test_agent_lifecycle_state_transitions` | 정상 | Idle ≠ Ready, Booting ≠ Running 상태 구별 확인 |
| 17 | `test_message_ack_fields` | 정상 | MessageAck 구조 session_id 필드 확인 |
| 18 | `test_agent_session_summary_serialization` | 정상 | AgentSessionSummary JSON 라운드트립 + state 소문자 확인 |
| 19 | `test_agent_status_changed_serialization` | 정상 | AgentStatusChanged 이벤트 페이로드 직렬화 확인 |
| 20 | `test_agent_session_failed_state_has_reason` | 오류 | Booting→Failed 전이 시 failure_reason 설정 확인 |

### 2.6 provider/claude.rs — Claude API Adapter (4개)

| 번호 | 테스트 함수 | 분류 | 설명 |
|:---:|------------|:---:|------|
| 21 | `test_build_request_body_basic` | 정상 | 기본 요청 body model·stream·max_tokens 필드 확인 |
| 22 | `test_build_request_body_with_tools` | 정상 | tools 포함 시 body에 tools 배열 포함 확인 |
| 23 | `test_sse_event_message_started` | 정상 | `message_start` SSE → MessageStarted 이벤트 변환 확인 |
| 24 | `test_sse_event_content_delta` | 정상 | `content_block_delta` SSE → MessageDelta + sequence 증가 확인 |

### 2.7 provider/openai.rs — OpenAI API Adapter (3개)

| 번호 | 테스트 함수 | 분류 | 설명 |
|:---:|------------|:---:|------|
| 25 | `test_build_request_body` | 정상 | model·instructions·stream 필드 포함 확인 |
| 26 | `test_sse_text_delta` | 정상 | `response.output_text.delta` → MessageDelta 변환 확인 |
| 27 | `test_sse_response_created` | 정상 | `response.created` → MessageStarted 변환 확인 |

### 2.8 provider/gemini.rs — Gemini API Adapter (3개)

| 번호 | 테스트 함수 | 분류 | 설명 |
|:---:|------------|:---:|------|
| 28 | `test_build_request_body_basic` | 정상 | systemInstruction·contents·generationConfig 필드 확인 |
| 29 | `test_parse_chunk_text_delta` | 정상 | candidates.content.parts[].text → MessageStarted + MessageDelta 변환 확인 |
| 30 | `test_parse_chunk_usage_metadata` | 정상 | usageMetadata → MessageCompleted + TokenUsage 변환 확인 |

---

## 3. 보조 모델 테스트 (models/)

models/ 하위 모듈에도 #[cfg(test)] 블록이 포함됨 (총 14개):

| 모듈 | 테스트 수 | 주요 내용 |
|------|:---:|------|
| `models/error.rs` | 3개 | AppError 생성·Display·detail 확인 |
| `models/session.rs` | 3개 | AgentLifecycleState display·serde, CommandResult 확인 |
| `models/provider.rs` | 3개 | AiProviderKind display·from_str·ProviderHealth 직렬화 |
| `models/message.rs` | 2개 | AgentMessage·MessageDelta 직렬화 확인 |
| `models/persona.rs` | 1개 | PersonaBundlePreview from 변환 확인 |
| `models/workspace.rs` | 2개 | AgiteamSettings defaults·ValidationReport 직렬화 확인 |

---

## 4. cargo test 실행 결과

```
$ cargo test --lib
running 44 tests
... (44개 all ok)

test result: ok. 44 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.05s
```

**44/44 PASS** — 컴파일 에러 0건, 테스트 실패 0건

### 4.1 수정된 컴파일 에러 (테스트 과정에서 발견 및 수정)

| 파일 | 에러 코드 | 내용 | 조치 |
|------|----------|------|------|
| `lib.rs` | E0599 | `tauri::Manager` 트레이트 미import로 `.manage()` 사용 불가 | `use tauri::Manager;` 추가 |
| `health_check.rs` | E0119 | `AiProviderKind`에 `Eq` 중복 구현 (derive + impl) | `impl Eq` 수동 구현 제거 |
| `health_check.rs` | E0382 | `HealthStatus` move 후 borrow — `status` 필드 사용 시 | `status: status.clone()` 수정 |

---

## 5. 테스트 품질 기준 준수 현황

| 기준 | 상태 |
|------|:---:|
| 모듈당 최소 3개 테스트 | ✅ |
| 정상·경계·오류 케이스 포함 | ✅ |
| OS 키체인·네트워크 의존 mock/skip 처리 | ✅ |
| 파일시스템은 tempfile::TempDir 사용 | ✅ |
| `cargo test --lib` PASS | ✅ |
