import { createHash } from "node:crypto";
import { readFile, readdir } from "node:fs/promises";
import { join } from "node:path";
import { fileURLToPath } from "node:url";

const root = fileURLToPath(new URL("..", import.meta.url));
const read = (...parts) => readFile(join(root, ...parts), "utf8");
const normalizeWhitespace = (value) => value.replace(/\s+/gu, " ").trim();

async function readTree(relativeParts, suffix) {
  const directory = join(root, ...relativeParts);
  const entries = await readdir(directory, { withFileTypes: true });
  const contents = [];
  for (const entry of entries) {
    const child = [...relativeParts, entry.name];
    if (entry.isDirectory()) {
      contents.push(await readTree(child, suffix));
    } else if (entry.isFile() && entry.name.endsWith(suffix)) {
      contents.push(await read(...child));
    }
  }
  return contents.join("\n");
}

const [
  migration,
  database,
  rustCommands,
  authoring,
  stage3Transfer,
  practiceSelection,
  ipc,
  app,
  editor,
  clozeEditor,
  templateEntry,
  templatesScreen,
  cardsScreen,
  styles,
  roadmap,
  architecture,
  dataModel,
] = await Promise.all([
  read("src-tauri", "migrations", "0006_card_types_and_templates.sql"),
  read("src-tauri", "src", "database.rs"),
  read("src-tauri", "src", "lib.rs"),
  readTree(["src-tauri", "src", "authoring"], ".rs"),
  read("src-tauri", "src", "stage3_transfer.rs"),
  read("src-tauri", "src", "practice_selection.rs"),
  read("src", "ipc.ts"),
  read("src", "App.tsx"),
  read("src", "screens", "CardEditorScreen.tsx"),
  read("src", "screens", "card-editor", "ClozeEditor.tsx"),
  read("src", "screens", "card-editor", "TemplateEntryForm.tsx"),
  read("src", "screens", "TemplatesScreen.tsx"),
  read("src", "screens", "CardsScreen.tsx"),
  read("src", "card-types.css"),
  read("docs", "PHASE-2-ROADMAP.md"),
  read("docs", "ARCHITECTURE.md"),
  read("docs", "CARD-FORMAT-AND-DATA-MODEL.md"),
]);

const failures = [];
const checksum = createHash("sha256").update(migration).digest("hex");
if (!database.includes(`checksum: "${checksum}"`)) {
  failures.push(`database.rs checksum mismatch for migration 0006: ${checksum}`);
}
if (!database.includes("LATEST_SCHEMA_VERSION: i64 = 6")) {
  failures.push("database schema version was not advanced to 6");
}
for (const fragment of [
  "ALTER TABLE notes ADD COLUMN reverse_enabled",
  "ALTER TABLE notes ADD COLUMN cloze_source",
  "ALTER TABLE notes ADD COLUMN cloze_mode",
  "CREATE TABLE card_templates",
  "CREATE TABLE card_template_fields",
  "CREATE TABLE card_template_rules",
]) {
  if (!migration.includes(fragment)) failures.push(`migration 0006 missing: ${fragment}`);
}
if (/template_id\s+TEXT[^\n]*REFERENCES[^\n]*notes/iu.test(migration)) {
  failures.push("generated notes incorrectly depend on reusable templates");
}

const commands = [
  "get_card_type_info",
  "create_generated_card",
  "update_generated_card",
  "list_card_templates",
  "get_card_template",
  "create_card_template",
  "update_card_template",
  "delete_card_template",
  "preview_template_cards",
  "generate_cards_from_template",
];
for (const command of commands) {
  if (!rustCommands.includes(command)) failures.push(`Rust command not registered: ${command}`);
  if (!ipc.includes(`"${command}"`)) failures.push(`frontend command missing: ${command}`);
}
for (const command of [
  "transfer_generated_cards",
  "export_deck_with_card_types",
  "import_deck_with_card_types",
]) {
  if (!rustCommands.includes(command)) failures.push(`Stage 3 command not registered: ${command}`);
}

