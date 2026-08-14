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
export OSTUDY_ROOT_DIR="$ROOT_DIR"
export PATH="$HOME/.local/bin:$HOME/.cargo/bin:$PATH"
export CARGO_BUILD_JOBS="${CARGO_BUILD_JOBS:-$(nproc)}"
cd "$ROOT_DIR"

CURRENT_STEP="initialization"
step() { CURRENT_STEP="$*"; printf '\n\033[1;34m==> %s\033[0m\n' "$CURRENT_STEP"; }
trap 'status=$?; printf "\n\033[1;31mFAST CHECK FAILED during: %s (exit %s)\033[0m\n" "$CURRENT_STEP" "$status" >&2; exit "$status"' ERR

if [[ "${OSTUDY_FORCE_CHECK:-0}" != "1" ]] \
  && ostudy_javascript_dependencies_present \
  && ostudy_check_stamp_is_current fast; then
  printf '\n\033[1;32mREUSE Fast qualification already passed for this exact source tree.\033[0m\n'
  exit 0
fi

if [[ "${OSTUDY_STATIC_CHECK_PASSED:-0}" == "1" ]] \
  || { ostudy_javascript_dependencies_present && ostudy_check_stamp_is_current static; }; then
  printf '\n\033[1;32mREUSE Static qualification already passed for this exact source tree.\033[0m\n'
else
  bash "$SCRIPT_DIR/check-static.sh"
fi

step "Provision audited PDFium runtime"
"$SCRIPT_DIR/install-native-deps.sh" --pdfium-only

prepare_cargo_target "$ROOT_DIR"
printf 'Cargo target: %s\n' "$CARGO_TARGET_DIR"
printf 'Cargo parallel jobs: %s\n' "$CARGO_BUILD_JOBS"

step "React tests"
pnpm test

step "Rust compile check"
cargo check --manifest-path src-tauri/Cargo.toml --locked --all-targets --all-features

step "External file transfer and recovery contracts"
cargo test \
  --manifest-path src-tauri/Cargo.toml \
  --locked \
  --all-features \
  --lib \
  external_file_

step "Document domain and persistence contracts"
cargo test \
  --manifest-path src-tauri/Cargo.toml \
  --locked \
  --all-features \
  --lib \
  document_

step "PDF extraction, rendering, and OCR contracts"
cargo test \
  --manifest-path src-tauri/Cargo.toml \
  --locked \
  --all-features \
  --lib \
  pdf_

step "Latest-schema migration contract"
cargo test \
  --manifest-path src-tauri/Cargo.toml \
  --locked \
  --all-features \
  --lib \
  database::tests::migrations_and_repairs_are_idempotent_and_reach_latest_schema \
  -- --exact

step "Forward-upgrade integrity contract"
cargo test \
  --manifest-path src-tauri/Cargo.toml \
  --locked \
  --all-features \
  --lib \
  database::tests::released_schema_upgrades_forward_without_losing_integrity \
  -- --exact

ostudy_write_check_stamp fast
trap - ERR
printf '\n\033[1;32mFast ostudy checks passed.\033[0m\n'
