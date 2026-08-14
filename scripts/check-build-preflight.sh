#!/usr/bin/env bash
set -Eeuo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd -- "$SCRIPT_DIR/.." && pwd)"
# shellcheck disable=SC1091
source "$SCRIPT_DIR/cargo-target.sh"

export PATH="$HOME/.local/bin:$HOME/.cargo/bin:$PATH"
export OSTUDY_ROOT_DIR="$ROOT_DIR"
cd "$ROOT_DIR"

log() { printf '\n\033[1;34m==> %s\033[0m\n' "$*"; }
die() { printf '\n\033[1;31mERROR: %s\033[0m\n' "$*" >&2; exit 1; }

[[ -f pnpm-lock.yaml && -f src-tauri/Cargo.lock ]] || \
  die "Committed pnpm and Cargo lockfiles are required."

prepare_cargo_target "$ROOT_DIR"

log "Verifying project manifests and the Tauri IPC boundary"
node "$SCRIPT_DIR/verify-project-manifests.mjs"
node "$SCRIPT_DIR/verify-ipc-contract.mjs"
node "$SCRIPT_DIR/verify-platform-boundaries.mjs"

log "Checking Rust formatting before compilation"
cargo fmt --manifest-path src-tauri/Cargo.toml --all -- --check

log "Type-checking the shared frontend"
pnpm exec tsc -b --pretty false

log "Verifying that Cargo.lock exactly matches the Rust dependency graph"
cargo metadata \
  --manifest-path src-tauri/Cargo.toml \
  --locked \
  --format-version 1 >/dev/null

log "Compile-checking and linting all Rust targets and features with warnings denied"
cargo clippy \
  --manifest-path src-tauri/Cargo.toml \
  --locked \
  --all-targets \
  --all-features \
  -- \
  -D warnings

printf '\nOK   Build preflight passed with canonical Rust formatting and a warning-clean Rust tree.\n'
