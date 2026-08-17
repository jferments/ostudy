#!/usr/bin/env bash
set -Eeuo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd -- "$SCRIPT_DIR/.." && pwd)"
# shellcheck disable=SC1091
source "$ROOT_DIR/versions.env"
# shellcheck disable=SC1091
source "$SCRIPT_DIR/check-state.sh"
# shellcheck disable=SC1091
source "$SCRIPT_DIR/cargo-target.sh"
# shellcheck disable=SC1091
source "$SCRIPT_DIR/lib/diagnostics.bash"
export OSTUDY_ROOT_DIR="$ROOT_DIR"
export OSTUDY_QUALIFICATION_COMMAND="./scripts/check.sh"
export PATH="$HOME/.local/bin:$HOME/.cargo/bin:$PATH"
export CARGO_BUILD_JOBS="${CARGO_BUILD_JOBS:-$(nproc)}"
ostudy_parse_diagnostic_args "$@"
if (( ${#OSTUDY_DIAGNOSTIC_REMAINDER[@]} > 0 )); then
  printf 'Usage: ./scripts/check.sh [diagnostic options]\n' >&2
  ostudy_diagnostic_usage >&2
  exit 2
fi
cd "$ROOT_DIR"
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
  fi
  CURRENT_STEP="$*"
  ostudy_reset_worktree_baseline "$WORKTREE_BASELINE"
  printf '\n\033[1;34m==> %s\033[0m\n' "$CURRENT_STEP"
}
trap 'status=$?; ostudy_assert_worktree_unchanged "$WORKTREE_BASELINE" "$CURRENT_STEP" || true; printf "\n\033[1;31mFULL CHECK FAILED during: %s (exit %s)\033[0m\n" "$CURRENT_STEP" "$status" >&2; ostudy_capture_failure_artifacts "$CURRENT_STEP" "$status"; exit "$status"' ERR

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
  bash "$SCRIPT_DIR/check-fast.sh" "${FAST_ARGS[@]}"
fi

step "Verify audited PDFium runtime"
"$SCRIPT_DIR/install-native-deps.sh" --pdfium-only

step "Native environment qualification"
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

step "Standalone Tauri build"
pnpm tauri build --debug --no-bundle --ci -- --locked
"$SCRIPT_DIR/verify-rust-dependencies.sh"
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
ostudy_write_check_stamp full
trap - ERR
printf '\n\033[1;32mAll ostudy checks passed.\033[0m\n'
printf 'The current standalone build is ready for ./run-app.sh.\n'
