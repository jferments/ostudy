#!/usr/bin/env bash
set -Eeuo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd -- "$SCRIPT_DIR/.." && pwd)"
# shellcheck disable=SC1091
source "$ROOT_DIR/versions.env"
# shellcheck disable=SC1091
source "$SCRIPT_DIR/cargo-target.sh"
export OSTUDY_ROOT_DIR="$ROOT_DIR"
export PATH="$HOME/.local/bin:$HOME/.cargo/bin:$PATH"
export CARGO_BUILD_JOBS="${CARGO_BUILD_JOBS:-$(nproc)}"
cd "$ROOT_DIR"

CURRENT_STEP="initialization"
step() { CURRENT_STEP="$*"; printf '\n\033[1;34m==> %s\033[0m\n' "$CURRENT_STEP"; }
trap 'status=$?; printf "\n\033[1;31mFAST CHECK FAILED during: %s (exit %s)\033[0m\n" "$CURRENT_STEP" "$status" >&2; exit "$status"' ERR

step "Fast environment and repository diagnostics"
"$SCRIPT_DIR/doctor.sh"
node "$SCRIPT_DIR/verify-project-manifests.mjs"
node "$SCRIPT_DIR/verify-stage2.mjs"
node "$SCRIPT_DIR/verify-stage3.mjs"
node "$SCRIPT_DIR/verify-stage33.mjs"
node "$SCRIPT_DIR/verify-stage34-35.mjs"
[[ -f pnpm-lock.yaml && -f src-tauri/Cargo.lock ]] || {
  printf 'ERROR: committed pnpm and Cargo lockfiles are required.\n' >&2
  exit 1
}
prepare_cargo_target "$ROOT_DIR"
printf 'Cargo parallel jobs: %s\n' "$CARGO_BUILD_JOBS"

step "Test policy and repository script syntax"
node "$SCRIPT_DIR/verify-test-policy.mjs"
bash "$SCRIPT_DIR/verify-script-syntax.sh"

step "Frozen JavaScript installation and dependency verification"
pnpm install --frozen-lockfile
pnpm run verify:versions
[[ "$(pnpm exec esbuild --version)" == "$ESBUILD_VERSION" ]]

step "React tests"
pnpm test

step "Frontend type checking"
pnpm exec tsc -b --pretty false

step "Rust formatting"
cargo fmt --manifest-path src-tauri/Cargo.toml --all -- --check

step "Rust compile check"
cargo check --manifest-path src-tauri/Cargo.toml --locked --all-targets --all-features

trap - ERR
printf '\n\033[1;32mFast ostudy checks passed.\033[0m\n'
