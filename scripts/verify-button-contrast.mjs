#!/usr/bin/env node

import { readFile } from "node:fs/promises";
import { join } from "node:path";
import { fileURLToPath } from "node:url";

const root = fileURLToPath(new URL("..", import.meta.url));
const [foundations, primitive, baseline] = await Promise.all([
  read("src/ui/foundations/index.css"),
  read("src/ui/primitives/Button.module.css"),
  readJson("scripts/style-ownership-baseline.json"),
]);
const failures = [];

expectContains(
  foundations,
  "--on-accent: var(--ostudy-color-accent-on);",
  "semantic on-accent compatibility alias",
);
expectContains(
  primitive,
  "color: var(--ostudy-color-accent-on)",
  "Button primitive semantic accent foreground",
);

for (const theme of ["light", "dark", "console", "synthwave"]) {
  const source = await read(`src/ui/tokens/generated/theme-${theme}.css`);
  expectContains(source, "--ostudy-color-accent-on:", `${theme} on-accent token`);
}

// During the ratchet phase the remaining legacy controls must keep the same
// contrast contract. Zero-legacy mode deliberately removes this branch.
if (baseline.mode === "ratchet" && baseline.legacyFiles?.["src/styles/base/application.css"]) {
  const legacy = await read("src/styles/base/application.css");
  expectContains(legacy, ".primary-button,", "legacy primary-button contrast override");
  expectContains(legacy, ".small-primary-button,", "legacy small-primary-button contrast override");
  expectContains(legacy, ".practice-reveal-button {", "legacy practice-reveal-button contrast override");
  expectContains(legacy, "color: var(--on-accent);", "legacy semantic on-accent foreground");
}

if (failures.length > 0) {
  console.error("Button contrast contract failed:");
  for (const failure of failures) console.error(`  - ${failure}`);
  process.exit(1);
}
console.log(
  "OK   Accent buttons use theme-specific semantic foreground tokens across active control owners.",
);

async function read(relativePath) {
  return readFile(join(root, relativePath), "utf8");
}

async function readJson(relativePath) {
  return JSON.parse(await read(relativePath));
}

function expectContains(source, expected, label) {
  if (!source.includes(expected)) failures.push(`Missing ${label}: ${expected}`);
}
