#!/bin/bash

# =============================================================================
# AgiTeam - AI 에이전트 팀 부팅 엔진
# 설정 파일(agiteam.json) 기반 범용 멀티 프로젝트 팀 프로비저닝
# =============================================================================

set -u

# ---------------------------------------------------------------------------
# 실행 옵션
# ---------------------------------------------------------------------------
FORCE_REUSE=0
CLEANUP_EXISTING=0
POSITIONAL_ARGS=()

while [ "$#" -gt 0 ]; do
  case "$1" in
    --force-reuse)
      FORCE_REUSE=1
      shift
      ;;
    --cleanup-existing)
      CLEANUP_EXISTING=1
      shift
      ;;
    --help|-h)
      cat <<EOF
AgiTeam - AI 에이전트 팀 부팅 엔진

사용법:
  ./agiteam.sh [작업디렉토리]
  ./agiteam.sh --force-reuse [작업디렉토리]
  ./agiteam.sh --cleanup-existing [작업디렉토리]

설정:
  작업디렉토리에 agiteam.json 파일이 있어야 합니다.
  없으면 기본값으로 팀을 구성합니다.

옵션:
  --force-reuse       기존 팀 pane이 있어도 경고만 하고 계속 진행
  --cleanup-existing  기존 팀 pane을 정리한 뒤 새로 시작
EOF
      exit 0
      ;;
    *)
      POSITIONAL_ARGS+=("$1")
      shift
      ;;
  esac
done

# ---------------------------------------------------------------------------
# 전역 설정 (agiteam.json에서 오버라이드됨)
# ---------------------------------------------------------------------------
WORK_DIR="${POSITIONAL_ARGS[0]:-$(pwd)}"
CONFIG_FILE="${WORK_DIR}/agiteam.json"
PROJECT_STATE_FILE="${WORK_DIR}/project_state.yaml"

# 기본값 (agiteam.json이 없을 때 폴백)
PERSONA_DIR="${WORK_DIR}/brain"
COMMON_FILE="${WORK_DIR}/brain/Shared/persona.md"
WORKSPACE_DISPLAY_NAME="AGI개발팀"
WORKSPACE_COLOR="Indigo"
BUSINESS_TYPE="Unknown"
CURRENT_MODE="unknown"
MILESTONE="unknown"
WBS_TRACK="unknown"
READY_TIMEOUT=30
POST_LAUNCH_DELAY=3
READY_SIGNAL_TIMEOUT=60
MAX_AUTO_SUBMITS=5
PM_NAME="박피엠"
PM_COMMAND="claude --dangerously-skip-permissions"
PM_STARTUP_MESSAGE="팀 워크스페이스 셋업이 완료되었습니다."

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

TEMP_FILES=()
CREATED_SURFACES=()

# 팀 구성 (agiteam.json에서 동적 로드)
ROLE_CONFIG=()

# PM 시작 파일 목록
PM_STARTUP_FILES=()

# ---------------------------------------------------------------------------
# 공통 출력 함수
# ---------------------------------------------------------------------------
log() { echo "$1"; }
warn() { echo "⚠️  $1"; }
fail() { echo "오류: $1" >&2; exit 1; }
debug() { echo "  [debug] $1"; }


# ---------------------------------------------------------------------------
# JSON 값 추출 유틸리티 (python3 기반, jq 불필요)
# ---------------------------------------------------------------------------
json_value() {
  local file="$1"
  local jpath="$2"
  python3 -c "
import json, sys
with open(sys.argv[1], encoding='utf-8') as f:
    d = json.load(f)
for k in sys.argv[2].split('.'):
    if isinstance(d, list):
        d = d[int(k)]
    else:
        d = d[k]
if isinstance(d, (dict, list)):
    print(json.dumps(d, ensure_ascii=False))
else:
    print(d)
" "$file" "$jpath" 2>/dev/null
}

json_array_len() {
  local file="$1"
  local jpath="$2"
  python3 -c "
import json, sys
with open(sys.argv[1], encoding='utf-8') as f:
    d = json.load(f)
for k in sys.argv[2].split('.'):
    if isinstance(d, list):
        d = d[int(k)]
    else:
        d = d[k]
print(len(d) if isinstance(d, list) else 0)
" "$file" "$jpath" 2>/dev/null
}

json_string_array() {
  local file="$1"
  local jpath="$2"
  python3 -c "
import json, sys
with open(sys.argv[1], encoding='utf-8') as f:
    d = json.load(f)
for k in sys.argv[2].split('.'):
    if isinstance(d, list):
        d = d[int(k)]
    else:
        d = d[k]
if isinstance(d, list):
    for item in d:
        print(item)
" "$file" "$jpath" 2>/dev/null
}


