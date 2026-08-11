#!/usr/bin/env bash
set -Eeuo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd -- "$SCRIPT_DIR/.." && pwd)"
# shellcheck disable=SC1091
source "$ROOT_DIR/versions.env"
# shellcheck disable=SC1091
source "$SCRIPT_DIR/check-state.sh"
export OSTUDY_ROOT_DIR="$ROOT_DIR"
export PATH="$HOME/.local/bin:$HOME/.cargo/bin:$PATH"
cd "$ROOT_DIR"

CURRENT_STEP="initialization"
step() { CURRENT_STEP="$*"; printf '\n\033[1;34m==> %s\033[0m\n' "$CURRENT_STEP"; }
trap 'status=$?; printf "\n\033[1;31mSTATIC CHECK FAILED during: %s (exit %s)\033[0m\n" "$CURRENT_STEP" "$status" >&2; exit "$status"' ERR

if [[ "${OSTUDY_FORCE_CHECK:-0}" != "1" ]] \
  && ostudy_javascript_dependencies_present \
  && ostudy_check_stamp_is_current static; then
  printf '\n\033[1;32mREUSE Static qualification already passed for this exact source tree.\033[0m\n'
  exit 0
fi

step "Environment and repository contracts"
"$SCRIPT_DIR/doctor.sh"
node "$SCRIPT_DIR/verify-project-manifests.mjs"
node "$SCRIPT_DIR/verify-ipc-contract.mjs"
node "$SCRIPT_DIR/verify-repository-policy.mjs"
node "$SCRIPT_DIR/verify-documentation.mjs"
[[ -f pnpm-lock.yaml && -f src-tauri/Cargo.lock ]] || {
  printf 'ERROR: committed pnpm and Cargo lockfiles are required.\n' >&2
  exit 1
}

step "Behavior-test policy and repository script validation"
node "$SCRIPT_DIR/verify-test-policy.mjs"
bash "$SCRIPT_DIR/verify-script-syntax.sh"
python3 -m unittest discover -s tests/scripts -p 'test_*.py'

step "Frozen JavaScript installation and dependency verification"
pnpm install --frozen-lockfile
pnpm run verify:versions
[[ "$(pnpm exec esbuild --version)" == "$ESBUILD_VERSION" ]]

step "Frontend type checking"
pnpm exec tsc -b --pretty false

step "Rust formatting"
cargo fmt --manifest-path src-tauri/Cargo.toml --all -- --check

ostudy_write_check_stamp static
trap - ERR
printf '\n\033[1;32mStatic ostudy checks passed.\033[0m\n'
