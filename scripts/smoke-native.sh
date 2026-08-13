#!/usr/bin/env bash
set -Eeuo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd -- "$SCRIPT_DIR/.." && pwd)"
export OSTUDY_ROOT_DIR="$ROOT_DIR"
# shellcheck disable=SC1091
source "$SCRIPT_DIR/build-state.sh"
export PATH="$HOME/.local/bin:$HOME/.cargo/bin:$PATH"

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
export XDG_DATA_HOME="$TEMP_HOME/data"
export XDG_CONFIG_HOME="$TEMP_HOME/config"
export XDG_CACHE_HOME="$TEMP_HOME/cache"
export XDG_STATE_HOME="$TEMP_HOME/state"
mkdir -p "$XDG_DATA_HOME" "$XDG_CONFIG_HOME" "$XDG_CACHE_HOME" "$XDG_STATE_HOME"

run_smoke() {
  local label="$1"
  local script="$2"
  printf '\n==> %s\n' "$label"
  # SIGINT lets Python unwind through finally blocks and terminate tauri-driver.
  timeout --signal=INT 300s \
    python3 -u "$script" --application "$OSTUDY_BINARY"
}

run_smoke "Native durable-library WebDriver smoke" \
  "$ROOT_DIR/tests/webdriver/smoke.py"
run_smoke "Native responsive/overlay WebDriver smoke" \
  "$ROOT_DIR/tests/webdriver/ui_platform_smoke.py"
