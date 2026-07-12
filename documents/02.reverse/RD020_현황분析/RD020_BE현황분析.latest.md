---
doc: RD020_BE현황분析
version: v0.1
last_updated: 2026-06-22
author: DeveloperBE
---

# RD020 BE 현황분析 — AgiTeamBuilder 코드 인벤토리 역추출

## 개정이력

| 버전 | 일자 | 작성자 | 내용 |
|------|------|--------|------|
| v0.1 | 2026-06-22 | DeveloperBE | 최초 작성 |

---

## 1. 현재 시스템 기능 전체 목록 (What It Does)

### 1.1 CLI 진입점 및 옵션 처리
- `--force-reuse` : 기존 팀 pane이 남아 있어도 경고 후 계속 진행
- `--cleanup-existing` : 기존 팀 pane을 닫고 새로 시작
- `--help / -h` : 사용법 출력

### 1.2 설정 로드 (agiteam.json + project_state.yaml)
- `agiteam.json` 파싱 (python3 내장 파서, jq 미사용)
- 프로젝트 메타 (이름·워크스페이스 이름·색상) 로드
- 페르소나 디렉토리 경로 로드
- 타이밍 파라미터 로드 (readyTimeout, postLaunchDelay, readySignalTimeout, maxAutoSubmits)
- PM 설정 로드 (이름·명령·시작 메시지·시작 파일 목록)
- 팀 구성 동적 로드 (team 배열 → ROLE_CONFIG 셸 배열)
- `project_state.yaml` 스칼라 읽기 (business_type, current_mode, milestone, wbs_track)

### 1.3 환경 검증
- 필수 명령 존재 확인: `cmux`, `python3`, `claude`, `codex` (사용된 agent 타입 기준 동적 확인)
- Anthropic API (`https://api.anthropic.com/`) 네트워크 도달 여부 확인 (curl)
- OpenAI API (`https://api.openai.com/`) 네트워크 도달 여부 확인 (curl)
- Claude Code 인증 상태 확인 (macOS: keychain / Linux·기타: `~/.claude/.credentials.json`)
- Codex 로그인 상태 확인 (`codex login status`)
- 프로젝트 작업 디렉토리 존재 확인
- 역할별 페르소나 파일 존재 확인 (`brain/<ROLE>/persona.md`)
- PM 페르소나 파일 존재 확인

### 1.4 워크스페이스 관리
- 현재 cmux workspace ID 식별 (`cmux identify`)
- 워크스페이스 이름 설정 (`cmux rename-workspace`)
- 워크스페이스 색상 설정 (`cmux workspace-action set-color`)
- 현재 PM surface 식별 (`cmux list-pane-surfaces`, `cmux identify`)
- 기존 팀 pane 존재 처리 (정리 / 재사용 / 오류)

### 1.5 레이아웃 생성
- 6-pane 그리드 자동 생성 (PM 기준 right → right → down 조합)
  ```
  PM          | middle_top    | right_top
              | middle_mid    | right_mid
              | middle_bottom | right_bottom
  ```
- 레이아웃 슬롯 이름 → surface ID 레지스트리 관리 (eval 기반 동적 변수)

### 1.6 페르소나 번들 생성
- `Shared/persona.md` + 역할별 `persona.md` 병합
- 비-PM 역할: 부팅 대기 규칙("READY: {role}" 출력 지시) 자동 append
- `mktemp` 임시 파일로 생성, 종료 시 자동 정리

### 1.7 팀원 부팅
- 에이전트 타입별 실행 명령 조립 (claude/codex/gemini 분기)
  - `claude`: `--system-prompt "$(cat <파일>)"`
  - `codex`: `"$(cat <파일>)"`
  - `gemini`: `"$(cat <파일>)"`
- `cmux send` + `cmux send-key Enter`로 에이전트 실행 명령 전송
- `sleep POST_LAUNCH_DELAY` 후 READY 지시문 전송
- READY 신호 대기 루프 (`cmux read-screen` → "READY: {role}" 문자열 감지)
- 부팅 중 상호작용 프롬프트 자동 Enter 제출 (최대 MAX_AUTO_SUBMITS 회)
- 탭 이름 설정: `{displayName}({roleLabel})` 형식

