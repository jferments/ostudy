#!/usr/bin/env node

import { readdir, readFile } from "node:fs/promises";
import { extname, join, relative } from "node:path";
import { fileURLToPath } from "node:url";

const root = fileURLToPath(new URL("..", import.meta.url));
const checkedRoots = [
  "src/ui",
  "src/screens/flashcards",
  "src/screens/cards/CardFilters.tsx",
  "src/screens/cards/CardFilters.module.css",
  "src/screens/practice/ActivePractice.tsx",
  "src/screens/practice/ActivePractice.module.css",
  "src/screens/practice/PracticeComplete.tsx",
  "src/screens/practice/PracticeComplete.module.css",
  "src/screens/DocumentsSection.tsx",
  "src/screens/DocumentsSection.module.css",
];
const ignoredPrefixes = [
  "src/ui/tokens/generated/",
  "src/ui/tokens/source/",
  "src/ui/tokens/themes/",
];
const checkedExtensions = new Set([".css", ".ts", ".tsx"]);
const errors = [];

for (const configured of checkedRoots) {
  for (const path of await collect(join(root, configured))) {
    const repositoryPath = relative(root, path).replaceAll("\\", "/");
    if (ignoredPrefixes.some((prefix) => repositoryPath.startsWith(prefix))) continue;
    if (!checkedExtensions.has(extname(path))) continue;
    const content = await readFile(path, "utf8");
    verifyFile(repositoryPath, content);
  }
}

verifyRegistryRelationships(
  await readFile(join(root, "src/ui/help/helpRegistry.ts"), "utf8"),
  await readFile(join(root, "src/ui/tutorials/tutorialRegistry.ts"), "utf8"),
);

if (errors.length > 0) {
  console.error("UI modernization policy violations:");
  for (const error of errors) console.error(`- ${error}`);
  process.exit(1);
}

console.log("OK   semantic UI architecture policy");

function verifyFile(path, content) {
  if (path.endsWith(".css")) {
    const stripped = content.replaceAll(/\/\*[\s\S]*?\*\//gu, "");
    for (const match of stripped.matchAll(/#[0-9a-fA-F]{3,8}\b/gu)) {
      errors.push(`${path} contains raw color ${match[0]}; use a semantic token.`);
    }
    for (const match of stripped.matchAll(/\bz-index\s*:\s*(-?\d+)\b/gu)) {
      errors.push(`${path} contains numeric z-index ${match[1]}; use a layer token.`);
    }
  }

  if (/src\/screens\//u.test(path) && /data-theme|settings\.theme|switch\s*\([^)]*theme/gu.test(content)) {
    errors.push(`${path} branches feature behavior on a theme; use semantic tokens instead.`);
  }

  if (!path.includes("/artwork/") && /(?:import|from)\s*[('"].*\/(?:artwork|illustrations)\//gu.test(content)) {
    errors.push(`${path} imports artwork directly; use the semantic Artwork registry.`);
  }

  if (path.endsWith("DeckLibraryWorkspace.tsx")) {
    if (!content.includes('draggable={!disabled}') || !content.includes(">Move<")) {
      errors.push(`${path} must keep dragging optional and expose an explicit Move action.`);
    }
  }
}

function verifyRegistryRelationships(help, tutorials) {
  const helpIds = new Set(
    [...help.matchAll(/^\s{2}"([^"]+)"\s*:\s*\{/gmu)].map((match) => match[1]),
  );
  for (const match of tutorials.matchAll(/helpTopicId:\s*"([^"]+)"/gu)) {
    if (!helpIds.has(match[1])) {
      errors.push(`Tutorial registry references unknown help topic ${match[1]}.`);
    }
  }

  const anchors = [...tutorials.matchAll(/anchor:\s*"([^"]+)"/gu)].map((match) => match[1]);
  for (const anchor of new Set(anchors)) {
    if (anchors.filter((candidate) => candidate === anchor).length > 1) {
      errors.push(`Tutorial anchor ${anchor} is registered more than once.`);
    }
  }
}

async function collect(path) {
  try {
    const entries = await readdir(path, { withFileTypes: true });
    const output = [];
    for (const entry of entries) {
      const child = join(path, entry.name);
      if (entry.isDirectory()) output.push(...(await collect(child)));
      else if (entry.isFile()) output.push(child);
    }
    return output;
  } catch (error) {
    if (error?.code === "ENOTDIR") return [path];
    if (error?.code === "ENOENT") {
      errors.push(`Configured UI policy path is missing: ${relative(root, path)}`);
      return [];
    }
    throw error;
  }
}
