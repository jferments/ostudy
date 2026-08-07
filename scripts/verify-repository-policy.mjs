#!/usr/bin/env node

import { access, readdir, readFile } from "node:fs/promises";
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
  if (/\binclude!\s*\(/u.test(text)) {
    failures.push(`${displayPath}: lexical include! source composition is forbidden; use ordinary Rust modules`);
  }
  const staleSuppression = /#\[(?:allow|expect)\s*\(\s*(?:dead_code|unused|unused_imports|unused_variables)\s*\)\]/gu;
  for (const match of text.matchAll(staleSuppression)) {
    failures.push(`${displayPath}:${lineNumber(text, match.index ?? 0)}: stale-code diagnostics are suppressed`);
  }
}

for (const path of allFiles) {
  const displayPath = relative(root, path).replaceAll("\\", "/");
  if (displayPath.startsWith("src-tauri/src/authoring/") && /(?:^|\/)stage\d/iu.test(displayPath)) {
    failures.push(`${displayPath}: production modules must use domain names rather than implementation-stage names`);
  }
}
for (const forbiddenPath of [
  "src-tauri/src/authoring.rs",
  "scripts/verify-stage2.mjs",
  "scripts/verify-stage3.mjs",
  "scripts/verify-stage33.mjs",
  "scripts/verify-stage34-35.mjs",
]) {
  if (await exists(join(root, forbiddenPath))) failures.push(`Superseded structural artifact remains: ${forbiddenPath}`);
}
if (!(await exists(join(root, "src-tauri", "src", "authoring", "mod.rs")))) {
  failures.push("Explicit authoring module root is missing: src-tauri/src/authoring/mod.rs");
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

const productionAuthoringFiles = allFiles.filter((path) => {
  const displayPath = relative(root, path).replaceAll("\\", "/");
  return displayPath.startsWith("src-tauri/src/authoring/")
    && path.endsWith(".rs")
    && !/(?:^|\/)(?:tests|history_tests|asset_tests)\.rs$/u.test(displayPath);
});
const authoringText = (
  await Promise.all(productionAuthoringFiles.map((path) => readFile(path, "utf8")))
).join("\n");
for (const [label, pattern] of [
  [
    "process execution",
    /\b(?:std::process::Command|use\s+std::process\s*(?:::?\s*Command|::\s*\{[^}]*\bCommand\b))/u,
  ],
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
  `OK   Repository policy: ${rustFiles.length} Rust files use explicit modules, workflows are immutable/self-hosted, and stale structural gates are absent.`,
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

async function exists(path) {
  try {
    await access(path);
    return true;
  } catch {
    return false;
  }
}

function lineNumber(text, index) {
  return text.slice(0, index).split("\n").length;
}
