#!/usr/bin/env bash
# Verify that every high-value fast-contract family still selects real Rust
# tests. Execution is handled once by check-fast.sh's complete library suite.
set -Eeuo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd -- "$SCRIPT_DIR/.." && pwd)"
LIST_FILE="$(mktemp -t ostudy-fast-native-list.XXXXXXXX)"
trap 'rm -f -- "$LIST_FILE"' EXIT

filters=(
  "database::entry_tests::"
  "current_format_tests::"
  "external_file_"
  "document_"
  "documents::image_sets::"
  "library_backup::image_set_backup_tests::"
  "tutor::"
  "knowledge::"
  "task_activity::"
  "task_cancellation::"
  "device_sync::scope::"
  "device_sync::package::"
  "library_backup::media_backup_tests"
  "library_backup::tutor_backup_tests"
  "cards::content::tests::accepts_versioned_tables_tasks_and_subscripts"
  "documents::lifecycle::tests::startup_interruption_preserves_ready_state_for_a_guide_only_document"
  "cards::"
  "authoring::card_commands::interactive::"
  "database::interactive_migration_tests::"
  "practice::question_attempts::"
  "asset_lifecycle::question_attempt_assets_keep_images_referenced"
  "asset_lifecycle::occlusion::"
  "pdf_"
  "database::tests::latest_schema_is_idempotent_and_public_baseline_fixture_is_immutable"
  "database::tests::anki_apkg_migration_preserves_existing_staging_rows_and_accepts_apkg"
  "database::tests::public_migration_checksum_and_future_version_are_enforced"
)

cargo test \
  --manifest-path "$ROOT_DIR/src-tauri/Cargo.toml" \
  --locked \
  --all-features \
  --lib \
  -- \
  --list \
  --color never > "$LIST_FILE"

total="$(awk '/: test$/ {n++} END {print n+0}' "$LIST_FILE")"
(( total > 0 )) || {
  printf 'ERROR: Rust library test inventory is empty.\n' >&2
  exit 1
}

for filter in "${filters[@]}"; do
  count="$(awk -v needle="$filter" '/: test$/ && index($0, needle) > 0 {n++} END {print n+0}' "$LIST_FILE")"
  if (( count == 0 )); then
    printf 'ERROR: fast native contract filter matched no tests: %s\n' "$filter" >&2
    exit 1
  fi
  printf 'CONTRACT %-94s %s test(s)\n' "$filter" "$count"
done

printf 'PASS Fast native inventory: %s library tests listed; every critical contract family is present.\n' "$total"
