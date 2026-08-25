#!/usr/bin/env node

import { readFile } from "node:fs/promises";
import { join } from "node:path";
import { fileURLToPath } from "node:url";

const root = fileURLToPath(new URL("..", import.meta.url));
const [primitive, sharedControls, practice] = await Promise.all([
  read("src/ui/primitives/Button.module.css"),
  read("src/ui/primitives/ControlPatterns.module.css"),
  read("src/screens/practice/Practice.module.css"),
]);
const failures = [];

expectContains(
  primitive,
  "color: var(--ostudy-color-accent-on)",
  "Button primitive semantic accent foreground",
);
expectContains(
  sharedControls,
  "color: var(--ostudy-color-accent-on)",
  "shared control semantic accent foreground",
);
expectContains(
  practice,
  "color: var(--ostudy-color-confidence-on)",
  "practice confidence semantic foreground",
);
expectContains(
  practice,
  "color: var(--ostudy-color-status-danger-on)",
  "practice destructive-action semantic foreground",
);

for (const theme of ["light", "dark", "console", "synthwave"]) {
  const source = await read(`src/ui/tokens/generated/theme-${theme}.css`);
  expectContains(source, "--ostudy-color-accent-on:", `${theme} accent foreground token`);
  expectContains(source, "--ostudy-color-confidence-on:", `${theme} confidence foreground token`);
  expectContains(source, "--ostudy-color-status-danger-on:", `${theme} danger foreground token`);
}

if (failures.length > 0) {
  console.error("Button contrast contract failed:");
  for (const failure of failures) console.error(`  - ${failure}`);
  process.exit(1);
}
console.log(
  "OK   Accent, confidence, and destructive controls use theme-specific semantic foreground tokens.",
);

async function read(relativePath) {
  return readFile(join(root, relativePath), "utf8");
}

function expectContains(source, expected, label) {
  if (!source.includes(expected)) failures.push(`Missing ${label}: ${expected}`);
}
