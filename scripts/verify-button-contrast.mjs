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

// During the ratchet phase the legacy controls still render, so a later theme
// layer must override their historical hard-coded foreground. Stage 5 removes
// these selectors; zero-legacy mode deliberately removes this assertion.
if (baseline.mode === "ratchet" && baseline.legacyFiles?.["src/styles/base/application.css"]) {
  expectContains(foundations, ".primary-button,", "transitional primary-button contrast override");
  expectContains(foundations, ".small-primary-button,", "transitional small-primary-button contrast override");
  expectContains(foundations, ".practice-reveal-button {", "transitional practice-reveal-button contrast override");
  expectContains(foundations, "color: var(--on-accent);", "transitional semantic on-accent foreground");
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
