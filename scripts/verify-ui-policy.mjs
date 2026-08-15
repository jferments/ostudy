#!/usr/bin/env node

import { access, readFile } from "node:fs/promises";
import { spawnSync } from "node:child_process";
import { join, relative } from "node:path";
import { fileURLToPath } from "node:url";

const root = fileURLToPath(new URL("..", import.meta.url));
const failures = [];

const tokenCheck = spawnSync(
  process.execPath,
  [join(root, "scripts/generate-ui-tokens.mjs"), "--check"],
  { cwd: root, encoding: "utf8" },
);
if (tokenCheck.status !== 0) {
  failures.push(
    tokenCheck.stderr || tokenCheck.stdout || "Generated UI tokens are stale.",
  );
}

const requiredFiles = [
  "src/ui/tokens/themes/light.tokens.json",
  "src/ui/tokens/themes/dark.tokens.json",
  "src/ui/tokens/themes/console.tokens.json",
  "src/ui/tokens/themes/synthwave.tokens.json",
  "src/ui/artwork/ARTWORK-LICENSES.md",
];
for (const path of requiredFiles) {
  try {
    await access(join(root, path));
  } catch {
    failures.push(`Missing required UI-system file: ${path}`);
  }
}

const governedCss = [
  "src/ui/feedback/experience.module.css",
  "src/ui/tutorials/TourProvider.module.css",
  "src/ui/help/Help.module.css",
  "src/ui/patterns/Metrics.module.css",
  "src/screens/flashcards/FlashcardsWorkspace.module.css",
  "src/screens/flashcards/DeckLibraryWorkspace.module.css",
  "src/screens/cards/CardFilters.module.css",
  "src/screens/practice/ActivePractice.module.css",
  "src/screens/practice/PracticeComplete.module.css",
  "src/screens/DocumentsSection.module.css",
];

for (const path of governedCss) {
  const absolute = join(root, path);
  let source;
  try {
    source = await readFile(absolute, "utf8");
  } catch {
    failures.push(`Missing governed CSS module: ${path}`);
    continue;
  }

  for (const match of source.matchAll(/z-index\s*:\s*(-?\d+)\s*;/gu)) {
    const value = Number(match[1]);
    if (Math.abs(value) > 1) {
      failures.push(
        `${path} uses numeric z-index ${value}; use a generated layer token.`,
      );
    }
  }

  if (/(?:^|[\s:,(])#[0-9a-f]{3,8}\b|\brgba?\(|\bhsla?\(/iu.test(source)) {
    failures.push(`${path} contains a raw color literal; use semantic tokens.`);
  }

  const declarations = source.match(/(?:animation|transition)[^;]*;/gu) ?? [];
  for (const declaration of declarations) {
    if (/\b\d+(?:\.\d+)?m?s\b/u.test(declaration)) {
      failures.push(
        `${path} contains an arbitrary animation duration in: ${declaration.trim()}`,
      );
    }
  }
}

const governedTypeScript = [
  "src/screens/flashcards/DeckLibraryWorkspace.tsx",
  "src/screens/practice/PracticeComplete.tsx",
  "src/screens/DocumentsDevelopmentStub.tsx",
  "src/ui/artwork/Artwork.tsx",
];
for (const path of governedTypeScript) {
  const source = await readFile(join(root, path), "utf8");
  if (/from\s+["'][^"']+\.(?:png|jpe?g|webp|svg)["']/iu.test(source)) {
    failures.push(
      `${path} imports an artwork file directly; use the semantic artwork registry.`,
    );
  }
}

if (failures.length > 0) {
  console.error("UI policy verification failed:\n");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log(
  `OK   UI token generation, theme completeness, governed styles, layer usage, timing, and artwork metadata are valid (${relative(root, import.meta.filename)}).`,
);
