#!/usr/bin/env node

import { readdir, readFile } from "node:fs/promises";
import { join, relative } from "node:path";
import { fileURLToPath } from "node:url";

const root = fileURLToPath(new URL("..", import.meta.url));
const rustRoot = await readFile(join(root, "src-tauri", "src", "lib.rs"), "utf8");
const failures = [];

const rustFiles = (await collectFiles(join(root, "src-tauri", "src"))).filter((path) =>
  path.endsWith(".rs"),
);
const rustText = (await Promise.all(rustFiles.map((path) => readFile(path, "utf8")))).join("\n");
const renamedCommands = new Map();
for (const match of rustText.matchAll(
  /#\[tauri::command\s*\(\s*rename\s*=\s*"([a-z][a-z0-9_]*)"\s*\)\]\s*pub\s+fn\s+([a-z][a-z0-9_]*)/gu,
)) {
  renamedCommands.set(match[2], match[1]);
}

const handlerBody = rustRoot.match(/tauri::generate_handler!\s*\[([\s\S]*?)\]\s*\)/u)?.[1];
if (!handlerBody) failures.push("Could not locate the Tauri generate_handler command registry.");

const registered = new Set();
if (handlerBody) {
  for (const rawEntry of handlerBody.split(",")) {
    const entry = rawEntry.replace(/\/\/.*$/gmu, "").trim();
    if (entry === "") continue;
    const functionName = entry.split("::").at(-1)?.trim();
    if (!functionName || !/^[a-z][a-z0-9_]*$/u.test(functionName)) {
      failures.push(`Invalid Tauri command registry entry: ${entry}`);
      continue;
    }
    const command = renamedCommands.get(functionName) ?? functionName;
    if (registered.has(command)) failures.push(`Tauri command is registered twice: ${command}`);
    registered.add(command);
  }
}

const invoked = new Map();
for (const path of await collectFiles(join(root, "src"))) {
  if (!/\.[cm]?[jt]sx?$/u.test(path)) continue;
  const text = await readFile(path, "utf8");
  const displayPath = relative(root, path).replaceAll("\\", "/");
  const pattern = /\binvoke(?:\s*<[^>]+>)?\s*\(\s*["']([a-z][a-z0-9_]*)["']/gu;
  for (const match of text.matchAll(pattern)) {
    const command = match[1];
    const locations = invoked.get(command) ?? [];
    locations.push(displayPath);
    invoked.set(command, locations);
  }
}

for (const [command, locations] of invoked) {
  if (!registered.has(command)) {
    failures.push(`Frontend invokes unregistered command ${command} from ${[...new Set(locations)].join(", ")}`);
  }
}
for (const command of registered) {
  if (!invoked.has(command)) failures.push(`Registered Tauri command has no frontend wrapper: ${command}`);
}

for (const obsolete of [
  "list_cards",
  "list_card_templates",
  "get_card",
  "create_card",
  "update_card",
  "start_practice",
]) {
  if (registered.has(obsolete) || invoked.has(obsolete)) {
    failures.push(`Obsolete unbounded or superseded IPC command remains: ${obsolete}`);
  }
}

if (registered.size < 40) {
  failures.push(`Expected the complete application IPC boundary, found only ${registered.size} commands.`);
}

if (failures.length > 0) {
  console.error("IPC contract verification failed:");
  for (const failure of failures) console.error(`  - ${failure}`);
  process.exit(1);
}

console.log(
  `OK   IPC contract: ${registered.size} registered commands exactly match frontend invoke wrappers; obsolete commands are absent.`,
);

async function collectFiles(directory) {
  const entries = await readdir(directory, { withFileTypes: true });
  const output = [];
  for (const entry of entries) {
    const path = join(directory, entry.name);
    if (entry.isDirectory()) output.push(...(await collectFiles(path)));
    else if (entry.isFile()) output.push(path);
  }
  return output;
}
