#!/usr/bin/env node

import { readdir, readFile } from "node:fs/promises";
import { join, relative } from "node:path";
import { fileURLToPath } from "node:url";
import { duplicateFunctionNames } from "./lib/ipc-symbol-contract.mjs";

const root = fileURLToPath(new URL("..", import.meta.url));
const rustRoot = await readFile(join(root, "src-tauri", "src", "lib.rs"), "utf8");
const viteConfig = await readFile(join(root, "vite.config.ts"), "utf8");
const tsConfig = await readFile(join(root, "tsconfig.app.json"), "utf8");
const transportSource = await readFile(join(root, "src", "ipc", "transport.ts"), "utf8");
const failures = [];

if (viteConfig.includes('@tauri-apps/api/core') || viteConfig.includes('@ostudy/tauri-core')) {
  failures.push("Vite must not intercept the Tauri core module; domain gateways import the application transport explicitly.");
}
if (tsConfig.includes('@ostudy/tauri-core')) {
  failures.push("TypeScript must not retain the private raw-Tauri alias.");
}
if (!transportSource.includes('invoke as nativeInvoke') || !transportSource.includes('from "@tauri-apps/api/core"')) {
  failures.push("The application transport must own the one raw native invoke import.");
}

const rustFiles = (await collectFiles(join(root, "src-tauri", "src"))).filter((path) =>
  path.endsWith(".rs"),
);

// Tauri emits global helper symbols from the Rust FUNCTION name, even with
// an explicit wire-name rename. Unique forwarding adapters may share a wire
// name with an unregistered domain command; duplicate Rust symbols and duplicate
// registered wire names are both forbidden and verified independently.
const commandDefinitions = new Map();
const definitionsByFunction = new Map();
const commandPattern =
  /#\[tauri::command(?<attribute>\s*\([^\]]*\))?\]\s*pub(?:\([^)]*\))?\s+(?:async\s+)?fn\s+(?<function>[a-z][a-z0-9_]*)/gu;
for (const path of rustFiles) {
  const text = await readFile(path, "utf8");
  const displayPath = relative(root, path).replaceAll("\\", "/");
  for (const match of text.matchAll(commandPattern)) {
    const functionName = match.groups?.function;
    if (!functionName) continue;
    const rename = match.groups?.attribute?.match(
      /\brename\s*=\s*"([a-z][a-z0-9_]*)"/u,
    )?.[1];
    const commandName = rename ?? functionName;
    const definition = { functionName, commandName, displayPath };

    const commandLocations = commandDefinitions.get(commandName) ?? [];
    commandLocations.push(definition);
    commandDefinitions.set(commandName, commandLocations);

    const functionLocations = definitionsByFunction.get(functionName) ?? [];
    functionLocations.push(definition);
    definitionsByFunction.set(functionName, functionLocations);
  }
}

for (const [functionName, definitions] of duplicateFunctionNames(definitionsByFunction)) {
  failures.push(
    `Tauri command Rust symbol is defined more than once: ${functionName} (${definitions
      .map((definition) => definition.displayPath)
      .join(", ")})`,
  );
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

    const definitions = definitionsByFunction.get(functionName) ?? [];
    if (definitions.length === 0) {
      failures.push(`Registered Tauri command does not resolve to a #[tauri::command] function: ${entry}`);
      continue;
    }
    if (definitions.length > 1) {
      failures.push(
        `Registered Tauri command name is ambiguous: ${entry} (${definitions
          .map((definition) => definition.displayPath)
          .join(", ")})`,
      );
      continue;
    }

    const command = definitions[0].commandName;
    if (registered.has(command)) failures.push(`Tauri command is registered twice: ${command}`);
    registered.add(command);
  }
}

const invoked = new Map();
for (const path of await collectFiles(join(root, "src"))) {
  if (!/\.[cm]?[jt]sx?$/u.test(path)) continue;
  const text = await readFile(path, "utf8");
  const displayPath = relative(root, path).replaceAll("\\", "/");
  if (displayPath.startsWith("src/ipc/") && displayPath !== "src/ipc/transport.ts") {
    const coreInvokeImport = /import\s*\{[^}]*\binvoke\b[^}]*\}\s*from\s*["']@tauri-apps\/api\/core["']/u;
    if (coreInvokeImport.test(text)) {
      failures.push(`IPC gateway bypasses the application transport: ${displayPath}`);
    }
    if (/\bimport\s*\{[^}]*\binvoke\b[^}]*\}\s*from\s*["']\.\/transport["']/u.test(text) === false
        && /\binvoke(?:\s*<[^>]+>)?\s*\(/u.test(text)
        && displayPath !== "src/ipc/libraryTransport.ts") {
      failures.push(`IPC module invokes native commands without importing ./transport: ${displayPath}`);
    }
  }
  const pattern = /\binvoke(?:\s*<[^>]+>)?\s*\(\s*["']([a-z][a-z0-9_]*)["']/gu;
  for (const match of text.matchAll(pattern)) {
    const command = match[1];
    if (!displayPath.startsWith("src/ipc/")) {
      failures.push(`Direct Tauri invoke is outside a typed IPC gateway: ${displayPath} (${command})`);
    }
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
  "create_organized_card",
  "update_organized_card",
  "start_practice",
  "get_organized_card",
  "get_card_type_info",
  "create_generated_card",
  "update_generated_card",
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
  `OK   IPC contract: ${commandDefinitions.size} unique native command definitions, ${registered.size} registered commands, and frontend invoke wrappers agree.`,
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