for (const fragment of [
  "sync_generated_cards",
  "deleted_at = NULL",
  "insert_new_schedule",
  "render_stored_card_presentation",
  "direction: \"reverse\"",
  "direction: \"cloze\"",
  "parse_cloze",
  "render_template_previews",
  "generate_template_cards_in_connection",
  "insert_organized_card",
]) {
  if (!authoring.includes(fragment)) failures.push(`Stage 3 domain behavior missing: ${fragment}`);
}
for (const fragment of [
  "deny_unknown_fields",
  "parse_pattern_placeholders",
  "unknown_template_field",
  "invalid_template_placeholder",
  "render_pattern",
]) {
  if (!authoring.includes(fragment)) failures.push(`template safety boundary missing: ${fragment}`);
}
for (const forbidden of [
  "eval(",
  "new Function",
  "Command::new",
  "std::process",
  "reqwest",
]) {
  if (authoring.includes(forbidden)) {
    failures.push(`template implementation contains executable behavior: ${forbidden}`);
  }
}

if (!practiceSelection.includes("get_generated_card_for_practice")) {
  failures.push("practice does not hydrate the exact generated card identity");
}
if (/c\.direction\s*=\s*'forward'/u.test(practiceSelection)) {
  failures.push("practice still filters out reverse or cloze cards");
}
for (const fragment of [
  "card_type: rendered.card_type",
  "card_label: rendered.card_label",
  "render_stored_card_presentation",
]) {
  if (!authoring.includes(fragment)) failures.push(`generated search rendering missing: ${fragment}`);
}

for (const fragment of [
  "CREATE TABLE IF NOT EXISTS card_type_metadata",
  "normalize_package_for_core_import",
  "restore_imported_card_types",
  "insert_new_schedule",
]) {
  if (!stage3Transfer.includes(fragment)) failures.push(`studydeck support missing: ${fragment}`);
}
if (stage3Transfer.includes("card_templates")) {
  failures.push("reusable authoring templates must not be exported with decks");
}

for (const fragment of [
  'label="Templates"',
  "<TemplatesScreen",
  "hasUnsavedTemplate",
]) {
  if (!app.includes(fragment)) failures.push(`Templates navigation missing: ${fragment}`);
}
for (const fragment of [
  'aria-label="Card template"',
  '<option value="basic">Basic</option>',
  '<option value="cloze">Fill in the blank</option>',
  "Create reverse card",
  "<ClozeEditor",
  "<TemplateEntryForm",
  "generateCardsFromTemplate",
]) {
  if (!editor.includes(fragment)) failures.push(`card editor missing: ${fragment}`);
}
for (const fragment of [
  "Mark selection as blank",
  "Hide one blank at a time",
  "Hide all blanks together",
]) {
  if (!clozeEditor.includes(fragment)) failures.push(`cloze editor missing: ${fragment}`);
}
const normalizedTemplateEntry = normalizeWhitespace(templateEntry);
for (const fragment of [
  "literal field substitution only",
  "normal Basic cards",
  "remain after this template is deleted",
]) {
  if (!normalizedTemplateEntry.includes(fragment)) {
    failures.push(`template entry safety copy missing: ${fragment}`);
  }
}
for (const fragment of [
  "New template",
  "Input fields",
  "Cards generated",
  "Available:",
  "Existing cards were not changed",
]) {
  if (!templatesScreen.includes(fragment)) failures.push(`Templates screen missing: ${fragment}`);
}
for (const fragment of [
  "selectedCards.has(card.cardId)",
  "setPracticeCards(selected)",
  "card.cardLabel",
  "uniqueNotes",
]) {
  if (!cardsScreen.includes(fragment)) failures.push(`Search/Filter generated-card behavior missing: ${fragment}`);
}
for (const fragment of [
  ".cloze-editor",
  ".template-library-grid",
  ".template-rule-card",
  ".card-type-badge",
]) {
  if (!styles.includes(fragment)) failures.push(`Stage 3 styles missing: ${fragment}`);
}

for (const [name, document, fragments] of [
  ["roadmap", roadmap, ["Stage 3", "Reverse cards", "Fill in the blank"]],
  ["architecture", architecture, ["data-only", "template"]],
  ["data model", dataModel, ["template", "data-only"]],
]) {
  for (const fragment of fragments) {
    if (!document.toLowerCase().includes(fragment.toLowerCase())) {
      failures.push(`${name} documentation missing: ${fragment}`);
    }
  }
}

if (failures.length > 0) {
  console.error("Stage 3 verification failed:");
  for (const failure of failures) console.error(`  - ${failure}`);
  process.exit(1);
}

console.log(
  "OK   Stage 3 schema, stable generated-card identities, safe templates, UI, practice, search, and studydeck preservation are wired.",
);
