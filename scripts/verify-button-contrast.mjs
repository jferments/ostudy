#!/usr/bin/env node
import { readFile } from "node:fs/promises";
import { join } from "node:path";
import { fileURLToPath } from "node:url";

const root = fileURLToPath(new URL("..", import.meta.url));
const [primitive, sharedControls, practice, activePractice] = await Promise.all([
  read("src/ui/vendor/shadcn/button.tsx"),
  read("src/ui/vendor/shadcn/form-controls.tsx"),
  read("src/screens/practice/presentation.ts"),
  read("src/screens/practice/ActivePractice.tsx"),
]);
const failures = [];
const themeBridge = await read("src/ui/tokens/generated/tailwind-theme.css");

// Nova uses a soft destructive surface with destructive text rather than a filled
// destructive background. Verify the semantic role, not the retired migration-era recipe.
expectContains(primitive, "tw:text-destructive", "Button destructive semantic foreground");
expectContains(primitive, "tw:bg-destructive-soft", "Button destructive semantic surface");
expectContains(themeBridge, "--color-primary-foreground: var(--ostudy-color-accent-on)", "Tailwind accent foreground mapping");
expectContains(themeBridge, "--color-destructive-foreground: var(--ostudy-color-status-danger-on)", "Tailwind destructive foreground mapping");
expectContains(primitive, "tw:text-primary-foreground", "Button primitive semantic accent foreground");
expectContains(sharedControls, "tw:text-primary-foreground", "shared control semantic accent foreground");
expectContains(practice, "tw:text-(--ostudy-color-confidence-on)", "practice confidence semantic foreground");
expectContains(activePractice, 'variant="danger" data-practice-action="end"', "practice destructive-action semantic intent");

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
console.log("OK   Accent, confidence, and destructive controls use theme-specific semantic roles.");

async function read(path) {
  return readFile(join(root, path), "utf8");
}
function expectContains(source, expected, label) {
  if (!source.includes(expected)) failures.push(`Missing ${label}: ${expected}`);
}
