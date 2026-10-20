#!/usr/bin/env bash
set -Eeuo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd -- "$SCRIPT_DIR/.." && pwd)"
# shellcheck disable=SC1091
source "$ROOT_DIR/versions.env"
# shellcheck disable=SC1091
source "$SCRIPT_DIR/lib/node-toolchain.bash"
ostudy_activate_node_toolchain
# shellcheck disable=SC1091
source "$SCRIPT_DIR/lib/script-linters.bash"
# shellcheck disable=SC1091
source "$SCRIPT_DIR/check-state.sh"
# shellcheck disable=SC1091
source "$SCRIPT_DIR/cargo-target.sh"
# shellcheck disable=SC1091
source "$SCRIPT_DIR/lib/diagnostics.bash"
# shellcheck disable=SC1091
source "$SCRIPT_DIR/lib/qualification-timing.bash"
export OSTUDY_ROOT_DIR="$ROOT_DIR"
export OSTUDY_QUALIFICATION_COMMAND="./scripts/check.sh"
export CARGO_BUILD_JOBS="${CARGO_BUILD_JOBS:-$(nproc)}"
ostudy_parse_diagnostic_args "$@"
if (( ${#OSTUDY_DIAGNOSTIC_REMAINDER[@]} > 0)); then
  printf 'Usage: ./scripts/check.sh [diagnostic options]\n' >&2
  ostudy_diagnostic_usage >&2
  exit 2
fi
cd "$ROOT_DIR"
ostudy_timing_init full

if [[ "${OSTUDY_ALLOW_QUALIFICATION_REUSE:-0}" == "1" ]] \
  && [[ "${OSTUDY_FORCE_CHECK:-0}" != "1" ]] \
  && ostudy_frontend_prerequisites_present \
  && ostudy_check_stamp_is_current static \
  && ostudy_check_stamp_is_current full; then
  reuse_sha="$(ostudy_check_stamp_source_sha full || true)"
  printf '\n\033[1;32mREUSE Full Linux qualification from %s: the current exact source passed static qualification and the product qualification fingerprint is unchanged.\033[0m\n' \
    "${reuse_sha:-an earlier qualified source}"
  ostudy_timing_finish_total reused
  exit 0
fi

prepare_cargo_target "$ROOT_DIR"
# shellcheck disable=SC1091
source "$SCRIPT_DIR/build-state.sh"

WORKTREE_BASELINE="$(mktemp -t ostudy-full-worktree.XXXXXXXX)"
cleanup_worktree_baseline() { rm -f -- "$WORKTREE_BASELINE"; }
trap cleanup_worktree_baseline EXIT
ostudy_reset_worktree_baseline "$WORKTREE_BASELINE"

CURRENT_STEP="initialization"
step() {
  if [[ "$CURRENT_STEP" != initialization ]]; then
    ostudy_assert_worktree_unchanged "$WORKTREE_BASELINE" "$CURRENT_STEP"
    ostudy_timing_finish_step passed
  fi
  CURRENT_STEP="$*"
  ostudy_reset_worktree_baseline "$WORKTREE_BASELINE"
  ostudy_timing_start_step "$CURRENT_STEP"
  printf '\n\033[1;34m==> %s\033[0m\n' "$CURRENT_STEP"
}
trap 'status=$?; ostudy_timing_finish_step failed; ostudy_timing_finish_total failed; ostudy_assert_worktree_unchanged "$WORKTREE_BASELINE" "$CURRENT_STEP" || true; printf "\n\033[1;31mFULL CHECK FAILED during: %s (exit %s)\033[0m\n" "$CURRENT_STEP" "$status" >&2; ostudy_capture_failure_artifacts "$CURRENT_STEP" "$status"; exit "$status"' ERR

FAST_NATIVE_DEFERRED=0
step "Fast qualification"
if [[ "${OSTUDY_FAST_CHECK_PASSED:-0}" == "1" ]]; then
  printf '\n\033[1;32mREUSE Fast qualification passed earlier in this workflow.\033[0m\n'
elif [[ "${OSTUDY_FORCE_CHECK:-0}" != "1" ]] \
  && ostudy_frontend_prerequisites_present \
  && ostudy_check_stamp_is_current fast; then
  printf '\n\033[1;32mREUSE Fast qualification already passed for this exact source tree.\033[0m\n'
else
  FAST_ARGS=()
  if [[ "${OSTUDY_DIAGNOSTICS_ACTIVE:-0}" == 1 && "${OSTUDY_DIAGNOSTICS_AUTOMATIC:-0}" != 1 ]]; then
    FAST_ARGS+=("--log-filter=$OSTUDY_LOG")
  fi
  if [[ "${OSTUDY_KEEP_FAILURE_ARTIFACTS:-0}" == 1 ]]; then
    FAST_ARGS+=(--keep-artifacts)
  fi
  OSTUDY_FULL_NATIVE_SUITE_FOLLOWS=1 bash "$SCRIPT_DIR/check-fast.sh" "${FAST_ARGS[@]}"
  FAST_NATIVE_DEFERRED=1
fi

step "Verify audited PDFium runtime"
"$SCRIPT_DIR/install-native-deps.sh" --pdfium-only

step "Native environment qualification"
ostudy_ensure_script_linters
if [[ "${OSTUDY_SKIP_NATIVE_SMOKE:-0}" == "1" ]]; then
  "$SCRIPT_DIR/doctor.sh"
else
  "$SCRIPT_DIR/doctor.sh" --native-smoke
fi
printf 'Cargo target: %s\n' "$CARGO_TARGET_DIR"
printf 'Cargo parallel jobs: %s\n' "$CARGO_BUILD_JOBS"

step "Frontend production build"
pnpm run build

step "Rust linting"
cargo clippy --manifest-path src-tauri/Cargo.toml --locked --all-targets --all-features -- -D warnings

step "Rust unit, integration, and migration-fixture tests"
cargo test --manifest-path src-tauri/Cargo.toml --locked --all-features
if (( FAST_NATIVE_DEFERRED == 1 )); then
  # Every part of fast qualification has now passed on this tree: its frontend
  # tests and Clippy in the fast step, and its native tests inside this suite.
  ostudy_write_check_stamp fast
fi

step "Standalone Tauri build"
pnpm tauri build --config src-tauri/tauri.dev.conf.json --debug --no-bundle --ci -- --locked
"$SCRIPT_DIR/verify-rust-dependencies.sh"
[[ -x "$OSTUDY_CARGO_BINARY" ]]
ostudy_stage_standalone_build
[[ -x "$OSTUDY_BINARY" ]]
ostudy_write_build_stamp "$(node -p "require('./package.json').version")"

if [[ "${OSTUDY_SKIP_NATIVE_SMOKE:-0}" == "1" ]]; then
  printf '\nSKIP Native startup compatibility smoke (OSTUDY_SKIP_NATIVE_SMOKE=1).\n'
  printf 'SKIP Native WebDriver smoke test (OSTUDY_SKIP_NATIVE_SMOKE=1).\n'
else
  step "Native startup compatibility smoke"
  bash "$SCRIPT_DIR/smoke-startup-compatibility.sh"

  step "Native Tauri/WebDriver smoke test"
  "$SCRIPT_DIR/smoke-native.sh"
fi

ostudy_assert_worktree_unchanged "$WORKTREE_BASELINE" "$CURRENT_STEP"
ostudy_timing_finish_step passed
ostudy_write_check_stamp full
ostudy_timing_finish_total passed
trap - ERR
printf '\n\033[1;32mAll ostudy checks passed.\033[0m\n'
printf 'The exact standalone artifact exercised by native smoke is ready for ./run-app.sh.\n'
