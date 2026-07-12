---
title: RD020 FE 현황분석 — CLI→GUI 화면 역추출
version: v0.1
last_updated: 2026-06-22
author: DeveloperFE (프개발)
---

# RD020 FE 현황분석 — CLI→GUI 화면 역추출

## 개정이력

| 버전 | 일시 | 작성자 | 변경 내용 |
|------|------|--------|-----------|
| v0.1 | 2026-06-22 | DeveloperFE | 최초 작성 |

---

## 1. 분석 개요

### 1.1 분석 대상

| 항목 | 내용 |
|------|------|
| 분석 소스 | `system/patricks/agiteam.sh`, `system/patricks/agiteam.json` |
| 현재 시스템 | AgiTeamBuilder (bash 기반 CLI + cmux 터미널 멀티플렉서) |
| 목표 시스템 | Windows/Mac 설치형 GUI 앱 |
| 분석 방법 | CLI 인터랙션 → GUI 화면 역추출 |

### 1.2 현재 시스템 구조 요약

```
[agiteam.sh 실행]
   ↓
agiteam.json 로드 → 환경/인증 검증 → workspace 식별
   ↓
레이아웃 슬롯 생성 (cmux pane 분할)
   ↓
팀원 순차 부팅 (페르소나 번들 생성 → 에이전트 실행 → READY 신호 대기)
   ↓
PM 실행 (시스템 프롬프트 주입 + startupFiles 제공)
   ↓
PM이 cmux send/read-screen으로 팀원과 비동기 소통
```

**현재 레이아웃 (cmux pane 구조):**
```
PM(박피엠)     | DeveloperBE(박개발, middle_top)   | Architect(김아키, right_top)
               | DeveloperFE(프개발, middle_mid)   | DevOps(김데옵, right_mid)
               | QA(홍감리,   middle_bottom)       | Designer(장이너, right_bottom)
```

---

## 2. CLI 주요 인터랙션 목록화

### 2.1 PM이 터미널에서 하는 행위

| # | cmux 명령 | 행위 설명 | GUI 전환 시 대응 동작 |
|---|-----------|-----------|----------------------|
| 1 | `cmux send --surface <surf> "메시지"` | 팀원 입력창에 텍스트 입력 | 해당 팀원 패널 입력창에 타이핑 |
| 2 | `cmux send-key --surface <surf> Enter` | 입력된 메시지 제출 | 전송 버튼 클릭 or Enter 키 |
| 3 | `cmux read-screen --surface <surf> --lines N` | 팀원 화면 스냅샷 읽기 | 팀원 패널 출력 실시간 렌더링으로 대체 |
| 4 | `cmux identify` | 현재 workspace/surface ID 확인 | 앱 내부 상태로 자동 관리 |
| 5 | `cmux list-pane-surfaces --workspace <ws>` | workspace 내 pane 목록 조회 | 팀 구성 패널 목록으로 표시 |
| 6 | `cmux rename-workspace --workspace <ws> "이름"` | workspace 이름 변경 | 프로젝트 설정 → workspace 이름 편집 |
| 7 | `cmux workspace-action ... --action set-color` | workspace 색상 설정 | 프로젝트 설정 → 색상 선택 |
| 8 | `cmux rename-tab --surface <surf> "이름"` | 탭 이름 변경 | 팀원 패널 헤더 (자동 설정) |
| 9 | `cmux new-split <direction> --surface <surf>` | pane 분할 생성 | 팀 부팅 시 GUI 패널 자동 배치 |
| 10 | `cmux close-pane --surface <surf>` | pane 닫기 | 팀원 패널 종료 버튼 |
| 11 | `cmux browser --surface <surf> <명령>` | 브라우저 제어 | 내장 브라우저 패널로 대체 |

### 2.2 팀원 pane에서 일어나는 표시

