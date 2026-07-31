#!/usr/bin/env bash
set -Eeuo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd -- "$SCRIPT_DIR/.." && pwd)"
export PATH="$HOME/.local/bin:$HOME/.cargo/bin:$PATH"

TIMESTAMP="$(date -u +%Y%m%dT%H%M%SZ)"
DEFAULT_NAME="ostudy-diagnostics-$TIMESTAMP.tar.gz"
OUTPUT="${1:-$ROOT_DIR/$DEFAULT_NAME}"
if [[ -d "$OUTPUT" ]]; then
  OUTPUT="$OUTPUT/$DEFAULT_NAME"
fi
OUTPUT="$(realpath -m "$OUTPUT")"
TEMP_DIR="$(mktemp -d -t ostudy-diagnostics.XXXXXXXX)"
cleanup() { rm -rf -- "$TEMP_DIR"; }
trap cleanup EXIT
mkdir -p "$TEMP_DIR/logs"

{
  printf 'ostudy diagnostic export\n'
  printf 'Created (UTC): %s\n' "$TIMESTAMP"
  printf 'This archive intentionally excludes the SQLite library, cards, and managed images.\n'
} > "$TEMP_DIR/README.txt"

(
  set +e
  "$SCRIPT_DIR/doctor.sh" --native-smoke
  printf '\nExit status: %s\n' "$?"
) > "$TEMP_DIR/doctor.txt" 2>&1

{
  uname -a
  printf '\nDesktop: %s\n' "${XDG_CURRENT_DESKTOP:-unknown}"
  printf 'Session: %s\n' "${XDG_SESSION_TYPE:-unknown}"
  printf 'Display: %s\n' "${DISPLAY:-unset}"
} > "$TEMP_DIR/environment.txt"

if command -v git >/dev/null 2>&1 && git -C "$ROOT_DIR" rev-parse --is-inside-work-tree >/dev/null 2>&1; then
  {
    git -C "$ROOT_DIR" rev-parse HEAD
    git -C "$ROOT_DIR" status --short
    git -C "$ROOT_DIR" remote -v | sed -E 's#(https://)[^/@]+@#\1REDACTED@#g'
  } > "$TEMP_DIR/git.txt" 2>&1
fi

cp "$ROOT_DIR/package.json" "$ROOT_DIR/versions.env" "$TEMP_DIR/"
LOG_PATH="${XDG_STATE_HOME:-$HOME/.local/state}/ostudy/ostudy.jsonl"
if [[ -f "$LOG_PATH" ]]; then
  tail -n 1000 "$LOG_PATH" > "$TEMP_DIR/logs/ostudy.jsonl"
fi
if [[ -f "$LOG_PATH.1" ]]; then
  tail -n 1000 "$LOG_PATH.1" > "$TEMP_DIR/logs/ostudy.jsonl.1"
fi

mkdir -p "$(dirname -- "$OUTPUT")"
tar -czf "$OUTPUT" -C "$TEMP_DIR" .
printf 'Diagnostic archive created: %s\n' "$OUTPUT"
