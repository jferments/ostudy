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

expectPattern(css, /\.toolbar\s*\{[^}]*display:\s*flex;[^}]*justify-content:\s*flex-end;/su, "deck toolbar");
expectPattern(css, /\.toolbar\s*>\s*button\s*\{[^}]*flex:\s*0\s+0\s+auto;[^}]*white-space:\s*nowrap;/su, "stable toolbar action sizing");
expectPattern(css, /\.library\s*\{[^}]*container-name:\s*deck-library;[^}]*container-type:\s*inline-size;/su, "deck-library container boundary");
expectPattern(css, /\.sectionActions\s*\{[^}]*display:\s*flex;/su, "folder section action group");
expectContains(css, "@container deck-library (max-width: 760px)", "medium container breakpoint");
expectContains(css, "@container deck-library (max-width: 560px)", "compact container breakpoint");
expectPattern(
  css,
  /@container deck-library \(max-width: 560px\)[\s\S]*?\.sectionActions\s*\{[^}]*width:\s*100%;[^}]*justify-content:\s*stretch;/u,
  "compact folder controls",
);
expectPattern(
  css,
  /\.deckMain\s*\{[^}]*padding:\s*8px\s+var\(--ostudy-space-3\);/su,
  "single compact deck presentation",
);
expectPattern(css, /@media \(pointer:\s*coarse\)/u, "coarse-pointer touch targets");

if (/\.density\b/u.test(css) || /data-density/u.test(css)) {
  failures.push("Deck density controls or density-specific styling must not be reintroduced.");
}
if (/\.dragHandle\s*\{[^}]*border-inline-end:/su.test(css)) {
  failures.push("The deck move handle must not render a vertical inline separator.");
}

const tauriConfig = JSON.parse(tauriConfigText);
const mainWindow = tauriConfig?.app?.windows?.find((window) => window?.label === "main");
if (!mainWindow || typeof mainWindow.width !== "number" || mainWindow.width < 1280) {
  failures.push("The default desktop main window must be at least 1280 CSS pixels wide.");
}

if (failures.length > 0) {
  console.error("Deck library layout contract failed:");
  for (const failure of failures) console.error(`  - ${failure}`);
  process.exit(1);
}
console.log(
  "OK   Desktop sizing, compact deck density, container responsiveness, and touch targets satisfy the deck-library layout contract.",
);

function expectContains(source, expected, label) {
  if (!source.includes(expected)) failures.push(`Missing ${label}: ${expected}`);
}

function expectPattern(source, pattern, label) {
  if (!pattern.test(source)) failures.push(`Missing ${label}: ${pattern}`);
}