| # | 현재 pane 표시 | 의미 | GUI 전환 시 표현 방식 |
|---|---------------|------|----------------------|
| 1 | `READY: {역할명}` 텍스트 출력 | 부팅 완료 신호 | 패널 상태 뱃지: 🟢 READY |
| 2 | 에이전트 부팅 진행 로그 | 초기화 중 | 부팅 진행 화면 프로그레스바 |
| 3 | 작업 처리 중 출력 (스트리밍) | 실행 중 | 패널 상태 뱃지: 🟡 BUSY + 스트리밍 텍스트 |
| 4 | 완료 보고 텍스트 | 작업 완료 | 대화 버블 + 상태 READY 복귀 |
| 5 | 승인/확인 요청 프롬프트 | 부팅 중 상호작용 필요 | 알림 토스트 + Enter 자동 제출 옵션 |
| 6 | 에러 메시지 | 오류 발생 | 패널 상태 뱃지: 🔴 ERROR + 에러 텍스트 |

### 2.3 시스템(agiteam.sh)이 하는 자동 행위

| # | 행위 | GUI 전환 시 대응 |
|---|------|-----------------|
| 1 | agiteam.json 로드·파싱 | 앱 시작 시 자동 로드, GUI 폼으로 편집 |
| 2 | 환경 검증 (cmux, python3, claude, codex 등) | 부팅 전 환경 체크 화면에서 자동 실행 |
| 3 | API 연결 확인 (Anthropic, OpenAI 엔드포인트) | 연결 상태 표시기 |
| 4 | 로그인 상태 확인 (Keychain/credentials 파일) | 로그인 상태 패널 |
| 5 | project_state.yaml 읽기 (business_type, mode, milestone, wbs_track) | 프로젝트 상태 대시보드 |
| 6 | Shared + 역할 persona.md 번들 생성 | 페르소나 에디터 + 자동 번들링 |
| 7 | 레이아웃 슬롯 생성 (pane 분할) | 팀 워크스페이스 패널 자동 배치 |
| 8 | 팀원 순차 부팅 (명령 실행 → READY 지시문 → READY 신호 대기) | 부팅 진행 화면 |
| 9 | READY 신호 타임아웃 감시 (maxAutoSubmits, readySignalTimeout) | 타임아웃 경고 및 재시도 버튼 |
| 10 | PM 시스템 파일 생성 (persona + 팀 구성 + surface 매핑) | 내부 처리, PM 채팅 패널에 컨텍스트 표시 |

---

## 3. GUI 화면 인벤토리

### Screen-01: 런처 / 시작 화면

**목적:** 앱 진입점. 기존 프로젝트 불러오기 또는 새 프로젝트 생성.

**주요 컴포넌트:**

| 컴포넌트 | 유형 | 설명 |
|---------|------|------|
| 앱 로고 / 타이틀 | 정적 | AgiTeamBuilder 브랜드 |
| 최근 프로젝트 목록 | 리스트 | 최근 작업 프로젝트 (이름, 경로, 마지막 접속 시간) |
| 새 프로젝트 버튼 | 버튼 | 프로젝트 설정 화면(Screen-02)으로 이동 |
| 기존 프로젝트 열기 버튼 | 버튼 | OS 파일 선택 다이얼로그 → agiteam.json 선택 |
| 버전 정보 | 정적 | 앱 버전 표시 |

**이벤트:**

| 이벤트 | 트리거 | 결과 |
|--------|--------|------|
| 최근 프로젝트 클릭 | 사용자 클릭 | 해당 프로젝트 로드 → Screen-03(부팅 진행) |
| 새 프로젝트 | 버튼 클릭 | Screen-02(프로젝트 설정)으로 이동 |
| 기존 프로젝트 열기 | 버튼 클릭 | 파일 탐색기 → agiteam.json 선택 → Screen-02 |

---

### Screen-02: 프로젝트 설정 화면 (agiteam.json GUI 에디터)

**목적:** agiteam.json 설정값을 GUI 폼으로 편집. CLI에서 JSON 직접 수정하던 것을 대체.

**주요 컴포넌트:**

#### 2-1. 프로젝트 기본 정보 섹션

| 필드 | 유형 | 기존 JSON 키 | 설명 |
|------|------|-------------|------|
| 프로젝트 이름 (내부) | 텍스트 입력 | `project.name` | 영문, 식별자용 |
| 프로젝트 표시 이름 | 텍스트 입력 | `project.displayName` | 화면에 표시할 이름 |
| 워크스페이스 이름 | 텍스트 입력 | `project.workspace.name` | 팀 워크스페이스 타이틀 |
| 워크스페이스 색상 | 색상 선택 | `project.workspace.color` | Indigo/Red/Green 등 |

