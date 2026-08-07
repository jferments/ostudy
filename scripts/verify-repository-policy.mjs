#!/usr/bin/env node

import { readdir, readFile } from "node:fs/promises";
import { join, relative } from "node:path";
import { fileURLToPath } from "node:url";

const root = fileURLToPath(new URL("..", import.meta.url));
const failures = [];
const excludedDirectories = new Set([".git", "node_modules", "dist", "target", "gen", "artifacts"]);
const allFiles = await collectFiles(root);

const rustFiles = allFiles.filter((path) => path.endsWith(".rs"));
for (const path of rustFiles) {
  const text = await readFile(path, "utf8");
  const displayPath = relative(root, path).replaceAll("\\", "/");
  const staleSuppression = /#\[(?:allow|expect)\s*\(\s*(?:dead_code|unused|unused_imports|unused_variables)\s*\)\]/gu;
  for (const match of text.matchAll(staleSuppression)) {
    failures.push(`${displayPath}:${lineNumber(text, match.index ?? 0)}: stale-code diagnostics are suppressed`);
  }
}

const workflowDirectory = join(root, ".github", "workflows");
for (const entry of await readdir(workflowDirectory, { withFileTypes: true })) {
  if (!entry.isFile() || !/\.ya?ml$/u.test(entry.name)) continue;
  if (entry.name.startsWith("one-off-")) failures.push(`One-off mutation workflow is committed: ${entry.name}`);
  const path = join(workflowDirectory, entry.name);
  const text = await readFile(path, "utf8");
  if (/runs-on:\s*(?:ubuntu|windows|macos)-/u.test(text)) {
    failures.push(`${entry.name}: GitHub-hosted operating-system runner is forbidden`);
  }
  for (const [index, line] of text.split(/\r?\n/u).entries()) {
    const match = line.match(/^\s*uses:\s*([^\s#]+)(?:\s+#\s*(.*))?$/u);
    if (!match || match[1].startsWith("./")) continue;
    const at = match[1].lastIndexOf("@");
    const action = at >= 0 ? match[1].slice(0, at) : match[1];
    const ref = at >= 0 ? match[1].slice(at + 1) : "";
    if (!/^[0-9a-f]{40}$/u.test(ref)) {
      failures.push(`${entry.name}:${index + 1}: ${action} must be pinned to an immutable 40-character commit SHA`);
    }
    if (!/^v\d/u.test((match[2] ?? "").trim())) {
      failures.push(`${entry.name}:${index + 1}: pinned action ${action} needs a human-readable version comment`);
    }
  }
}

const authoringFiles = allFiles.filter((path) =>
  relative(root, path).replaceAll("\\", "/").startsWith("src-tauri/src/authoring"),
);
const authoringText = (
  await Promise.all(authoringFiles.filter((path) => path.endsWith(".rs")).map((path) => readFile(path, "utf8")))
).join("\n");
for (const [label, pattern] of [
  ["process execution", /\b(?:std::process|Command::new\s*\()/u],
  ["network client", /\b(?:reqwest|ureq|hyper::Client)\b/u],
]) {
  if (pattern.test(authoringText)) failures.push(`Authoring/template subsystem contains forbidden ${label}.`);
}
const frontendText = (
  await Promise.all(
    allFiles
      .filter((path) => /\.[cm]?[jt]sx?$/u.test(path) && relative(root, path).startsWith("src"))
      .map((path) => readFile(path, "utf8")),
  )
).join("\n");
for (const forbidden of ["eval(", "new Function"]) {
  if (frontendText.includes(forbidden)) failures.push(`Frontend contains executable dynamic-code primitive: ${forbidden}`);
}

if (failures.length > 0) {
  console.error("Repository policy verification failed:");
  for (const failure of failures) console.error(`  - ${failure}`);
  process.exit(1);
}

console.log(
  `OK   Repository policy: workflows are immutable/self-hosted, stale-code suppressions are absent, and authoring remains data-only.`,
);

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
