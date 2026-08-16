#!/usr/bin/env node

import { readFile } from "node:fs/promises";
import { join } from "node:path";
import { fileURLToPath } from "node:url";

const root = fileURLToPath(new URL("..", import.meta.url));
const [css, tauriConfigText] = await Promise.all([
  readFile(join(root, "src/screens/flashcards/DeckLibraryWorkspace.module.css"), "utf8"),
  readFile(join(root, "src-tauri/tauri.conf.json"), "utf8"),
]);
const failures = [];

expectContains(css, ".toolbarActions { display: flex;", "deck toolbar action group");
expectContains(css, "flex-wrap: nowrap;", "non-wrapping medium/desktop action row");
expectContains(css, ".toolbarActions > button { flex: 0 0 auto; white-space: nowrap; }", "stable action button sizing");
expectContains(css, "@media (max-width: 1200px)", "medium desktop breakpoint");
expectContains(css, ".toolbar { flex-direction: column; }", "medium toolbar stacking as a whole");
expectContains(css, ".toolbarActions { width: 100%; flex-wrap: nowrap; justify-content: flex-end; }", "medium action row remains intact");
expectContains(css, "@media (max-width: 640px)", "compact breakpoint");
expectContains(css, ".toolbarActions { flex-wrap: wrap; justify-content: stretch; }", "wrapping limited to compact screens");

const tauriConfig = JSON.parse(tauriConfigText);
const mainWindow = tauriConfig?.app?.windows?.find((window) => window?.label === "main");
if (!mainWindow || typeof mainWindow.width !== "number" || mainWindow.width < 1280) {
  failures.push("The default desktop main window must be at least 1280 CSS pixels wide.");
}

if (failures.length > 0) {
  console.error("Deck toolbar layout contract failed:");
  for (const failure of failures) console.error(`  - ${failure}`);
  process.exit(1);
}
console.log(
  "OK   Default desktop sizing and responsive deck toolbar keep New folder/New deck together until compact widths.",
);

function expectContains(source, expected, label) {
  if (!source.includes(expected)) failures.push(`Missing ${label}: ${expected}`);
}
