#!/usr/bin/env bash
set -Eeuo pipefail

ROOT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
export OSTUDY_ROOT_DIR="$ROOT_DIR"
# shellcheck disable=SC1091
source "$ROOT_DIR/scripts/build-state.sh"
# shellcheck disable=SC1091
source "$ROOT_DIR/scripts/lib/diagnostics.bash"
# shellcheck disable=SC1091
source "$ROOT_DIR/scripts/lib/console-events.bash"
export PATH="$HOME/.local/bin:$HOME/.cargo/bin:$PATH"
ostudy_parse_diagnostic_args "$@"
if (( ${#OSTUDY_DIAGNOSTIC_REMAINDER[@]} > 0 )); then
  printf 'Usage: ./run-app.sh [diagnostic options]\n' >&2
  ostudy_diagnostic_usage >&2
  exit 2
fi

ostudy_require_current_build

LOG_DIR="${XDG_STATE_HOME:-$HOME/.local/state}/ostudy"
LOG_PATH="$LOG_DIR/ostudy.jsonl"
VERSION="$(ostudy_stamp_value version)"
mkdir -p "$LOG_DIR"

ostudy_console_stage run "Launching current standalone build"
printf 'Starting ostudy %s from the existing build.\n' "$VERSION"
printf 'Launcher log: %s\n' "$LOG_PATH"
printf 'Application filter: %s\n\n' "${OSTUDY_LOG:-info}"
exec python3 "$ROOT_DIR/scripts/log-run.py" \
  --log "$LOG_PATH" \
  --version "$VERSION" \
  -- "$OSTUDY_BINARY"