### 1.8 PM 실행
- PM 시스템 프롬프트 파일 동적 생성
  - PM 페르소나 번들 + 팀 구성 정보(role·surface·displayName) + surface 배정 표 + 프로젝트 상태(business_type/current_mode/milestone/wbs_track) + startupFiles 목록
- `exec PM_COMMAND --system-prompt "$(cat <파일>)" "$PM_STARTUP_MESSAGE"` 로 PM 에이전트 실행

### 1.9 진단 도구 (doctor.sh)
- 필수 도구 7종 존재 확인: `git`, `uv`, `python3`, `mempalace`, `cmux`, `claude`, `codex`
- 각 도구 버전 출력
- 누락 도구 설치 명령 안내 (brew/curl/npm)
- `--quiet` 옵션: 누락 도구만 출력

### 1.10 데이터 관리
- `entities.json`: 프로젝트·인물·토픽 목록 관리 (현재 projects: tos, healing-chatbot-frontend)
- 종료 시 임시 파일 일괄 정리 (trap EXIT)
- 오류 발생 시 생성된 pane surface 역순 정리 (trap EXIT)

---

## 2. agiteam.sh 주요 실행 흐름 (단계별 상세)

```
main()
 ├─ [1] load_config()
 ├─ [2] validate_environment()
 ├─ [3] validate_auth_and_network()
 ├─ [4] validate_workspace()
 ├─ [5] resolve_workspace_context()
 ├─ [6] spawn_team_members()
 ├─ [7] print_team_summary()
 └─ [8] launch_pm()     ← exec으로 프로세스 교체 (이후 코드 실행 없음)
```

### [1] load_config() (L.191–241)
```
CONFIG_FILE(agiteam.json) 존재 확인
  → json_value() (python3) 로 각 키 추출
    → WORKSPACE_DISPLAY_NAME, WORKSPACE_COLOR
    → PERSONA_DIR, COMMON_FILE
    → 타이밍 파라미터 4종
    → PM_NAME, PM_COMMAND, PM_STARTUP_MESSAGE
    → PM_STARTUP_FILES 배열
  → yaml_scalar_value() (grep+sed) 로 project_state.yaml 4종 스칼라 읽기
  → load_team_from_json()
      → json_array_len()으로 team 배열 길이 확인
      → 각 인덱스별 role/name/agent/command/layout 읽기
      → build_command_template()으로 에이전트 타입별 페르소나 인자 템플릿 생성
      → ROLE_CONFIG+=("role|name|agent|command_template|layout")
```

### [2] validate_environment() (L.448–461)
```
require_command("cmux")
require_command("python3")
ROLE_CONFIG[]에서 agent_type 중복 제거 후
  → require_command("<agent_type>")  ← claude, codex 등
```

### [3] validate_auth_and_network() (L.500–547)
```
team_or_pm_uses_agent("claude") → needs_claude 플래그
team_or_pm_uses_agent("codex")  → needs_codex 플래그

[claude 사용 시]
  curl -m 6 https://api.anthropic.com/ → 도달 불가 시 exit 1
  Darwin(macOS): security find-generic-password -l "Claude Code-credentials"
  기타:          ~/.claude/.credentials.json 존재 확인

[codex 사용 시]
  curl -m 6 https://api.openai.com/ → 도달 불가 시 exit 1
  codex login status 확인
```

### [4] validate_workspace() (L.425–441)
```
WORK_DIR 디렉토리 존재 확인
PERSONA_DIR 디렉토리 존재 확인
ROLE_CONFIG[] 각 role의 ${PERSONA_DIR}/${role}/persona.md 존재 확인
${PERSONA_DIR}/PM/persona.md 존재 확인
COMMON_FILE 존재 확인 (없으면 경고만)
```