#### 2-2. 팀 구성 섹션 (현재 6개 역할)

각 역할 카드 형태로 표시:

| 필드 | 유형 | 기존 JSON 키 | 설명 |
|------|------|-------------|------|
| 역할 ID | 텍스트 입력 (고정) | `team[].role` | DeveloperBE, DeveloperFE 등 |
| 담당자 이름 | 텍스트 입력 | `team[].name` | 박개발, 프개발 등 |
| 에이전트 타입 | 드롭다운 | `team[].agent` | claude / codex / gemini |
| 실행 명령어 | 텍스트 입력 | `team[].command` | 실행 명령 (고급 옵션) |
| 레이아웃 슬롯 | 드롭다운 | `team[].layout` | middle_top/mid/bottom, right_top/mid/bottom |
| 역할 추가/삭제 버튼 | 버튼 | — | 팀원 추가 또는 제거 |

#### 2-3. PM 설정 섹션

| 필드 | 유형 | 기존 JSON 키 | 설명 |
|------|------|-------------|------|
| PM 이름 | 텍스트 입력 | `pm.name` | 박피엠 |
| 에이전트 타입 | 드롭다운 | `pm.agent` | claude / codex / gemini |
| 실행 명령어 | 텍스트 입력 | `pm.command` | — |
| 시작 메시지 | 텍스트 영역 | `pm.startupMessage` | PM 부팅 시 첫 메시지 |
| 시작 파일 목록 | 파일 경로 리스트 | `pm.startupFiles[]` | PM이 세션 시작 시 읽을 파일 목록 (드래그&드롭 순서 변경) |

#### 2-4. 타이밍 설정 섹션

| 필드 | 유형 | 기존 JSON 키 | 기본값 |
|------|------|-------------|--------|
| READY 대기 타임아웃 (초) | 숫자 입력 | `settings.readyTimeout` | 30 |
| 부팅 후 지연 (초) | 숫자 입력 | `settings.postLaunchDelay` | 3 |
| READY 신호 대기 최대 (초) | 숫자 입력 | `settings.readySignalTimeout` | 60 |
| 자동 Enter 최대 횟수 | 숫자 입력 | `settings.maxAutoSubmits` | 5 |

#### 2-5. 페르소나 경로 섹션

| 필드 | 유형 | 기존 JSON 키 | 설명 |
|------|------|-------------|------|
| 페르소나 디렉토리 | 경로 입력 | `persona.dir` | brain/ |
| Shared 공통 파일 | 파일 경로 | `persona.commonFile` | brain/Shared/persona.md |

**이벤트:**

| 이벤트 | 트리거 | 결과 |
|--------|--------|------|
| 저장 | 저장 버튼 | agiteam.json 파일 덮어쓰기 |
| 팀원 추가 | + 버튼 | 빈 역할 카드 추가 |
| 팀원 삭제 | 카드 × 버튼 | 해당 역할 제거 (확인 다이얼로그) |
| 팀 부팅 시작 | 부팅 시작 버튼 | Screen-03(부팅 진행) 이동 |
| 페르소나 파일 열기 | 파일 경로 클릭 | Screen-05(산출물 뷰어)에서 파일 열기 |

---

### Screen-03: 팀 부팅 진행 화면

**목적:** agiteam.sh main() 실행 흐름을 시각화. 환경 검증부터 PM 실행까지 단계별 진행 상태 표시.

**주요 컴포넌트:**

| 컴포넌트 | 설명 | 기존 CLI 대응 |
|---------|------|--------------|
| 단계별 진행 바 | 부팅 전체 흐름 (7단계) | agiteam.sh main() 순서 |
| 환경 검증 결과 | cmux/python3/claude/codex 체크 | `validate_environment()` |
| API 연결 상태 | Anthropic API / OpenAI API 연결 표시 | `check_api_reachable()` |
| 로그인 상태 표시 | Claude Code / Codex 로그인 여부 | `validate_auth_and_network()` |
| 팀원 부팅 상태 카드 | 역할별 부팅 진행 표시 (대기 → 실행 중 → READY) | `spawn_member()` |
| READY 신호 카운터 | `n/6 READY 수신` 표시 | `wait_for_ready_signal()` |
| 경고/에러 메시지 | 경고 및 오류 상세 표시 | `warn()`, `fail()` |
| 재시도 버튼 | 부팅 실패 시 재시도 | `--cleanup-existing`, `--force-reuse` 옵션 |

