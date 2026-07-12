---
doc_id: DV-40
title: 프론트엔드 코드 INDEX
version: v0.1
last_updated: 2026-06-24
author: DeveloperFE
status: draft
---

# DV-40 프론트엔드 코드 INDEX

> **위치**: `system/tos/src/`  
> **기술 스택**: Vue 3 + TypeScript + Vite + Pinia + Vue Router  
> **빌드 상태**: ✅ `pnpm run build` 통과 (2026-06-24)

---

## 1. 디렉토리 구조 개요

```
system/tos/src/
├── assets/
│   └── main.css              # 디자인 토큰 + 전역 스타일
├── composables/
│   └── toast.ts              # 전역 Toast 싱글톤
├── components/
│   ├── AppToast.vue          # Toast 컨테이너 컴포넌트
│   ├── AppModal.vue          # 범용 모달 (ESC close, Teleport)
│   ├── StatusBadge.vue       # 에이전트 상태 뱃지 (dot + label)
│   ├── PmChatPanel.vue       # PM 채팅 패널 (Screen-04a)
│   ├── RoleChatPanel.vue     # 팀원 채팅 패널 (Screen-04b)
│   └── BrowserPanel.vue      # 내장 브라우저 패널
├── ipc/
│   ├── types.ts              # 전체 TypeScript 타입 정의
│   ├── workspace.ts          # Workspace IPC 래퍼
│   ├── agent.ts              # Agent IPC 래퍼
│   ├── document.ts           # Document IPC 래퍼
│   ├── credential.ts         # Credential IPC 래퍼
│   ├── health.ts             # Health check IPC 래퍼
│   └── events.ts             # Tauri 이벤트 리스너 래퍼
├── router/
│   └── index.ts              # Vue Router 라우트 정의
├── stores/
│   ├── project.ts            # 프로젝트/워크스페이스 상태
│   ├── boot.ts               # 부팅 7단계 상태 머신
│   ├── workspace.ts          # 워크스페이스 UI 상태
│   ├── role.ts               # 에이전트 세션 + 스트리밍
│   ├── deliverable.ts        # 산출물 파일 트리 + 에디터
│   ├── redmine.ts            # 레드마인 이슈 상태
│   ├── settings.ts           # 설정/인증 상태
│   └── browser.ts            # 내장 브라우저 상태
└── views/
    ├── LauncherView.vue       # Screen-01: 런처
    ├── ProjectSettingsView.vue # Screen-02: 프로젝트 설정
    ├── BootView.vue           # Screen-03: 팀 부팅
    ├── WorkspaceView.vue      # Screen-04: 워크스페이스
    ├── DeliverableView.vue    # Screen-05: 산출물 뷰어
    ├── RedmineView.vue        # Screen-06: 레드마인 패널
    └── SettingsView.vue       # Screen-07: 시스템 설정
```

---

## 2. 라우트 구조 (router/index.ts)

| 경로 | 컴포넌트 | 메타 가드 | 설명 |
|------|---------|----------|------|
| `/` | redirect → `/launcher` | — | 루트 리다이렉트 |
| `/launcher` | LauncherView | — | 시작 화면 (Screen-01) |
| `/settings` | ProjectSettingsView | — | 설정 (Screen-02) |
| `/boot` | BootView | `requiresConfig` | 부팅 (Screen-03) |
| `/workspace` | WorkspaceView | `requiresBoot` | 워크스페이스 (Screen-04) |
| `/workspace/deliverables` | DeliverableView | `requiresBoot` | 산출물 (Screen-05) |
| `/workspace/redmine` | RedmineView | `requiresBoot` | 레드마인 (Screen-06) |
| `/workspace/browser` | BrowserPanel | `requiresBoot` | 브라우저 |

**네비게이션 가드**:
- `requiresConfig` → `projectStore.workspaceId` 없으면 `/launcher` 리다이렉트
- `requiresBoot` → `bootStore.isDone` false면 `/boot` 리다이렉트

---

## 3. Pinia 스토어 명세

### 3.1 `useProjectStore` (stores/project.ts)

| 상태/액션 | 타입 | 설명 |
|---------|------|------|
| `workspaceId` | `string \| null` | 열린 워크스페이스 경로 |
| `config` | `AgiteamConfig \| null` | agiteam.json 파싱 결과 |
| `projectState` | `ProjectState \| null` | project_state.yaml |
| `recentProjects` | `RecentProject[]` | localStorage 영속화 |
| `load(path)` | action | IPC openWorkspace + loadWorkspaceConfig |
| `name` / `displayName` | computed | 프로젝트 명칭 |
| `teamMembers` | computed | PM 제외 역할 목록 |
| `pmConfig` / `businessType` / `milestone` | computed | 설정값 파생 |

### 3.2 `useBootStore` (stores/boot.ts)

7단계 부팅 상태 머신. `BootStepId` enum:  
`check_config → open_workspace → validate → boot_pm → boot_team → wait_ready → done`

| 상태 | 타입 | 설명 |
|------|------|------|
| `steps` | `BootStep[]` | 7단계 상태 배열 |
| `roleStates` | `RoleBootState[]` | 역할별 부팅 상태 |
| `isDone` | computed | 모든 단계 완료 여부 |
| `startStep(id)` / `completeStep(id)` / `failStep(id, msg)` | action | 상태 전이 |

### 3.3 `useWorkspaceStore` (stores/workspace.ts)

