#!/usr/bin/env node

import { readFile, writeFile } from "node:fs/promises";

const path = "src-tauri/src/deck_folders.rs";
let source = await readFile(path, "utf8");

const replacements = [
  [
    '"SELECT folder_id FROM decks WHERE id = ?1 AND deleted_at IS NULL",',
    '"SELECT folder_id FROM decks\n             WHERE id = ?1 AND deleted_at IS NULL AND archived_at IS NULL",',
  ],
  [
    '"UPDATE decks SET folder_id = ?1, updated_at = ?2\n             WHERE id = ?3 AND deleted_at IS NULL",',
    '"UPDATE decks SET folder_id = ?1, updated_at = ?2\n             WHERE id = ?3 AND deleted_at IS NULL AND archived_at IS NULL",',
  ],
  [
    '"SELECT id FROM decks\n             WHERE deleted_at IS NULL\n               AND folder_id IS ?1',
    '"SELECT id FROM decks\n             WHERE deleted_at IS NULL\n               AND archived_at IS NULL\n               AND folder_id IS ?1',
  ],
  [
    '"UPDATE decks\n                 SET folder_id = ?1, sort_order = ?2, updated_at = ?3\n                 WHERE id = ?4 AND deleted_at IS NULL",',
    '"UPDATE decks\n                 SET folder_id = ?1, sort_order = ?2, updated_at = ?3\n                 WHERE id = ?4 AND deleted_at IS NULL AND archived_at IS NULL",',
  ],
];

for (const [before, after] of replacements) {
  if (source.includes(after)) continue;
  if (!source.includes(before)) {
    throw new Error(`Could not locate expected folder-query fragment:\n${before}`);
  }
  source = source.replace(before, after);
}

await writeFile(path, source, "utf8");
