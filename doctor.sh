#!/bin/bash
# =============================================================================
# doctor.sh - AgiTeamBuilder host environment preflight
# =============================================================================

set -u

QUIET=0

usage() {
  cat <<EOF
Usage:
  ./doctor.sh
  ./doctor.sh --quiet

Options:
  --quiet     Print only missing tools.
  -h, --help  Show this help.
EOF
}

case "${1:-}" in
  "")
    ;;
  --quiet)
    QUIET=1
    ;;
  -h|--help)
    usage
    exit 0
    ;;
  *)
    echo "Unknown option: $1" >&2
    usage >&2
    exit 2
    ;;
esac

install_command() {
  case "$1" in
    git)
      echo "xcode-select --install   또는   brew install git"
      ;;
    uv)
      echo "curl -LsSf https://astral.sh/uv/install.sh | sh"
      ;;
    python3)
      echo "uv python install   또는   brew install python"
      ;;
    mempalace)
      echo "uv tool install mempalace"
      ;;
    cmux)
      echo "brew install --cask cmux   또는   https://www.cmux.dev/docs/getting-started"
      ;;
    claude)
      echo "npm install -g @anthropic-ai/claude-code"
      ;;
    codex)
      echo "npm install -g @openai/codex"
      ;;
  esac
}

tool_version() {
  local tool="$1"
  local version=""

  version=$("$tool" --version 2>&1 | head -n 1 || true)
  [ -n "$version" ] && echo " ($version)"
}

tool_row() {
  local rank="$1"
  local tool="$2"
  local path=""
  local status=""
  local install=""

  install=$(install_command "$tool")
  path=$(command -v "$tool" 2>/dev/null || true)

  if [ -n "$path" ]; then
    if [ "$QUIET" -eq 0 ]; then
      status="✅ ${path}$(tool_version "$tool")"
      printf '| %s | %s | %s | — |\n' "$rank" "$tool" "$status"
    fi
    return 0
  fi

  printf '| %s | %s | ❌ 없음 | %s |\n' "$rank" "$tool" "$install"
  return 1
}

TMP_ROWS=$(mktemp -t agiteam_doctor_rows) || exit 1
trap 'rm -f "$TMP_ROWS"' EXIT

MISSING=0

check_tool() {
  local rank="$1"
  local tool="$2"

  if ! tool_row "$rank" "$tool" >> "$TMP_ROWS"; then
    MISSING=$((MISSING + 1))
  fi
}

check_tool 1 git
check_tool 2 uv
check_tool 3 python3
check_tool 4 mempalace
check_tool 5 cmux
check_tool 6 claude
check_tool 7 codex

if [ "$MISSING" -eq 0 ]; then
  if [ "$QUIET" -eq 0 ]; then
    echo "✅ doctor: 모든 필수 도구 정상 (git · uv · python3 · mempalace · cmux · claude · codex)"
  fi
  exit 0
fi

if [ "$QUIET" -eq 0 ]; then
  echo "❌ doctor: 다음 도구가 부족합니다."
  echo ""
else
  echo "❌ doctor: 부족 도구"
fi

echo "| 순위 | 도구 | 상태 | 설치 명령 |"
echo "|:---:|---|:---:|---|"
cat "$TMP_ROWS"
echo ""
echo "→ 위→아래 순서로 설치하세요. 의존 사슬: git → uv → python3 → mempalace."

exit 1
