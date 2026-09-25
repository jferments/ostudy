#!/usr/bin/env node

import { readFile } from "node:fs/promises";
import { fileURLToPath } from "node:url";

const root = fileURLToPath(new URL("..", import.meta.url));
const read = (path) => readFile(new URL(`../${path}`, import.meta.url), "utf8");
const failures = [];

const [
  baseConfigText,
  devConfigText,
  androidConfigText,
  profileSource,
  secretSource,
  devScript,
  buildScript,
  checkScript,
  buildState,
  smokeNative,
  smokeStartup,
  diagnostics,
  runApp,
  crashDiagnostic,
  releaseScript,
] = await Promise.all([
  read("src-tauri/tauri.conf.json"),
  read("src-tauri/tauri.dev.conf.json"),
  read("src-tauri/tauri.android.conf.json"),
  read("src-tauri/src/application_profile.rs"),
  read("src-tauri/src/platform/desktop/secrets.rs"),
  read("scripts/dev.sh"),
  read("scripts/build.sh"),
  read("scripts/check.sh"),
  read("scripts/build-state.sh"),
  read("scripts/smoke-native.sh"),
  read("scripts/smoke-startup-compatibility.sh"),
  read("scripts/lib/diagnostics.bash"),
  read("run-app.sh"),
  read("scripts/diagnose-last-crash.py"),
  read("scripts/release.sh"),
]);

const baseConfig = JSON.parse(baseConfigText);
const devConfig = JSON.parse(devConfigText);
const androidConfig = JSON.parse(androidConfigText);
const productionId = "io.github.jferments.ostudy";
const developmentId = "io.github.jferments.ostudy.dev";
const devConfigArgument = "--config src-tauri/tauri.dev.conf.json";

function equal(label, actual, expected) {
  if (actual !== expected) failures.push(`${label}: expected ${expected}, found ${String(actual)}`);
}

function requireText(label, source, text) {
  if (!source.includes(text)) failures.push(`${label} missing ${JSON.stringify(text)}`);
}

equal("production Tauri identifier", baseConfig.identifier, productionId);
equal("development Tauri identifier", devConfig.identifier, developmentId);
equal("Android debug suffix", androidConfig.bundle?.android?.debugApplicationIdSuffix, ".dev");

for (const [label, source] of [
  ["desktop hot reload", devScript],
  ["standalone development build", buildScript],
  ["full Linux qualification build", checkScript],
]) {
  requireText(label, source, devConfigArgument);
}

if (releaseScript.includes("tauri.dev.conf.json")) {
  failures.push("release.sh must build the production profile, not the development Tauri overlay");
}

for (const source of [smokeNative, smokeStartup, diagnostics]) {
  requireText("development diagnostic/test data", source, developmentId);
}

requireText("development launcher state", runApp, '/ostudy-dev"');
requireText("diagnostic launcher state", diagnostics, '/ostudy-dev"');
requireText("crash diagnostic launcher state", crashDiagnostic, '"ostudy-dev" / "ostudy.jsonl"');

requireText("build fingerprint", buildState, "src-tauri/tauri.dev.conf.json");
requireText("build stamp", buildState, "applicationProfile=development");
requireText("launcher guard", buildState, 'applicationProfile)" != "development"');
requireText("native production identity", profileSource, productionId);
requireText("native development identity", profileSource, developmentId);
requireText("debug production guard", profileSource, "debug/development build resolved the production oStudy identifier");
requireText("data-directory identity guard", profileSource, "resolved application data under");
requireText("dynamic keyring service", secretSource, "KeyringSecretStore::new(application_identifier)");
if (secretSource.includes('const KEYRING_SERVICE: &str = "io.github.jferments.ostudy"')) {
  failures.push("desktop keyring service must not be hard-coded to the production identifier");
}

if (failures.length > 0) {
  console.error("Development-profile verification failed:");
  for (const failure of failures) console.error(`  - ${failure}`);
  process.exit(1);
}

console.log(
  "OK   Production, desktop development, Android debug, credential, launcher, smoke-test, and release identities are separated.",
);