# ---------------------------------------------------------------------------
# project_state.yaml 값 추출 유틸리티
# 단순 key: value 스칼라만 읽는다. startupFiles에는 등록하지 않는다.
# ---------------------------------------------------------------------------
yaml_scalar_value() {
  local file="$1"
  local key="$2"
  local value=""

  [ -f "$file" ] || return 1

  # 최상위 스칼라만 읽음(동일 키 중첩 금지)
  value=$(grep -E "^${key}:" "$file" 2>/dev/null | head -1 | sed -E "s/^${key}:[[:space:]]*//")
  value=$(printf '%s' "$value" | sed -E 's/[[:space:]]+#.*$//; s/^"//; s/"$//; s/^'\''//; s/'\''$//')
  [ -n "$value" ] || return 1
  printf '%s\n' "$value"
}


display_current_mode() {
  case "$1" in
    project)
      echo "프로젝트 모드"
      ;;
    operation)
      echo "운영 모드"
      ;;
    *)
      echo "$1"
      ;;
  esac
}


# ---------------------------------------------------------------------------
# agiteam.json 로드
# ---------------------------------------------------------------------------
load_config() {
  if [ ! -f "$CONFIG_FILE" ]; then
    warn "agiteam.json이 없습니다: $CONFIG_FILE"
    warn "기본 설정으로 진행합니다."
    return 0
  fi

  log "📋 설정 파일 로드: $CONFIG_FILE"

  # 프로젝트 설정
  local val=""

  val=$(json_value "$CONFIG_FILE" "project.workspace.name") && [ -n "$val" ] && WORKSPACE_DISPLAY_NAME="$val"
  val=$(json_value "$CONFIG_FILE" "project.workspace.color") && [ -n "$val" ] && WORKSPACE_COLOR="$val"

  # 페르소나 경로
  val=$(json_value "$CONFIG_FILE" "persona.dir") && [ -n "$val" ] && PERSONA_DIR="${WORK_DIR}/${val}"
  val=$(json_value "$CONFIG_FILE" "persona.commonFile") && [ -n "$val" ] && COMMON_FILE="${WORK_DIR}/${val}"

  # 타이밍 설정
  val=$(json_value "$CONFIG_FILE" "settings.readyTimeout") && [ -n "$val" ] && READY_TIMEOUT="$val"
  val=$(json_value "$CONFIG_FILE" "settings.postLaunchDelay") && [ -n "$val" ] && POST_LAUNCH_DELAY="$val"
  val=$(json_value "$CONFIG_FILE" "settings.readySignalTimeout") && [ -n "$val" ] && READY_SIGNAL_TIMEOUT="$val"
  val=$(json_value "$CONFIG_FILE" "settings.maxAutoSubmits") && [ -n "$val" ] && MAX_AUTO_SUBMITS="$val"

  # PM 설정
  val=$(json_value "$CONFIG_FILE" "pm.name") && [ -n "$val" ] && PM_NAME="$val"
  val=$(json_value "$CONFIG_FILE" "pm.command") && [ -n "$val" ] && PM_COMMAND="$val"
  val=$(json_value "$CONFIG_FILE" "pm.startupMessage") && [ -n "$val" ] && PM_STARTUP_MESSAGE="$val"

  # PM 시작 파일 목록
  PM_STARTUP_FILES=()
  while IFS= read -r line; do
    [ -n "$line" ] && PM_STARTUP_FILES+=("$line")
  done < <(json_string_array "$CONFIG_FILE" "pm.startupFiles")

  # 프로젝트 상태 메타. PM startupFiles에는 등록하지 않고 시스템 프롬프트 한 줄로만 주입한다.
  val=$(yaml_scalar_value "$PROJECT_STATE_FILE" "business_type") && [ -n "$val" ] && BUSINESS_TYPE="$val"
  val=$(yaml_scalar_value "$PROJECT_STATE_FILE" "current_mode") && [ -n "$val" ] && CURRENT_MODE="$val"
  val=$(yaml_scalar_value "$PROJECT_STATE_FILE" "milestone") && [ -n "$val" ] && MILESTONE="$val"
  val=$(yaml_scalar_value "$PROJECT_STATE_FILE" "wbs_track") && [ -n "$val" ] && WBS_TRACK="$val"

  # 팀 구성 로드
  load_team_from_json

  log "  ✅ 프로젝트: $(json_value "$CONFIG_FILE" "project.name") ($(json_value "$CONFIG_FILE" "project.displayName"))"
  log "  ✅ 사업 유형: ${BUSINESS_TYPE}"
  log "  ✅ 현재 모드: $(display_current_mode "$CURRENT_MODE")"
  log "  ✅ 팀원: ${#ROLE_CONFIG[@]}명"
  echo ""
}

