#!/usr/bin/env node

import { spawnSync } from "node:child_process";
import { readFileSync, writeFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const scriptDirectory = dirname(fileURLToPath(import.meta.url));
const root = process.env.OSTUDY_CARGO_LOCK_ROOT
  ? resolve(process.env.OSTUDY_CARGO_LOCK_ROOT)
  : join(scriptDirectory, "..");
const cargoCommand = process.env.OSTUDY_CARGO_COMMAND || "cargo";
const manifestPath = join(root, "src-tauri", "Cargo.toml");
const lockPath = join(root, "src-tauri", "Cargo.lock");
const [command, ...args] = process.argv.slice(2);

if (!command) {
  console.error("Usage: with-immutable-cargo-lock.mjs <command> [arguments...]");
  process.exit(64);
}

const gitWorkTree = spawnSync("git", ["rev-parse", "--is-inside-work-tree"], {
  cwd: root,
  encoding: "utf8",
  shell: process.platform === "win32",
});
if (gitWorkTree.status === 0 && gitWorkTree.stdout.trim() === "true") {
  const unstagedLockfile = spawnSync(
    "git",
    ["diff", "--quiet", "--", "src-tauri/Cargo.lock"],
    { cwd: root, stdio: "ignore", shell: process.platform === "win32" },
  );
  if (unstagedLockfile.status === 1) {
    console.error(
      "ERROR: src-tauri/Cargo.lock has unstaged changes before the build. " +
        "Builds are not allowed to rewrite it. Inspect with `git diff -- src-tauri/Cargo.lock`; " +
        "restore an accidental change with `git restore --worktree src-tauri/Cargo.lock`, " +
        "or stage an intentional dependency update before building.",
    );
    process.exit(65);
  }
}

let original;
try {
  original = readFileSync(lockPath);
} catch (error) {
  console.error(`ERROR: The committed Cargo lockfile is required: ${error.message}`);
  process.exit(66);
}

const metadata = spawnSync(
  cargoCommand,
  ["metadata", "--manifest-path", manifestPath, "--locked", "--no-deps", "--format-version", "1"],
  { cwd: root, stdio: "inherit", shell: process.platform === "win32" },
);
if (metadata.error) {
  console.error(`ERROR: Could not run Cargo lockfile validation: ${metadata.error.message}`);
  process.exit(1);
}
if (metadata.status !== 0) {
  console.error(
    "ERROR: Cargo.toml and src-tauri/Cargo.lock do not agree. Resolve and commit the lockfile intentionally before building.",
  );
  process.exit(metadata.status ?? 1);
}

const result = spawnSync(command, args, {
  cwd: root,
  env: process.env,
  stdio: "inherit",
  shell: process.platform === "win32",
});

let current = null;
try {
  current = readFileSync(lockPath);
} catch {
  // Treat deletion as a mutation and restore the committed bytes below.
}

if (current === null || !original.equals(current)) {
  writeFileSync(lockPath, original);
  console.error(
    "ERROR: The build attempted to modify src-tauri/Cargo.lock. The original file was restored. Dependency changes must be reviewed and committed separately.",
  );
  process.exit(67);
}

if (result.error) {
  console.error(`ERROR: Could not start ${command}: ${result.error.message}`);
  process.exit(1);
}
if (result.signal) {
  console.error(`${command} was terminated by signal ${result.signal}.`);
  process.exit(1);
}
process.exit(result.status ?? 1);