### [5] resolve_workspace_context() (L.553–587)
```
cmux identify --no-caller → workspace:N 추출 → WS_ID
cmux rename-workspace → WORKSPACE_DISPLAY_NAME 설정
cmux workspace-action set-color → WORKSPACE_COLOR 설정
cmux list-pane-surfaces → PM surface 추출 → PM_SURFACE
set_role_surface("PM", PM_SURFACE)
handle_existing_team_workspace()
  → cmux list-pane-surfaces로 기존 pane 수 확인
  → CLEANUP_EXISTING=1: 기존 pane close
  → FORCE_REUSE=1: 경고 후 계속
  → 둘 다 0: fail()
```

### [6] spawn_team_members() (L.809–818)
```
create_layout_slots()
  → right split PM → middle_block, right_block
  → right split right_block → middle_top, right_top
  → down split middle_top → middle_mid → middle_bottom
  → down split right_top → right_mid → right_bottom
  → set_layout_surface("슬롯명", surface)

ROLE_CONFIG[] 순회 → spawn_member(role, name, agent, cmd_template, layout):
  1. get_layout_surface(layout_slot) → surface
  2. set_role_surface(role, surface)
  3. cmux rename-tab → "{name}({role_label})"
  4. create_persona_bundle(role)
      → mktemp → TEMP_FILES 등록
      → COMMON_FILE cat → 임시 파일
      → 역할 persona.md cat >> 임시 파일
      → 비-PM: 부팅 대기 규칙 heredoc >> 임시 파일
  5. build_launch_command(cmd_template, persona_file)
      → "__PERSONA_FILE__" → 실제 경로 치환
  6. send_and_submit(surface, launch_cmd)
      → cmux send → cmux send-key Enter
  7. sleep POST_LAUNCH_DELAY
  8. build_ready_instruction(role) → send_and_submit()
  9. wait_for_ready_signal(surface, role)
      → 루프: cmux read-screen --lines 80
        → "READY: {role}" 감지 시 return 0
        → should_auto_submit_enter() 감지 시 Enter 자동 제출
        → READY_SIGNAL_TIMEOUT 초과 시 return 1 → fail()
```

### [7] print_team_summary() (L.824–840)
```
PM surface 및 각 역할 surface 표 출력
작업 디렉토리, 설정 파일 경로 출력
```

### [8] launch_pm() (L.912–926)
```
cmux rename-tab PM_SURFACE → "{PM_NAME}(PM)"
build_pm_system_file()
  → create_persona_bundle("PM") → pm_persona_file
  → mktemp → pm_system_file
  → pm_persona_file cat > pm_system_file
  → 팀 구성 정보 append:
      팀 구성: {agent_label}{role} ({surface}): {role} ({name})
      별칭 매핑: {name} → {role} ({surface})
      팀원별 배정 surface
      작업 디렉토리
      project_state 값 (business_type/current_mode/milestone/wbs_track)
      startupFiles 목록 (인덱스+절대경로)
exec PM_COMMAND \
  --system-prompt "$(cat pm_system_file)" \
  "$PM_STARTUP_MESSAGE"
```

---

## 3. Windows 이식 불가 요소

### 3.1 Bash 전용 문법

| 구문 | 위치(행) | 대체 방법 (크로스플랫폼) |
|------|---------|------------------------|
| `#!/bin/bash` 셔뱅 | L.1 | Windows에 Bash 없음. PowerShell/Python 런처로 교체 |
| `set -u` (미정의 변수 오류) | L.8 | Python strict 모드 또는 타입 힌트로 대응 |
| `[[ "$seen" != *" ${agent} "* ]]` 더블 브라켓 | L.456 | Python `in` 연산자 |
| `ROLE_CONFIG+=("...")` 배열 append | L.265 | Python list.append() |
| `for ((i=0; i<count; i++))` C 스타일 for | L.253 | Python range() |
| `IFS='|' read -r ... <<< "$line"` herestring | L.815 | Python str.split('|') |
| `eval "ROLE_SURFACE_${role}=..."` 동적 변수 | L.298–329 | Python dict |
| `< <(json_string_array ...)` 프로세스 치환 | L.223 | Python 직접 파싱 |
| `trap "..." EXIT` 트랩 | L.389 | Python atexit / try-finally |
| `mktemp -t "prefix"` | L.598 | Python tempfile.mkstemp() |
| heredoc `<<'EOF'` | L.614 | Python 삼중 따옴표 문자열 |
| `exec PM_COMMAND ...` 프로세스 교체 | L.923 | Python os.execvp() / subprocess |

