#!/usr/bin/env bash
set -Eeuo pipefail

ROOT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck disable=SC1091
source "$ROOT_DIR/versions.env"
export PATH="$HOME/.local/bin:$HOME/.cargo/bin:$PATH"
cd "$ROOT_DIR"

log() { printf '\n\033[1;34m==> %s\033[0m\n' "$*"; }
die() { printf '\n\033[1;31mERROR: %s\033[0m\n' "$*" >&2; exit 1; }
on_error() {
  local status=$?
  local line="${BASH_LINENO[0]:-${LINENO:-unknown}}"
  printf "\n\033[1;31mStartup failed near line %s (exit %s).\033[0m\n" "$line" "$status" >&2
  exit "$status"
}
trap on_error ERR

log "Checking the pinned toolchain and project manifests"
"$ROOT_DIR/scripts/doctor.sh"
node "$ROOT_DIR/scripts/verify-project-manifests.mjs"

# This is the application launcher, not the release-quality gate. It performs
# the real frontend and Rust builds required to open the program. Formatting,
# Clippy, and the complete test suite live in scripts/check.sh so a test or
# style-maintenance mistake cannot make the ordinary launcher unusable.
if [[ -f pnpm-lock.yaml ]]; then
  log "Installing JavaScript dependencies from the frozen lockfile"
  pnpm install --frozen-lockfile
else
  log "Resolving the JavaScript graph from exact direct pins"
  pnpm install --no-frozen-lockfile
  pnpm install --frozen-lockfile
fi

log "Verifying the installed JavaScript/Tiptap dependency graph"
pnpm run verify:versions
[[ "$(pnpm exec esbuild --version)" == "$ESBUILD_VERSION" ]] || \
  die "Expected esbuild $ESBUILD_VERSION, found $(pnpm exec esbuild --version)."

log "Building the complete React/Tiptap frontend"
pnpm run build

if [[ ! -f src-tauri/Cargo.lock ]]; then
  log "Resolving the Rust graph from exact crate pins"
  cargo generate-lockfile --manifest-path src-tauri/Cargo.toml
fi

log "Compiling and linking the Rust/Tauri application from Cargo.lock"
cargo build --manifest-path src-tauri/Cargo.toml --locked
"$ROOT_DIR/scripts/verify-rust-dependencies.sh"

trap - ERR
log "Starting Study App"
printf 'The terminal remains attached in development mode. Press Ctrl+C to stop the app.\n\n'
exec pnpm tauri dev
