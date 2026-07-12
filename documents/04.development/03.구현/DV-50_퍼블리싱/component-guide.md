---
doc: DV-50 Vue 컴포넌트 CSS 가이드
version: v0.1
last_updated: "2026-06-24"
author: Designer
status: drafting
project: tos (AgiTeamBuilder Desktop)
---

# DV-50 Vue 컴포넌트 CSS 가이드

## 개정이력

| 버전 | 일자 | 작성자 | 내용 |
|------|------|--------|------|
| v0.1 | 2026-06-24 | Designer | DS-55 시안 기반 HTML/CSS 퍼블리싱 CSS 분리 작성 |

## 1. 사용 순서

DeveloperFE는 Vue 엔트리 CSS에서 아래 순서로 import한다.

```css
@import "./design-tokens.css";
@import "./status-badge.css";
@import "./chat-panel.css";
@import "./workspace.css";
@import "./launcher.css";
@import "./boot.css";
@import "./deliverable.css";
```

## 2. 공통 토큰

| 토큰 | 용도 |
|------|------|
| `--bg-base` | 앱 전체 배경 |
| `--bg-panel` | 주요 패널 배경 |
| `--bg-surface` | 버튼, 칩, 카드 내부 표면 |
| `--accent`, `--accent-hover` | 주요 버튼, 포커스, 활성 상태 |
| `--ok` | READY, 성공 상태 |
| `--busy` | BUSY, 진행 상태 |
| `--error` | ERROR, 실패 상태 |
| `--approval` | 승인 프롬프트, 정보 상태 |
| `--text-primary`, `--text-secondary`, `--text-muted` | 텍스트 계층 |
| `--border` | 패널, 입력, 표 테두리 |
| `--radius` | 공통 8px radius |
| `--font-mono` | 로그, 코드, 파일 트리 |

색상은 직접 HEX를 쓰지 않고 토큰을 우선 사용한다. 화면별 강조가 필요하면 컴포넌트 modifier 클래스를 추가한다.

## 3. 공통 클래스

| 클래스 | 역할 | 적용 컴포넌트 |
|--------|------|---------------|
| `.dv-app-shell` | Tauri 앱 최상위 배경과 최소 크기 | `App.vue` |
| `.dv-app-header` | 64px 고정 상단 헤더 | `AppHeader.vue` |
| `.dv-brand`, `.dv-brand-mark` | 브랜드 영역과 A 마크 | `AppHeader.vue`, `LauncherView.vue` |
| `.dv-toolbar` | 헤더 우측 버튼 묶음 | `AppHeader.vue` |
| `.dv-button` | 공통 버튼 | 모든 화면 |
| `.dv-button--primary` | 주요 액션 버튼 | 새 프로젝트, 전송, 저장 |
| `.dv-button--danger` | 위험 액션 버튼 | 팀 종료, 거절 |
| `.dv-button--ghost` | 보조 버튼 | 뒤로, 새로고침 |
| `.dv-input`, `.dv-textarea`, `.dv-select` | 폼 입력 요소 | 설정, 채팅, 레드마인 |
| `.dv-label` | 폼 라벨 | 설정 화면 |
| `.dv-panel` | 공통 패널 컨테이너 | 카드/패널 공통 |
| `.dv-muted` | 보조 텍스트 | 전체 |
| `.dv-mono` | 모노스페이스 텍스트 | 로그, 코드 |

## 4. 상태 뱃지

| 클래스 | 역할 | 적용 컴포넌트 |
|--------|------|---------------|
| `.status-badge` | 상태 뱃지 기본 | `StatusBadge.vue` |
| `.status-badge--ready` | READY 상태 | PM/팀원 패널, 부팅 카드 |
| `.status-badge--busy` | BUSY 상태 | 스트리밍 중 |
| `.status-badge--error` | ERROR 상태 | 오류 상태 |
| `.status-badge--approval` | 승인 요청 상태 | 자동 Enter/수동 승인 알림 |
| `.status-badge--compact` | 좁은 테이블/카드용 | 레드마인 표, 작은 카드 |
| `.status-dot` 및 modifier | 도트만 필요한 상태 표시 | 부팅 역할 카드 |

## 5. Screen-01 런처

| 클래스 | 역할 | 적용 컴포넌트 |
|--------|------|---------------|
| `.launcher-view` | 470px 히어로 + 최근 프로젝트 2컬럼 레이아웃 | `LauncherView.vue` |
| `.launcher-hero` | 좌측 설명/액션 영역 | `LauncherView.vue` |
| `.launcher-hero__title` | 런처 대형 제목 | `LauncherView.vue` |
| `.launcher-actions` | 새 프로젝트/기존 열기 버튼 2분할 | `LauncherView.vue` |
| `.recent-projects` | 최근 프로젝트 패널 | `RecentProjectList.vue` |
| `.recent-projects__grid` | 프로젝트 카드 2컬럼 | `RecentProjectList.vue` |
| `.project-card` | 최근 프로젝트 카드 | `RecentProjectCard.vue` |
| `.project-card--active` | 현재 유효/선택 카드 | `RecentProjectCard.vue` |
| `.project-card--empty` | 파일에서 열기/빈 카드 | `RecentProjectCard.vue` |

## 6. Screen-03 부팅

