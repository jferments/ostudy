#!/usr/bin/env node

import { access, readFile, readdir, stat } from "node:fs/promises";
import { join, relative, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const ownRoot = fileURLToPath(new URL("..", import.meta.url));
const requestedRoot = process.argv.find((argument) => argument.startsWith("--root="));
const root = resolve(requestedRoot ? requestedRoot.slice("--root=".length) : ownRoot);
const aiRoot = join(root, "src-tauri", "src", "ai");
const moduleRoot = join(aiRoot, "document_intelligence");
const aiModulePath = join(aiRoot, "mod.rs");
const requiredModules = [
  "mod.rs",
  "context.rs",
  "contracts.rs",
  "curation.rs",
  "flashcards.rs",
  "outline.rs",
  "schemas.rs",
  "support.rs",
  "transport.rs",
];
const forbiddenLegacyPaths = [
  join(aiRoot, "document_intelligence.rs"),
  join(aiRoot, "document_intelligence_adapter.rs"),
  join(moduleRoot, "legacy.rs"),
];
const maximumModuleBytes = 48 * 1024;
const providerWireTerms = [
  "response_format",
  "tool_choice",
  "chat_template_kwargs",
  "DEEPSEEK_STRICT_TOOL_NAME",
  "openrouter_document_body",
  "deepseek_strict_schema",
  "beta/chat/completions",
];
const violations = [];

await access(aiModulePath).catch(() => {
  violations.push("src-tauri/src/ai/mod.rs is missing");
});
await access(moduleRoot).catch(() => {
  violations.push("src-tauri/src/ai/document_intelligence/ is missing");
});

for (const path of forbiddenLegacyPaths) {
  if (await exists(path)) {
    violations.push(`${display(path)}: legacy document-intelligence entry point must not exist`);
  }
}

for (const name of requiredModules) {
  const path = join(moduleRoot, name);
  if (!(await exists(path))) {
    violations.push(`${display(path)}: required document-intelligence module is missing`);
    continue;
  }
  const metadata = await stat(path);
  if (metadata.size > maximumModuleBytes) {
    violations.push(
      `${display(path)}: ${metadata.size} bytes exceeds the ${maximumModuleBytes}-byte anti-monolith limit`,
    );
  }
}

if (await exists(aiModulePath)) {
  const aiModule = await readFile(aiModulePath, "utf8");
  if (!/\bpub\(crate\)\s+mod\s+document_intelligence\s*;/u.test(aiModule)) {
    violations.push(
      "src-tauri/src/ai/mod.rs: document_intelligence must be declared as a normal Rust module",
    );
  }
  if (/#\s*\[\s*path\s*=\s*["'][^"']*document_intelligence/u.test(aiModule)) {
    violations.push(
      "src-tauri/src/ai/mod.rs: path-attributed document-intelligence aliases are forbidden",
    );
  }
}

const facadePath = join(moduleRoot, "mod.rs");
if (await exists(facadePath)) {
  const facade = productionSource(await readFile(facadePath, "utf8"));
  for (const moduleName of requiredModules
    .filter((name) => name !== "mod.rs")
    .map((name) => name.slice(0, -3))) {
    if (!new RegExp(`\\bmod\\s+${escapeRegExp(moduleName)}\\s*;`, "u").test(facade)) {
      violations.push(
        `src-tauri/src/ai/document_intelligence/mod.rs: missing private ${moduleName} module declaration`,
      );
    }
  }
  if (/#\s*\[\s*path\s*=/u.test(facade)) {
    violations.push(
      "src-tauri/src/ai/document_intelligence/mod.rs: path-attributed child modules are forbidden",
    );
  }
}

// Provider wire dialects belong to ai/providers/. Document intelligence owns
// prompts, source grounding, semantic validation, request-size policy, and the
// provider-neutral call into that package; even transport.rs must not recreate
// DeepSeek/OpenRouter/llama.cpp request envelopes.
for (const name of requiredModules) {
  const path = join(moduleRoot, name);
  if (!(await exists(path))) continue;
  const source = productionSource(await readFile(path, "utf8"));
  for (const term of providerWireTerms) {
    if (source.includes(term)) {
      violations.push(
        `${display(path)}: provider wire term ${JSON.stringify(term)} belongs in ai/providers/`,
      );
    }
  }
}

const transportPath = join(moduleRoot, "transport.rs");
if (await exists(transportPath)) {
  const transport = productionSource(await readFile(transportPath, "utf8"));
  for (const forbidden of [
    "SemanticWindow",
    "document_semantics",
    "CanonicalDocument",
    "SourceLocator",
    "ProviderKind::",
  ]) {
    if (transport.includes(forbidden)) {
      violations.push(
        `src-tauri/src/ai/document_intelligence/transport.rs: provider-neutral transport must not depend on ${forbidden}`,
      );
    }
  }
  for (const required of [
    "request_json",
    "record_document_usage",
    "prepare_document_request",
    "decode_document_output",
    "post_document_json",
  ]) {
    if (!transport.includes(required)) {
      violations.push(
        `src-tauri/src/ai/document_intelligence/transport.rs: missing provider-neutral transport boundary ${required}`,
      );
    }
  }
}

if (violations.length > 0) {
  console.error("Document-AI architecture verification failed:");
  for (const violation of violations) console.error(`  - ${violation}`);
  process.exit(1);
}

const entries = await readdir(moduleRoot, { withFileTypes: true });
const rustFiles = entries.filter((entry) => entry.isFile() && entry.name.endsWith(".rs"));
console.log(
  `OK   Document-AI boundaries: ${rustFiles.length} bounded modules; provider wire dialects remain isolated in ai/providers/.`,
);

function productionSource(source) {
  const testMarker = source.search(/\n\s*#\s*\[\s*cfg\s*\(\s*test\s*\)\s*\]/u);
  return testMarker === -1 ? source : source.slice(0, testMarker);
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