**부팅 단계 목록:**

| 단계 | 표시 이름 | 완료 조건 | 기존 함수 |
|------|-----------|-----------|-----------|
| 1 | 설정 파일 로드 | agiteam.json 파싱 완료 | `load_config()` |
| 2 | 환경 검증 | 필수 명령어 모두 확인 | `validate_environment()` |
| 3 | 인증·네트워크 확인 | API 연결 + 로그인 정상 | `validate_auth_and_network()` |
| 4 | 프로젝트 구조 검증 | 페르소나 파일 존재 확인 | `validate_workspace()` |
| 5 | 워크스페이스 설정 | workspace 이름/색상 설정 | `resolve_workspace_context()` |
| 6 | 팀원 부팅 | 전원 READY 신호 수신 | `spawn_team_members()` |
| 7 | PM 실행 | PM 채팅 패널 활성화 | `launch_pm()` |

**이벤트:**

| 이벤트 | 트리거 | 결과 |
|--------|--------|------|
| 자동 Enter 제출 | 승인 프롬프트 감지 | Enter 자동 전송 (maxAutoSubmits 제한 내) |
| 타임아웃 경고 | readySignalTimeout 초과 | 알림 + 재시도 or 강제 진행 버튼 |
| 부팅 완료 | 전원 READY 수신 + PM 실행 | Screen-04(팀 워크스페이스)로 자동 전환 |
| 강제 재사용 | 재시도 버튼 클릭 | 기존 pane 정리 후 재부팅 |
| 설정으로 이동 | 설정 버튼 클릭 | Screen-02로 이동 |

---

### Screen-04: 팀 워크스페이스 화면 (메인 작업 화면)

**목적:** PM과 팀원 전체가 보이는 메인 작업 공간. 현재 cmux 터미널 레이아웃을 GUI로 전환.

**주요 컴포넌트:**

| 컴포넌트 | 위치 | 크기 | 기존 CLI 대응 |
|---------|------|------|--------------|
| 워크스페이스 헤더 바 | 상단 전체 | 고정 | workspace 이름/색상 |
| PM 채팅 패널 | 좌측 1/3 | 대형 | PM pane (현재 터미널 좌측 pane) |
| 팀원 채팅 패널 × 6 | 우측 2/3 (2열 3행) | 균등 | middle/right top/mid/bottom |
| 사이드바 (산출물) | 가장 우측 (옵션) | 축소 가능 | 없음 (신규 기능) |
| 하단 상태 바 | 하단 전체 | 고정 | 없음 (신규 기능) |

**레이아웃 매핑 (agiteam.json layout 값 → GUI 위치):**

```
[PM 패널 (좌측 대형)]  [middle_top: DeveloperBE]  [right_top: Architect]
                       [middle_mid: DeveloperFE]  [right_mid: DevOps]
                       [middle_bottom: QA]         [right_bottom: Designer]
```

**하단 상태 바 필드:**

| 필드 | 설명 |
|------|------|
| 프로젝트 이름 | 현재 프로젝트 |
| business_type | 사업 유형 (project_state.yaml) |
| current_mode | 프로젝트/운영 모드 |
| milestone | 현재 마일스톤 |
| wbs_track | WBS 트랙 (A/B) |
| 전체 팀 READY 상태 | n/7 READY 카운터 |

**이벤트:**

| 이벤트 | 트리거 | 결과 |
|--------|--------|------|
| 패널 클릭 | 팀원 패널 클릭 | 해당 패널 포커스 + 입력창 활성화 |
| 산출물 뷰어 열기 | 사이드바 토글 | Screen-05(산출물 뷰어) 패널 확장 |
| 레드마인 패널 열기 | 사이드바 아이콘 클릭 | Screen-06(레드마인 패널) 표시 |
| 설정 열기 | 헤더 설정 아이콘 | Screen-07(환경 설정) 오버레이 |
| 팀 종료 | 헤더 종료 버튼 | 종료 확인 다이얼로그 → 앱 종료 |

---

### Screen-04a: PM 채팅 패널 (상세)

**목적:** PM(박피엠) 에이전트와의 대화 창. 현재 PM pane 역할.

