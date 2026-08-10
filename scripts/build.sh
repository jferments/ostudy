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

log() { printf '\n\033[1;34m==> %s\033[0m\n' "$*"; }
die() { printf '\n\033[1;31mERROR: %s\033[0m\n' "$*" >&2; exit 1; }
trap 'printf "\n\033[1;31mBuild failed near line %s.\033[0m\n" "${BASH_LINENO[0]:-${LINENO:-unknown}}" >&2' ERR

[[ -f pnpm-lock.yaml && -f src-tauri/Cargo.lock ]] || \
  die "Committed pnpm and Cargo lockfiles are required."

log "Checking the pinned development environment"
"$SCRIPT_DIR/doctor.sh"
node "$SCRIPT_DIR/verify-project-manifests.mjs"
prepare_cargo_target "$ROOT_DIR"

log "Installing JavaScript dependencies from pnpm-lock.yaml"
pnpm install --frozen-lockfile
pnpm run verify:versions
[[ "$(pnpm exec esbuild --version)" == "$ESBUILD_VERSION" ]] || \
  die "Expected esbuild $ESBUILD_VERSION, found $(pnpm exec esbuild --version)."

log "Preparing pinned PDFium $PDFIUM_BUILD through the locked Rust dependency graph"
printf 'pdfium-bundled %s will reuse its local build cache or provision/embed the target-specific PDFium runtime during Cargo compilation.\n' "$PDFIUM_BUNDLED_VERSION"

log "Building the standalone ostudy application with $CARGO_BUILD_JOBS Cargo jobs"
# Tauri forwards arguments after `--` to Cargo. `--locked` makes a stale or
# inconsistent Cargo.lock fail explicitly instead of silently rewriting it.
pnpm tauri build --debug --no-bundle --ci -- --locked
"$SCRIPT_DIR/verify-rust-dependencies.sh"

[[ -x "$OSTUDY_BINARY" ]] || die "Expected application binary was not created at $OSTUDY_BINARY."
ostudy_write_build_stamp "$(node -p "require('./package.json').version")"

printf '\nBuild complete: %s\n' "$OSTUDY_BINARY"
printf 'Start it with: ./run-app.sh\n'