| 상태 | 타입 | 설명 |
|------|------|------|
| `activeSidebar` | `SidebarPanel \| null` | 열린 사이드바 패널 |
| `maximizedRole` | `string \| null` | 최대화된 역할 |
| `showSettingsOverlay` | `boolean` | 설정 오버레이 노출 |
| `showExitConfirm` | `boolean` | 종료 확인 다이얼로그 |
| `toggleSidebar(panel)` / `closeSidebar()` | action | 사이드바 토글 |

### 3.4 `useRoleStore` (stores/role.ts)

| 상태/액션 | 설명 |
|---------|------|
| `sessions` | `Map<role, RoleSession>` — 역할별 세션 |
| `applyStatusChanged({role, to})` | 에이전트 수명주기 상태 업데이트 |
| `startStreaming(role, msgId)` | 스트리밍 시작 |
| `appendMessageDelta(p)` | sequence 기반 순서 보장 델타 적용 |
| `completeStreaming` / `failStreaming` | 스트리밍 종료 |
| `readyCount` / `totalCount` | READY 카운터 computed |

**스트리밍 sequence 처리**: `pendingSequences: Map<number, string>` 버퍼로  
순서 어긋난 delta 보관 → lastSequence+1이 도착할 때 순서대로 flush (DS-60 §4.3 준수)

### 3.5 `useDeliverableStore` (stores/deliverable.ts)

파일 트리(DocumentNode), 뷰어/에디터 상태, 아카이브 이력 관리.  
`isDirty` = editBuffer !== currentContent.content

### 3.6 `useRedmineStore` (stores/redmine.ts)

Mock 이슈 목록, 이슈 상세 선택, 생성/수정 모달 상태.  
`REDMINE_STATUSES` 상수: 신규(1)·진행(2)·해결(3)·의견(4)·완료(5)·거절(6)

### 3.7 `useSettingsStore` (stores/settings.ts)

에이전트 인증 상태(claude/codex/gemini), 네트워크 체크, 레드마인 URL/API 키, project_state 편집 버퍼.

### 3.8 `useBrowserStore` (stores/browser.ts)

URL 히스토리 스택, canGoBack/canGoForward computed, navigate/goBack/goForward.

---

## 4. IPC 레이어 (ipc/)

> **규칙**: Vue 컴포넌트는 `invoke()` 직접 호출 금지. 반드시 `src/ipc/*.ts` 래퍼를 경유.

| 파일 | 커맨드 | 설명 |
|------|--------|------|
| `workspace.ts` | `open_workspace`, `load_workspace_config`, `validate_workspace` | 워크스페이스 |
| `agent.ts` | `boot_team`, `boot_role`, `stop_role`, `send_agent_message` | 에이전트 제어 |
| `document.ts` | `list_documents`, `read_document`, `write_latest_document` | 산출물 CRUD |
| `credential.ts` | `save_credential`, `delete_credential`, `validate_credential` | 자격증명 |
| `health.ts` | `run_health_check` | 진단 |
| `events.ts` | Tauri 이벤트 listen 래퍼 | `agent:*`, `document:*`, `workspace:*` |

**Tauri 이벤트 목록**:

| 이벤트명 | 페이로드 타입 | 구독 위치 |
|---------|-------------|---------|
| `agent:status_changed` | `AgentStatusChanged` | WorkspaceView |
| `agent:message_started` | `{session_id, message_id}` | WorkspaceView |
| `agent:message_delta` | `AgentMessageDelta` | WorkspaceView → roleStore |
| `agent:message_completed` | `AgentMessageCompleted` | WorkspaceView |
| `agent:message_failed` | `AgentMessageFailed` | WorkspaceView |
| `document:updated` | `{path}` | WorkspaceView |

---

## 5. 디자인 토큰 (assets/main.css)

```css
--bg-base:     #0f1117   /* 최하위 배경 */
--bg-panel:    #1a1d27   /* 패널 배경 */
--bg-panel-2:  #1e2130   /* 패널 2차 */
--bg-input:    #151823   /* 입력 필드 */
--accent:      #6366f1   /* 주요 강조색 (Indigo) */
--ok:          #22c55e   /* 성공/READY */
--busy:        #fbbf24   /* 실행중/경고 */
--error:       #ef4444   /* 오류 */
--text-primary:#e2e8f0
--text-soft:   #94a3b8
--text-muted:  #64748b
```

DS-55 mockup.css의 `--primary: #5eead4` (teal)도 `--teal` 변수로 공존 보존.

---

## 6. 주요 구현 패턴

### 6.1 백엔드 없는 화면 — Mock 폴백

모든 IPC 호출은 `try/catch`로 감싸며, 에러 시 mock 데이터를 사용합니다:

```ts
try {
  const tree = await listDocuments(workspaceId)
  store.setTree(tree.root)
} catch {
  store.setTree(mockTree)  // 백엔드 미연동 시 mock
}
```

### 6.2 스트리밍 delta 순서 보장

```ts
// sequence가 연속적이지 않으면 pendingSequences에 버퍼링
if (p.sequence !== lastSeq + 1) {
  pending.set(p.sequence, p.delta)
  return
}
// 연속된 sequence flush
while (pending.has(nextSeq)) { ... }
```

### 6.3 Toast 전역 사용

```ts
import { showToast } from '@/composables/toast'
showToast('저장 완료', 'ok')
showToast('오류 발생', 'error')  // error는 자동 닫힘 없음
```

---

## 7. 개정 이력

| 버전 | 일자 | 내용 | 작성자 |
|------|------|------|--------|
| v0.1 | 2026-06-24 | 최초 작성 — DV-40 구현 완료 후 INDEX 생성 | DeveloperFE |
