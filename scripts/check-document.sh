#!/usr/bin/env bash
set -Eeuo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd -- "$SCRIPT_DIR/.." && pwd)"
# shellcheck disable=SC1091
source "$SCRIPT_DIR/cargo-target.sh"
export OSTUDY_ROOT_DIR="$ROOT_DIR"
export PATH="$HOME/.local/bin:$HOME/.cargo/bin:$PATH"
export CARGO_BUILD_JOBS="${CARGO_BUILD_JOBS:-$(nproc)}"
cd "$ROOT_DIR"

prepare_cargo_target "$ROOT_DIR"
printf 'Cargo target: %s\n' "$CARGO_TARGET_DIR"

printf '\n==> Document-domain, planner, checkpoint, ingestion, and persistence contracts\n'
# A Rust test filter still compiles the complete lib test target, so this catches
# stale cross-module APIs while executing the high-value document-focused tests.
cargo test \
  --manifest-path src-tauri/Cargo.toml \
  --locked \
  --all-features \
  --lib \
  document_

printf '\n==> Latest-schema migration contract\n'
cargo test \
  --manifest-path src-tauri/Cargo.toml \
  --locked \
  --all-features \
  --lib \
  database::tests::migrations_and_repairs_are_idempotent_and_reach_latest_schema \
  --exact

printf '\n==> Forward-upgrade integrity contract\n'
cargo test \
  --manifest-path src-tauri/Cargo.toml \
  --locked \
  --all-features \
  --lib \
  database::tests::released_schema_upgrades_forward_without_losing_integrity \
  --exact

printf '\nDocument contract checks passed.\n'