**주요 컴포넌트:**

| 컴포넌트 | 유형 | 설명 |
|---------|------|------|
| 패널 헤더 | 정적 | "박피엠 (PM)" + 에이전트 타입 뱃지 (claude) + 상태 |
| 대화 로그 영역 | 스크롤 가능 텍스트 | PM 입력 + 에이전트 응답 스트리밍 표시 |
| 메시지 입력창 | 멀티라인 텍스트 입력 | — |
| 전송 버튼 | 버튼 | `cmux send` + `cmux send-key Enter` 대응 |
| startupFiles 빠른 링크 | 칩 목록 | pm.startupFiles 파일들 빠른 열기 |
| 화면 스냅샷 버튼 | 버튼 | `cmux read-screen` 대응 (수동 갱신) |

**이벤트:**

| 이벤트 | 트리거 | 결과 |
|--------|--------|------|
| 메시지 전송 | 전송 버튼 or Ctrl+Enter | PM에게 메시지 전달 |
| 파일 링크 클릭 | startupFiles 칩 클릭 | Screen-05에서 해당 파일 열기 |
| 스트리밍 수신 | PM 에이전트 응답 | 대화 로그 실시간 업데이트 |

---

### Screen-04b: 팀원 채팅 패널 (상세, × 6)

**목적:** 각 팀원(DeveloperBE·FE·QA·Architect·DevOps·Designer) 에이전트와의 대화 창.  
현재 `cmux send/read-screen`으로 소통하던 것을 패널 UI로 대체.

**주요 컴포넌트:**

| 컴포넌트 | 유형 | 설명 | 기존 CLI 대응 |
|---------|------|------|--------------|
| 패널 헤더 | 정적 | 역할명 + 담당자명 + 에이전트 타입 뱃지 | `rename-tab` 탭 이름 |
| 상태 뱃지 | 상태 표시 | 🟢 READY / 🟡 BUSY / 🔴 ERROR | `READY: {role}` 신호 |
| 대화 로그 영역 | 스크롤 가능 | PM 메시지 + 팀원 응답 스트리밍 | `read-screen` 출력 |
| 빠른 전송 입력창 | 단행 텍스트 | PM이 이 팀원에게 직접 메시지 | `cmux send` |
| 전송 버튼 | 버튼 | — | `cmux send-key Enter` |
| 화면 갱신 버튼 | 버튼 | 수동 스냅샷 갱신 | `cmux read-screen` |
| 패널 최대화 버튼 | 버튼 | 해당 패널 전체화면 확대 | — |

**이벤트:**

| 이벤트 | 트리거 | 결과 |
|--------|--------|------|
| 메시지 전송 | 전송 버튼 or Enter | 해당 팀원에게 메시지 전달 |
| READY 감지 | 에이전트 출력 분석 | 상태 뱃지 🟢 READY로 변경 |
| BUSY 감지 | 응답 스트리밍 시작 | 상태 뱃지 🟡 BUSY로 변경 |
| 승인 프롬프트 감지 | 출력 패턴 매칭 | 알림 표시 + 자동/수동 Enter 전송 선택 |
| 패널 최대화 | 최대화 버튼 | 해당 패널을 메인 영역으로 확대 |

---

### Screen-05: 산출물 뷰어/에디터

**목적:** 프로젝트 문서(.latest.md 파일) 조회 및 편집. 현재 파일시스템 직접 접근을 대체.  
버전관리 정책(Shared persona §6) 시각화 지원.

**주요 컴포넌트:**

| 컴포넌트 | 유형 | 설명 |
|---------|------|------|
| 파일 트리 패널 | 트리 뷰 | 프로젝트 디렉토리 구조 (`.latest.md` 강조) |
| 마크다운 렌더 뷰어 | 읽기 전용 렌더링 | .latest.md 파일 렌더링 (테이블·헤딩 등) |
| 편집 모드 토글 | 버튼 | 원문 편집 모드 전환 |
| Frontmatter 메타 표시 | 정보 패널 | version, last_updated, author 표시 |
| _archive/ 이력 탐색기 | 드롭다운 or 사이드 패널 | `_archive/<이름>_YYYYMMDDhhmmss.md` 타임스탬프 목록 |
| 버전 비교 뷰 | diff 뷰어 | 현재 latest vs 이전 archive 비교 |
| 저장 버튼 | 버튼 | 버전관리 정책 자동 적용 (백업→갱신) |

