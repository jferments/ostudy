#!/usr/bin/env bash
# Shared qualification commands. This file is intentionally option-neutral so it
# can be sourced by fail-fast qualification scripts and the multi-failure
# diagnostic runner without changing the caller's shell behavior.

OSTUDY_QUALIFICATION_SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
OSTUDY_QUALIFICATION_ROOT="$(cd -- "$OSTUDY_QUALIFICATION_SCRIPT_DIR/.." && pwd)"
# shellcheck disable=SC1091
source "$OSTUDY_QUALIFICATION_ROOT/versions.env"

ostudy_gate_doctor() { "$OSTUDY_QUALIFICATION_SCRIPT_DIR/doctor.sh"; }
ostudy_gate_project_manifests() { node "$OSTUDY_QUALIFICATION_SCRIPT_DIR/verify-project-manifests.mjs"; }
ostudy_gate_ui_token_generation() { node "$OSTUDY_QUALIFICATION_SCRIPT_DIR/generate-ui-tokens.mjs" --check; }
ostudy_gate_ui_token_usage() { node "$OSTUDY_QUALIFICATION_SCRIPT_DIR/verify-ui-token-usage.mjs"; }
ostudy_gate_ipc_contract() { node "$OSTUDY_QUALIFICATION_SCRIPT_DIR/verify-ipc-contract.mjs"; }
ostudy_gate_repository_policy() { node "$OSTUDY_QUALIFICATION_SCRIPT_DIR/verify-repository-policy.mjs"; }
ostudy_gate_platform_boundaries() { node "$OSTUDY_QUALIFICATION_SCRIPT_DIR/verify-platform-boundaries.mjs"; }
ostudy_gate_external_file_streaming() { node "$OSTUDY_QUALIFICATION_SCRIPT_DIR/verify-external-file-streaming.mjs"; }
ostudy_gate_documentation() { node "$OSTUDY_QUALIFICATION_SCRIPT_DIR/verify-documentation.mjs"; }
ostudy_gate_lockfiles() {
  [[ -f "$OSTUDY_QUALIFICATION_ROOT/pnpm-lock.yaml" && -f "$OSTUDY_QUALIFICATION_ROOT/src-tauri/Cargo.lock" ]] || {
    printf 'ERROR: committed pnpm and Cargo lockfiles are required.\n' >&2
    return 1
  }
}
ostudy_gate_test_policy() { node "$OSTUDY_QUALIFICATION_SCRIPT_DIR/verify-test-policy.mjs"; }
ostudy_gate_script_syntax() { bash "$OSTUDY_QUALIFICATION_SCRIPT_DIR/verify-script-syntax.sh"; }
ostudy_gate_script_tests() { python3 -m unittest discover -s "$OSTUDY_QUALIFICATION_ROOT/tests/scripts" -p 'test_*.py'; }
ostudy_gate_pnpm_install() { pnpm install --frozen-lockfile; }
ostudy_gate_js_versions() {
  pnpm run verify:versions
  [[ "$(pnpm exec esbuild --version)" == "$ESBUILD_VERSION" ]]
}
ostudy_gate_typescript() { pnpm exec tsc -b --pretty false; }
ostudy_gate_rustfmt() { cargo fmt --manifest-path "$OSTUDY_QUALIFICATION_ROOT/src-tauri/Cargo.toml" --all -- --check; }
ostudy_gate_pdfium_runtime() { "$OSTUDY_QUALIFICATION_SCRIPT_DIR/install-native-deps.sh" --pdfium-only; }
ostudy_gate_cargo_metadata_locked() {
  cargo metadata --manifest-path "$OSTUDY_QUALIFICATION_ROOT/src-tauri/Cargo.toml" --format-version 1 --locked > /dev/null
}
ostudy_gate_vitest() { pnpm test; }
ostudy_gate_cargo_check() {
  cargo check --manifest-path "$OSTUDY_QUALIFICATION_ROOT/src-tauri/Cargo.toml" --locked --all-targets --all-features
}
ostudy_gate_clippy() {
  cargo clippy --manifest-path "$OSTUDY_QUALIFICATION_ROOT/src-tauri/Cargo.toml" --locked --all-targets --all-features -- -D warnings
}
ostudy_gate_external_file_tests() {
  cargo test --manifest-path "$OSTUDY_QUALIFICATION_ROOT/src-tauri/Cargo.toml" --locked --all-features --lib external_file_
}
ostudy_gate_document_tests() {
  cargo test --manifest-path "$OSTUDY_QUALIFICATION_ROOT/src-tauri/Cargo.toml" --locked --all-features --lib document_
}
ostudy_gate_pdf_tests() {
  cargo test --manifest-path "$OSTUDY_QUALIFICATION_ROOT/src-tauri/Cargo.toml" --locked --all-features --lib pdf_
}
ostudy_gate_latest_schema_test() {
  cargo test --manifest-path "$OSTUDY_QUALIFICATION_ROOT/src-tauri/Cargo.toml" --locked --all-features --lib \
    database::tests::migrations_and_repairs_are_idempotent_and_reach_latest_schema -- --exact
}
ostudy_gate_forward_upgrade_test() {
  cargo test --manifest-path "$OSTUDY_QUALIFICATION_ROOT/src-tauri/Cargo.toml" --locked --all-features --lib \
    database::tests::released_schema_upgrades_forward_without_losing_integrity -- --exact
}
ostudy_gate_frontend_build() { pnpm run build; }
ostudy_gate_rust_tests() {
  local test_threads="${1:-}"
  if [[ -n "$test_threads" ]]; then
    cargo test --manifest-path "$OSTUDY_QUALIFICATION_ROOT/src-tauri/Cargo.toml" --locked --all-features -- --test-threads "$test_threads"
  else
    cargo test --manifest-path "$OSTUDY_QUALIFICATION_ROOT/src-tauri/Cargo.toml" --locked --all-features
  fi
}
