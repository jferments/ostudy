#!/usr/bin/env bash
set -Eeuo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd -- "$SCRIPT_DIR/.." && pwd)"
# shellcheck disable=SC1091
source "$ROOT_DIR/versions.env"
# shellcheck disable=SC1091
source "$SCRIPT_DIR/cargo-target.sh"
export OSTUDY_ROOT_DIR="$ROOT_DIR"
# shellcheck disable=SC1091
source "$SCRIPT_DIR/build-state.sh"
export PATH="$HOME/.local/bin:$HOME/.cargo/bin:$PATH"
export CARGO_BUILD_JOBS="${CARGO_BUILD_JOBS:-$(nproc)}"
cd "$ROOT_DIR"

CURRENT_STEP="initialization"
step() { CURRENT_STEP="$*"; printf '\n\033[1;34m==> %s\033[0m\n' "$CURRENT_STEP"; }
trap 'status=$?; printf "\n\033[1;31mFAILED during: %s (exit %s)\033[0m\n" "$CURRENT_STEP" "$status" >&2; exit "$status"' ERR

step "Environment and repository diagnostics"
if [[ "${OSTUDY_SKIP_NATIVE_SMOKE:-0}" == "1" ]]; then
  "$SCRIPT_DIR/doctor.sh"
else
  "$SCRIPT_DIR/doctor.sh" --native-smoke
fi
node "$SCRIPT_DIR/verify-project-manifests.mjs"
node "$SCRIPT_DIR/verify-stage2.mjs"
node "$SCRIPT_DIR/verify-stage3.mjs"
[[ -f pnpm-lock.yaml && -f src-tauri/Cargo.lock ]] || {
  printf 'ERROR: committed pnpm and Cargo lockfiles are required.\n' >&2
  exit 1
}
prepare_cargo_target "$ROOT_DIR"
printf 'Cargo parallel jobs: %s\n' "$CARGO_BUILD_JOBS"

step "Generate the Stage 5 analytics JavaScript lockfile once"
pnpm install --no-frozen-lockfile
printf 'The regenerated pnpm lockfile is ready for the failure diagnostic artifact.\n'
exit 91
