#!/usr/bin/env node

import { access, readFile, readdir, stat } from "node:fs/promises";
import { join, relative, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const ownRoot = fileURLToPath(new URL("..", import.meta.url));
const requestedRoot = process.argv.find((argument) => argument.startsWith("--root="));
const root = resolve(requestedRoot ? requestedRoot.slice("--root=".length) : ownRoot);
const sourceRoot = join(root, "src-tauri", "src");
const violations = [];
const packageNames = [
  "documents",
  "document_ingestion",
  "anydoc_document_adapter",
  "pdf_document_ingestion",
  "pdf_backend",
];
const forbiddenLegacyFiles = [
  "documents.rs",
  "document_adapters.rs",
  "document_ingestion.rs",
  "anydoc_document_adapter.rs",
  "pdf_backend.rs",
];
const forbiddenTransitionalDirectories = [
  "document_core",
  "document_ingestion_core",
  "anydoc_document_adapter_core",
  "pdf_document_ingestion_core",
  "pdf_backend_core",
];
const maximumModuleBytes = 64 * 1024;

const libPath = join(sourceRoot, "lib.rs");
if (!(await exists(libPath))) {
  violations.push("src-tauri/src/lib.rs is missing");
} else {
  const lib = productionSource(await readFile(libPath, "utf8"));
  for (const moduleName of packageNames) {
    if (!new RegExp(`^mod\\s+${escapeRegExp(moduleName)}\\s*;$`, "mu").test(lib)) {
      violations.push(`src-tauri/src/lib.rs: missing normal module declaration for ${moduleName}`);
    }
  }
  if (/#\s*\[\s*path\s*=/u.test(lib)) {
    for (const moduleName of packageNames) {
      if (new RegExp(`#\\s*\\[\\s*path[^\\]]*(?:${escapeRegExp(moduleName)}|document_adapters)`, "u").test(lib)) {
        violations.push(`src-tauri/src/lib.rs: path-attributed alias for ${moduleName} is forbidden`);
      }
    }
  }
}

for (const name of forbiddenLegacyFiles) {
  const path = join(sourceRoot, name);
  if (await exists(path)) {
    violations.push(`${display(path)}: retired root monolith/facade must not exist`);
  }
}
for (const name of forbiddenTransitionalDirectories) {
  const path = join(sourceRoot, name);
  if (await exists(path)) {
    violations.push(`${display(path)}: transitional package name must not exist`);
  }
}

for (const packageName of packageNames) {
  const packageRoot = join(sourceRoot, packageName);
  const facade = join(packageRoot, "mod.rs");
  if (!(await exists(facade))) {
    violations.push(`${display(facade)}: required package facade is missing`);
    continue;
  }
  for (const path of await rustFiles(packageRoot)) {
    const metadata = await stat(path);
    if (metadata.size > maximumModuleBytes) {
      violations.push(
        `${display(path)}: ${metadata.size} bytes exceeds the ${maximumModuleBytes}-byte anti-monolith limit`,
      );
    }
  }
}

const documentsRoot = join(sourceRoot, "documents");
if (await exists(documentsRoot)) {
  for (const path of await rustFiles(documentsRoot)) {
    const normalized = display(path);
    if (normalized === "src-tauri/src/documents/commands.rs") continue;
    const source = productionSource(await readFile(path, "utf8"));
    if (/\btauri::State\b/u.test(source) || /#\s*\[\s*tauri::command\s*\]/u.test(source)) {
      violations.push(
        `${normalized}: document core modules must use ordinary Rust dependencies; Tauri state/commands belong in commands.rs`,
      );
    }
  }
}

for (const path of await rustFiles(sourceRoot)) {
  const source = productionSource(await readFile(path, "utf8"));
  const normalized = display(path);
  if (
    /(?:^|\n)\s*use\s+anydoc(?:::|\s*\{)|\banydoc::(?:model|to_document|Format|ConvertError)\b/u.test(source) &&
    !normalized.startsWith("src-tauri/src/anydoc_document_adapter/")
  ) {
    violations.push(`${normalized}: third-party AnyDoc types must remain inside the AnyDoc package`);
  }
}

const contractsPath = join(sourceRoot, "document_ingestion", "contracts.rs");
if (await exists(contractsPath)) {
  const contracts = productionSource(await readFile(contractsPath, "utf8"));
  for (const required of [
    "trait DocumentAdapter",
    "CanonicalIngestion",
    "DocumentFormat",
    "IngestionProgressSink",
  ]) {
    if (!contracts.includes(required)) {
      violations.push(`${display(contractsPath)}: missing app-owned adapter contract ${required}`);
    }
  }
  if (/\banydoc::|\boxidize_pdf::|\bPdfDocument\b/u.test(contracts)) {
    violations.push(`${display(contractsPath)}: backend library types leaked into the app-owned port`);
  }
}

const registryPath = join(sourceRoot, "document_ingestion", "registry.rs");
if (await exists(registryPath)) {
  const registry = productionSource(await readFile(registryPath, "utf8"));
  for (const required of ["match format", "DocumentFormat::Pdf", "DocumentFormat::Docx", "DocumentFormat::Csv"]) {
    if (!registry.includes(required)) {
      violations.push(`${display(registryPath)}: missing explicit registry routing marker ${required}`);
    }
  }
  if (/\.find\s*\(\s*\|adapter\|\s*adapter\.supports\s*\(\s*format\s*\)\s*\)/u.test(registry)) {
    violations.push(`${display(registryPath)}: first-supporting-backend selection is forbidden`);
  }
}

const ingestionRoot = join(sourceRoot, "document_ingestion");
if (await exists(ingestionRoot)) {
  for (const path of await rustFiles(ingestionRoot)) {
    const source = productionSource(await readFile(path, "utf8"));
    for (const forbidden of [
      "document_intelligence",
      "document_learning_plan",
      "document_study_guide",
      "document_topic_links",
    ]) {
      if (source.includes(forbidden)) {
        violations.push(`${display(path)}: ingestion must not depend on downstream ${forbidden}`);
      }
    }
  }
}

if (violations.length > 0) {
  console.error("Document-processing architecture verification failed:");
  for (const violation of violations) console.error(`  - ${violation}`);
  process.exit(1);
}

let moduleCount = 0;
for (const packageName of packageNames) {
  moduleCount += (await rustFiles(join(sourceRoot, packageName))).length;
}
console.log(
  `OK   Document processing: ${packageNames.length} physical packages and ${moduleCount} bounded Rust modules; app-owned adapter routing and backend isolation are enforced.`,
);

function productionSource(source) {
  const testMarker = source.search(/\n\s*#\s*\[\s*cfg\s*\(\s*test\s*\)\s*\]/u);
  return testMarker === -1 ? source : source.slice(0, testMarker);
}

async function rustFiles(directory) {
  if (!(await exists(directory))) return [];
  const output = [];
  for (const entry of await readdir(directory, { withFileTypes: true })) {
    const path = join(directory, entry.name);
    if (entry.isDirectory()) output.push(...(await rustFiles(path)));
    else if (entry.isFile() && entry.name.endsWith(".rs")) output.push(path);
  }
  return output;
}

function display(path) {
  return relative(root, path).replaceAll("\\", "/");
}

function escapeRegExp(value) {
  return value.replace(/[.*+?^${}()|[\]\\]/gu, "\\$&");
}

async function exists(path) {
  try {
    await access(path);
    return true;
  } catch {
    return false;
  }
}
