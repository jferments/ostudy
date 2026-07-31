#!/usr/bin/env bash
set -Eeuo pipefail

ROOT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
export OSTUDY_ROOT_DIR="$ROOT_DIR"
# shellcheck disable=SC1091
source "$ROOT_DIR/scripts/build-state.sh"
export PATH="$HOME/.local/bin:$HOME/.cargo/bin:$PATH"

if (( $# > 0 )); then
  printf 'Usage: ./run-app.sh\n' >&2
  exit 2
fi

ostudy_require_current_build

LOG_DIR="${XDG_STATE_HOME:-$HOME/.local/state}/ostudy"
LOG_PATH="$LOG_DIR/ostudy.jsonl"
VERSION="$(ostudy_stamp_value version)"
mkdir -p "$LOG_DIR"

printf 'Starting ostudy %s from the existing build.\n' "$VERSION"
printf 'Log: %s\n\n' "$LOG_PATH"
exec python3 "$ROOT_DIR/scripts/log-run.py" \
  --log "$LOG_PATH" \
  --version "$VERSION" \
  -- "$OSTUDY_BINARY"
