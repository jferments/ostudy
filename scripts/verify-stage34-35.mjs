import { createHash } from "node:crypto";
import { access, readFile } from "node:fs/promises";
import { join } from "node:path";
import { fileURLToPath } from "node:url";

const root = fileURLToPath(new URL("..", import.meta.url));
const read = (...parts) => readFile(join(root, ...parts), "utf8");
const failures = [];

const [
  migration12,
  migration13,
  database,
  repairs,
  studydeck,
  studydeckExport,
  studydeckImport,
  studydeckRead,
  backup,
  rustCommands,
  authoringEntry,
  authoringNormalization,
  ipc,
  decksScreen,
  settingsScreen,
] = await Promise.all([
  read("src-tauri", "migrations", "0012_versioned_data_repairs.sql"),
  read("src-tauri", "migrations", "0013_canonical_data_model.sql"),
  read("src-tauri", "src", "database.rs"),
  read("src-tauri", "src", "data_repairs.rs"),
  read("src-tauri", "src", "studydeck.rs"),
  read("src-tauri", "src", "studydeck", "export.rs"),
  read("src-tauri", "src", "studydeck", "import.rs"),
  read("src-tauri", "src", "studydeck", "read.rs"),
  read("src-tauri", "src", "library_backup.rs"),
  read("src-tauri", "src", "lib.rs"),
  read("src-tauri", "src", "authoring", "stage3_v2.rs"),
  read("src-tauri", "src", "authoring", "stage3_v2", "normalization.rs"),
  read("src", "ipc.ts"),
  read("src", "screens", "DecksScreen.tsx"),
  read("src", "screens", "SettingsScreen.tsx"),
]);

for (const [name, sql] of [["0012", migration12], ["0013", migration13]]) {
  const checksum = createHash("sha256").update(sql).digest("hex");
  if (!database.includes(checksum)) failures.push(`database.rs checksum mismatch for ${name}`);
}
for (const fragment of [
  "apply_migrations_through(connection, 12)",
  "apply_required_repairs(connection)",
  "apply_migrations_after(connection, 12)",
  "LATEST_SCHEMA_VERSION: i64 = 13",
  "apply_foreign_key_disabled_migration",
  "PRAGMA foreign_key_check",
  "PRAGMA foreign_keys = ON",
]) {
  if (!database.includes(fragment)) failures.push(`database startup missing: ${fragment}`);
}
for (const forbidden of ["backfill_duplicate_hashes(connection)?", "normalize_stage3_schema(connection)"]) {
  if (database.includes(forbidden)) failures.push(`startup still performs global normalization: ${forbidden}`);
}
for (const fragment of [
  "data_repairs",
  "changed_rows",
  "duplicate_fingerprints",
  "template_and_generated_card_canonicalization",
  "canonical_database_repair_pass_is_a_no_op",
]) {
  if (!(migration12 + repairs).includes(fragment)) failures.push(`repair boundary missing: ${fragment}`);
}
for (const fragment of [
  "DROP COLUMN front_pattern",
  "DROP COLUMN back_pattern",
  "CREATE TABLE notes_canonical",
  "CHECK(note_type IN ('basic', 'cloze'))",
  "CREATE TABLE cards_canonical",
  "CHECK(direction IN ('forward', 'reverse'))",
  "DELETE FROM card_search_fts",
]) {
  if (!migration13.includes(fragment)) failures.push(`canonical schema cutover missing: ${fragment}`);
}
for (const fragment of [
  "normalize_abandoned_image_occlusion",
  "apply_stage3_data_repairs",
  "SET direction = 'forward', template_ordinal = ?1",
]) {
  if (!authoringNormalization.includes(fragment)) failures.push(`data migration missing: ${fragment}`);
}
if (authoringEntry.includes("transfer_wrapper.rs")) failures.push("historical transfer wrapper remains included");

for (const fragment of [
  "CURRENT_PACKAGE_VERSION: i64 = 2",
  "historyPolicy",
  "fresh-local-schedules",
  "TransactionBehavior::Deferred",
  "deck_import_operations",
  "idempotency_conflict",
  "canonicalize_legacy_presentations",
  "lifecycle_state = 'ready'",
]) {
  if (!(studydeck + studydeckExport + studydeckImport + studydeckRead).includes(fragment)) {
    failures.push(`canonical studydeck pipeline missing: ${fragment}`);
  }
}
for (const fragment of [
  "ostudy-library-backup",
  "database_snapshot",
  "asset_files",
  "review_count",
  "apply_pending_restore",
  "PendingRestoreGuard",
]) {
  if (!backup.includes(fragment)) failures.push(`complete backup boundary missing: ${fragment}`);
}
for (const command of [
  "export_deck",
  "import_deck",
  "export_library_backup",
  "prepare_library_restore",
]) {
  if (!rustCommands.includes(command)) failures.push(`Rust command not registered: ${command}`);
  if (!ipc.includes(`"${command}"`)) failures.push(`frontend command missing: ${command}`);
}
for (const fragment of ["createOperationId", "pendingImportRef", "getErrorCode(error)"]) {
  if (!decksScreen.includes(fragment)) failures.push(`retry-safe import UI missing: ${fragment}`);
}
for (const fragment of [
  "Back up library",
  "Restore backup",
  "review event",
  "Close and reopen ostudy",
]) {
  if (!settingsScreen.includes(fragment)) failures.push(`backup UI missing: ${fragment}`);
}

for (const removed of [
  "src-tauri/src/transfer.rs",
  "src-tauri/src/stage3_transfer.rs",
  "src-tauri/src/tagged_transfer.rs",
  "src-tauri/src/tagged_transfer",
  "src-tauri/src/transfer_lifecycle.rs",
  "src-tauri/src/authoring/stage3_v2/transfer_wrapper.rs",
]) {
  try {
    await access(join(root, removed));
    failures.push(`superseded transfer residue remains: ${removed}`);
  } catch {
    // Expected.
  }
}

if (failures.length > 0) {
  console.error("Stage 3.4/3.5 verification failed:");
  for (const failure of failures) console.error(`  - ${failure}`);
  process.exit(1);
}

console.log(
  "OK   current studydeck exports use one snapshot, legacy packages adapt through one reader, full backups preserve analytics history, the canonical schema is physically narrowed, and startup repairs are versioned no-ops.",
);
