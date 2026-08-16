#!/usr/bin/env node

import { readFile } from "node:fs/promises";
import { join } from "node:path";
import { fileURLToPath } from "node:url";

const root = fileURLToPath(new URL("..", import.meta.url));
const css = await readFile(
  join(root, "src/screens/flashcards/DeckLibraryWorkspace.module.css"),
  "utf8",
);
const failures = [];

expectContains(css, ".toolbarActions { display: flex;", "deck toolbar action group");
expectContains(css, "flex-wrap: nowrap;", "non-wrapping medium/desktop action row");
expectContains(css, ".toolbarActions > button { flex: 0 0 auto; white-space: nowrap; }", "stable action button sizing");
expectContains(css, "@media (max-width: 1200px)", "medium desktop breakpoint");
expectContains(css, ".toolbar { flex-direction: column; }", "medium toolbar stacking as a whole");
expectContains(css, ".toolbarActions { width: 100%; flex-wrap: nowrap; justify-content: flex-end; }", "medium action row remains intact");
expectContains(css, "@media (max-width: 640px)", "compact breakpoint");
expectContains(css, ".toolbarActions { flex-wrap: wrap; justify-content: stretch; }", "wrapping limited to compact screens");

if (failures.length > 0) {
  console.error("Deck toolbar layout contract failed:");
  for (const failure of failures) console.error(`  - ${failure}`);
  process.exit(1);
}
console.log(
  "OK   Deck toolbar keeps New folder/New deck together at medium desktop widths and wraps only on compact screens.",
);

function expectContains(source, expected, label) {
  if (!source.includes(expected)) failures.push(`Missing ${label}: ${expected}`);
}
