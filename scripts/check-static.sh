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
source "$SCRIPT_DIR/lib/diagnostics.bash"
# shellcheck disable=SC1091
source "$SCRIPT_DIR/lib/qualification-timing.bash"
export OSTUDY_ROOT_DIR="$ROOT_DIR"
export OSTUDY_QUALIFICATION_COMMAND="./scripts/check-static.sh"
ostudy_parse_diagnostic_args "$@"
if (( ${#OSTUDY_DIAGNOSTIC_REMAINDER[@]} > 0 )); then
  printf 'Usage: ./scripts/check-static.sh [diagnostic options]\n' >&2
  ostudy_diagnostic_usage >&2
  exit 2
fi
cd "$ROOT_DIR"
ostudy_timing_init static

WORKTREE_BASELINE="$(mktemp -t ostudy-static-worktree.XXXXXXXX)"
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
trap 'status=$?; ostudy_timing_finish_step failed; ostudy_timing_finish_total failed; ostudy_assert_worktree_unchanged "$WORKTREE_BASELINE" "$CURRENT_STEP" || true; printf "\n\033[1;31mSTATIC CHECK FAILED during: %s (exit %s)\033[0m\n" "$CURRENT_STEP" "$status" >&2; ostudy_capture_failure_artifacts "$CURRENT_STEP" "$status"; exit "$status"' ERR

if [[ "${OSTUDY_FORCE_CHECK:-0}" != "1" ]] \
  && ostudy_frontend_prerequisites_present \
  && ostudy_check_stamp_is_current static; then
  printf '\n\033[1;32mREUSE Static qualification already passed for this exact source tree.\033[0m\n'
  ostudy_timing_finish_total reused
  exit 0
fi

step "Pinned script correctness tooling"
ostudy_ensure_script_linters

step "Environment and repository contracts"
"$SCRIPT_DIR/doctor.sh"
node "$SCRIPT_DIR/verify-project-manifests.mjs"
node "$SCRIPT_DIR/verify-development-profile.mjs"
node "$SCRIPT_DIR/verify-development-tooling.mjs"
node "$SCRIPT_DIR/verify-current-data-contracts.mjs"
node "$SCRIPT_DIR/verify-linux-package-data-safety.mjs"
node "$SCRIPT_DIR/verify-release-readiness-cleanup.mjs"
node "$SCRIPT_DIR/verify-xberg-dependency-policy.mjs"
node "$SCRIPT_DIR/generate-ui-tokens.mjs" --check
node "$SCRIPT_DIR/verify-ui-token-usage.mjs"
node --test tests/scripts/ui-theme.node-test.mjs tests/scripts/ui-import-policy.node-test.mjs tests/scripts/ui-presentation-policy.node-test.mjs
node "$SCRIPT_DIR/verify-style-ownership.mjs"
node "$SCRIPT_DIR/verify-button-contrast.mjs"
node "$SCRIPT_DIR/verify-deck-toolbar-layout.mjs"
node "$SCRIPT_DIR/verify-device-sync-qr-ui.mjs"
node "$SCRIPT_DIR/verify-android-usb-accessory-manifest.mjs"
node "$SCRIPT_DIR/verify-usb-sync-hardening.mjs"
node "$SCRIPT_DIR/verify-android-qr-scanner.mjs"
node "$SCRIPT_DIR/verify-library-backup-threading.mjs"
node "$SCRIPT_DIR/verify-ipc-contract.mjs"
node "$SCRIPT_DIR/verify-repository-policy.mjs"
node "$SCRIPT_DIR/verify-actions-storage-policy.mjs"
node "$SCRIPT_DIR/verify-route-smoke-contract.mjs"
node "$SCRIPT_DIR/verify-agent-workflow-policy.mjs"
node "$SCRIPT_DIR/verify-agent-recovery-policy.mjs"
node "$SCRIPT_DIR/verify-platform-boundaries.mjs"
node "$SCRIPT_DIR/verify-ai-infrastructure-boundaries.mjs"
node "$SCRIPT_DIR/verify-document-ai-boundaries.mjs"
node "$SCRIPT_DIR/verify-document-processing-boundaries.mjs"
node "$SCRIPT_DIR/verify-external-file-streaming.mjs"
node "$SCRIPT_DIR/verify-android-project.mjs"
node "$SCRIPT_DIR/verify-android-plugin-routing.mjs"
node "$SCRIPT_DIR/verify-diagnostics-workflow.mjs"
node "$SCRIPT_DIR/verify-ci-performance-contracts.mjs"
node "$SCRIPT_DIR/verify-startup-boundary.mjs"
node "$SCRIPT_DIR/verify-local-fonts.mjs"
node "$SCRIPT_DIR/verify-documentation.mjs"
[[ -f pnpm-lock.yaml && -f src-tauri/Cargo.lock ]] || {
  printf 'ERROR: committed pnpm and Cargo lockfiles are required.\n' >&2
  exit 1
}

step "Behavior-test policy and repository script syntax"
node "$SCRIPT_DIR/verify-test-policy.mjs"
bash "$SCRIPT_DIR/verify-script-syntax.sh"

step "Shell and Python correctness linting"
bash "$SCRIPT_DIR/lint-scripts.sh"

step "Frozen JavaScript installation and dependency verification"
if ! pnpm install --frozen-lockfile; then
  bash "$SCRIPT_DIR/emit-js-lockfile-candidate.sh"
  exit 1
fi
pnpm run fonts:install
pnpm run verify:versions
node "$SCRIPT_DIR/generate-ipc-command-contracts.mjs" --check
pnpm run verify:fonts
[[ "$(pnpm exec esbuild --version)" == "$ESBUILD_VERSION" ]]

step "Repository script unit tests and owned-class verification"
python3 -m unittest discover -s tests/scripts -p 'test_*.py'
node "$SCRIPT_DIR/verify-owned-class-usage.mjs"
node "$SCRIPT_DIR/verify-ui-boundaries.mjs"
node --test \
  tests/scripts/ui-build.node-test.mjs \
  tests/scripts/pr-contract-update.node-test.mjs \
  tests/scripts/frontend-correctness.node-test.mjs \
  tests/scripts/release-readiness-cleanup.node-test.mjs

step "React hook correctness"
pnpm run lint

step "Frontend type checking"
pnpm exec tsc -b --pretty false

step "Rust formatting"
cargo fmt --manifest-path src-tauri/Cargo.toml --all -- --check

ostudy_assert_worktree_unchanged "$WORKTREE_BASELINE" "$CURRENT_STEP"
ostudy_timing_finish_step passed
ostudy_write_check_stamp static
ostudy_timing_finish_total passed
trap - ERR
printf '\n\033[1;32mStatic ostudy checks passed.\033[0m\n'