load_team_from_json() {
  local count
  count=$(json_array_len "$CONFIG_FILE" "team")

  if [ "$count" -eq 0 ]; then
    warn "팀 구성이 비어 있습니다."
    return 0
  fi

  ROLE_CONFIG=()
  for ((i = 0; i < count; i++)); do
    local role name agent command layout
    role=$(json_value "$CONFIG_FILE" "team.${i}.role")
    name=$(json_value "$CONFIG_FILE" "team.${i}.name")
    agent=$(json_value "$CONFIG_FILE" "team.${i}.agent")
    command=$(json_value "$CONFIG_FILE" "team.${i}.command")
    layout=$(json_value "$CONFIG_FILE" "team.${i}.layout")

    # agent 타입별 페르소나 인자 템플릿 자동 생성
    local command_template
    command_template=$(build_command_template "$agent" "$command")

    ROLE_CONFIG+=("${role}|${name}|${agent}|${command_template}|${layout}")
  done
}

# ---------------------------------------------------------------------------
# 에이전트 타입별 명령어 템플릿 생성
# JSON에는 기본 명령만 넣고, 페르소나 인자는 엔진이 자동 부착
# ---------------------------------------------------------------------------
build_command_template() {
  local agent_type="$1"
  local base_command="$2"

  case "$agent_type" in
    claude)
      echo "${base_command}"' --system-prompt "$(cat "__PERSONA_FILE__")"'
      ;;
    codex)
      echo "${base_command}"' "$(cat "__PERSONA_FILE__")"'
      ;;
    gemini)
      echo "${base_command}"' "$(cat "__PERSONA_FILE__")"'
      ;;
    *)
      warn "알 수 없는 에이전트 타입: ${agent_type}, 기본 명령 사용"
      echo "${base_command}"
      ;;
  esac
}


# ---------------------------------------------------------------------------
# 역할 레지스트리 (Bash 3 호환)
# ---------------------------------------------------------------------------
set_role_surface() {
  local role="$1"; local surface="$2"
  eval "ROLE_SURFACE_${role}=\"\$surface\""
}

get_role_surface() {
  local role="$1"; local value=""
  eval "value=\"\${ROLE_SURFACE_${role}:-}\""
  echo "$value"
}

set_role_display_name() {
  local role="$1"; local display_name="$2"
  eval "ROLE_DISPLAY_NAME_${role}=\"\$display_name\""
}

get_role_display_name() {
  local role="$1"; local value=""
  eval "value=\"\${ROLE_DISPLAY_NAME_${role}:-}\""
  echo "$value"
}

set_layout_surface() {
  local slot="$1"; local surface="$2"
  eval "LAYOUT_SURFACE_${slot}=\"\$surface\""
}

get_layout_surface() {
  local slot="$1"; local value=""
  eval "value=\"\${LAYOUT_SURFACE_${slot}:-}\""
  echo "$value"
}


# ---------------------------------------------------------------------------
# 종료 시 임시 파일 정리
# ---------------------------------------------------------------------------
cleanup_temp_files() {
  local file
  for file in "${TEMP_FILES[@]:-}"; do
    [ -n "$file" ] && [ -f "$file" ] && rm -f "$file"
  done
}

# ---------------------------------------------------------------------------
# 필수 명령어 확인
# ---------------------------------------------------------------------------
require_command() {
  local cmd="$1"
  command -v "$cmd" >/dev/null 2>&1 || fail "필수 명령어가 없습니다: $cmd"
}


# ---------------------------------------------------------------------------
# cmux 명령 실행 래퍼
# ---------------------------------------------------------------------------
run_cmux() {
  local output=""
  output=$(cmux "$@" 2>&1)
  local status=$?
  if [ $status -ne 0 ]; then
    fail "cmux 명령 실패: cmux $* / $output"
  fi
  echo "$output"
}


# ---------------------------------------------------------------------------
# surface 화면 덤프 (디버깅용)
# ---------------------------------------------------------------------------
dump_surface_screen() {
  local surface="$1"; local label="$2"
  [ -z "$surface" ] && return 0
  echo "----- ${label} 최근 화면 (${surface}) -----" >&2
  cmux read-screen --surface "$surface" --lines 80 2>/dev/null >&2 || true
  echo "----------------------------------------" >&2
}


