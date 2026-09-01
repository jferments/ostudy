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

# Native smoke is an X11/WebKit automation contract, not a GPU qualification.
# The unified guest has no DRI3 device, and letting WebKit probe accelerated
# compositing there can reset WebKitWebDriver mid-session. Pin deterministic
# software rendering for every application/driver attempt.
export GDK_BACKEND=x11
export WEBKIT_DISABLE_COMPOSITING_MODE=1
export WEBKIT_DISABLE_DMABUF_RENDERER=1
export LIBGL_ALWAYS_SOFTWARE=1

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
command -v openbox >/dev/null 2>&1 || {
  printf 'ERROR: openbox is required for native maximize/full-screen smoke tests. Re-run ./scripts/setup-ubuntu.sh.\n' >&2
  exit 1
}
command -v xprop >/dev/null 2>&1 || {
  printf 'ERROR: xprop is required to verify the headless window manager. Re-run ./scripts/setup-ubuntu.sh.\n' >&2
  exit 1
}

if [[ -z "${DISPLAY:-}" && "${OSTUDY_IN_XVFB:-0}" != "1" ]]; then
  command -v xvfb-run >/dev/null 2>&1 || {
    printf 'ERROR: xvfb-run is required for a headless native smoke test.\n' >&2
    exit 1
  }
  exec env OSTUDY_IN_XVFB=1 xvfb-run -a "$0" "$@"
fi

TEMP_HOME="$(mktemp -d -t ostudy-smoke.XXXXXXXX)"
WINDOW_MANAGER_PID=""
WINDOW_MANAGER_LOG="$TEMP_HOME/openbox.log"

cleanup() {
  if [[ -n "$WINDOW_MANAGER_PID" ]] && kill -0 "$WINDOW_MANAGER_PID" 2>/dev/null; then
    kill "$WINDOW_MANAGER_PID" 2>/dev/null || true
    wait "$WINDOW_MANAGER_PID" 2>/dev/null || true
  fi
  rm -rf -- "$TEMP_HOME"
}
trap cleanup EXIT

window_manager_ready() {
  xprop -root _NET_SUPPORTING_WM_CHECK 2>/dev/null |
    grep -Eq 'window id # 0x[1-9a-fA-F][0-9a-fA-F]*'
}

start_headless_window_manager() {
  local attempt
  mkdir -p "$TEMP_HOME/openbox-config" "$TEMP_HOME/openbox-cache"
  XDG_CONFIG_HOME="$TEMP_HOME/openbox-config" \
  XDG_CACHE_HOME="$TEMP_HOME/openbox-cache" \
    openbox --sm-disable >"$WINDOW_MANAGER_LOG" 2>&1 &
  WINDOW_MANAGER_PID="$!"

  for attempt in $(seq 1 100); do
    if ! kill -0 "$WINDOW_MANAGER_PID" 2>/dev/null; then
      printf 'ERROR: openbox exited before claiming the X11 window-manager selection.\n' >&2
      sed -n '1,160p' "$WINDOW_MANAGER_LOG" >&2 || true
      exit 1
    fi
    if window_manager_ready; then
      printf 'Verified EWMH window manager for native smoke: %s\n' "$(
        xprop -root _NET_SUPPORTING_WM_CHECK 2>/dev/null
      )"
      return 0
    fi
    sleep 0.05
  done

  printf 'ERROR: openbox did not publish _NET_SUPPORTING_WM_CHECK on %s.\n' "${DISPLAY:-unset}" >&2
  sed -n '1,160p' "$WINDOW_MANAGER_LOG" >&2 || true
  exit 1
}

append_window_manager_diagnostics() {
  local log_file="$1"
  {
    printf '\n--- headless window-manager diagnostics ---\n'
    printf 'display=%s pid=%s alive=%s\n' \
      "${DISPLAY:-unset}" \
      "${WINDOW_MANAGER_PID:-external}" \
      "$(
        if [[ -n "$WINDOW_MANAGER_PID" ]] && kill -0 "$WINDOW_MANAGER_PID" 2>/dev/null; then
          printf true
        elif [[ -z "$WINDOW_MANAGER_PID" ]] && window_manager_ready; then
          printf external
        else
          printf false
        fi
      )"
    xprop -root _NET_SUPPORTING_WM_CHECK 2>&1 || true
    if [[ -s "$WINDOW_MANAGER_LOG" ]]; then
      sed -n '1,160p' "$WINDOW_MANAGER_LOG"
    fi
  } >> "$log_file"
}

if ! window_manager_ready; then
  if [[ "${OSTUDY_IN_XVFB:-0}" == "1" ]]; then
    start_headless_window_manager
  else
    printf 'ERROR: DISPLAY=%s has no EWMH window manager; native window-state smoke cannot be qualified.\n' \
      "${DISPLAY:-unset}" >&2
    exit 1
  fi
fi

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
      append_window_manager_diagnostics "$log_file"
      ostudy_capture_transient_artifacts \
        "$label (recovered WebDriver transport failure)" \
        "$status" \
        "$log_file" \
        "$application_log"
      printf 'WARN: WebDriver transport disconnected; retrying once with isolated application state.\n' >&2
      continue
    fi
    append_window_manager_diagnostics "$log_file"
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
