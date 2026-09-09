#!/usr/bin/env bash
set -Eeuo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd -- "$SCRIPT_DIR/.." && pwd)"
# shellcheck disable=SC1091
source "$ROOT_DIR/versions.env"
# shellcheck disable=SC1091
source "$SCRIPT_DIR/lib/node-toolchain.bash"
ostudy_activate_node_toolchain
# shellcheck disable=SC1091
source "$SCRIPT_DIR/check-state.sh"
# shellcheck disable=SC1091
source "$SCRIPT_DIR/cargo-target.sh"
# shellcheck disable=SC1091
source "$SCRIPT_DIR/lib/diagnostics.bash"
export OSTUDY_ROOT_DIR="$ROOT_DIR"
export OSTUDY_QUALIFICATION_COMMAND="./scripts/check-fast.sh"
export CARGO_BUILD_JOBS="${CARGO_BUILD_JOBS:-$(nproc)}"
ostudy_parse_diagnostic_args "$@"
if (( ${#OSTUDY_DIAGNOSTIC_REMAINDER[@]} > 0 )); then
  printf 'Usage: ./scripts/check-fast.sh [diagnostic options]\n' >&2
  ostudy_diagnostic_usage >&2
  exit 2
fi
cd "$ROOT_DIR"

WORKTREE_BASELINE="$(mktemp -t ostudy-fast-worktree.XXXXXXXX)"
cleanup_worktree_baseline() { rm -f -- "$WORKTREE_BASELINE"; }
trap cleanup_worktree_baseline EXIT
ostudy_reset_worktree_baseline "$WORKTREE_BASELINE"

CURRENT_STEP="initialization"
step() {
  if [[ "$CURRENT_STEP" != initialization ]]; then
    ostudy_assert_worktree_unchanged "$WORKTREE_BASELINE" "$CURRENT_STEP"
  fi
  CURRENT_STEP="$*"
  ostudy_reset_worktree_baseline "$WORKTREE_BASELINE"
  printf '\n\033[1;34m==> %s\033[0m\n' "$CURRENT_STEP"
}
trap 'status=$?; ostudy_assert_worktree_unchanged "$WORKTREE_BASELINE" "$CURRENT_STEP" || true; printf "\n\033[1;31mFAST CHECK FAILED during: %s (exit %s)\033[0m\n" "$CURRENT_STEP" "$status" >&2; ostudy_capture_failure_artifacts "$CURRENT_STEP" "$status"; exit "$status"' ERR

if [[ "${OSTUDY_FORCE_CHECK:-0}" != "1" ]] \
  && ostudy_frontend_prerequisites_present \
  && ostudy_check_stamp_is_current fast; then
  printf '\n\033[1;32mREUSE Fast qualification already passed for this exact source tree.\033[0m\n'
  exit 0
fi

step "Static qualification"
if [[ "${OSTUDY_STATIC_CHECK_PASSED:-0}" == "1" ]] \
  || { ostudy_frontend_prerequisites_present && ostudy_check_stamp_is_current static; }; then
  printf '\n\033[1;32mREUSE Static qualification already passed for this exact source tree.\033[0m\n'
else
  STATIC_ARGS=()
  if [[ "${OSTUDY_DIAGNOSTICS_ACTIVE:-0}" == 1 && "${OSTUDY_DIAGNOSTICS_AUTOMATIC:-0}" != 1 ]]; then
    STATIC_ARGS+=("--log-filter=$OSTUDY_LOG")
  fi
  if [[ "${OSTUDY_KEEP_FAILURE_ARTIFACTS:-0}" == 1 ]]; then
    STATIC_ARGS+=(--keep-artifacts)
  fi
  bash "$SCRIPT_DIR/check-static.sh" "${STATIC_ARGS[@]}"
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

step "Rust warnings-as-errors lint gate"
cargo clippy --manifest-path src-tauri/Cargo.toml --locked --all-targets --all-features -- -D warnings

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

step "Tutor, shared knowledge, and privacy contracts"
for test_filter in tutor:: knowledge:: device_sync::privacy_tests library_backup::tutor_backup_tests; do
  cargo test \
    --manifest-path src-tauri/Cargo.toml \
    --locked \
    --all-features \
    --lib \
    "$test_filter"
done

step "Stage 10 rich-content and document-lifecycle regressions"
cargo test \
  --manifest-path src-tauri/Cargo.toml \
  --locked \
  --all-features \
  --lib \
  cards::content::tests::accepts_versioned_tables_tasks_and_subscripts \
  -- --exact
cargo test \
  --manifest-path src-tauri/Cargo.toml \
  --locked \
  --all-features \
  --lib \
  documents::lifecycle::tests::startup_interruption_preserves_ready_state_for_a_guide_only_document \
  -- --exact

step "Document metadata migration regression"
cargo test \
  --manifest-path src-tauri/Cargo.toml \
  --locked \
  --all-features \
  --test document_artifact_metadata_migration7

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
  database::tests::latest_schema_is_idempotent_and_public_baseline_fixture_is_immutable \
  -- --exact

step "Anki APKG staging migration regression"
cargo test \
  --manifest-path src-tauri/Cargo.toml \
  --locked \
  --all-features \
  --lib \
  database::tests::anki_apkg_migration_preserves_existing_staging_rows_and_accepts_apkg \
  -- --exact

step "Public migration immutability contract"
cargo test \
  --manifest-path src-tauri/Cargo.toml \
  --locked \
  --all-features \
  --lib \
  database::tests::public_migration_checksum_and_future_version_are_enforced \
  -- --exact

ostudy_assert_worktree_unchanged "$WORKTREE_BASELINE" "$CURRENT_STEP"
ostudy_write_check_stamp fast
trap - ERR
printf '\n\033[1;32mFast ostudy checks passed.\033[0m\n'