# ---------------------------------------------------------------------------
# 생성한 surface 정리
# ---------------------------------------------------------------------------
cleanup_created_surfaces() {
  local idx surface
  for (( idx=${#CREATED_SURFACES[@]}-1; idx>=0; idx-- )); do
    surface="${CREATED_SURFACES[$idx]}"
    [ -n "$surface" ] || continue
    cmux close-pane --surface "$surface" >/dev/null 2>&1 || true
  done
}

trap "cleanup_temp_files; cleanup_created_surfaces" EXIT


# ---------------------------------------------------------------------------
# 기존 팀 워크스페이스 처리
# ---------------------------------------------------------------------------
handle_existing_team_workspace() {
  local surfaces_output="" existing_count=0
  [ -n "${WS_ID:-}" ] || return 0

  surfaces_output="$(cmux list-pane-surfaces --workspace "$WS_ID" 2>/dev/null || true)"
  existing_count=$(echo "$surfaces_output" | grep -c 'surface:' || true)

  if [ "$existing_count" -le 1 ]; then return 0; fi

  if [ "$CLEANUP_EXISTING" -eq 1 ]; then
    warn "기존 팀 pane ${existing_count}개를 정리합니다."
    echo "$surfaces_output" | grep -o 'surface:[0-9]*' | while read -r surface; do
      [ "$surface" = "$PM_SURFACE" ] && continue
      cmux close-pane --surface "$surface" >/dev/null 2>&1 || true
    done
    return 0
  fi

  if [ "$FORCE_REUSE" -eq 1 ]; then
    warn "기존 팀 pane이 남아 있지만 --force-reuse로 계속 진행합니다."
    return 0
  fi

  fail "이미 팀 pane이 남아 있습니다. --cleanup-existing 또는 --force-reuse를 사용하세요."
}


# ---------------------------------------------------------------------------
# 프로젝트 구조 검증
# ---------------------------------------------------------------------------
validate_workspace() {
  [ -d "$WORK_DIR" ] || fail "작업 디렉토리가 존재하지 않습니다: $WORK_DIR"
  [ -d "$PERSONA_DIR" ] || fail "페르소나 디렉토리가 존재하지 않습니다: $PERSONA_DIR"

  # 팀 구성에서 역할 목록 추출하여 페르소나 파일 검증
  local line role
  for line in "${ROLE_CONFIG[@]}"; do
    IFS='|' read -r role _ _ _ _ <<< "$line"
    [ -f "${PERSONA_DIR}/${role}/persona.md" ] || fail "페르소나 파일이 없습니다: ${PERSONA_DIR}/${role}/persona.md"
  done

  # PM 페르소나 필수
  [ -f "${PERSONA_DIR}/PM/persona.md" ] || fail "PM 페르소나 파일이 없습니다: ${PERSONA_DIR}/PM/persona.md"

  if [ ! -f "$COMMON_FILE" ]; then
    warn "Shared 공용 페르소나 파일이 없습니다: $COMMON_FILE"
  fi
}


# ---------------------------------------------------------------------------
# 실행 환경 검증
# ---------------------------------------------------------------------------
validate_environment() {
  local line seen=" cmux " agent_type

  require_command "cmux"
  require_command "python3"

  for line in "${ROLE_CONFIG[@]}"; do
    IFS='|' read -r _ _ agent_type _ _ <<< "$line"
    if [[ "$seen" != *" ${agent_type} "* ]]; then
      require_command "$agent_type"
      seen="${seen}${agent_type} "
    fi
  done
}


# ---------------------------------------------------------------------------
# 인증/네트워크 사전 진단
# ---------------------------------------------------------------------------
command_uses_agent() {
  local command_line="$1"; local agent_type="$2"
  local command_name="${command_line%% *}"
  command_name="${command_name##*/}"
  [ "$command_name" = "$agent_type" ]
}

team_or_pm_uses_agent() {
  local target_agent="$1"
  local line agent_type

  if command_uses_agent "$PM_COMMAND" "$target_agent"; then
    return 0
  fi

  for line in "${ROLE_CONFIG[@]}"; do
    IFS='|' read -r _ _ agent_type _ _ <<< "$line"
    [ "$agent_type" = "$target_agent" ] && return 0
  done

  return 1
}

check_api_reachable() {
  local url="$1"
  local http_code="" curl_status=0

  http_code=$(curl -sS -m 6 -o /dev/null -w "%{http_code}" "$url" 2>/dev/null)
  curl_status=$?

  [ "$curl_status" -eq 0 ] && [ "$http_code" != "000" ]
}

validate_auth_and_network() {
  local needs_claude=1 needs_codex=1
  local auth_failed=0

  team_or_pm_uses_agent "claude" && needs_claude=0
  team_or_pm_uses_agent "codex" && needs_codex=0

  [ "$needs_claude" -ne 0 ] && [ "$needs_codex" -ne 0 ] && return 0

  require_command "curl"

  if [ "$needs_claude" -eq 0 ]; then
    if ! check_api_reachable "https://api.anthropic.com/"; then
      echo "❌ 인터넷에 연결할 수 없습니다. 네트워크 연결을 확인한 뒤 다시 실행하세요." >&2
      exit 1
    fi
  fi

  if [ "$needs_codex" -eq 0 ]; then
    if ! check_api_reachable "https://api.openai.com/"; then
      echo "❌ 인터넷에 연결할 수 없습니다. 네트워크 연결을 확인한 뒤 다시 실행하세요." >&2
      exit 1
    fi
  fi

  if [ "$needs_claude" -eq 0 ]; then
    if [ "$(uname -s)" = "Darwin" ]; then
      if ! security find-generic-password -l "Claude Code-credentials" -w >/dev/null 2>&1; then
        echo "❌ Claude Code 로그인이 필요합니다. 터미널에서 claude 를 실행해 로그인한 뒤 다시 실행하세요." >&2
        auth_failed=1
      fi
    elif [ ! -f "${HOME}/.claude/.credentials.json" ]; then
      echo "❌ Claude Code 로그인이 필요합니다. 터미널에서 claude 를 실행해 로그인한 뒤 다시 실행하세요." >&2
      auth_failed=1
    fi
  fi

  if [ "$needs_codex" -eq 0 ]; then
    if ! codex login status >/dev/null 2>&1; then
      echo "❌ Codex 로그인이 필요합니다. 터미널에서 codex login 을 실행한 뒤 다시 실행하세요." >&2
      auth_failed=1
    fi
  fi

  [ "$auth_failed" -eq 0 ] || exit 1

  log "✅ 인증·네트워크 확인됨"
}


# ---------------------------------------------------------------------------
# workspace / PM surface 식별
# ---------------------------------------------------------------------------
resolve_workspace_context() {
  local current_ws="" pm_surface="" identify_output=""

  current_ws=$(cmux identify --no-caller 2>&1 | grep -o 'workspace:[0-9]*' | head -1 || true)
  if [ -z "$current_ws" ]; then
    current_ws=$(cmux identify 2>&1 | grep -o 'workspace:[0-9]*' | head -1 || true)
  fi

  if [ -n "$current_ws" ]; then
    WS_ID="$current_ws"
    cmux rename-workspace --workspace "$WS_ID" "$WORKSPACE_DISPLAY_NAME" >/dev/null 2>&1 || true
    cmux workspace-action --workspace "$WS_ID" --action set-color --color "$WORKSPACE_COLOR" >/dev/null 2>&1 || true
  else
    WS_ID=""
    warn "현재 workspace를 식별하지 못했습니다."
  fi

  if [ -n "$WS_ID" ]; then
    pm_surface=$(cmux list-pane-surfaces --workspace "$WS_ID" 2>&1 | grep -o 'surface:[0-9]*' | head -1 || true)
  fi
  if [ -z "$pm_surface" ]; then
    pm_surface=$(cmux identify --no-caller 2>&1 | grep -o 'surface:[0-9]*' | head -1 || true)
  fi
  if [ -z "$pm_surface" ]; then
    pm_surface=$(cmux identify 2>&1 | grep -o 'surface:[0-9]*' | head -1 || true)
  fi

  [ -n "$pm_surface" ] || fail "PM 기준 surface를 식별할 수 없습니다."

  PM_SURFACE="$pm_surface"
  set_role_surface "PM" "$PM_SURFACE"
  set_role_display_name "PM" "$PM_NAME"

  handle_existing_team_workspace
}


# ---------------------------------------------------------------------------
# 페르소나 번들 생성
# ---------------------------------------------------------------------------
create_persona_bundle() {
  local role="$1"
  local role_file="${PERSONA_DIR}/${role}/persona.md"
  local tmp_file

  tmp_file=$(mktemp -t "agiteam_persona_${role}") || fail "임시 파일 생성 실패: $role"
  [ -n "$tmp_file" ] || fail "임시 파일 경로가 비어 있습니다: $role"
  TEMP_FILES+=("$tmp_file")

  if [ -f "$COMMON_FILE" ]; then
    cat "$COMMON_FILE" > "$tmp_file"
    printf '\n---\n\n' >> "$tmp_file"
  fi

  cat "$role_file" >> "$tmp_file"

  if [ "$role" != "PM" ]; then
    cat >> "$tmp_file" <<EOF

---

## 부팅 직후 공통 대기 규칙 (최우선)

이 규칙은 다른 초기 행동 지침보다 우선합니다.

1. PM(${PM_NAME})의 명시적 작업 지시가 내려오기 전까지 어떤 작업도 시작하지 마세요.
2. 지시 전에는 파일 읽기, 문서 탐색, 레드마인 조회, 코드 작성, 시안 작성, 명령 실행, 계획 수립을 하지 마세요.
3. 도구/CLI가 시작 과정에서 확인, 승인, Continue, Action Required 같은 상호작용을 요구하면 부팅 완료를 위해 필요한 최소 입력만 처리하세요.
4. 부팅이 완료되고 추가 상호작용이 더 이상 필요 없으면 정확히 한 줄만 출력하세요.

READY: ${role}

5. READY 출력 후에는 PM의 다음 지시가 있을 때까지 추가 행동이나 추가 출력 없이 대기하세요.
EOF
  fi

  echo "$tmp_file"
}


# ---------------------------------------------------------------------------
# pane 생성
# ---------------------------------------------------------------------------
create_split_from_surface() {
  local anchor_surface="$1"; local direction="$2"
  local result="" new_surface=""

  [ -n "$anchor_surface" ] || fail "기준 surface가 비어 있습니다."

  result=$(run_cmux new-split "$direction" --surface "$anchor_surface")
  new_surface=$(echo "$result" | grep -o 'surface:[0-9]*' | head -1 || true)
  [ -n "$new_surface" ] || fail "pane 생성 결과에서 surface를 추출하지 못했습니다: $result"
  CREATED_SURFACES+=("$new_surface")

  echo "$new_surface"
}


# ---------------------------------------------------------------------------
# 레이아웃 슬롯 생성
# PM | middle_top    | right_top
#    | middle_mid    | right_mid
#    | middle_bottom | right_bottom
# ---------------------------------------------------------------------------
create_layout_slots() {
  local right_block_top middle_top middle_mid middle_bottom
  local right_top right_mid right_bottom

  right_block_top=$(create_split_from_surface "$PM_SURFACE" "right")
  right_top=$(create_split_from_surface "$right_block_top" "right")
  middle_top="$right_block_top"

  set_layout_surface "middle_top" "$middle_top"
  set_layout_surface "right_top" "$right_top"

  middle_mid=$(create_split_from_surface "$middle_top" "down")
  middle_bottom=$(create_split_from_surface "$middle_mid" "down")

  right_mid=$(create_split_from_surface "$right_top" "down")
  right_bottom=$(create_split_from_surface "$right_mid" "down")

  set_layout_surface "middle_mid" "$middle_mid"
  set_layout_surface "middle_bottom" "$middle_bottom"
  set_layout_surface "right_mid" "$right_mid"
  set_layout_surface "right_bottom" "$right_bottom"
}


# ---------------------------------------------------------------------------
# 문자열 전송 + Enter 제출
# ---------------------------------------------------------------------------
send_and_submit() {
  local surface="$1"; local message="$2"
  run_cmux send --surface "$surface" "$message" >/dev/null
  run_cmux send-key --surface "$surface" Enter >/dev/null
}


# ---------------------------------------------------------------------------
# READY 지시문 생성
# ---------------------------------------------------------------------------
build_ready_instruction() {
  local role="$1"
  cat <<EOF
지금부터 PM의 명시적 작업 지시가 있을 때까지 작업에 착수하지 마세요.
파일 읽기, 문서 탐색, 레드마인 조회, 코드/시안 작성, 계획 수립도 하지 마세요.
시작 과정에서 확인/승인 프롬프트가 나오면 부팅 완료에 필요한 최소 입력만 처리하세요.
초기 로딩과 자기 점검이 끝나면 정확히 아래 한 줄만 출력하세요.
READY: ${role}

그 전에는 다른 응답을 하지 말고, READY 출력 후에는 추가 작업을 하지 말고 대기하세요.
EOF
}


# ---------------------------------------------------------------------------
# 부팅 중 상호작용 프롬프트 감지
# ---------------------------------------------------------------------------
should_auto_submit_enter() {
  local screen="$1"
  echo "$screen" | grep -Eiq \
    'press enter|hit enter|enter to continue|continue\??|confirm|confirmation|approve|approval|allow|permission|grant|proceed|press return|continue with|continue setup|submit to continue|승인|허용|계속하시겠|계속하려면|진행하시겠|확인하려면|확인 필요|승인 필요|권한|허가|계속 진행|엔터를 누르|enter 키|return 키|curl 승인'
}


# ---------------------------------------------------------------------------
# READY 신호 대기
# ---------------------------------------------------------------------------
wait_for_ready_signal() {
  local surface="$1"; local role="$2"
  local elapsed=0 pattern="READY: ${role}" screen="" auto_submit_count=0

  log "  ⏳ ${role} READY 신호 대기 중..."
  while [ "$elapsed" -lt "$READY_SIGNAL_TIMEOUT" ]; do
    screen="$(cmux read-screen --surface "$surface" --lines 80 2>/dev/null || true)"

    if echo "$screen" | grep -q "$pattern"; then
      log "  ✅ ${role} READY 확인 완료"
      return 0
    fi

    if [ "$auto_submit_count" -lt "$MAX_AUTO_SUBMITS" ] && should_auto_submit_enter "$screen"; then
      auto_submit_count=$((auto_submit_count + 1))
      log "  ↪ ${role} 부팅 중 상호작용 감지, Enter 자동 제출 (${auto_submit_count}/${MAX_AUTO_SUBMITS})"
      run_cmux send-key --surface "$surface" Enter >/dev/null
      sleep 2; elapsed=$((elapsed + 2)); continue
    fi

    sleep 2; elapsed=$((elapsed + 2))
  done

  return 1
}


# ---------------------------------------------------------------------------
# 실행 명령 조립
# ---------------------------------------------------------------------------
build_launch_command() {
  local command_template="$1"; local persona_file="$2"
  echo "${command_template//__PERSONA_FILE__/$persona_file}"
}


get_role_label() {
  local role="$1"
  case "$role" in
    DeveloperBE) echo "BE" ;;
    DeveloperFE) echo "FE" ;;
    *) echo "$role" ;;
  esac
}


