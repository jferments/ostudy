#!/usr/bin/env bash
set -Eeuo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd -- "$SCRIPT_DIR/.." && pwd)"
BASELINE_DIR="$ROOT_DIR/tests/performance-baselines/ui-workflow-redesign-stage-0"
COMMAND="${1:-verify}"
CAPTURE_COMMAND="$COMMAND"

usage() {
  printf 'Usage: ./scripts/ui-performance-baselines.sh verify|verify-reference|capture|compare <candidate-directory>|capture-candidate <absolute-new-directory>|compare-candidate <candidate-directory>\n' >&2
}

if [[ "$COMMAND" == "verify" && "$#" -eq 1 ]]; then
  exec python3 "$ROOT_DIR/tests/webdriver/ui_performance_baseline.py" \
    verify --baseline-dir "$BASELINE_DIR"
fi
if [[ "$COMMAND" == "verify-reference" && "$#" -eq 1 ]]; then
  exec python3 "$ROOT_DIR/tests/webdriver/ui_performance_baseline.py" \
    verify-reference --baseline-dir "$BASELINE_DIR"
fi
if [[ "$COMMAND" == "compare" && "$#" -eq 2 ]]; then
  exec python3 "$ROOT_DIR/tests/webdriver/ui_performance_baseline.py" \
    compare --baseline-dir "$BASELINE_DIR" --candidate-dir "$2"
fi
if [[ "$COMMAND" == "compare-candidate" && "$#" -eq 2 ]]; then
  exec python3 "$ROOT_DIR/tests/webdriver/ui_performance_baseline.py" \
    compare-candidate --baseline-dir "$BASELINE_DIR" --candidate-dir "$2"
fi
if [[ "$COMMAND" == "capture-candidate" && "$#" -eq 2 ]]; then
  CANDIDATE_DIR="$2"
  python3 "$ROOT_DIR/tests/webdriver/ui_performance_baseline.py" \
    validate-candidate-path --candidate-dir "$CANDIDATE_DIR" >/dev/null
elif [[ "$COMMAND" != "capture" || "$#" -ne 1 ]]; then
  usage
  exit 2
fi

for command_name in cargo git pnpm python3 tauri-driver WebKitWebDriver timeout; do
  command -v "$command_name" >/dev/null 2>&1 || {
    printf 'ERROR: %s is required for native UI performance capture.\n' "$command_name" >&2
    exit 1
  }
done

if [[ -n "$(git -C "$ROOT_DIR" status --porcelain --untracked-files=all)" ]]; then
  printf 'ERROR: Performance capture requires a clean harness checkpoint. Commit the reviewed harness first.\n' >&2
  exit 1
fi
SOURCE_SHA="$(git -C "$ROOT_DIR" rev-parse HEAD)"

if [[ "${OSTUDY_PERFORMANCE_IN_XVFB:-0}" != "1" ]]; then
  command -v xvfb-run >/dev/null 2>&1 || {
    printf 'ERROR: xvfb-run is required for headless UI performance capture.\n' >&2
    exit 1
  }
  if [[ "$CAPTURE_COMMAND" == "capture-candidate" ]]; then
    exec env OSTUDY_PERFORMANCE_IN_XVFB=1 xvfb-run -a \
      --server-args="-screen 0 1920x1200x24 -dpi 96 -nolisten tcp" \
      "$0" capture-candidate "$CANDIDATE_DIR"
  fi
  exec env OSTUDY_PERFORMANCE_IN_XVFB=1 xvfb-run -a \
    --server-args="-screen 0 1920x1200x24 -dpi 96 -nolisten tcp" \
    "$0" capture
fi

CAPTURE_HOME="$(mktemp -d -t ostudy-ui-performance.XXXXXXXX)"
cleanup() { rm -rf -- "$CAPTURE_HOME"; }
trap cleanup EXIT
mkdir -p "$CAPTURE_HOME"/{data,config,cache,state,runtime,target}
chmod 0700 "$CAPTURE_HOME/runtime"
APP_DATA_DIR="$CAPTURE_HOME/data/io.github.jferments.ostudy"

printf '[ui-performance] Building isolated deterministic fixture\n'
FIXTURE_TEST="library_backup::extended_tests::performance_fixture_tests::writes_stage_0_ui_performance_pending_restore"
if [[ "$CAPTURE_COMMAND" == "capture-candidate" ]]; then
  FIXTURE_TEST="library_backup::extended_tests::performance_fixture_tests::writes_stage_4a_large_library_drag_pending_restore"
fi
OSTUDY_UI_PERFORMANCE_FIXTURE_APP_DATA="$APP_DATA_DIR" \
  cargo test --manifest-path "$ROOT_DIR/src-tauri/Cargo.toml" --locked --all-features --lib \
    "$FIXTURE_TEST" \
    -- --exact

printf '[ui-performance] Building isolated React profiling application\n'
(
  cd "$ROOT_DIR"
  CARGO_TARGET_DIR="$CAPTURE_HOME/target" \
  OSTUDY_UI_PERFORMANCE_CAPTURE=1 \
    pnpm tauri build --debug --no-bundle --ci -- --locked
)
APPLICATION="$CAPTURE_HOME/target/debug/ostudy"
[[ -x "$APPLICATION" ]] || {
  printf 'ERROR: Isolated profiling application was not built at %s.\n' "$APPLICATION" >&2
  exit 1
}

if [[ "$CAPTURE_COMMAND" == "capture-candidate" ]]; then
  printf '[ui-performance] Capturing 3 warmups and 12 samples for deck-drag-v1 only\n'
  env XDG_DATA_HOME="$CAPTURE_HOME/data" \
    XDG_CONFIG_HOME="$CAPTURE_HOME/config" \
    XDG_CACHE_HOME="$CAPTURE_HOME/cache" \
    XDG_STATE_HOME="$CAPTURE_HOME/state" \
    XDG_RUNTIME_DIR="$CAPTURE_HOME/runtime" \
    timeout --signal=INT 1200s python3 -u \
      "$ROOT_DIR/tests/webdriver/ui_performance_baseline.py" capture-candidate \
        --application "$APPLICATION" \
        --candidate-dir "$CANDIDATE_DIR" \
        --source-sha "$SOURCE_SHA"
else
  printf '[ui-performance] Capturing 3 warmups and 12 samples for six scenarios\n'
  env XDG_DATA_HOME="$CAPTURE_HOME/data" \
    XDG_CONFIG_HOME="$CAPTURE_HOME/config" \
    XDG_CACHE_HOME="$CAPTURE_HOME/cache" \
    XDG_STATE_HOME="$CAPTURE_HOME/state" \
    XDG_RUNTIME_DIR="$CAPTURE_HOME/runtime" \
    timeout --signal=INT 1200s python3 -u \
      "$ROOT_DIR/tests/webdriver/ui_performance_baseline.py" capture \
        --application "$APPLICATION" \
        --baseline-dir "$BASELINE_DIR" \
        --source-sha "$SOURCE_SHA"
fi
