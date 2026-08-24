#!/usr/bin/env node

import { access, readFile, readdir, stat } from "node:fs/promises";
import { join, relative, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const ownRoot = fileURLToPath(new URL("..", import.meta.url));
const requestedRoot = process.argv.find((argument) => argument.startsWith("--root="));
const root = resolve(requestedRoot ? requestedRoot.slice("--root=".length) : ownRoot);
const aiRoot = join(root, "src-tauri", "src", "ai");
const maximumModuleBytes = 48 * 1024;
const violations = [];

const requiredPackages = {
  gateway: [
    "mod.rs",
    "contracts.rs",
    "client.rs",
    "request.rs",
    "response.rs",
    "retry.rs",
    "errors.rs",
    "metadata.rs",
    "streaming/mod.rs",
    "streaming/sse.rs",
    "streaming/usage.rs",
  ],
  profiles: [
    "mod.rs",
    "contracts.rs",
    "commands.rs",
    "service.rs",
    "repository.rs",
    "policy.rs",
    "projections.rs",
    "defaults.rs",
  ],
  probes: [
    "mod.rs",
    "contracts.rs",
    "commands.rs",
    "runner.rs",
    "phases.rs",
    "capabilities.rs",
    "repository.rs",
  ],
};
const requiredTestModules = {
  gateway: ["redaction", "request", "response", "retry", "streaming", "transport", "usage"],
  profiles: ["contracts", "policy", "projections", "repository", "service"],
  probes: ["capabilities", "phases", "repository", "runner"],
};

// Device synchronization and startup compatibility inspect or sanitize complete
// database snapshots. They are reviewed schema-compatibility boundaries, not
// alternate runtime profile/probe repositories.
const profileCompatibilityOwners = new Set([
  "src-tauri/src/device_sync/credentials.rs",
  "src-tauri/src/device_sync/merge.rs",
  "src-tauri/src/device_sync/mod.rs",
  "src-tauri/src/device_sync/package.rs",
  "src-tauri/src/startup/pre_public_transition.rs",
]);
const probeCompatibilityOwners = new Set([
  "src-tauri/src/device_sync/credentials.rs",
  "src-tauri/src/device_sync/package.rs",
]);

for (const packageName of Object.keys(requiredPackages)) {
  const legacy = join(aiRoot, `${packageName}.rs`);
  if (await exists(legacy)) {
    violations.push(`${display(legacy)}: retired AI-infrastructure monolith must not exist`);
  }
  const packageRoot = join(aiRoot, packageName);
  for (const child of requiredPackages[packageName]) {
    const path = join(packageRoot, child);
    if (!(await exists(path))) {
      violations.push(`${display(path)}: required AI-infrastructure owner is missing`);
      continue;
    }
    const metadata = await stat(path);
    if (metadata.size > maximumModuleBytes) {
      violations.push(
        `${display(path)}: ${metadata.size} bytes exceeds the ${maximumModuleBytes}-byte anti-monolith limit`,
      );
    }
    const source = productionSource(await readFile(path, "utf8"));
    if (/#\s*\[\s*path\s*=/u.test(source)) {
      violations.push(`${display(path)}: path-attributed module aliases are forbidden`);
    }
  }

  const testsRoot = join(packageRoot, "tests");
  const testsModule = join(testsRoot, "mod.rs");
  if (!(await exists(testsModule))) {
    violations.push(`${display(testsModule)}: package test module root is missing`);
    continue;
  }
  const testsSource = await readFile(testsModule, "utf8");
  for (const moduleName of requiredTestModules[packageName]) {
    const testPath = join(testsRoot, `${moduleName}.rs`);
    if (!(await exists(testPath))) {
      violations.push(`${display(testPath)}: required package test module is missing`);
      continue;
    }
    const declaration = new RegExp(`\\bmod\\s+${escapeRegExp(moduleName)}\\s*;`, "u");
    if (!declaration.test(testsSource)) {
      violations.push(
        `${display(testsModule)}: test module ${moduleName}.rs exists but is not compiled`,
      );
    }
  }
}

const rustFiles = await collectFiles(join(root, "src-tauri", "src"), ".rs");
for (const path of rustFiles) {
  const source = productionSource(await readFile(path, "utf8"));
  const relativePath = display(path);
  const isGateway = relativePath.startsWith("src-tauri/src/ai/gateway/");
  const isGatewayClient = relativePath === "src-tauri/src/ai/gateway/client.rs";
  const isProfileCommand = relativePath === "src-tauri/src/ai/profiles/commands.rs";
  const isProbeCommand = relativePath === "src-tauri/src/ai/probes/commands.rs";
  const isProfileRepository = relativePath === "src-tauri/src/ai/profiles/repository.rs";
  const isProfileDefaults = relativePath === "src-tauri/src/ai/profiles/defaults.rs";
  const isProbeRepository = relativePath === "src-tauri/src/ai/probes/repository.rs";
  const isTest = relativePath.includes("/tests/")
    || /(?:^|\/)(?:[^/]+_tests|tests)\.rs$/u.test(relativePath);

  if (/\breqwest\b/u.test(source) && !isGateway) {
    violations.push(`${relativePath}: provider HTTP belongs only in ai/gateway/`);
  }
  if (/\bClient::builder\s*\(/u.test(source) && !isGatewayClient) {
    violations.push(`${relativePath}: AI HTTP client construction belongs only in gateway/client.rs`);
  }
  if (isGateway && /\b(?:rusqlite|Database|tauri::State|profiles::|probes::|providers::)\b/u.test(source)) {
    violations.push(
      `${relativePath}: gateway transport may not depend on persistence, Tauri, profiles, probes, or provider adapters`,
    );
  }
  if (
    relativePath.startsWith("src-tauri/src/ai/profiles/")
    && /\b(?:gateway::|providers::|probes::(?:runner|phases|capabilities))\b/u.test(source)
  ) {
    violations.push(`${relativePath}: profiles may not depend on transport, provider adapters, or probe execution`);
  }
  if (
    relativePath.startsWith("src-tauri/src/ai/profiles/")
    && (/\btauri::State\b/u.test(source) || /#\s*\[\s*tauri::command\s*\]/u.test(source))
    && !isProfileCommand
  ) {
    violations.push(`${relativePath}: profile Tauri state belongs only in commands.rs`);
  }
  if (
    relativePath.startsWith("src-tauri/src/ai/probes/")
    && (/\btauri::State\b/u.test(source) || /#\s*\[\s*tauri::command\s*\]/u.test(source))
    && !isProbeCommand
  ) {
    violations.push(`${relativePath}: probe Tauri state belongs only in commands.rs`);
  }

  if (/\b(?:ai_provider_profiles|ai_profile_operations|ai_task_defaults)\b/u.test(source)) {
    if (
      !isProfileRepository
      && !isProfileDefaults
      && !profileCompatibilityOwners.has(relativePath)
      && !isTest
    ) {
      violations.push(
        `${relativePath}: runtime profile/default SQL belongs to profiles/repository.rs or profiles/defaults.rs`,
      );
    }
  }

  if (/\b(?:ai_provider_probe_results|ai_provider_capabilities)\b/u.test(source)) {
    const readsOrWritesProbeEvidence = /(?:SELECT[^";]*FROM|INSERT\s+INTO|UPDATE)\s+(?:ai_provider_probe_results|ai_provider_capabilities)\b/iu.test(source);
    const profileInvalidationOnly = isProfileRepository && !readsOrWritesProbeEvidence;
    if (
      !isProbeRepository
      && !profileInvalidationOnly
      && !probeCompatibilityOwners.has(relativePath)
      && !isTest
    ) {
      violations.push(
        `${relativePath}: runtime probe/capability SQL belongs to probes/repository.rs`,
      );
    }
  }
}

const facadeRequirements = {
  gateway: ["AiGateway", "AiGatewayError", "SafeAiError"],
  profiles: ["commands", "ProfileSnapshot", "load_profile_request"],
  probes: ["commands", "AiProviderProbeResultDto", "load_document_probe_evidence"],
};
for (const [packageName, markers] of Object.entries(facadeRequirements)) {
  const path = join(aiRoot, packageName, "mod.rs");
  if (!(await exists(path))) continue;
  const source = productionSource(await readFile(path, "utf8"));
  for (const marker of markers) {
    if (!source.includes(marker)) {
      violations.push(`${display(path)}: stable facade marker ${marker} is missing`);
    }
  }
}

if (violations.length > 0) {
  console.error("AI-infrastructure architecture verification failed:");
  for (const violation of violations) console.error(`  - ${violation}`);
  process.exit(1);
}

console.log(
  "OK   AI infrastructure: bounded gateway, profile, and probe packages with explicit runtime network, command, persistence, executable-test, and audited compatibility owners.",
);

function productionSource(source) {
  const marker = source.search(/\n\s*#\s*\[\s*cfg\s*\(\s*test\s*\)\s*\]/u);
  return marker === -1 ? source : source.slice(0, marker);
}

async function collectFiles(directory, suffix) {
  const output = [];
  for (const entry of await readdir(directory, { withFileTypes: true })) {
    const path = join(directory, entry.name);
    if (entry.isDirectory()) output.push(...(await collectFiles(path, suffix)));
    else if (entry.isFile() && path.endsWith(suffix)) output.push(path);
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
