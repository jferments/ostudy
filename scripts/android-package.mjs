#!/usr/bin/env node

import { existsSync, readdirSync, statSync } from "node:fs";
import { resolve, dirname, relative } from "node:path";
import { fileURLToPath } from "node:url";
import { spawnSync } from "node:child_process";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const outputRoot = resolve(root, "src-tauri/gen/android/app/build/outputs");
const command = process.argv[2] ?? "help";

function run(program, args, extraEnv = {}) {
  const result = spawnSync(program, args, {
    cwd: root,
    env: { ...process.env, ...extraEnv },
    stdio: "inherit",
    shell: process.platform === "win32",
  });
  if (result.error) throw result.error;
  if (result.status !== 0) process.exit(result.status ?? 1);
}

function walk(directory, extension) {
  if (!existsSync(directory)) return [];
  const results = [];
  for (const entry of readdirSync(directory, { withFileTypes: true })) {
    const path = resolve(directory, entry.name);
    if (entry.isDirectory()) results.push(...walk(path, extension));
    else if (entry.isFile() && entry.name.endsWith(extension)) results.push(path);
  }
  return results;
}

function newestArtifact(extension, predicate = () => true) {
  const matches = walk(outputRoot, extension)
    .filter(predicate)
    .sort((left, right) => statSync(right).mtimeMs - statSync(left).mtimeMs);
  if (matches.length === 0) {
    throw new Error(`No ${extension} artifact exists under ${relative(root, outputRoot)}.`);
  }
  return matches[0];
}

function buildDebugApk() {
  run("pnpm", ["exec", "tauri", "android", "build", "--debug", "--apk"]);
  return newestArtifact(".apk", (path) => !path.includes("androidTest"));
}

function buildReleaseApk() {
  run("pnpm", ["exec", "tauri", "android", "build", "--apk"]);
  return newestArtifact(".apk", (path) => !path.includes("androidTest"));
}

function buildReleaseBundle() {
  run("pnpm", ["exec", "tauri", "android", "build", "--aab"]);
  return newestArtifact(".aab");
}

function printArtifact(path) {
  console.log(`\nostudy Android artifact:\n${path}\n`);
}

switch (command) {
  case "build-debug":
    printArtifact(buildDebugApk());
    break;
  case "install-debug": {
    const apk = buildDebugApk();
    run("adb", ["install", "-r", "-d", apk]);
    printArtifact(apk);
    break;
  }
  case "install": {
    const supplied = process.argv[3];
    if (!supplied) throw new Error("Usage: node scripts/android-package.mjs install <apk-path>");
    const apk = resolve(process.cwd(), supplied);
    if (!existsSync(apk)) throw new Error(`APK does not exist: ${apk}`);
    run("adb", ["install", "-r", "-d", apk]);
    printArtifact(apk);
    break;
  }
  case "build-release":
    printArtifact(buildReleaseApk());
    break;
  case "bundle-release":
    printArtifact(buildReleaseBundle());
    break;
  case "artifacts": {
    const artifacts = [...walk(outputRoot, ".apk"), ...walk(outputRoot, ".aab")]
      .sort((left, right) => statSync(right).mtimeMs - statSync(left).mtimeMs);
    if (artifacts.length === 0) console.log("No Android APK or AAB artifacts have been built.");
    else artifacts.forEach((path) => console.log(path));
    break;
  }
  default:
    console.log(`ostudy Android packaging\n\nCommands:\n  build-debug     Build an installable debug APK\n  install-debug   Build and install the debug APK on the connected device\n  build-release   Build a release APK\n  bundle-release  Build a release AAB\n  install <apk>   Install an existing APK\n  artifacts       List built APK/AAB artifacts\n`);
}
