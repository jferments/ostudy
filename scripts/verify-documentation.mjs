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
  .map((path) => path.replaceAll("\\", "/"));
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
  "plugins",
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

const rootEntries = new Set(trackedFiles.map((path) => path.split("/")[0]));
for (const entry of [...rootEntries].sort()) {
  if (!allowedRootEntries.has(entry)) {
    failures.push(
      `${entry}: tracked top-level entry is not a conventional repository entry point or tool-discovered configuration; place subject-specific material under docs/, scripts/, tests/, or the owning source directory`,
    );
  }
}

// Native Tauri plugins are independent Rust/Gradle source packages rather than
// application modules, so `plugins/` is a canonical source root. Keep it just as
// constrained as the other top-level roots instead of allowing arbitrary
// subject-specific directories to accumulate there.
const allowedPluginEntries = new Set(["plugins/ostudy-platform"]);
const pluginEntries = new Set(
  trackedFiles
    .filter((path) => path.startsWith("plugins/"))
    .map((path) => path.split("/").slice(0, 2).join("/")),
);
for (const entry of [...pluginEntries].sort()) {
  if (!allowedPluginEntries.has(entry)) {
    failures.push(`${entry}: unrecognized top-level native plugin package`);
  }
}
for (const path of [
  "plugins/ostudy-platform/Cargo.toml",
  "plugins/ostudy-platform/build.rs",
  "plugins/ostudy-platform/permissions/default.toml",
  "plugins/ostudy-platform/src/lib.rs",
  "plugins/ostudy-platform/android/build.gradle.kts",
  "plugins/ostudy-platform/android/src/main/java/OstudyPlatformPlugin.kt",
]) {
  if (!tracked.has(path)) failures.push(`Required native platform plugin source is missing: ${path}`);
}

const allowedDocsEntries = new Set([
  "README.md",
  "agents",
  "architecture",
  "decisions",
  "development",
  "examples",
  "history",
  "installation",
  "ostudy-flashcards-v1.schema.json",
  "reference",
  "roadmap.md",
  "user-guide",
]);
const docsEntries = new Set(
  trackedFiles
    .filter((path) => path.startsWith("docs/"))
    .map((path) => path.slice("docs/".length).split("/")[0]),
);
for (const entry of [...docsEntries].sort()) {
  if (!allowedDocsEntries.has(entry)) {
    failures.push(
      `docs/${entry}: maintained documentation must be placed in the user, installation, architecture, development, reference, decision, agent, example, or history section`,
    );
  }
}

const requiredPaths = [
  "README.md",
  "CONTRIBUTING.md",
  "AGENTS.md",
  "docs/README.md",
  "docs/roadmap.md",
  "docs/installation/README.md",
  "docs/installation/linux.md",
  "docs/installation/windows.md",
  "docs/installation/macos.md",
  "docs/installation/mobile.md",
  "docs/user-guide/getting-started.md",
  "docs/user-guide/ai-providers.md",
  "docs/user-guide/documents-and-ai-generation.md",
  "docs/user-guide/backups-imports-and-sharing.md",
  "docs/architecture/overview.md",
  "docs/architecture/ai-and-document-processing.md",
  "docs/architecture/card-format-and-data-model.md",
  "docs/architecture/ui.md",
  "docs/development/guide.md",
  "docs/development/workflow.md",
  "docs/development/testing-and-ci.md",
  "docs/development/dependency-audit.md",
  "docs/reference/generated-json-import.md",
  "docs/reference/flashcard-import-agent.md",
  "docs/history/README.md",
  "docs/ostudy-flashcards-v1.schema.json",
  "docs/examples/cardiovascular.ostudy.json",
  ".github/ISSUE_TEMPLATE/bug_report.yml",
  ".github/ISSUE_TEMPLATE/feature_request.yml",
  ".github/ISSUE_TEMPLATE/config.yml",
];
for (const path of requiredPaths) {
  if (!tracked.has(path)) failures.push(`Required maintained documentation is missing: ${path}`);
}

