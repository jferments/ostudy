#!/usr/bin/env node

import { readdir, readFile } from "node:fs/promises";
import { join, relative } from "node:path";
import { fileURLToPath } from "node:url";

const root = fileURLToPath(new URL("..", import.meta.url));
const sourceRoot = join(root, "src");
const excludedDirectories = new Set(["node_modules", "dist", "target", "gen", "artifacts"]);
const files = (await collectFiles(sourceRoot)).filter((path) => /\.(?:css|ts|tsx)$/u.test(path));
const definitions = new Map();
const usages = [];

for (const path of files) {
  const text = await readFile(path, "utf8");
  const displayPath = relative(root, path).replaceAll("\\", "/");

  for (const match of text.matchAll(/(--ostudy-[a-z0-9-]+)\s*:/giu)) {
    recordDefinition(match[1], displayPath, text, match.index ?? 0);
  }
  for (const match of text.matchAll(/setProperty\(\s*["'](--ostudy-[a-z0-9-]+)["']/giu)) {
    recordDefinition(match[1], displayPath, text, match.index ?? 0);
  }
  for (const match of text.matchAll(/var\(\s*(--ostudy-[a-z0-9-]+)/giu)) {
    usages.push({
      name: match[1],
      path: displayPath,
      line: lineNumber(text, match.index ?? 0),
    });
  }
}

const failures = usages.filter((usage) => !definitions.has(usage.name));
if (failures.length > 0) {
  console.error("Semantic UI token usage verification failed:");
  for (const failure of failures) {
    console.error(
      `  - ${failure.path}:${failure.line}: ${failure.name} has no committed definition`,
    );
  }
  process.exit(1);
}

console.log(
  `OK   Semantic UI tokens: ${definitions.size} committed variables cover ${usages.length} frontend references.`,
);

function recordDefinition(name, path, text, index) {
  const existing = definitions.get(name) ?? [];
  existing.push({ path, line: lineNumber(text, index) });
  definitions.set(name, existing);
}

async function collectFiles(directory) {
  const entries = await readdir(directory, { withFileTypes: true });
  const output = [];
  for (const entry of entries) {
    if (excludedDirectories.has(entry.name)) continue;
    const path = join(directory, entry.name);
    if (entry.isDirectory()) output.push(...(await collectFiles(path)));
    else if (entry.isFile()) output.push(path);
  }
  return output;
}

function lineNumber(text, index) {
  return text.slice(0, index).split("\n").length;
}
