#!/usr/bin/env node

import { readFile } from "node:fs/promises";
import { join } from "node:path";
import { fileURLToPath } from "node:url";

const root = fileURLToPath(new URL("..", import.meta.url));
const [foundations, primitive] = await Promise.all([
  read("src/ui/foundations/index.css"),
  read("src/ui/primitives/Button.module.css"),
]);
const failures = [];

expectContains(foundations, ".primary-button,", "legacy primary-button contrast override");
expectContains(foundations, ".small-primary-button,", "legacy small-primary-button contrast override");
expectContains(foundations, ".practice-reveal-button {", "legacy practice-reveal-button contrast override");
expectContains(foundations, "color: var(--on-accent);", "semantic on-accent foreground");
expectContains(
  primitive,
  "color: var(--ostudy-color-accent-on)",
  "Button primitive semantic accent foreground",
);

for (const theme of ["light", "dark", "console", "synthwave"]) {
  const source = await read(`src/ui/tokens/generated/theme-${theme}.css`);
  expectContains(source, "--ostudy-color-accent-on:", `${theme} on-accent token`);
}

if (failures.length > 0) {
  console.error("Button contrast contract failed:");
  for (const failure of failures) console.error(`  - ${failure}`);
  process.exit(1);
}
console.log(
  "OK   Accent buttons use theme-specific semantic foreground tokens across legacy and primitive controls.",
);

async function read(relativePath) {
  return readFile(join(root, relativePath), "utf8");
}

function expectContains(source, expected, label) {
  if (!source.includes(expected)) failures.push(`Missing ${label}: ${expected}`);
}
