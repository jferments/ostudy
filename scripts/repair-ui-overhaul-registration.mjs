#!/usr/bin/env node

import { readFile, writeFile } from "node:fs/promises";

const databasePath = "src-tauri/src/database.rs";
const libraryPath = "src-tauri/src/lib.rs";

let database = await readFile(databasePath, "utf8");
if (!database.includes('include_str!("../migrations/0026_deck_folders.sql")')) {
  const insertion = `    Migration {
        version: 26,
        name: "deck_folders",
        checksum: "855a163ab842832cd83c0b6f9d24c2a74eb359e40ff167496a0af0527412d2f8",
        sql: include_str!("../migrations/0026_deck_folders.sql"),
    },
`;
  const migrationsEnd = database.indexOf("\n];", database.indexOf("const MIGRATIONS"));
  if (migrationsEnd < 0) {
    throw new Error("Could not locate the migration registry terminator.");
  }
  database = `${database.slice(0, migrationsEnd)}\n${insertion}${database.slice(migrationsEnd)}`;
}
database = database.replace(
  /pub\(crate\) const LATEST_SCHEMA_VERSION: i64 = 25;/u,
  "pub(crate) const LATEST_SCHEMA_VERSION: i64 = 26;",
);
if (!database.includes("LATEST_SCHEMA_VERSION: i64 = 26")) {
  throw new Error("Could not update LATEST_SCHEMA_VERSION to 26.");
}
await writeFile(databasePath, database, "utf8");

let library = await readFile(libraryPath, "utf8");
if (!library.includes("mod deck_folders;")) {
  const moduleAnchor = "mod deck_cards;";
  if (!library.includes(moduleAnchor)) {
    throw new Error("Could not locate deck_cards module registration.");
  }
  library = library.replace(moduleAnchor, `${moduleAnchor}\nmod deck_folders;`);
}
const commandBlock = `            deck_folders::get_deck_library_organization,
            deck_folders::create_deck_folder,
            deck_folders::update_deck_folder,
            deck_folders::delete_deck_folder,
            deck_folders::move_deck_in_library,
            deck_folders::move_deck_folder,
`;
if (!library.includes("deck_folders::get_deck_library_organization")) {
  const commandAnchor = "            decks::set_deck_archived,\n";
  if (!library.includes(commandAnchor)) {
    throw new Error("Could not locate deck command registration anchor.");
  }
  library = library.replace(commandAnchor, `${commandAnchor}${commandBlock}`);
}
await writeFile(libraryPath, library, "utf8");
