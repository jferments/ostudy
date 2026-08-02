#!/usr/bin/env node
import { createHash } from "node:crypto";
import { readFileSync } from "node:fs";
import { join } from "node:path";

const root = process.cwd();
const read = (path) => readFileSync(join(root, path), "utf8");
const failures = [];

function requireFragment(path, fragment, label = fragment) {
  const source = read(path);
  if (!source.includes(fragment)) failures.push(`${path}: missing ${label}`);
}

function rejectFragment(path, fragment, label = fragment) {
  const source = read(path);
  if (source.includes(fragment)) failures.push(`${path}: contains forbidden ${label}`);
}

const migration = read("src-tauri/migrations/0006_card_types_templates.sql");
const checksum = createHash("sha256").update(migration).digest("hex");
requireFragment("src-tauri/src/database.rs", "LATEST_SCHEMA_VERSION: i64 = 6", "schema version 6");
requireFragment("src-tauri/src/database.rs", `checksum: "${checksum}"`, "migration 6 checksum");
requireFragment("src-tauri/src/database.rs", "0006_card_types_templates.sql", "Stage 3 migration registration");
requireFragment("src-tauri/migrations/0006_card_types_templates.sql", "CREATE TABLE card_templates", "template table");
requireFragment("src-tauri/migrations/0006_card_types_templates.sql", "ALTER TABLE notes ADD COLUMN reverse_enabled", "reverse source data");
requireFragment("src-tauri/migrations/0006_card_types_templates.sql", "ALTER TABLE notes ADD COLUMN cloze_text", "cloze source data");
rejectFragment("src-tauri/migrations/0006_card_types_templates.sql", "template_id", "template-to-note ownership");
rejectFragment("src-tauri/migrations/0006_card_types_templates.sql", "generation_key", "unneeded generated-card shim");

for (const command of [
  "list_card_templates",
  "get_card_template",
  "create_card_template",
  "update_card_template",
  "delete_card_template",
  "preview_template_cards",
  "generate_cards_from_template",
]) {
  requireFragment("src-tauri/src/lib.rs", `authoring::${command}`, `${command} registration`);
  requireFragment("src/ipc.ts", `"${command}"`, `${command} IPC wrapper`);
}

requireFragment("src/App.tsx", 'label="Templates"', "Templates navigation item");
requireFragment("src/screens/card-editor/CardEditorScreen.tsx", "Reusable entry templates", "template selector group");
requireFragment("src/screens/card-editor/CardEditorScreen.tsx", "generateCardsFromTemplate", "template generation command");
requireFragment("src/screens/card-editor/CardEditorScreen.tsx", "ordinary Basic cards", "ordinary-card semantics");
requireFragment("src/screens/card-editor/CardEditorScreen.tsx", "reverseEnabled", "reverse-card checkbox state");
requireFragment("src/screens/card-editor/ClozeEditor.tsx", "Mark selection as blank", "cloze selection control");
requireFragment("src/screens/card-editor/cloze.ts", "one_at_a_time", "one-at-a-time cloze mode");
requireFragment("src/screens/card-editor/cloze.ts", "all_together", "all-together cloze mode");
requireFragment("src/screens/TemplatesScreen.tsx", "generated cards remain", "safe template deletion copy");
rejectFragment("src/screens/TemplatesScreen.tsx", "template.noteCount", "template usage lock");
rejectFragment("src/screens/TemplatesScreen.tsx", "fieldsLocked", "template field lock");

requireFragment("src-tauri/src/authoring/stage3.rs", "insert_organized_card(", "ordinary Basic insertion path");
requireFragment("src-tauri/src/authoring/stage3.rs", "deleting_a_template_does_not_delete_generated_basic_cards", "template independence test");
requireFragment("src-tauri/src/authoring/stage3.rs", "reverse_card_reactivation_reuses_its_schedule", "reverse schedule revival test");
requireFragment("src-tauri/src/practice_selection.rs", "get_card_by_id_from_connection", "card-level practice hydration");
rejectFragment("src-tauri/src/practice_selection.rs", "c.direction = 'forward'", "forward-only practice filter");
rejectFragment("src-tauri/src/authoring/search_and_tags.rs", "c.direction = 'forward'", "forward-only browser filter");

requireFragment("src-tauri/src/transfer.rs", "PACKAGE_FORMAT_VERSION: i64 = 2", "studydeck format version 2");
requireFragment("src-tauri/src/transfer.rs", "reverse_enabled", "reverse transfer data");
requireFragment("src-tauri/src/transfer.rs", "cloze_text", "cloze transfer data");
rejectFragment("src-tauri/src/transfer.rs", "card_templates", "deck-owned templates");

for (const path of [
  "src-tauri/src/authoring/stage3.rs",
  "src/screens/TemplatesScreen.tsx",
  "src/screens/card-editor/CardEditorScreen.tsx",
]) {
  rejectFragment(path, "eval(", "eval");
  rejectFragment(path, "new Function", "dynamic function construction");
  rejectFragment(path, "dangerouslySetInnerHTML", "raw HTML rendering");
}

for (const stale of [
  "legacy_template",
  "template_values_json",
  "regenerate_template_notes",
  "apply_imported_typed_note",
]) {
  rejectFragment("src-tauri/src/authoring/stage3.rs", stale, `stale template-note path ${stale}`);
}

requireFragment(
  "docs/agents/STAGE-IMPLEMENTATION-PLAYBOOK.md",
  "How do I access the repo?",
  "repository-access cue",
);
requireFragment(
  "docs/agents/STAGE-IMPLEMENTATION-PLAYBOOK.md",
  "Do not retain dead",
  "no stale compatibility rule",
);

if (failures.length > 0) {
  console.error(
    "Stage 3 verification failed:\n" + failures.map((failure) => `- ${failure}`).join("\n"),
  );
  process.exit(1);
}
console.log(
  "Stage 3 reverse/cloze cards and independent data-only template generation are wired consistently.",
);
