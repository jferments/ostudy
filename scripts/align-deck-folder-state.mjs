#!/usr/bin/env node

import { readFile, writeFile } from "node:fs/promises";

const decksPath = "src-tauri/src/decks.rs";
const foldersPath = "src-tauri/src/deck_folders.rs";

const decks = await readFile(decksPath, "utf8");
let folders = await readFile(foldersPath, "utf8");
const managedAsArc = /State<'_,\s*Arc<Database>>/u.test(decks);

if (managedAsArc) {
  if (!folders.includes("use std::sync::Arc;")) {
    folders = `use std::sync::Arc;\n\n${folders}`;
  }
  folders = folders.replaceAll("State<'_, Database>", "State<'_, Arc<Database>>");
} else {
  folders = folders.replace("use std::sync::Arc;\n\n", "");
  folders = folders.replaceAll("State<'_, Arc<Database>>", "State<'_, Database>");
}

await writeFile(foldersPath, folders, "utf8");
