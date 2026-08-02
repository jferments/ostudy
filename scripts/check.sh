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

step "Frozen JavaScript installation and dependency verification"
pnpm install --frozen-lockfile
pnpm run verify:versions
[[ "$(pnpm exec esbuild --version)" == "$ESBUILD_VERSION" ]]

step "React tests (Vitest + Testing Library)"
pnpm test

step "Rust formatting"
cargo fmt --manifest-path src-tauri/Cargo.toml --all -- --check

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
  printf '\nSKIP Native WebDriver smoke test (OSTUDY_SKIP_NATIVE_SMOKE=1).\n'
else
  step "Native Tauri/WebDriver smoke test"
  "$SCRIPT_DIR/smoke-native.sh"
fi

trap - ERR
printf '\n\033[1;32mAll ostudy checks passed.\033[0m\n'
printf 'The current standalone build is ready for ./run-app.sh.\n'
