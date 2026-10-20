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
# shellcheck disable=SC1091
source "$SCRIPT_DIR/lib/qualification-timing.bash"
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
ostudy_timing_init fast

WORKTREE_BASELINE="$(mktemp -t ostudy-fast-worktree.XXXXXXXX)"
cleanup_worktree_baseline() { rm -f -- "$WORKTREE_BASELINE"; }
trap cleanup_worktree_baseline EXIT
ostudy_reset_worktree_baseline "$WORKTREE_BASELINE"

CURRENT_STEP="initialization"
step() {
  if [[ "$CURRENT_STEP" != initialization ]]; then
    ostudy_assert_worktree_unchanged "$WORKTREE_BASELINE" "$CURRENT_STEP"
    ostudy_timing_finish_step passed
  fi
  CURRENT_STEP="$*"
  ostudy_reset_worktree_baseline "$WORKTREE_BASELINE"
  ostudy_timing_start_step "$CURRENT_STEP"
  printf '\n\033[1;34m==> %s\033[0m\n' "$CURRENT_STEP"
}
trap 'status=$?; ostudy_timing_finish_step failed; ostudy_timing_finish_total failed; ostudy_assert_worktree_unchanged "$WORKTREE_BASELINE" "$CURRENT_STEP" || true; printf "\n\033[1;31mFAST CHECK FAILED during: %s (exit %s)\033[0m\n" "$CURRENT_STEP" "$status" >&2; ostudy_capture_failure_artifacts "$CURRENT_STEP" "$status"; exit "$status"' ERR

if [[ "${OSTUDY_FORCE_CHECK:-0}" != "1" ]] \
  && ostudy_frontend_prerequisites_present \
  && ostudy_check_stamp_is_current fast; then
  printf '\n\033[1;32mREUSE Fast qualification already passed for this exact source tree.\033[0m\n'
  ostudy_timing_finish_total reused
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

step "Rust warnings-as-errors lint gate"
cargo clippy --manifest-path src-tauri/Cargo.toml --locked --all-targets --all-features -- -D warnings

DURABILITY_CONTRACTS=(util::durable_fs::tests:: ui_checkpoint::tests:: studydeck::import::)
EXECUTION_CONTRACTS=(database::entry_tests:: current_format_tests:: library_session::execution::tests:: database::startup_validation_tests:: startup::upgrade:: startup::upgrade_dialog:: library_backup::activation::checkpoint::)
STAGING_CONTRACTS=(
  database::release::staging::tests::
  database::release::h09_tests::low_space_staging_failure_preserves_the_live_legacy_library
  database::release::h09_tests::abandoned_disk_candidate_never_changes_live_lineage
  database::release::h09_tests::disk_conversion_preserves_sibling_managed_files_and_cleans_candidate
  database::release::tests::schema30_conversion_preserves_all_nonledger_values
)
IMAGE_SET_CONTRACTS=(documents::image_sets:: library_backup::image_set_backup_tests::)

# check.sh sets this when its whole native suite (`cargo test --all-features`)
# runs next: that suite runs every native test below, so running them here too
# only doubled the full gate. Keep the contract helper's guard that each
# selection still lists tests and ignores none, and let check.sh write the fast
# stamp once its suite passes.
if [[ "${OSTUDY_FULL_NATIVE_SUITE_FOLLOWS:-0}" == "1" ]]; then
  step "Native contract selections (the full native suite runs them)"
  OSTUDY_NATIVE_CONTRACTS_LIST_ONLY=1 bash "$SCRIPT_DIR/test-native-contracts.sh" \
    "${DURABILITY_CONTRACTS[@]}" "${EXECUTION_CONTRACTS[@]}" "${STAGING_CONTRACTS[@]}" "${IMAGE_SET_CONTRACTS[@]}"
  ostudy_assert_worktree_unchanged "$WORKTREE_BASELINE" "$CURRENT_STEP"
  ostudy_timing_finish_step passed
  ostudy_timing_finish_total passed
  exit 0
fi

step "Native IPC wire contract and serialization fixtures"
cargo test \
  --manifest-path src-tauri/Cargo.toml \
  --locked \
  --all-features \
  --lib \
  ipc_wire_contract::tests::

step "Native database migration metadata contract"
cargo test \
  --manifest-path src-tauri/Cargo.toml \
  --locked \
  --all-features \
  --lib \
  migration_contract::tests::

step "Filesystem durability and checkpoint publication contracts"
bash "$SCRIPT_DIR/test-native-contracts.sh" "${DURABILITY_CONTRACTS[@]}"

step "StudyTarget composition parallel-isolation regression"
cargo test \
  --manifest-path src-tauri/Cargo.toml \
  --locked \
  --all-features \
  --lib \
  study_materials::composition::tests::

step "Current-format entry, preservation and native execution contracts"
bash "$SCRIPT_DIR/test-native-contracts.sh" "${EXECUTION_CONTRACTS[@]}"

step "H-09 disk-backed conversion staging contracts"
bash "$SCRIPT_DIR/test-native-contracts.sh" "${STAGING_CONTRACTS[@]}"

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

step "Ordered image-set source and backup contracts"
bash "$SCRIPT_DIR/test-native-contracts.sh" "${IMAGE_SET_CONTRACTS[@]}"

step "Tutor, shared knowledge, privacy, and managed-media portability contracts"
for test_filter in tutor:: knowledge:: task_activity:: task_cancellation:: device_sync::scope:: device_sync::package:: library_backup::media_backup_tests library_backup::tutor_backup_tests; do
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

step "Native rich-document compatibility"
cargo test \
  --manifest-path src-tauri/Cargo.toml \
  --locked \
  --all-features \
  --lib \
  cards::

step "Interactive question authoring and migration preservation"
for test_filter in authoring::card_commands::interactive:: database::interactive_migration_tests:: practice::question_attempts:: asset_lifecycle::question_attempt_assets_keep_images_referenced asset_lifecycle::occlusion::; do
  cargo test \
    --manifest-path src-tauri/Cargo.toml \
    --locked \
    --all-features \
    --lib \
    "$test_filter"
done

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

step "H-09 startup and conversion measurement"
cargo test \
  --manifest-path src-tauri/Cargo.toml \
  --locked \
  --all-features \
  --lib \
  database::release::h09_tests::representative_h09_startup_and_conversion_measurement \
  -- --exact --nocapture

ostudy_assert_worktree_unchanged "$WORKTREE_BASELINE" "$CURRENT_STEP"
ostudy_timing_finish_step passed
ostudy_write_check_stamp fast
ostudy_timing_finish_total passed
trap - ERR
printf '\n\033[1;32mFast ostudy checks passed.\033[0m\n'
