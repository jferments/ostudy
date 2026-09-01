#!/usr/bin/env bash
set -Eeuo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd -- "$SCRIPT_DIR/.." && pwd)"
export OSTUDY_ROOT_DIR="$ROOT_DIR"
export OSTUDY_QUALIFICATION_COMMAND="./scripts/smoke-native.sh"
# shellcheck disable=SC1091
source "$SCRIPT_DIR/build-state.sh"
# shellcheck disable=SC1091
source "$SCRIPT_DIR/lib/diagnostics.bash"
export PATH="$HOME/.local/bin:$HOME/.cargo/bin:$PATH"

ostudy_parse_diagnostic_args "$@"
if (( ${#OSTUDY_DIAGNOSTIC_REMAINDER[@]} > 0 )); then
  printf 'Usage: ./scripts/smoke-native.sh [diagnostic options]\n' >&2
  ostudy_diagnostic_usage >&2
  exit 2
fi

ostudy_require_current_build
command -v tauri-driver >/dev/null 2>&1 || {
  printf 'ERROR: tauri-driver is missing. Re-run ./scripts/setup-ubuntu.sh.\n' >&2
  exit 1
}
command -v WebKitWebDriver >/dev/null 2>&1 || {
  printf 'ERROR: WebKitWebDriver is missing. Install webkit2gtk-driver.\n' >&2
  exit 1
}
command -v timeout >/dev/null 2>&1 || {
  printf 'ERROR: GNU timeout is required to bound native smoke tests.\n' >&2
  exit 1
}

if [[ -z "${DISPLAY:-}" && "${OSTUDY_IN_XVFB:-0}" != "1" ]]; then
  command -v xvfb-run >/dev/null 2>&1 || {
    printf 'ERROR: xvfb-run is required for a headless native smoke test.\n' >&2
    exit 1
  }
  exec env OSTUDY_IN_XVFB=1 xvfb-run -a "$0"
fi

TEMP_HOME="$(mktemp -d -t ostudy-smoke.XXXXXXXX)"
cleanup() { rm -rf -- "$TEMP_HOME"; }
trap cleanup EXIT

is_retryable_driver_failure() {
  local log_file="$1"
  grep -Eq \
    'RemoteDisconnected|ConnectionResetError|Connection refused|remote end closed connection|tauri-driver.*(closed|terminated)' \
    "$log_file"
}

run_smoke() {
  local label="$1"
  local script="$2"
  local slug attempt attempt_home log_file application_log status
  slug="$(printf '%s' "$label" | tr '[:upper:] /' '[:lower:]--' | tr -cd '[:alnum:]_-')"

  for attempt in 1 2; do
    attempt_home="$TEMP_HOME/$slug/attempt-$attempt"
    log_file="$TEMP_HOME/$slug/attempt-$attempt.log"
    application_log="$attempt_home/data/io.github.jferments.ostudy/diagnostics/ostudy.jsonl"
    rm -rf -- "$attempt_home"
    mkdir -p \
      "$attempt_home/data" \
      "$attempt_home/config" \
      "$attempt_home/cache" \
      "$attempt_home/state"

    printf '\n==> %s%s\n' \
      "$label" \
      "$([[ "$attempt" -eq 1 ]] && printf '' || printf ' (driver retry)')"

    set +e
    # SIGINT lets Python unwind through finally blocks and terminate tauri-driver.
    XDG_DATA_HOME="$attempt_home/data" \
    XDG_CONFIG_HOME="$attempt_home/config" \
    XDG_CACHE_HOME="$attempt_home/cache" \
    XDG_STATE_HOME="$attempt_home/state" \
      timeout --signal=INT --kill-after=30s 300s \
        python3 -u "$script" --application "$OSTUDY_BINARY" 2>&1 | tee "$log_file"
    status="${PIPESTATUS[0]}"
    set -e

    if [[ "$status" -eq 0 ]]; then
      return 0
    fi
    if [[ "$attempt" -eq 1 ]] && is_retryable_driver_failure "$log_file"; then
      ostudy_capture_transient_artifacts \
        "$label (recovered WebDriver transport failure)" \
        "$status" \
        "$log_file" \
        "$application_log"
      printf 'WARN: WebDriver transport disconnected; retrying once with isolated application state.\n' >&2
      continue
    fi
    ostudy_capture_failure_artifacts "$label" "$status" "$log_file" "$application_log"
    return "$status"
  done
}

run_smoke "Native durable-library WebDriver smoke" \
  "$ROOT_DIR/tests/webdriver/durable_library_smoke.py"
run_smoke "Native responsive/overlay WebDriver smoke" \
  "$ROOT_DIR/tests/webdriver/ui_platform_smoke.py"
run_smoke "Native card-listing follow-up WebDriver smoke" \
  "$ROOT_DIR/tests/webdriver/card_listing_followup_smoke.py"
run_smoke "Native deck-toolbar layout WebDriver smoke" \
  "$ROOT_DIR/tests/webdriver/deck_toolbar_layout_smoke.py"