| 클래스 | 역할 | 적용 컴포넌트 |
|--------|------|---------------|
| `.boot-view` | 부팅 본문 + 로그 410px 레이아웃 | `BootView.vue` |
| `.boot-main` | 좌측 진행 영역 | `BootView.vue` |
| `.boot-log` | 우측 실시간 로그 영역 | `BootLog.vue` |
| `.boot-steps` | 7단계 진행 그리드 | `StepBar.vue` |
| `.boot-step` | 단계 카드 기본 | `BootStep.vue` |
| `.boot-step--done`, `.boot-step--active`, `.boot-step--error` | 단계 상태 | `BootStep.vue` |
| `.boot-progress`, `.boot-progress__bar` | 전체 진행 바 | `BootProgress.vue` |
| `.boot-summary-grid` | 환경/API/자동제출 요약 | `BootSummary.vue` |
| `.role-boot-grid` | 팀원 3x2 부팅 카드 | `RoleBootGrid.vue` |
| `.role-boot-card` 및 상태 modifier | 역할별 부팅 상태 | `RoleBootCard.vue` |
| `.boot-log__stream` | 모노스페이스 로그 스트림 | `BootLog.vue` |

## 7. Screen-04 워크스페이스

| 클래스 | 역할 | 적용 컴포넌트 |
|--------|------|---------------|
| `.workspace-view` | PM 430px + 팀원 그리드 + 64px 사이드바 + 36px 상태바 | `WorkspaceView.vue` |
| `.workspace-view__pm` | 좌측 PM 패널 영역 | `PmChatPanel.vue` wrapper |
| `.workspace-view__team-grid` | 팀원 2열 3행 그리드 | `RolePanelGrid.vue` |
| `.workspace-view__sidebar` | 우측 도구 사이드바 | `WorkspaceSidebar.vue` |
| `.workspace-view__sidebar-button` | 산출물/레드마인/브라우저/설정 버튼 | `WorkspaceSidebar.vue` |
| `.workspace-view__statusbar` | 하단 project_state 상태 표시 | `WorkspaceStatusBar.vue` |
| `.workspace-header-project` | 헤더 프로젝트 정보 묶음 | `AppHeader.vue` |
| `.workspace-project-title` | 프로젝트명 표시 | `AppHeader.vue` |

## 8. 채팅 패널

| 클래스 | 역할 | 적용 컴포넌트 |
|--------|------|---------------|
| `.chat-panel` | 채팅 패널 기본 3단 구조 | `PmChatPanel.vue`, `RoleChatPanel.vue` |
| `.chat-panel--role` | 팀원 패널용 압축 variant | `RoleChatPanel.vue` |
| `.chat-panel--maximized` | 팀원 패널 최대화 상태 | `RoleChatPanel.vue` |
| `.chat-panel__header` | 역할/상태 헤더 | `PanelHeader.vue` |
| `.chat-panel__log` | 스크롤 대화 로그 | `ChatLog.vue` |
| `.chat-panel__message` | 메시지 버블 기본 | `ChatMessage.vue` |
| `.chat-panel__message--pm` | PM 발신 버블 | `ChatMessage.vue` |
| `.chat-panel__message--agent` | 에이전트 응답 버블 | `ChatMessage.vue` |
| `.chat-panel__chips`, `.chat-panel__chip` | startupFiles 빠른 링크 | `FileChips.vue` |
| `.chat-panel__composer` | 입력 영역 | `ChatComposer.vue` |
| `.chat-panel__input-row` | 입력 + 전송 + 최대화 버튼 행 | `ChatComposer.vue` |
| `.chat-panel__hint` | Ctrl+Enter 안내 | `ChatComposer.vue` |

## 9. Screen-05 산출물 뷰어

| 클래스 | 역할 | 적용 컴포넌트 |
|--------|------|---------------|
| `.deliverable-view` | 파일 트리 300px + 뷰어 + 이력 330px 레이아웃 | `DeliverablePanel.vue` |
| `.deliverable-tree` | 좌측 파일 트리 패널 | `FileTree.vue` |
| `.deliverable-main` | 중앙 렌더/편집 영역 | `MarkdownWorkspace.vue` |
| `.deliverable-history` | 우측 메타/이력 패널 | `ArchiveHistory.vue` |
| `.file-tree` | 모노스페이스 파일 트리 | `FileTree.vue` |
| `.file-tree__item--latest` | `.latest.md` 강조 | `FileTreeItem.vue` |
| `.file-tree__item--active` | 선택 파일 | `FileTreeItem.vue` |
| `.markdown-viewer` | 마크다운 렌더 컨테이너 | `MarkdownViewer.vue` |
| `.frontmatter-table` | frontmatter 정보 표 | `FrontmatterPanel.vue` |
| `.archive-list`, `.archive-item` | `_archive` 이력 목록 | `ArchiveDropdown.vue` |
| `.diff-preview`, `.diff-preview__add`, `.diff-preview__remove` | diff 미리보기 | `DiffViewer.vue` |

## 10. 반응형 기준

본 앱은 데스크톱 전용이다. 별도 모바일 브레이크포인트는 두지 않는다.

| 기준 | 값 |
|------|----|
| 최소 앱 크기 | `1280 x 768` |
| DS-55 캡처 기준 | `1440 x 900` |
| 워크스페이스 PM 패널 | `430px` 고정 |
| 워크스페이스 사이드바 | `64px` 고정 |
| 워크스페이스 상태바 | `36px` 고정 |
| 팀원 패널 | 남은 영역 2열 3행 균등 |

1280px 미만 환경은 Tauri window minimum size로 차단한다.

## 11. DS-55 정합성

- 다크 개발자 도구 톤을 유지한다.
- 패널 radius는 `8px` 이하로 유지한다.
- PM 패널은 좌측 고정 430px, 팀원 패널은 2열 3행이다.
- 상태 색상은 READY/BUSY/ERROR/APPROVAL 네 가지로 제한한다.
- 로그, 파일 트리, diff는 `--font-mono`를 사용한다.
- CSS는 Vue template에서 클래스 적용만으로 사용할 수 있게 작성했으며 script/template 코드는 포함하지 않았다.