### 3.2 macOS 전용 명령

| 명령 | 위치(행) | Windows 대체 |
|------|---------|-------------|
| `security find-generic-password -l "Claude Code-credentials" -w` | L.527 | Windows Credential Manager API (`wincred` 또는 `keyring` 라이브러리) |
| `uname -s` → `Darwin` 분기 | L.526 | `platform.system()` == `"Darwin"/"Windows"/"Linux"` |
| `brew install` 설치 안내 (doctor.sh) | L.54,55 | winget/choco/직접 다운로드 안내로 분기 |
| `xcode-select --install` (doctor.sh) | L.41 | Windows 무관, 제거 |

### 3.3 cmux 의존 구간

> cmux는 macOS 전용 터미널 멀티플렉서 GUI 앱 (`brew install --cask cmux`). Windows 미지원.  
> GUI 앱 전환 시 아래 모든 cmux 호출이 **내부 IPC/API**로 대체되어야 한다.

| cmux 명령 | 호출 위치(행) | 역할 |
|-----------|------------|------|
| `cmux identify [--no-caller]` | L.556–558 | 현재 workspace/surface ID 식별 |
| `cmux rename-workspace` | L.563 | 워크스페이스 표시 이름 설정 |
| `cmux workspace-action set-color` | L.564 | 워크스페이스 색상 설정 |
| `cmux list-pane-surfaces --workspace` | L.399, 571 | workspace 내 surface 목록 조회 |
| `cmux new-split <direction> --surface` | L.641 | 창 분할 (pane 생성) |
| `cmux close-pane --surface` | L.385 | pane 닫기 |
| `cmux rename-tab --surface` | L.784, 919 | 탭 라벨 설정 |
| `cmux send --surface` | L.686 | 텍스트 입력창에 문자열 전송 |
| `cmux send-key --surface Enter` | L.687 | 입력창에 키 전송 (제출) |
| `cmux read-screen --surface --lines` | L.372, 727 | 현재 화면 텍스트 덤프 |

### 3.4 기타 Unix 전용 요소

| 요소 | 내용 |
|------|------|
| `~/.claude/.credentials.json` 경로 | Windows는 `%APPDATA%\Claude` 등 다른 경로 |
| `curl` 명령 | Windows 10 1803 이후 내장되어 있으나 옵션 차이 존재 |
| `/tmp` 경로 | Windows는 `%TEMP%` |
| `python3` 명령 | Windows는 `python` 또는 `py` |

---

## 4. 데이터 흐름: agiteam.json → agiteam.sh → 팀원 pane

### 4.1 agiteam.json 스키마 구조

```json
{
  "project": {
    "name": "프로젝트 식별자",
    "displayName": "표시 이름",
    "workspace": { "name": "워크스페이스명", "color": "색상코드" }
  },
  "persona": {
    "dir": "brain",                        ← 페르소나 루트 디렉토리
    "commonFile": "brain/Shared/persona.md" ← Shared 공용 파일
  },
  "team": [
    {
      "role": "DeveloperBE",               ← 역할 식별자 (persona.md 디렉토리명과 일치)
      "name": "박개발",                    ← 표시 이름
      "agent": "claude",                   ← 에이전트 타입 (claude|codex|gemini)
      "command": "claude --dangerously-skip-permissions",  ← 기본 실행 명령
      "layout": "middle_top"               ← 레이아웃 슬롯 배정
    },
    ...
  ],
  "pm": {
    "name": "박피엠",
    "agent": "claude",
    "command": "claude --dangerously-skip-permissions",
    "startupFiles": ["brain/PM/persona.md", "..."],  ← 세션 시작 시 읽을 파일
    "startupMessage": "..."                ← PM에게 전달할 첫 메시지
  },
  "settings": {
    "readyTimeout": 30,
    "postLaunchDelay": 3,
    "readySignalTimeout": 60,
    "maxAutoSubmits": 5
  }
}
```

