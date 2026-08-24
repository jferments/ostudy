#!/usr/bin/env node

import { readFile } from "node:fs/promises";
import { fileURLToPath } from "node:url";
import { resolve } from "node:path";

const root = fileURLToPath(new URL("..", import.meta.url));
const cargo = await readFile(resolve(root, "src-tauri/Cargo.toml"), "utf8");
const envText = await readFile(resolve(root, "versions.env"), "utf8");
const env = new Map(
  envText
    .split(/\r?\n/u)
    .map((line) => line.trim())
    .filter((line) => line && !line.startsWith("#") && line.includes("="))
    .map((line) => {
      const index = line.indexOf("=");
      return [line.slice(0, index), line.slice(index + 1).replace(/^['"]|['"]$/gu, "")];
    }),
);

const revision = env.get("XBERG_GIT_REV");
if (!revision || !/^[0-9a-f]{40}$/u.test(revision)) {
  fail("versions.env must define a full 40-character XBERG_GIT_REV");
}
const dependencyLine = cargo
  .split(/\r?\n/u)
  .find((line) => line.trimStart().startsWith("xberg = "));
if (!dependencyLine) fail("src-tauri/Cargo.toml is missing the Xberg dependency");
for (const required of [
  'git = "https://github.com/xberg-io/xberg"',
  `rev = "${revision}"`,
  "default-features = false",
  'features = ["tokio-runtime", "office", "excel"]',
]) {
  if (!dependencyLine.includes(required)) fail(`Xberg dependency policy missing: ${required}`);
}
if (/^anydoc\s*=/mu.test(cargo)) fail("AnyDoc must not remain in the production dependency graph");

const roxmltreeVersion = env.get("ROXMLTREE_VERSION");
const zipVersion = env.get("ZIP_VERSION");
if (!roxmltreeVersion || !cargo.includes(`roxmltree = "=${roxmltreeVersion}"`)) {
  fail("The OPC compatibility parser must use the exact audited roxmltree pin");
}
const zipLine = cargo
  .split(/\r?\n/u)
  .find((line) => line.trimStart().startsWith("zip = "));
if (!zipVersion || !zipLine) fail("The OPC compatibility layer is missing its exact zip dependency");
for (const required of [
  `version = "=${zipVersion}"`,
  "default-features = false",
  'features = ["deflate-flate2"]',
]) {
  if (!zipLine.includes(required)) fail(`OPC zip dependency policy missing: ${required}`);
}

for (const feature of [
  "pdf",
  "pdf-native",
  "pdf-pdfium",
  "ocr",
  "ocr-pipeline",
  "paddle-ocr",
  "layout-detection",
  "embeddings",
  "transcription",
  "url-ingestion",
  "formats",
  "full",
  "android-target",
]) {
  const pattern = new RegExp(`features\\s*=\\s*\\[[^\\]]*['\"]${escapeRegExp(feature)}['\"]`, "u");
  if (pattern.test(dependencyLine)) fail(`Xberg feature '${feature}' is forbidden in the document-adapter build`);
}

console.log(`OK   Xberg is pinned to ${revision}; its office/excel and bounded OPC helper dependencies are exact.`);

function escapeRegExp(value) {
  return value.replace(/[.*+?^${}()|[\]\\]/gu, "\\$&");
}

function fail(message) {
  console.error(`ERROR: ${message}`);
  process.exit(1);
}
