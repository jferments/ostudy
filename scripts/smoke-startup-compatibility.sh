#!/usr/bin/env bash
set -Eeuo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd -- "$SCRIPT_DIR/.." && pwd)"
export OSTUDY_ROOT_DIR="$ROOT_DIR"
export OSTUDY_QUALIFICATION_COMMAND="bash ./scripts/smoke-startup-compatibility.sh"
# shellcheck disable=SC1091
source "$SCRIPT_DIR/build-state.sh"
# shellcheck disable=SC1091
source "$SCRIPT_DIR/lib/diagnostics.bash"
export PATH="$HOME/.local/bin:$HOME/.cargo/bin:$PATH"

ostudy_parse_diagnostic_args "$@"
if (( ${#OSTUDY_DIAGNOSTIC_REMAINDER[@]} > 0 )); then
  printf 'Usage: bash ./scripts/smoke-startup-compatibility.sh [diagnostic options]\n' >&2
  ostudy_diagnostic_usage >&2
  exit 2
fi

cd "$ROOT_DIR"
ostudy_require_current_build
command -v sqlite3 >/dev/null 2>&1 || {
  printf 'ERROR: sqlite3 is required for the startup compatibility smoke test.\n' >&2
  exit 1
}
command -v sha256sum >/dev/null 2>&1 || {
  printf 'ERROR: sha256sum is required for the startup compatibility smoke test.\n' >&2
  exit 1
}
command -v timeout >/dev/null 2>&1 || {
  printf 'ERROR: GNU timeout is required for the startup compatibility smoke test.\n' >&2
  exit 1
}

if [[ -z "${DISPLAY:-}" && "${OSTUDY_IN_STARTUP_XVFB:-0}" != "1" ]]; then
  command -v xvfb-run >/dev/null 2>&1 || {
    printf 'ERROR: xvfb-run is required for a headless startup compatibility smoke test.\n' >&2
    exit 1
  }
  exec env OSTUDY_IN_STARTUP_XVFB=1 xvfb-run -a bash "$0" "$@"
fi

TEMP_ROOT="$(mktemp -d -t ostudy-startup-compat.XXXXXXXX)"
cleanup() { rm -rf -- "$TEMP_ROOT"; }
trap cleanup EXIT

DATA_HOME="$TEMP_ROOT/data"
CONFIG_HOME="$TEMP_ROOT/config"
CACHE_HOME="$TEMP_ROOT/cache"
STATE_HOME="$TEMP_ROOT/state"
APP_DATA="$DATA_HOME/io.github.jferments.ostudy"
DATABASE="$APP_DATA/library.sqlite3"
APPLICATION_LOG="$APP_DATA/diagnostics/ostudy.jsonl"
COMMAND_LOG="$TEMP_ROOT/startup.log"
mkdir -p "$APP_DATA" "$CONFIG_HOME" "$CACHE_HOME" "$STATE_HOME"

sqlite3 "$DATABASE" <<'SQL'
CREATE TABLE schema_migrations (
  version INTEGER PRIMARY KEY,
  name TEXT NOT NULL,
  checksum TEXT NOT NULL,
  applied_at INTEGER NOT NULL
) STRICT;
INSERT INTO schema_migrations VALUES (26, 'deck_folders', 'legacy', 0);
CREATE TABLE decks (id TEXT PRIMARY KEY) STRICT;
INSERT INTO decks VALUES ('legacy-deck');
SQL

before_sha="$(sha256sum "$DATABASE" | awk '{print $1}')"

printf '\n==> Built-binary pre-public database startup boundary\n'
set +e
XDG_DATA_HOME="$DATA_HOME" \
XDG_CONFIG_HOME="$CONFIG_HOME" \
XDG_CACHE_HOME="$CACHE_HOME" \
XDG_STATE_HOME="$STATE_HOME" \
OSTUDY_LOG='warn,ostudy.app=debug,ostudy.database=debug' \
  timeout --signal=TERM 5s "$OSTUDY_BINARY" >"$COMMAND_LOG" 2>&1
status=$?
set -e

if [[ "$status" -ne 124 ]]; then
  cat "$COMMAND_LOG" >&2
  printf 'ERROR: legacy-data startup exited with status %s instead of remaining alive behind the reset dialog.\n' "$status" >&2
  ostudy_capture_failure_artifacts \
    "Native startup compatibility smoke" \
    "$status" \
    "$COMMAND_LOG" \
    "$APPLICATION_LOG"
  exit 1
fi

[[ -f "$APPLICATION_LOG" ]] || {
  cat "$COMMAND_LOG" >&2
  printf 'ERROR: startup compatibility smoke did not create the structured application log.\n' >&2
  exit 1
}

grep -F '"event":"application_startup_blocked"' "$APPLICATION_LOG" >/dev/null || {
  cat "$COMMAND_LOG" >&2
  printf 'ERROR: legacy-data startup did not record application_startup_blocked.\n' >&2
  exit 1
}
grep -F '"reasonCode":"pre_public_database_requires_reset"' "$APPLICATION_LOG" >/dev/null || {
  cat "$COMMAND_LOG" >&2
  printf 'ERROR: legacy-data startup did not record the pre-public reset reason code.\n' >&2
  exit 1
}

if grep -Eq 'panicked at|Failed to setup app|panic_captured' "$COMMAND_LOG" "$APPLICATION_LOG"; then
  cat "$COMMAND_LOG" >&2
  printf 'ERROR: an expected legacy-database compatibility block reached a panic path.\n' >&2
  exit 1
fi

after_sha="$(sha256sum "$DATABASE" | awk '{print $1}')"
[[ "$before_sha" == "$after_sha" ]] || {
  printf 'ERROR: the legacy database bytes changed during rejected startup.\n' >&2
  exit 1
}
[[ ! -e "$DATABASE-wal" && ! -e "$DATABASE-shm" ]] || {
  printf 'ERROR: rejected legacy startup created SQLite WAL/shared-memory sidecars.\n' >&2
  exit 1
}

printf 'Native startup compatibility smoke passed: the built application blocked a pre-public database with a recoverable reset dialog, emitted structured diagnostics, did not panic, and left the legacy database bytes unchanged.\n'