### 4.2 project_state.yaml 스키마

```yaml
business_type: "Reverse"           ← 사업 유형
current_mode: project               ← 현재 모드 (project|operation)
milestone: M0                       ← 현재 마일스톤 코드
wbs_track: B                        ← WBS 트랙 (A|B)
milestones:                         ← 세부 마일스톤 목록 (구조화, yaml_scalar_value 미대상)
  - code: B1
    status: 대기
    evidence: ""
```

### 4.3 데이터 변환 흐름

```
[agiteam.json]
   team[i].role/name/agent/command/layout
         │
         ▼ load_team_from_json()
   ROLE_CONFIG[] = "role|name|agent|command_template|layout"
   (command_template: __PERSONA_FILE__ 플레이스홀더 포함)
         │
         ▼ spawn_member()
   ┌────────────────────────────────────────────────────┐
   │ create_persona_bundle(role)                         │
   │   → tmpfile = Shared/persona.md                     │
   │             + brain/{role}/persona.md               │
   │             + 부팅 대기 규칙 ("READY: {role}")       │
   │ build_launch_command(cmd_template, tmpfile)          │
   │   → "claude --dangerously-skip-permissions          │
   │       --system-prompt "$(cat /tmp/agiteam_...)"     │
   └────────────────────────────────────────────────────┘
         │
         ▼ send_and_submit(surface, launch_cmd)
   [cmux pane] ←── 텍스트 전송: 에이전트 실행 명령
         │
         ▼ wait_for_ready_signal()
   [cmux pane] ──→ cmux read-screen → "READY: {role}" 감지
         │
         ▼ (모든 팀원 완료 후)

[agiteam.json pm.* + project_state.yaml 4종 스칼라]
         │
         ▼ build_pm_system_file()
   ┌────────────────────────────────────────────────────┐
   │ PM 시스템 프롬프트 파일:                             │
   │   - Shared/persona.md                              │
   │   - brain/PM/persona.md                            │
   │   - 팀 구성 정보 (role, surface, name 매핑)          │
   │   - project_state 4종 (business_type/mode/ms/wbs)   │
   │   - startupFiles 목록 (절대경로)                     │
   └────────────────────────────────────────────────────┘
         │
         ▼ exec PM_COMMAND --system-prompt "$(cat ...)" "$PM_STARTUP_MESSAGE"
   [PM pane] ←── 시스템 프롬프트 주입 + 시작 메시지
```

### 4.4 역할 레지스트리 (런타임 상태)

```
set_role_surface(role, surface)    → ROLE_SURFACE_{role} 변수
get_role_surface(role)             → cmux send 대상 조회

set_role_display_name(role, name)  → ROLE_DISPLAY_NAME_{role} 변수

set_layout_surface(slot, surface)  → LAYOUT_SURFACE_{slot} 변수
get_layout_surface(slot)           → spawn_member에서 pane 배정 시 조회
```

---

## 5. GUI 대체 시 핵심 인터페이스 포인트

### 5.1 대체 인터페이스 맵

| cmux 명령 | GUI 대체 방식 |
|-----------|-------------|
| `cmux identify` | 앱 내부 세션 컨텍스트 — 항상 알고 있으므로 API 불필요 |
| `cmux rename-workspace` | GUI 창 제목 바 또는 탭 그룹 레이블 설정 |
| `cmux workspace-action set-color` | GUI 테마/색상 API |
| `cmux list-pane-surfaces` | 앱 내부 세션 목록 상태 조회 |
| `cmux new-split <direction>` | GUI 분할 창 생성 (SplitView/Pane 컴포넌트 생성) |
| `cmux close-pane` | GUI 패널 닫기 / 세션 종료 |
| `cmux rename-tab` | GUI 탭/패널 라벨 설정 API |
| **`cmux send --surface` + `send-key Enter`** | **에이전트 프로세스 stdin에 직접 write (IPC Pipe)** |
| **`cmux read-screen --surface --lines`** | **에이전트 프로세스 stdout 구독 (버퍼 읽기)** |