const removedCanonicalPaths = [
  "ARCHITECTURE-AND-PLAN.md",
  "DEPENDENCY-AUDIT.md",
  "VALIDATION-REPORT.md",
  "templates/0001_initial.sql",
  "docs/ARCHITECTURE.md",
  "docs/AI-ARCHITECTURE.md",
  "docs/CARD-FORMAT-AND-DATA-MODEL.md",
  "docs/UI-ARCHITECTURE.md",
  "docs/DEVELOPMENT-AND-QUALITY.md",
  "docs/GIT-WORKFLOW.md",
  "docs/OSTUDY-GIT-AND-CODING-WORKFLOW.md",
  "docs/GENERATED-JSON-IMPORT.md",
  "docs/OSTUDY-FLASHCARD-IMPORT-AGENT.md",
  "docs/PHASE-2-ROADMAP.md",
  "docs/STAGE-3-CARD-TYPES-AND-TEMPLATES.md",
  "docs/CODE-REVIEW-2026-08-03.md",
];
for (const path of removedCanonicalPaths) {
  if (tracked.has(path)) failures.push(`Stale or superseded canonical path remains tracked: ${path}`);
}

const textExtensions = new Set([
  ".cjs",
  ".css",
  ".env",
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
    existsSync(resolve(root, path))
    && path !== "scripts/verify-documentation.mjs"
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
  (path) =>
    existsSync(resolve(root, path))
    && path.endsWith(".md")
    && !path.startsWith("docs/history/"),
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
    const problem = await validateLocalTarget(path, target);
    if (problem) failures.push(`${path}:${lineNumber(original, candidate.index)}: ${problem}`);
  }
}

if (failures.length > 0) {
  console.error("Documentation and repository organization verification failed:");
  for (const failure of failures) console.error(`  - ${failure}`);
  process.exit(1);
}

console.log(
  `OK   Documentation: ${maintainedMarkdown.length} maintained Markdown files, ${checkedLinks} local links, and ${staleReferenceScanFiles.length} tracked text files verified; root and docs organization are canonical.`,
);

function stripCode(text) {
  let fence = null;
  return text
    .split(/\r?\n/u)
    .map((line) => {
      const fenceMatch = line.match(/^\s*(`{3,}|~{3,})/u);
      if (fenceMatch) {
        const marker = fenceMatch[1][0];
        if (fence === null) fence = marker;
        else if (fence === marker) fence = null;
        return "";
      }
      if (fence !== null) return "";
      return line.replace(/`[^`\n]*`/gu, "");
    })
    .join("\n");
}

function normalizeTarget(raw) {
  let target = raw.trim();
  if (target.startsWith("<") && target.endsWith(">")) target = target.slice(1, -1);
  if (target === "" || target.startsWith("#") || target.startsWith("//")) return null;
  if (/^(?:https?|mailto|tel):/iu.test(target)) return null;
  if (/^[a-z][a-z0-9+.-]*:/iu.test(target)) return `__UNSAFE_SCHEME__${target}`;
  return target;
}

async function validateLocalTarget(sourcePath, target) {
  if (target.startsWith("__UNSAFE_SCHEME__")) {
    return `unsupported or unsafe link scheme: ${target.slice("__UNSAFE_SCHEME__".length)}`;
  }

  const withoutFragment = target.split("#", 1)[0].split("?", 1)[0];
  if (withoutFragment === "") return null;

  let decoded;
  try {
    decoded = decodeURIComponent(withoutFragment);
  } catch {
    return `link target is not valid percent-encoding: ${target}`;
  }
  if (decoded.includes("\\")) return `local Markdown links must use forward slashes: ${target}`;

  const destination = decoded.startsWith("/")
    ? resolve(root, `.${decoded}`)
    : resolve(dirname(resolve(root, sourcePath)), decoded);
  const relativeDestination = relative(root, destination);
  if (relativeDestination === ".." || relativeDestination.startsWith(`..${sep}`)) {
    return `link escapes the repository: ${target}`;
  }

  try {
    const info = await stat(destination);
    if (!info.isFile() && !info.isDirectory()) return `link target is not a file or directory: ${target}`;
  } catch {
    return `broken local link: ${target}`;
  }
  return null;
}

function lineNumber(text, index) {
  return text.slice(0, index).split("\n").length;
}
