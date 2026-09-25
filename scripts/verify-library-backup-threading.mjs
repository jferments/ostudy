#!/usr/bin/env node

import { readFile } from "node:fs/promises";
import { join } from "node:path";
import { fileURLToPath } from "node:url";

const root = fileURLToPath(new URL("..", import.meta.url));
const [commands, compositionRoot, frontend, leasedCommands] = await Promise.all([
  read("src-tauri/src/library_backup_commands.rs"),
  read("src-tauri/src/lib.rs"),
  read("src/ipc/backup.ts"),
  read("src-tauri/src/library_session/commands.rs"),
]);
const failures = [];

expectContains(commands, "pub async fn prepare_library_backup_async", "async backup adapter");
expectContains(commands, "pub async fn prepare_library_restore_async", "async restore adapter");
expectContains(commands, "tauri::async_runtime::spawn_blocking", "blocking worker pool handoff");
expectContains(
  compositionRoot,
  "library_session::commands::leased_prepare_library_backup_async",
  "generation-leased nonblocking backup command registration",
);
const leasedBackup = leasedCommands.match(
  /#\[tauri::command\(rename = "prepare_library_backup_async"\)\]([\s\S]*?)(?=\n#\[tauri::command|$)/u,
)?.[1] ?? "";
expectContains(leasedBackup, "fn leased_prepare_library_backup_async(", "stable backup wire name and unique Rust adapter");
expectContains(leasedBackup, "super::enter(library_generation)?", "original-generation lease through backup future");
expectContains(leasedBackup, "crate::library_backup_commands::prepare_library_backup_async(", "forwarding into nonblocking backup adapter");
expectContains(leasedBackup, ".await", "backup lease retained through worker completion");
expectContains(
  compositionRoot,
  "library_backup_commands::prepare_library_restore_async",
  "nonblocking restore command registration",
);
expectContains(frontend, 'invoke<PreparedLibraryBackup>("prepare_library_backup_async"', "frontend async backup invocation");
expectContains(frontend, 'invoke<LibraryRestoreResult>("prepare_library_restore_async"', "frontend async restore invocation");
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
console.log("OK   Generation-leased backup and exclusive live restore run outside the Tauri UI/event-loop thread.");

async function read(relativePath) {
  return readFile(join(root, relativePath), "utf8");
}

function expectContains(source, expected, label) {
  if (!source.includes(expected)) failures.push(`Missing ${label}: ${expected}`);
}
