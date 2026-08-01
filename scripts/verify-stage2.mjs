import { createHash } from "node:crypto";
import { readFile } from "node:fs/promises";
import { join } from "node:path";
import { fileURLToPath } from "node:url";

const root = fileURLToPath(new URL("..", import.meta.url));
const read = (...parts) => readFile(join(root, ...parts), "utf8");
const readJoined = async (...paths) =>
  (await Promise.all(paths.map((parts) => read(...parts)))).join("\n");

const [
  migration,
  database,
  rustCommands,
  authoring,
  ipc,
  app,
  cardsScreen,
  editor,
  taggedTransfer,
] = await Promise.all([
  read("src-tauri", "migrations", "0004_card_organization.sql"),
  read("src-tauri", "src", "database.rs"),
  read("src-tauri", "src", "lib.rs"),
  readJoined(
    ["src-tauri", "src", "authoring.rs"],
    ["src-tauri", "src", "authoring", "types.rs"],
    ["src-tauri", "src", "authoring", "commands.rs"],
    ["src-tauri", "src", "authoring", "search_and_tags.rs"],
    ["src-tauri", "src", "authoring", "writes_and_transfer.rs"],
    ["src-tauri", "src", "authoring", "normalization.rs"],
  ),
  read("src", "ipc.ts"),
  read("src", "App.tsx"),
  read("src", "screens", "CardsScreen.tsx"),
  read("src", "screens", "CardEditorScreen.tsx"),
  readJoined(
    ["src-tauri", "src", "tagged_transfer.rs"],
    ["src-tauri", "src", "tagged_transfer", "types_and_commands.rs"],
    ["src-tauri", "src", "tagged_transfer", "export.rs"],
    ["src-tauri", "src", "tagged_transfer", "import.rs"],
  ),
]);

const failures = [];
const checksum = createHash("sha256").update(migration).digest("hex");

if (!database.includes(`checksum: "${checksum}"`)) {
  failures.push(`database.rs checksum mismatch for migration 0004: ${checksum}`);
}
if (!database.includes("LATEST_SCHEMA_VERSION: i64 = 4")) {
  failures.push("database schema version was not advanced to 4");
}
for (const fragment of [
  "CREATE TABLE tags",
  "CREATE TABLE note_tags",
  "CREATE VIRTUAL TABLE card_search_fts USING fts5",
  "duplicate_hash",
]) {
  if (!migration.includes(fragment)) failures.push(`migration 0004 missing: ${fragment}`);
}

for (const command of [
  "list_tags",
  "search_cards",
  "get_organized_card",
  "create_organized_card",
  "update_organized_card",
  "find_duplicate_cards",
  "bulk_create_cards",
  "transfer_cards",
]) {
  if (!rustCommands.includes(command)) failures.push(`Rust command not registered: ${command}`);
  if (!ipc.includes(`"${command}"`)) failures.push(`frontend command missing: ${command}`);
}

for (const fragment of [
  "bm25(card_search_fts)",
  "MAX_BULK_ROWS",
  "TransferMode::Move",
  "TransferMode::Copy",
  "skip_duplicates",
  "backfill_duplicate_hashes",
]) {
  if (!authoring.includes(fragment)) failures.push(`authoring boundary missing: ${fragment}`);
}

if (!app.includes('label="Cards"') || !app.includes("<CardsScreen")) {
  failures.push("Cards library is not registered in top-level navigation");
}
for (const fragment of [
  "Bulk add",
  "Move / copy",
  "Select visible",
  "aria-multiselectable",
  "tagIds",
]) {
  if (!cardsScreen.includes(fragment)) failures.push(`Cards screen missing: ${fragment}`);
}
for (const fragment of [
  "<TagInput",
  "findDuplicateCards",
  "Save duplicate anyway",
]) {
  if (!editor.includes(fragment)) failures.push(`card editor missing: ${fragment}`);
}
if (
  !taggedTransfer.includes("CREATE TABLE IF NOT EXISTS tags") ||
  !taggedTransfer.includes("apply_imported_tags")
) {
  failures.push("deck packages do not preserve optional tag metadata");
}

if (failures.length > 0) {
  console.error("Stage 2 verification failed:");
  for (const failure of failures) console.error(`  - ${failure}`);
  process.exit(1);
}

console.log(
  "OK   Stage 2 tags, FTS5 search, bulk entry, duplicate checks, and move/copy are wired.",
);