### 5.2 핵심 IPC 교체 포인트 (가장 중요)

**현재 구조 (cmux 경유):**
```
agiteam.sh → cmux send → 터미널 pane UI → 에이전트 stdin
에이전트 stdout → 터미널 pane UI → cmux read-screen → agiteam.sh
```

**GUI 앱 대체 구조 (직접 IPC):**
```
GUI 오케스트레이터 → subprocess stdin pipe → 에이전트 프로세스
에이전트 프로세스 stdout pipe → GUI 오케스트레이터 (버퍼/스트림 읽기)
                              → GUI 패널 화면 렌더링 (동시)
```

**구체적 대체 API (Python 예시):**
```python
# 현재: cmux send + send-key Enter
# 대체:
agent_proc.stdin.write(f"{command}\n".encode())
agent_proc.stdin.flush()

# 현재: cmux read-screen --surface --lines 80
# 대체:
output_buffer = agent_proc.stdout.read_available()  # 비동기
# 또는 stdout 스트림을 GUI 위젯과 공유 버퍼로 관리
```

### 5.3 READY 신호 감지 대체

**현재 구조:**
```
cmux read-screen --lines 80 → 전체 화면 텍스트 grep → "READY: {role}" 감지
```

**GUI 대체:**
```
에이전트 stdout 스트림 구독 → 라인 단위 파싱 → "READY: {role}" 감지
(화면 덤프 방식 → 스트림 이벤트 방식으로 더 효율적)
```

### 5.4 페르소나 번들 생성 대체

**현재 구조:**
- 파일 3개 concat → `mktemp` 임시 파일 → 에이전트 실행 시 `--system-prompt "$(cat <파일>)"` 전달

**GUI 대체:**
- 파일 내용을 메모리 문자열로 concat
- 에이전트 프로세스 실행 시 환경변수 또는 stdin 첫 메시지로 직접 주입
- 임시 파일 불필요 (보안 개선 효과)

### 5.5 레이아웃 슬롯 대체

**현재 구조:**
```
middle_top / middle_mid / middle_bottom / right_top / right_mid / right_bottom
→ cmux new-split으로 런타임에 동적 생성
→ eval 기반 셸 변수에 surface ID 저장
```

**GUI 대체:**
```
고정 그리드 레이아웃 (2열 × 3행 + 1 PM 패널)
각 셀은 컴포넌트 ID로 미리 정의
→ 역할 배정 시 컴포넌트 ID 조회
→ dict 자료구조로 관리 (eval 불필요)
```

### 5.6 부팅 흐름 대체 설계 요약

```
GUI 앱 시작
  │
  ├─ agiteam.json, project_state.yaml 로드 (Python/Native)
  ├─ 환경·인증 검증 (OS 네이티브 API 사용)
  ├─ GUI 레이아웃 렌더링 (7 패널: PM + 팀원 6)
  │
  ├─ 팀원별 subprocess 실행:
  │    subprocess.Popen(["claude", "--dangerously-skip-permissions"],
  │                     stdin=PIPE, stdout=PIPE, stderr=PIPE)
  │    → system_prompt 첫 메시지로 전송
  │    → stdout 스트림 → GUI 패널 렌더링 + "READY" 감지 루프
  │
  └─ 모든 팀원 READY 확인 후:
       PM subprocess 실행 (동일 패턴)
       PM 패널에 startup_message 전송
```

---

## 6. 참조 파일 목록

| 파일 | 크기 | 역할 |
|------|------|------|
| `system/patricks/agiteam.sh` | 974줄 | 핵심 부팅 엔진 |
| `system/patricks/agiteam.json` | 77줄 | 팀 구성·PM 설정 |
| `system/patricks/project_state.yaml` | 15줄 | 프로젝트 상태 (모드·마일스톤) |
| `system/patricks/entities.json` | 7줄 | 프로젝트·인물·토픽 목록 |
| `system/patricks/doctor.sh` | 137줄 | 호스트 환경 진단 도구 |