# ---------------------------------------------------------------------------
# 팀원 1명 부팅
# ---------------------------------------------------------------------------
spawn_member() {
  local role="$1" display_name="$2" agent_type="$3" command_template="$4" layout_slot="$5"
  local surface="" persona_file="" launch_command="" ready_instruction=""
  local role_label tab_name

  log "--- ${role} (${display_name}) ---"

  surface="$(get_layout_surface "$layout_slot")"
  [ -n "$surface" ] || fail "${role}에 할당된 레이아웃 슬롯이 비어 있습니다: ${layout_slot}"
  set_role_surface "$role" "$surface"
  set_role_display_name "$role" "$display_name"

  role_label="$(get_role_label "$role")"
  tab_name="${display_name}(${role_label})"
  cmux rename-tab --surface "$surface" "$tab_name" >/dev/null 2>&1 || true
  log "  패인 생성: $surface"

  persona_file=$(create_persona_bundle "$role") || fail "${role} 페르소나 번들 생성 실패"
  [ -n "$persona_file" ] || fail "${role} 페르소나 파일 경로가 비어 있습니다."
  launch_command=$(build_launch_command "$command_template" "$persona_file")

  send_and_submit "$surface" "$launch_command"
  sleep "$POST_LAUNCH_DELAY"

  ready_instruction=$(build_ready_instruction "$role")
  send_and_submit "$surface" "$ready_instruction"

  if ! wait_for_ready_signal "$surface" "$role"; then
    dump_surface_screen "$surface" "${role}"
    fail "${role} READY 신호 대기 타임아웃. surface=${surface}"
  fi

  echo ""
}


