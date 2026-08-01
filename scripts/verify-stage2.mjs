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
  cardsCore,
  practice,
  practiceSelection,
  ipc,
  app,
  cardsScreen,
  editor,
  bulkEntry,
  cardStyles,
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
    ["src-tauri", "src", "authoring", "tests.rs"],
  ),
  read("src-tauri", "src", "cards.rs"),
  read("src-tauri", "src", "practice.rs"),
  read("src-tauri", "src", "practice_selection.rs"),
  read("src", "ipc.ts"),
  read("src", "App.tsx"),
  read("src", "screens", "CardsScreen.tsx"),
  read("src", "screens", "CardEditorScreen.tsx"),
  read("src", "screens", "cards", "BulkEntryModal.tsx"),
  read("src", "card-organization.css"),
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
  "list_matching_cards",
  "get_organized_card",
  "create_organized_card",
  "update_organized_card",
  "find_duplicate_cards",
  "analyze_bulk_cards",
  "bulk_create_cards",
  "transfer_cards",
  "bulk_tag_cards",
  "delete_cards",
]) {
  if (!rustCommands.includes(command)) failures.push(`Rust command not registered: ${command}`);
  if (!ipc.includes(`"${command}"`)) failures.push(`frontend command missing: ${command}`);
}
if (
  !rustCommands.includes("practice_selection::start_card_practice") ||
  !ipc.includes("startCardPractice") ||
  !ipc.includes("start_card_practice")
) {
  failures.push("temporary filtered practice command is not registered end to end");
}

for (const fragment of [
  "bm25(card_search_fts)",
  "reviewed_filter",
  "deck_ids",
  "MAX_SEARCH_SELECTION",
  "MAX_BULK_ROWS",
  "TransferMode::Move",
  "TransferMode::Copy",
  "TagChangeMode::Add",
  "backfill_duplicate_hashes",
]) {
  if (!authoring.includes(fragment)) failures.push(`authoring boundary missing: ${fragment}`);
}

const strictInputs = [
  "ListTagsInput",
  "SearchCardsInput",
  "CreateOrganizedCardInput",
  "UpdateOrganizedCardInput",
  "DuplicateCheckInput",
  "BulkCardRowInput",
  "AnalyzeBulkCardsInput",
  "BulkCreateCardsInput",
  "TransferCardsInput",
  "BulkTagCardsInput",
  "DeleteCardsInput",
];
for (const inputName of strictInputs) {
  const strictDeclaration = new RegExp(
    `#\\[serde\\(rename_all = "camelCase", deny_unknown_fields\\)\\]\\s*pub struct ${inputName}\\b`,
    "u",
  );
  if (!strictDeclaration.test(authoring)) {
    failures.push(`${inputName} must reject unknown IPC fields`);
  }
}

const bulkCreateBlock =
  authoring.match(/pub struct BulkCreateCardsInput\s*\{(?<body>[\s\S]*?)\n\}/u)?.groups?.body ?? "";
if (bulkCreateBlock.includes("skip_duplicates")) {
  failures.push("BulkCreateCardsInput still contains the removed skipDuplicates option");
}
if (
  authoring.includes("legacy_deck_id") ||
  /pub deck_id:\s*Option<String>/u.test(authoring)
) {
  failures.push("authoring still contains the removed single-deck compatibility input");
}
if (/\#\[(?:allow|expect)\(dead_code\)\]/u.test(authoring)) {
  failures.push("authoring suppresses dead-code detection instead of removing stale code");
}
if (!authoring.includes("removed_authoring_fields_are_rejected")) {
  failures.push("removed authoring fields are not covered by strict-deserialization tests");
}

for (const removedCommand of [
  "cards::get_card,",
  "cards::create_card,",
  "cards::update_card,",
  "practice::start_practice,",
]) {
  if (rustCommands.includes(removedCommand)) {
    failures.push(`obsolete Rust command is still registered: ${removedCommand}`);
  }
}
for (const removedWrapper of [
  "export async function getCard(",
  "export async function createCard(",
  "export async function updateCard(",
  "export async function startPractice(",
]) {
  if (ipc.includes(removedWrapper)) {
    failures.push(`obsolete frontend IPC wrapper remains: ${removedWrapper}`);
  }
}
for (const removedFunction of [
  "pub fn get_card(",
  "pub fn create_card(",
  "pub fn update_card(",
  "create_card_in_connection(",
  "update_card_in_connection(",
]) {
  if (cardsCore.includes(removedFunction)) {
    failures.push(`obsolete card API remains: ${removedFunction}`);
  }
}
if (practice.includes("pub fn start_practice(") || practice.includes("start_practice_in_connection(")) {
  failures.push("obsolete strict-due practice implementation remains");
}

if (!practiceSelection.includes("start_card_practice") || !practiceSelection.includes("card_ids")) {
  failures.push("filtered result cards cannot start a temporary practice session");
}
if (!app.includes('label="Search/Filter"') || !app.includes("onPracticeCards")) {
  failures.push("Search/Filter is not registered in top-level navigation and practice routing");
}
for (const fragment of [
  "Search/Filter",
  "deckIds",
  "reviewedFilter",
  "Select visible",
  "Select all",
  "Deselect all",
  "Actions for selected cards",
  "Practice results",
  "BulkTagModal",
  "DeleteCardsModal",
]) {
  if (!cardsScreen.includes(fragment)) failures.push(`Search/Filter screen missing: ${fragment}`);
}
for (const fragment of [
  "<TagInput",
  "findDuplicateCards",
  "Save duplicate anyway",
  "deckId: selectedDeckId",
  'aria-label="Deck"',
]) {
  if (!editor.includes(fragment)) failures.push(`card editor missing: ${fragment}`);
}
if (!bulkEntry.includes("analyzeBulkCards") || bulkEntry.includes("skipDuplicates")) {
  failures.push("bulk entry must analyze duplicates automatically without a policy checkbox");
}
for (const fragment of [
  'input[type="checkbox"]',
  ".card-row-selector",
  ".modal-large",
  ".transfer-mode-option",
  ".bulk-entry-row",
]) {
  if (!cardStyles.includes(fragment)) failures.push(`responsive card styles missing: ${fragment}`);
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
  "OK   Search/Filter features are wired, strict IPC inputs reject removed fields, and obsolete APIs are absent.",
);