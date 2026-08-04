import { createHash } from "node:crypto";
import { readFile } from "node:fs/promises";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const read = (...parts) => readFile(join(root, ...parts), "utf8");

const [
  migration,
  database,
  rustRoot,
  assetLifecycle,
  transferLifecycle,
  transferWrapper,
  templateCommands,
  deletion,
  deletionCommands,
  ipc,
  cardEditor,
  templateEditor,
  deckScreen,
  bulkDelete,
  toast,
  main,
  managedStyles,
] = await Promise.all([
  read("src-tauri", "migrations", "0011_managed_asset_lifecycle.sql"),
  read("src-tauri", "src", "database.rs"),
  read("src-tauri", "src", "lib.rs"),
  read("src-tauri", "src", "asset_lifecycle.rs"),
  read("src-tauri", "src", "transfer_lifecycle.rs"),
  read("src-tauri", "src", "authoring", "stage3_v2", "transfer_wrapper.rs"),
  read("src-tauri", "src", "authoring", "stage3_v2", "types_commands.rs"),
  read("src-tauri", "src", "deletion.rs"),
  read("src-tauri", "src", "deletion_commands.rs"),
  read("src", "ipc.ts"),
  read("src", "screens", "CardEditorScreen.tsx"),
  read("src", "screens", "TemplatesScreen.tsx"),
  read("src", "screens", "DecksScreen.tsx"),
  read("src", "screens", "cards", "DeleteCardsModal.tsx"),
  read("src", "components", "Toast.tsx"),
  read("src", "main.tsx"),
  read("src", "managed-assets.css"),
]);

const failures = [];
const requireText = (source, needle, label) => {
  if (!source.includes(needle)) failures.push(`${label}: missing ${needle}`);
};

const checksum = createHash("sha256").update(migration).digest("hex");
requireText(database, 'version: 11', "schema registration");
requireText(database, 'name: "managed_asset_lifecycle"', "schema registration");
requireText(database, `checksum: "${checksum}"`, "schema checksum");
requireText(database, "const LATEST_SCHEMA_VERSION: i64 = 11", "latest schema");

for (const state of ["staged", "ready", "missing", "quarantined", "deleted"]) {
  requireText(migration, `'${state}'`, "asset lifecycle schema");
}
for (const table of ["asset_drafts", "asset_reconciliation_events", "deletion_operations"]) {
  requireText(migration, `CREATE TABLE ${table}`, "Stage 3.3 schema");
}

for (const required of [
  "stage_source_in_connection",
  "sync_all",
  "fs::rename",
  "reconcile_in_connection",
  "quarantine_unknown_files",
  "reference_count",
  "retire_unused_draft_assets",
  "rollback_promoted_files",
  "verify_referenced_asset",
  "ordinary_multi_asset_failure_returns_prior_files_to_staging",
  "corrupt_referenced_file_is_marked_missing_without_blocking_startup",
]) {
  requireText(assetLifecycle, required, "managed asset implementation");
}

for (const moduleName of ["mod asset_lifecycle;", "mod deletion;", "mod deletion_commands;", "mod transfer_lifecycle;"]) {
  requireText(rustRoot, moduleName, "native module registration");
}
for (const command of [
  "asset_lifecycle::begin_asset_draft",
  "asset_lifecycle::cancel_asset_draft",
  "asset_lifecycle::sync_asset_draft",
  "assets::import_image",
  "deletion::undo_deletion",
]) {
  requireText(rustRoot, command, "native command registration");
}

for (const required of ["begin_import", "finish_import", "abort_import"]) {
  requireText(transferLifecycle, required, "managed import journal");
  requireText(transferWrapper, `transfer_lifecycle::${required}`, "managed import wiring");
}
requireText(
  transferWrapper,
  "asset_import_finalize_failed",
  "committed import success boundary",
);

for (const required of [
  "finalize_assets_after_committed_write",
  "committed_asset_finalization_failed",
  "committed_template_refresh_failed",
]) {
  requireText(templateCommands, required, "committed template success boundary");
}

for (const required of [
  "DELETION_UNDO_WINDOW_MS",
  "deletion_undo_expired",
  "deletion_undo_conflict",
  "restore_exact_cards",
  "finalize_expired_deletions",
]) {
  requireText(deletion, required, "deletion recovery");
}
for (const command of ["delete_card_with_undo", "delete_cards_with_undo", "delete_deck_with_undo"]) {
  requireText(deletionCommands, `pub fn ${command}`, "deletion command boundary");
}

for (const command of [
  '"begin_asset_draft"',
  '"cancel_asset_draft"',
  '"sync_asset_draft"',
  '"import_image"',
  '"undo_deletion"',
]) {
  requireText(ipc, command, "frontend IPC");
}
requireText(ipc, "input: { filePath, draftId }", "draft-scoped image IPC");

for (const editor of [cardEditor, templateEditor]) {
  requireText(editor, "beginAssetDraft", "editor draft lifecycle");
  requireText(editor, "cancelAssetDraft", "editor draft lifecycle");
  requireText(editor, "syncAssetDraft", "editor draft lifecycle");
  requireText(editor, "assetDraftId", "editor durable save");
}

for (const deletionUi of [deckScreen, bulkDelete]) {
  requireText(deletionUi, "undoDeletion", "deletion Undo UI");
  requireText(deletionUi, 'label: "Undo"', "deletion Undo toast");
  requireText(deletionUi, "undoExpiresAt", "deletion Undo expiry");
}
requireText(toast, "toast-action", "actionable toast");
requireText(toast, "Restoring…", "actionable toast progress");
requireText(main, '"./managed-assets.css"', "managed asset styles");
requireText(managedStyles, ".toast .toast-action", "link-like Undo action");
requireText(managedStyles, "text-decoration: underline", "link-like Undo action");

if (failures.length > 0) {
  console.error("Stage 3.3 verification failed:");
  for (const failure of failures) console.error(`  - ${failure}`);
  process.exit(1);
}

console.log(
  "OK   Stage 3.3 managed assets, compensation, crash reconciliation, import journaling, committed-success boundaries, and exact deletion Undo are wired across schema, native commands, editors, and toasts.",
);