# ---------------------------------------------------------------------------
# 전체 팀원 부팅
# ---------------------------------------------------------------------------
spawn_team_members() {
  local line role display_name agent_type command_template layout_slot

  create_layout_slots

  for line in "${ROLE_CONFIG[@]}"; do
    IFS='|' read -r role display_name agent_type command_template layout_slot <<< "$line"
    spawn_member "$role" "$display_name" "$agent_type" "$command_template" "$layout_slot"
  done
}


# ---------------------------------------------------------------------------
# 팀 구성 요약
# ---------------------------------------------------------------------------
print_team_summary() {
  log "========================================="
  log "  AgiTeam 팀 구성 완료!"
  log "  프로젝트: $(json_value "$CONFIG_FILE" "project.name" 2>/dev/null || echo "$WORK_DIR")"
  log "  PM            : $(get_role_surface "PM")"

  local line role
  for line in "${ROLE_CONFIG[@]}"; do
    IFS='|' read -r role _ _ _ _ <<< "$line"
    printf "  %-15s: %s\n" "$role" "$(get_role_surface "$role")"
  done

  log "  작업 디렉토리 : $WORK_DIR"
  log "  설정 파일     : $CONFIG_FILE"
  log "========================================="
  echo ""
}


# ---------------------------------------------------------------------------
# PM 시스템 프롬프트 생성
# ---------------------------------------------------------------------------
build_pm_system_file() {
  local pm_persona_file pm_system_file

  pm_persona_file=$(create_persona_bundle "PM") || fail "PM 페르소나 번들 생성 실패"
  pm_system_file=$(mktemp -t "agiteam_persona_PM_system") || fail "PM 시스템 파일 생성 실패"
  TEMP_FILES+=("$pm_system_file")

  cat "$pm_persona_file" > "$pm_system_file"

  # 팀 구성 정보 동적 생성
  {
    echo ""
    echo "팀 구성:"

    local line role display_name agent_type
    for line in "${ROLE_CONFIG[@]}"; do
      IFS='|' read -r role display_name agent_type _ _ <<< "$line"
      local agent_label=""
      case "$agent_type" in
        claude) agent_label="" ;;
        codex)  agent_label="Codex-" ;;
        gemini) agent_label="Gemini-" ;;
      esac
      echo "- ${agent_label}${role} ($(get_role_surface "$role")): ${role} (${display_name})"
    done

    echo ""
    echo "별칭 매핑:"
    for line in "${ROLE_CONFIG[@]}"; do
      IFS='|' read -r role display_name _ _ _ <<< "$line"
      echo "- ${display_name} → ${role} ($(get_role_surface "$role"))"
    done

    echo ""
    echo "팀원별 배정 surface (cmux send 지시 시 사용):"
    for line in "${ROLE_CONFIG[@]}"; do
      IFS='|' read -r role _ _ _ _ <<< "$line"
      echo "- $role → $(get_role_surface "$role")"
    done

    echo ""
    echo "작업 디렉토리: ${WORK_DIR}"
    echo "현재 사업 유형: ${BUSINESS_TYPE} · 현재 모드: $(display_current_mode "$CURRENT_MODE") · 마일스톤: ${MILESTONE} · WBS 트랙: ${WBS_TRACK} (project_state.yaml 값. 마일스톤 코드 정의는 RACI매트릭스 §7.7. M1이면 project_state.yaml milestones에서 세부 마일스톤 진행 상태를 확인. 이 파일은 startupFiles로 읽지 말고 필요 시 PM 지시 후 명시적으로만 조회하세요.)"
    echo ""
    echo "cmux 기본 명령어·필수 규칙·에이전트별 특이점·팀 소통 원칙은 Shared/persona.md §5 참조 (이미 시스템 프롬프트에 주입됨)."
    echo "각 팀원은 부팅 직후 READY 신호를 출력했습니다. 추가 지시 전에는 READY 상태로 간주하세요."

    if [ "${#PM_STARTUP_FILES[@]}" -gt 0 ]; then
      echo ""
      echo "시작 시 아래 파일을 순서대로 읽어 이전 세션 맥락을 파악하세요:"
      local idx=1
      for sf in "${PM_STARTUP_FILES[@]}"; do
        echo "${idx}. ${WORK_DIR}/${sf}"
        idx=$((idx + 1))
      done
    fi

  } >> "$pm_system_file"

  echo "$pm_system_file"
}


