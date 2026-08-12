#!/usr/bin/env node

import { access, readFile, readdir } from "node:fs/promises";
import { join, relative, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const ownRoot = fileURLToPath(new URL("..", import.meta.url));
const requestedRoot = process.argv.find((argument) => argument.startsWith("--root="));
const root = resolve(requestedRoot ? requestedRoot.slice("--root=".length) : ownRoot);
const manifestPath = join(root, "scripts", "policy", "platform-boundary-exceptions.json");
const manifest = JSON.parse(await readFile(manifestPath, "utf8"));
if (manifest.schemaVersion !== 1 || !Array.isArray(manifest.exceptions)) {
  throw new Error("Invalid platform-boundary exception manifest.");
}

const exceptions = new Map();
for (const entry of manifest.exceptions) {
  const key = `${entry.rule}:${entry.path}`;
  if (exceptions.has(key)) throw new Error(`Duplicate platform-boundary exception: ${key}`);
  exceptions.set(key, { ...entry, matched: false });
}

const files = await collectFiles(root);
const violations = [];

for (const path of files) {
  const displayPath = relative(root, path).replaceAll("\\", "/");
  if (!/\.(?:rs|ts|tsx)$/u.test(displayPath)) continue;
  if (/(?:^|\/)[^/]+\.(?:test|spec)\.(?:ts|tsx)$/u.test(displayPath)) continue;
  if (
    displayPath.startsWith("src-tauri/src/")
    && (
      displayPath.includes("/tests/")
      || /(?:^|\/)(?:[^/]*_tests|conformance_tests|operation_tests)\.rs$/u.test(displayPath)
    )
  ) continue;
  const text = await readFile(path, "utf8");

  if (displayPath.startsWith("src/")) {
    if (
      /from\s+["']@tauri-apps\/api\/core["']/u.test(text)
      && !displayPath.startsWith("src/ipc/")
      && !displayPath.startsWith("src/platform/")
    ) {
      report("frontend-direct-tauri-core-import", displayPath);
    }
    if (/\bnavigator\.userAgent\b/u.test(text)) {
      report("frontend-user-agent-platform-detection", displayPath);
    }
    if (
      displayPath.startsWith("src/ipc/")
      && /\b(?:sourcePath|destinationPath|filePath)\??\s*:/u.test(text)
    ) {
      report("frontend-raw-path-contract", displayPath);
    }
  }

  if (displayPath.startsWith("src-tauri/src/")) {
    if (/\bpub\s+(?:source_path|destination_path|file_path)\s*:\s*String\b/u.test(text)) {
      report("rust-raw-path-contract", displayPath);
    }
    if (/\bblocking_(?:pick_file|pick_files|pick_folder|pick_folders|save_file)\s*\(/u.test(text)) {
      report("rust-blocking-native-dialog", displayPath);
    }
    const platformAdapter = displayPath.startsWith("src-tauri/src/platform/");
    if (!platformAdapter && /\bkeyring(?:::|\b)/u.test(text)) report("rust-keyring-outside-adapter", displayPath);
    if (!platformAdapter && /\bsysinfo(?:::|\b)/u.test(text)) report("rust-sysinfo-outside-adapter", displayPath);
    if (!platformAdapter && /\bSystemTime(?:::|\b)/u.test(text)) report("rust-direct-system-clock", displayPath);
    if (!platformAdapter && (
      /\bchrono::(?:[^;\n]*\bLocal\b|\{[^}]*\bLocal\b)/u.test(text)
      || /\/etc\/(?:localtime|timezone)/u.test(text)
    )) {
      report("rust-direct-local-time-zone", displayPath);
    }
    if (!platformAdapter && /\btarget_os\s*=\s*"/u.test(text)) report("rust-target-os-branch", displayPath);
    if (!platformAdapter && /\b(?:std::process::Command|Command::new)\b/u.test(text)) {
      report("rust-process-command-outside-adapter", displayPath);
    }
  }
}

for (const [key, entry] of exceptions) {
  if (!entry.matched) {
    violations.push(
      `${entry.path}: stale platform-boundary exception ${key}; remove the exception entry`,
    );
  }
}

if (violations.length > 0) {
  console.error("Platform boundary verification failed:");
  for (const violation of violations) console.error(`  - ${violation}`);
  process.exit(1);
}

console.log(
  `OK   Platform boundaries: no unlisted debt; ${exceptions.size} temporary exceptions remain tracked.`,
);

function report(rule, displayPath) {
  const key = `${rule}:${displayPath}`;
  const exception = exceptions.get(key);
  if (exception) {
    exception.matched = true;
    return;
  }
  violations.push(`${displayPath}: unlisted ${rule} violation`);
}

async function collectFiles(directory) {
  const output = [];
  for (const entry of await readdir(directory, { withFileTypes: true })) {
    if ([".git", "node_modules", "dist", "target", "gen", "artifacts"].includes(entry.name)) {
      continue;
    }
    const path = join(directory, entry.name);
    if (entry.isDirectory()) output.push(...(await collectFiles(path)));
    else if (entry.isFile()) output.push(path);
  }
  return output;
}

await access(manifestPath);
