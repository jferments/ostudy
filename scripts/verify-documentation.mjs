#!/usr/bin/env node

import { execFileSync } from "node:child_process";
import { existsSync } from "node:fs";
import { readFile, stat } from "node:fs/promises";
import { basename, dirname, extname, relative, resolve, sep } from "node:path";
import { fileURLToPath } from "node:url";

const root = fileURLToPath(new URL("..", import.meta.url));
const failures = [];
const trackedFiles = execFileSync("git", ["ls-files", "-z"], {
  cwd: root,
  encoding: "utf8",
})
  .split("\0")
  .filter(Boolean)
  .map((path) => path.replaceAll("\\", "/"))
  // Qualification workflows intentionally validate an all-or-nothing working
  // tree before committing. Files scheduled for deletion still appear in
  // `git ls-files`, so omit only entries that are already absent from that
  // candidate tree while retaining every existing tracked file in the scan.
  .filter((path) => existsSync(resolve(root, path)));
const tracked = new Set(trackedFiles);

const allowedRootEntries = new Set([
  ".cargo",
  ".editorconfig",
  ".github",
  ".gitignore",
  ".node-version",
  ".nvmrc",
  "AGENTS.md",
  "CONTRIBUTING.md",
  "LICENSE",
  "README.md",
  "assets",
  "docs",
  "index.html",
  "package.json",
  "pnpm-lock.yaml",
  "pnpm-workspace.yaml",
  "run-app.sh",
  "rust-toolchain.toml",
  "scripts",
  "src",
  "src-tauri",
  "tests",
  "tsconfig.app.json",
  "tsconfig.json",
  "tsconfig.node.json",
  "versions.env",
  "vite.config.mjs",
  "vite.config.ts",
  "vitest.config.mjs",
  "vitest.config.ts",
]);

const rootEntries = new Set(
  trackedFiles.map((path) => path.split("/")[0]).filter((entry) => entry !== ""),
);
for (const entry of rootEntries) {
  if (!allowedRootEntries.has(entry)) failures.push(`Unexpected repository-root entry: ${entry}`);
}

const requiredFiles = [
  "AGENTS.md",
  "CONTRIBUTING.md",
  "README.md",
  "docs/README.md",
  "docs/architecture/overview.md",
  "docs/architecture/platform-services.md",
  "docs/architecture/ui.md",
  "docs/development/platform-portability-inventory.md",
  "docs/development/testing-and-ci.md",
  "docs/installation/development.md",
  "docs/installation/linux.md",
  "docs/installation/mobile.md",
  "docs/installation/windows.md",
  "docs/roadmap.md",
];
for (const path of requiredFiles) {
  if (!tracked.has(path)) failures.push(`Required maintained document is missing: ${path}`);
}

const forbiddenRootFiles = [
  "ARCHITECTURE.md",
  "BUILD.md",
  "DEVELOPMENT.md",
  "INSTALL.md",
  "INSTALLATION.md",
  "MIGRATIONS.md",
  "ROADMAP.md",
  "TESTING.md",
];
for (const path of forbiddenRootFiles) {
  if (tracked.has(path)) failures.push(`Documentation belongs under docs/: ${path}`);
}

const markdownFiles = trackedFiles.filter((path) => path.endsWith(".md"));
const maintainedRootMarkdown = markdownFiles.filter((path) => !path.includes("/"));
for (const path of maintainedRootMarkdown) {
  if (!new Set(["AGENTS.md", "CONTRIBUTING.md", "README.md"]).has(path)) {
    failures.push(`Unexpected maintained Markdown at repository root: ${path}`);
  }
}

const requiredDecisionFiles = [
  "docs/decisions/0026-runtime-platform-ports-and-composition.md",
  "docs/decisions/0027-opaque-external-files-and-managed-content-protocol.md",
  "docs/decisions/0028-secret-storage-and-device-key-isolation.md",
  "docs/decisions/0029-lifecycle-time-and-resource-observation.md",
  "docs/decisions/0030-durable-authoring-drafts-and-recovery.md",
];
for (const path of requiredDecisionFiles) {
  if (!tracked.has(path)) failures.push(`Required architecture decision is missing: ${path}`);
}

const readme = tracked.has("README.md") ? await readFile(resolve(root, "README.md"), "utf8") : "";
for (const heading of ["## Features", "## Installation", "## Documentation"]) {
  if (!readme.includes(heading)) failures.push(`README.md: missing ${heading}`);
}
for (const phrase of [
  "failed PDF generations available for retry",
  "bounded multi-core",
  "ordered durable page",
  "source-window-aware diagnostic",
]) {
  if (readme.toLowerCase().includes(phrase.toLowerCase())) {
    failures.push(`README.md: contains implementation-detail marketing copy: ${phrase}`);
  }
}

const roadmap = tracked.has("docs/roadmap.md")
  ? await readFile(resolve(root, "docs/roadmap.md"), "utf8")
  : "";
