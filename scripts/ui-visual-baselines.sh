#!/usr/bin/env bash
set -Eeuo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd -- "$SCRIPT_DIR/.." && pwd)"
BASELINE_DIR="$ROOT_DIR/tests/visual-baselines/ui-workflow-redesign-stage-0"
COMMAND="${1:-verify}"

usage() {
  printf 'Usage: ./scripts/ui-visual-baselines.sh [verify|capture]\n' >&2
}

if [[ "$#" -gt 1 || "$COMMAND" != "verify" && "$COMMAND" != "capture" ]]; then
  usage
  exit 2
fi

if [[ "$COMMAND" == "verify" ]]; then
  exec python3 "$ROOT_DIR/tests/webdriver/ui_visual_baseline.py" \
    verify --baseline-dir "$BASELINE_DIR"
fi

export OSTUDY_ROOT_DIR="$ROOT_DIR"
# shellcheck disable=SC1091
source "$SCRIPT_DIR/build-state.sh"
export PATH="$HOME/.local/bin:$HOME/.cargo/bin:$PATH"

ostudy_require_current_build
for command_name in tauri-driver WebKitWebDriver timeout; do
  command -v "$command_name" >/dev/null 2>&1 || {
    printf 'ERROR: %s is required for native visual-baseline capture.\n' "$command_name" >&2
    exit 1
  }
done

if [[ "${OSTUDY_VISUAL_IN_XVFB:-0}" != "1" ]]; then
  command -v xvfb-run >/dev/null 2>&1 || {
    printf 'ERROR: xvfb-run is required for headless visual-baseline capture.\n' >&2
    exit 1
  }
  exec env OSTUDY_VISUAL_IN_XVFB=1 xvfb-run -a \
    --server-args="-screen 0 1920x1200x24 -dpi 96 -nolisten tcp" \
    "$0" capture
fi

CAPTURE_HOME="$(mktemp -d -t ostudy-ui-visual-baseline.XXXXXXXX)"
cleanup() { rm -rf -- "$CAPTURE_HOME"; }
trap cleanup EXIT
mkdir -p \
  "$CAPTURE_HOME/data" \
  "$CAPTURE_HOME/config" \
  "$CAPTURE_HOME/cache" \
  "$CAPTURE_HOME/state" \
  "$CAPTURE_HOME/runtime"
chmod 0700 "$CAPTURE_HOME/runtime"

XDG_DATA_HOME="$CAPTURE_HOME/data" \
XDG_CONFIG_HOME="$CAPTURE_HOME/config" \
XDG_CACHE_HOME="$CAPTURE_HOME/cache" \
XDG_STATE_HOME="$CAPTURE_HOME/state" \
XDG_RUNTIME_DIR="$CAPTURE_HOME/runtime" \
  timeout --signal=INT 600s \
    python3 -u "$ROOT_DIR/tests/webdriver/ui_visual_baseline.py" \
      capture \
      --application "$OSTUDY_BINARY" \
      --baseline-dir "$BASELINE_DIR"