**이벤트:**

| 이벤트 | 트리거 | 결과 |
|--------|--------|------|
| 파일 선택 | 파일 트리 클릭 | 해당 .latest.md 뷰어에 표시 |
| 편집 모드 | 편집 토글 | 마크다운 소스 편집 가능 |
| 저장 | 저장 버튼 | 자동: _archive/ 백업 → latest 갱신 (버전관리 정책 §6) |
| 이력 조회 | 이력 탐색기 항목 선택 | 해당 시점 아카이브 파일 뷰어 표시 |
| 비교 | 비교 버튼 | latest vs 선택 archive diff 표시 |

---

### Screen-06: 레드마인 패널

**목적:** 레드마인 이슈 관리. 현재 PM이 curl API를 직접 실행하던 것을 UI로 대체.

**주요 컴포넌트:**

| 컴포넌트 | 유형 | 설명 | 기존 CLI 대응 |
|---------|------|------|--------------|
| 프로젝트 선택 | 드롭다운 | 레드마인 project_id 선택 | `project_id` 파라미터 |
| 이슈 목록 | 테이블 | 번호·제목·상태·담당자·진척률 | `GET /issues.json` |
| 이슈 상세 | 슬라이드오버 | 이슈 내용·코멘트·이력 | `GET /issues/<id>.json` |
| 이슈 생성 폼 | 모달 | 트래커·제목·내용·담당자 | `POST /issues.json` |
| 상태 변경 버튼 | 버튼 세트 | 진행/해결/완료/거절 | `PUT /issues/<id>.json` |
| 코멘트 입력 | 텍스트 영역 + 버튼 | notes 추가 | `PUT /issues/<id>.json` (notes 필드) |
| 진척률 슬라이더 | 슬라이더 | done_ratio 0~100 | `done_ratio` 필드 |

**필드 상세 — 이슈 생성 폼:**

| 필드 | 유형 | 기존 JSON 키 | 설명 |
|------|------|-------------|------|
| 트래커 | 라디오 | `tracker_id` | 결함(1) / 새기능(2) / 지원(3) |
| 제목 | 텍스트 입력 | `subject` | — |
| 내용 | 텍스트 영역 | `description` | 마크다운 지원 |
| 담당자 | 드롭다운 | `assigned_to_id` | 팀원 목록 |
| 우선순위 | 드롭다운 | `priority_id` | 선택 |

**이벤트:**

| 이벤트 | 트리거 | 결과 |
|--------|--------|------|
| 이슈 생성 | 생성 버튼 | POST /issues.json 호출 |
| 상태 변경 | 상태 버튼 클릭 | PUT /issues/<id>.json (status_id 갱신) |
| 코멘트 추가 | 코멘트 전송 | PUT /issues/<id>.json (notes 필드) |
| 진척률 변경 | 슬라이더 조작 | PUT /issues/<id>.json (done_ratio 갱신) |
| 목록 새로고침 | 새로고침 버튼 or 자동 | GET /issues.json 재조회 |

---

### Screen-07: 설정 / 환경 화면

**목적:** 앱 전역 환경 설정. 에이전트 로그인 관리 및 project_state.yaml 편집.

**주요 컴포넌트:**

| 컴포넌트 | 유형 | 설명 | 기존 CLI 대응 |
|---------|------|------|--------------|
| Claude Code 로그인 상태 | 상태 표시 + 버튼 | Keychain/credentials 확인 | `validate_auth_and_network()` |
| Codex 로그인 상태 | 상태 표시 + 버튼 | `codex login status` | — |
| Gemini 로그인 상태 | 상태 표시 + 버튼 | 향후 확장 | — |
| API 연결 테스트 | 버튼 + 결과 표시 | Anthropic/OpenAI 엔드포인트 핑 | `check_api_reachable()` |
| project_state.yaml 에디터 | YAML 편집기 | business_type, current_mode, milestone, wbs_track | yaml_scalar_value 읽기 |
| 레드마인 API 키 관리 | 비밀번호 입력 | 역할별 API 키 (마스킹) | Shared persona §1 |
| 레드마인 서버 URL | 텍스트 입력 | `http://211.117.60.5:8080/` | Shared persona §1 |

