#!/usr/bin/env node

import { readFile } from "node:fs/promises";
import { join } from "node:path";
import { fileURLToPath } from "node:url";

const root = fileURLToPath(new URL("..", import.meta.url));
const [commands, compositionRoot] = await Promise.all([
  read("src-tauri/src/library_backup_commands.rs"),
  read("src-tauri/src/lib.rs"),
]);
const failures = [];

expectContains(commands, "pub async fn prepare_library_backup", "async backup adapter");
expectContains(commands, "pub async fn prepare_library_restore", "async restore adapter");
expectContains(commands, "tauri::async_runtime::spawn_blocking", "blocking worker pool handoff");
expectContains(
  compositionRoot,
  "library_backup_commands::prepare_library_backup",
  "nonblocking backup command registration",
);
expectContains(
  compositionRoot,
  "library_backup_commands::prepare_library_restore",
  "nonblocking restore command registration",
);
if (compositionRoot.includes("library_backup::prepare_library_backup,")) {
  failures.push("The blocking library backup implementation is registered directly as a Tauri command.");
}
if (compositionRoot.includes("library_backup::prepare_library_restore,")) {
  failures.push("The blocking library restore implementation is registered directly as a Tauri command.");
}

if (failures.length > 0) {
  console.error("Library backup threading contract failed:");
  for (const failure of failures) console.error(`  - ${failure}`);
  process.exit(1);
}
console.log("OK   Library backup and restore preparation run outside the Tauri UI/event-loop thread.");

async function read(relativePath) {
  return readFile(join(root, relativePath), "utf8");
}

function expectContains(source, expected, label) {
  if (!source.includes(expected)) failures.push(`Missing ${label}: ${expected}`);
}