# ---------------------------------------------------------------------------
# PM 실행
# ---------------------------------------------------------------------------
launch_pm() {
  
  log "PM 실행 중..."
  echo ""

  local pm_system_file

  cmux rename-tab --surface "$PM_SURFACE" "${PM_NAME}(PM)" >/dev/null 2>&1 || true

  pm_system_file=$(build_pm_system_file)

  exec ${PM_COMMAND} \
    --system-prompt "$(cat "$pm_system_file")" \
    "$PM_STARTUP_MESSAGE"
}


# ---------------------------------------------------------------------------
# 메인 실행 흐름
# ---------------------------------------------------------------------------
main() {
  log ""
  log "  ╔══════════════════════════════════════╗"
  log "          AgiTeam - 팀 부팅 엔진             "
  log "  ╚══════════════════════════════════════╝"
  log ""
  log "  작업 디렉토리: $WORK_DIR"
  log "  설정 파일:     $CONFIG_FILE"
  log ""

  # agiteam.json 로드
  load_config

  # 실행 환경 검증
  validate_environment

  # 인증/네트워크 사전 진단
  validate_auth_and_network

  # 프로젝트 구조 검증
  validate_workspace
  
  # workspace / PM surface 식별
  resolve_workspace_context

  log "--- 워크스페이스: ${WORKSPACE_DISPLAY_NAME} (${WORKSPACE_COLOR}) ---"
  if [ -n "${WS_ID:-}" ]; then
    log "  ✅ 워크스페이스 설정 완료: $WS_ID"
  fi
  log "  PM surface: $PM_SURFACE"
  echo ""

  # 전체 팀원 부팅
  spawn_team_members
  
  # 팀 구성 요약
  print_team_summary

  # PM 실행
  launch_pm
}

main "$@"