**이벤트:**

| 이벤트 | 트리거 | 결과 |
|--------|--------|------|
| 재로그인 | 로그인 버튼 | OS 브라우저/터미널로 인증 흐름 실행 |
| API 연결 테스트 | 테스트 버튼 | 연결 상태 즉시 갱신 |
| project_state 저장 | 저장 버튼 | project_state.yaml 파일 갱신 |
| API 키 저장 | 저장 버튼 | OS 키체인 or 로컬 암호화 저장 |

---

## 4. agiteam.json 설정값 → GUI 표현 매핑 요약

| JSON 경로 | 현재 CLI 의미 | GUI 화면 / 컴포넌트 |
|-----------|--------------|---------------------|
| `project.name` | 프로젝트 식별자 | Screen-02 프로젝트 이름 필드 |
| `project.displayName` | 표시 이름 | Screen-02 표시 이름 필드 |
| `project.workspace.name` | workspace 이름 | Screen-02 + Screen-04 헤더 |
| `project.workspace.color` | workspace 색상 | Screen-02 색상 선택 + Screen-04 헤더 색상 |
| `persona.dir` | 페르소나 디렉토리 | Screen-02 페르소나 경로 |
| `persona.commonFile` | Shared 공통 페르소나 | Screen-02 Shared 파일 경로 |
| `team[].role` | 역할 ID | Screen-04 패널 헤더 + Screen-02 역할 카드 |
| `team[].name` | 담당자 이름 | Screen-04b 패널 헤더 |
| `team[].agent` | 에이전트 타입 | Screen-04b 에이전트 뱃지 + Screen-02 드롭다운 |
| `team[].command` | 실행 명령어 | Screen-02 고급 옵션 (접힘 처리) |
| `team[].layout` | pane 위치 | Screen-04 패널 배치 위치 자동 결정 |
| `pm.name` | PM 이름 | Screen-04a 패널 헤더 |
| `pm.startupFiles[]` | PM 세션 시작 파일 목록 | Screen-04a startupFiles 빠른 링크 + Screen-02 파일 목록 |
| `pm.startupMessage` | PM 첫 메시지 | Screen-02 PM 설정 편집 |
| `settings.readyTimeout` | READY 대기 타임아웃 | Screen-02 타이밍 설정 / Screen-03 타임아웃 표시 |
| `settings.postLaunchDelay` | 에이전트 실행 후 지연 | Screen-02 타이밍 설정 |
| `settings.readySignalTimeout` | READY 신호 최대 대기 | Screen-02 타이밍 설정 / Screen-03 타임아웃 바 |
| `settings.maxAutoSubmits` | 자동 Enter 최대 횟수 | Screen-02 타이밍 설정 / Screen-03 자동 제출 카운터 |

---

## 5. GUI 화면 목록 요약

| Screen ID | 화면명 | CLI 대응 | 우선순위 |
|-----------|--------|----------|---------|
| Screen-01 | 런처 / 시작 화면 | ./agiteam.sh 실행 | P1 |
| Screen-02 | 프로젝트 설정 (agiteam.json 에디터) | agiteam.json 직접 편집 | P1 |
| Screen-03 | 팀 부팅 진행 화면 | agiteam.sh main() 실행 로그 | P1 |
| Screen-04 | 팀 워크스페이스 (메인) | cmux 터미널 레이아웃 전체 | P1 |
| Screen-04a | PM 채팅 패널 | PM pane + cmux send/read | P1 |
| Screen-04b | 팀원 채팅 패널 ×6 | 팀원 pane + cmux send/read | P1 |
| Screen-05 | 산출물 뷰어/에디터 | 파일시스템 직접 접근 (신규) | P2 |
| Screen-06 | 레드마인 패널 | curl API 직접 호출 (신규) | P2 |
| Screen-07 | 설정/환경 화면 | 환경변수·키체인 직접 관리 | P2 |

---

*본 문서는 DeveloperFE가 agiteam.sh + agiteam.json을 기반으로 역추출한 GUI 화면 초안이다.*  
*상세 UI 설계(와이어프레임·컴포넌트 스펙)는 Designer 시안 및 PM 확정 후 DD120(화면설계서)으로 이관한다.*