for (const heading of ["## Current priorities", "## Later work"]) {
  if (!roadmap.includes(heading)) failures.push(`docs/roadmap.md: missing ${heading}`);
}

const removedCanonicalPaths = [
  "docs/architecture.md",
  "docs/build.md",
  "docs/development.md",
  "docs/installation.md",
  "docs/migrations.md",
  "docs/testing.md",
];
const textExtensions = new Set([
  ".css",
  ".html",
  ".js",
  ".json",
  ".lock",
  ".md",
  ".mjs",
  ".py",
  ".rs",
  ".sh",
  ".sql",
  ".toml",
  ".ts",
  ".tsx",
  ".txt",
  ".yaml",
  ".yml",
]);
const textBasenames = new Set([
  ".editorconfig",
  ".gitignore",
  ".node-version",
  ".nvmrc",
  "LICENSE",
  "versions.env",
]);
const staleReferenceScanFiles = trackedFiles.filter(
  (path) =>
    path !== "scripts/verify-documentation.mjs"
    && !path.startsWith("docs/history/")
    && (textExtensions.has(extname(path)) || textBasenames.has(basename(path))),
);
for (const path of staleReferenceScanFiles) {
  const content = await readFile(resolve(root, path), "utf8");
  for (const stalePath of removedCanonicalPaths) {
    if (content.includes(stalePath)) failures.push(`${path}: references superseded path ${stalePath}`);
  }
}

const maintainedMarkdown = trackedFiles.filter(
  (path) => path.endsWith(".md") && !path.startsWith("docs/history/"),
);
let checkedLinks = 0;
for (const path of maintainedMarkdown) {
  const absolutePath = resolve(root, path);
  const original = await readFile(absolutePath, "utf8");
  const text = stripCode(original);
  const targets = [];
  for (const match of text.matchAll(/!?\[[^\]]*\]\(\s*(<[^>]+>|[^)\s]+)(?:\s+["'][^"']*["'])?\s*\)/gu)) {
    targets.push({ target: match[1], index: match.index ?? 0 });
  }
  for (const match of text.matchAll(/^\s*\[[^\]]+\]:\s*(<[^>]+>|\S+)/gmu)) {
    targets.push({ target: match[1], index: match.index ?? 0 });
  }
  for (const match of text.matchAll(/\b(?:href|src)=["']([^"']+)["']/gu)) {
    targets.push({ target: match[1], index: match.index ?? 0 });
  }

  for (const candidate of targets) {
    const target = normalizeTarget(candidate.target);
    if (target === null) continue;
    checkedLinks += 1;
    const [pathPart, fragment] = target.split("#", 2);
    const targetPath = pathPart === ""
      ? path
      : relative(root, resolve(dirname(absolutePath), decodeURIComponent(pathPart))).replaceAll(sep, "/");
    if (!tracked.has(targetPath)) {
      failures.push(`${path}: local link target is not tracked: ${target}`);
      continue;
    }
    if (fragment !== undefined && fragment !== "") {
      const targetText = await readFile(resolve(root, targetPath), "utf8");
      const anchors = markdownAnchors(targetText);
      if (!anchors.has(decodeURIComponent(fragment).toLowerCase())) {
        failures.push(`${path}: local link fragment does not exist: ${target}`);
      }
    }
  }
}

if (failures.length > 0) {
  console.error("Documentation verification failed:");
  for (const failure of failures) console.error(`  - ${failure}`);
  process.exit(1);
}

console.log(
  `OK   Documentation: ${maintainedMarkdown.length} maintained Markdown files and ${checkedLinks} local links are valid; stale canonical references are absent.`,
);

function stripCode(text) {
  return text
    .replace(/```[\s\S]*?```/gu, "")
    .replace(/`[^`\n]+`/gu, "");
}

function normalizeTarget(raw) {
  const unwrapped = raw.startsWith("<") && raw.endsWith(">") ? raw.slice(1, -1) : raw;
  if (
    unwrapped === ""
    || unwrapped.startsWith("#")
    || /^[a-z][a-z0-9+.-]*:/iu.test(unwrapped)
    || unwrapped.startsWith("//")
  ) {
    return unwrapped.startsWith("#") ? unwrapped : null;
  }
  return unwrapped;
}

function markdownAnchors(text) {
  const counts = new Map();
  const output = new Set();
  for (const line of stripCode(text).split("\n")) {
    const match = line.match(/^#{1,6}\s+(.+?)\s*#*\s*$/u);
    if (!match) continue;
    const base = match[1]
      .trim()
      .toLowerCase()
      .replace(/<[^>]+>/gu, "")
      .replace(/[^\p{L}\p{N}\s-]/gu, "")
      .replace(/\s+/gu, "-")
      .replace(/-+/gu, "-")
      .replace(/^-|-$/gu, "");
    const count = counts.get(base) ?? 0;
    counts.set(base, count + 1);
    output.add(count === 0 ? base : `${base}-${count}`);
  }
  return output;
}
