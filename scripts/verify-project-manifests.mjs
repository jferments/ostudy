#!/usr/bin/env node

import { createHash } from "node:crypto";
import { access, readdir, readFile } from "node:fs/promises";
import { join } from "node:path";
import { fileURLToPath } from "node:url";

const root = fileURLToPath(new URL("..", import.meta.url));
const read = (...parts) => readFile(join(root, ...parts), "utf8");
const failures = [];

const [
  envText,
  packageText,
  workspace,
  cargo,
  toolchain,
  tauriText,
  capabilityText,
  database,
  nodeVersion,
  nvmVersion,
  checkStatic,
  checkFast,
  checkFull,
  checkChanged,
  checkWorkflow,
  releaseWorkflow,
  pullRequestTemplate,
] = await Promise.all([
  read("versions.env"),
  read("package.json"),
  read("pnpm-workspace.yaml"),
  read("src-tauri", "Cargo.toml"),
  read("rust-toolchain.toml"),
  read("src-tauri", "tauri.conf.json"),
  read("src-tauri", "capabilities", "main.json"),
  read("src-tauri", "src", "database.rs"),
  read(".node-version"),
  read(".nvmrc"),
  read("scripts", "check-static.sh"),
  read("scripts", "check-fast.sh"),
  read("scripts", "check.sh"),
  read("scripts", "check-changed.sh"),
  read(".github", "workflows", "check.yml"),
  read(".github", "workflows", "release.yml"),
  read(".github", "pull_request_template.md"),
]);

const env = parseEnv(envText);
const pkg = JSON.parse(packageText);
const tauri = JSON.parse(tauriText);
const capability = JSON.parse(capabilityText);

function equal(label, actual, expected) {
  if (actual !== expected) failures.push(`${label}: expected ${String(expected)}, found ${String(actual)}`);
}
function requireText(label, text, needles) {
  for (const needle of needles) if (!text.includes(needle)) failures.push(`${label} missing: ${needle}`);
}

const packageVersions = {
  "@tauri-apps/api": env.get("TAURI_API_VERSION"),
  "@tauri-apps/cli": env.get("TAURI_CLI_VERSION"),
  "@tauri-apps/plugin-dialog": env.get("TAURI_DIALOG_VERSION"),
  "@tauri-apps/plugin-opener": env.get("TAURI_OPENER_VERSION"),
  react: env.get("REACT_VERSION"),
  "react-dom": env.get("REACT_VERSION"),
  "chart.js": env.get("CHART_JS_VERSION"),
  "react-chartjs-2": env.get("REACT_CHARTJS_2_VERSION"),
  "@types/react": env.get("REACT_TYPES_VERSION"),
  "@types/react-dom": env.get("REACT_DOM_TYPES_VERSION"),
  "@vitejs/plugin-react": env.get("VITE_REACT_PLUGIN_VERSION"),
  typescript: env.get("TYPESCRIPT_VERSION"),
  vite: env.get("VITE_VERSION"),
  esbuild: env.get("ESBUILD_VERSION"),
  vitest: env.get("VITEST_VERSION"),
  "@testing-library/react": env.get("TESTING_LIBRARY_REACT_VERSION"),
  "@testing-library/dom": env.get("TESTING_LIBRARY_DOM_VERSION"),
  jsdom: env.get("JSDOM_VERSION"),
};
for (const name of Object.keys(pkg.dependencies ?? {}).filter((name) => name.startsWith("@tiptap/"))) {
  packageVersions[name] = env.get("TIPTAP_VERSION");
}
for (const [name, expected] of Object.entries(packageVersions)) {
  const actual = pkg.dependencies?.[name] ?? pkg.devDependencies?.[name];
  equal(`package ${name}`, actual, expected);
  if (typeof actual === "string" && /[~^*<>]/u.test(actual)) failures.push(`${name} is not exactly pinned`);
}

equal("package name", pkg.name, "ostudy");
equal("package version", pkg.version, "0.1.0");
equal("packageManager", pkg.packageManager, `pnpm@${env.get("PNPM_VERSION")}`);
equal("Node engine", pkg.engines?.node, env.get("NODE_VERSION"));
equal("pnpm engine", pkg.engines?.pnpm, env.get("PNPM_VERSION"));
equal(".node-version", nodeVersion.trim(), env.get("NODE_VERSION"));
equal(".nvmrc", nvmVersion.trim(), env.get("NODE_VERSION"));
equal("license", pkg.license, "MIT");
equal("Cargo package name", tomlPackageValue(cargo, "name"), "ostudy");
equal("Cargo version", tomlPackageValue(cargo, "version"), pkg.version);
equal("Tauri product name", tauri.productName, "ostudy");
equal("Tauri identifier", tauri.identifier, "io.github.jferments.ostudy");
equal("Tauri version", tauri.version, pkg.version);

for (const [label, needle] of [
  ["Rust toolchain", `channel = "${env.get("RUST_VERSION")}"`],
  ["tauri", `version = "=${env.get("TAURI_RUST_VERSION")}"`],
  ["tauri-build", `version = "=${env.get("TAURI_BUILD_VERSION")}"`],
  ["rusqlite", `version = "=${env.get("RUSQLITE_VERSION")}"`],
  ["serde", `version = "=${env.get("SERDE_VERSION")}"`],
  ["serde_json", `serde_json = "=${env.get("SERDE_JSON_VERSION")}"`],
  ["chrono", `chrono = "=${env.get("CHRONO_VERSION")}"`],
  ["keyring", `keyring = "=${env.get("KEYRING_VERSION")}"`],
  ["reqwest", `version = "=${env.get("REQWEST_VERSION")}"`],
  ["rs-fsrs", `rs-fsrs = "=${env.get("RS_FSRS_VERSION")}"`],
  ["url", `url = "=${env.get("URL_VERSION")}"`],
]) {
  const source = label === "Rust toolchain" ? toolchain : cargo;
  if (!source.includes(needle)) failures.push(`${label} exact pin missing`);
}

for (const required of [
  "strictDepBuilds: true",
  "dangerouslyAllowAllBuilds: false",
  `'esbuild@${env.get("ESBUILD_VERSION")}': true`,
  "verifyDepsBeforeRun: error",
  "blockExoticSubdeps: true",
]) {
  if (!workspace.includes(required)) failures.push(`pnpm-workspace.yaml missing ${required}`);
}
const allowSection = workspace.split("allowBuilds:")[1]?.split(/\n(?=\S)/u)[0] ?? "";
if ((allowSection.match(/:\s*true/gmu) ?? []).length !== 1) failures.push("allowBuilds must permit exactly one package");

if (!tauri.app?.security?.assetProtocol?.enable) failures.push("asset protocol not enabled");
if (!(tauri.app?.security?.csp ?? "").includes("asset:")) failures.push("CSP does not allow local asset images");
for (const permission of ["core:default", "dialog:default", "opener:default"]) {
  if (!capability.permissions?.includes(permission)) failures.push(`capability missing ${permission}`);
}

const migrationDirectory = join(root, "src-tauri", "migrations");
const migrationFiles = (await readdir(migrationDirectory))
  .filter((name) => /^\d{4}_.+\.sql$/u.test(name))
  .sort();
const registry = new Map();
// Parse explicit Migration struct literals without depending on line wrapping.
for (const match of database.matchAll(/Migration\s*\{([\s\S]*?)\},/gu)) {
  const body = match[1];
  const version = Number(body.match(/version:\s*(\d+)/u)?.[1]);
  const checksum = body.match(/checksum:\s*"([0-9a-f]{64})"/u)?.[1];
  const file = body.match(/include_str!\("\.\.\/migrations\/([^"]+)"\)/u)?.[1];
  if (!Number.isInteger(version) || !checksum || !file) {
    failures.push("A migration registry entry is malformed.");
    continue;
  }
  if (registry.has(version)) failures.push(`Migration version ${version} is registered twice.`);
  registry.set(version, { checksum, file });
}
for (const file of migrationFiles) {
  const version = Number(file.slice(0, 4));
  const entry = registry.get(version);
  if (!entry) {
    failures.push(`Migration ${file} is not registered.`);
    continue;
  }
  if (entry.file !== file) failures.push(`Migration ${version} registry path mismatch: ${entry.file} vs ${file}`);
  const sql = await readFile(join(migrationDirectory, file), "utf8");
  const checksum = createHash("sha256").update(sql).digest("hex");
  if (entry.checksum !== checksum) failures.push(`Migration ${file} checksum mismatch: ${checksum}`);
}
for (const [version, entry] of registry) {
  if (!migrationFiles.includes(entry.file)) failures.push(`Migration registry version ${version} references missing ${entry.file}`);
}
const latest = Number(database.match(/LATEST_SCHEMA_VERSION:\s*i64\s*=\s*(\d+)/u)?.[1]);
const maximumMigration = Math.max(...migrationFiles.map((file) => Number(file.slice(0, 4))));
equal("latest schema version", latest, maximumMigration);

const releaseFixture = await read("src-tauri", "tests", "fixtures", "v0.1.0.sql");
requireText("0.1.0 release fixture", releaseFixture, [
  "Immutable database fixture captured for the ostudy 0.1.0 release",
  "CREATE TABLE schema_migrations",
  "PRAGMA user_version = 3",
]);

requireText("check-static.sh", checkStatic, [
  "verify-project-manifests.mjs",
  "verify-ipc-contract.mjs",
  "verify-repository-policy.mjs",
  "verify-test-policy.mjs",
  "python3 -m unittest",
  "pnpm install --frozen-lockfile",
  "pnpm exec tsc",
  "cargo fmt",
]);
requireText("check-fast.sh", checkFast, ["check-static.sh", "pnpm test", "cargo check"]);
requireText("check.sh", checkFull, ["pnpm run build", "cargo clippy", "cargo test", "smoke-native.sh"]);
requireText("check-changed.sh", checkChanged, ["vitest run --changed", "cargo test", "check-fast.sh", "iteration aid"]);
requireText("check workflow", checkWorkflow, [
  "runs-on: [self-hosted, linux, x64, ostudy]",
  "OSTUDY_CARGO_TARGET_DIR",
  "check-fast.sh",
  "check.sh",
]);
requireText("release workflow", releaseWorkflow, [
  "runs-on: [self-hosted, linux, x64, ostudy]",
  "./scripts/release.sh",
]);
if (!pullRequestTemplate.includes("Acceptance criteria") || !pullRequestTemplate.includes("./scripts/check.sh")) {
  failures.push("pull request template is missing acceptance criteria or full validation");
}

for (const [name, expected] of [
  ["check:static", "./scripts/check-static.sh"],
  ["check:fast", "./scripts/check-fast.sh"],
  ["check:changed", "./scripts/check-changed.sh"],
  ["check", "./scripts/check.sh"],
  ["verify:contracts", "node ./scripts/verify-ipc-contract.mjs && node ./scripts/verify-repository-policy.mjs"],
]) {
  equal(`package script ${name}`, pkg.scripts?.[name], expected);
}

for (const requiredPath of [
  "scripts/build.sh",
  "scripts/check-static.sh",
  "scripts/check-fast.sh",
  "scripts/check.sh",
  "scripts/check-changed.sh",
  "scripts/verify-ipc-contract.mjs",
  "scripts/verify-repository-policy.mjs",
  "scripts/log-run.py",
  "tests/scripts/test_log_run.py",
  "src/ipc/diagnostics.ts",
  "src/ipc/diagnostics.test.ts",
  "src-tauri/src/diagnostics.rs",
  "src-tauri/tests/fixtures/v0.1.0.sql",
]) {
  try {
    await access(join(root, requiredPath));
  } catch {
    failures.push(`required development file is missing: ${requiredPath}`);
  }
}

if (failures.length > 0) {
  console.error("Project manifest verification failed:");
  for (const failure of failures) console.error(`  - ${failure}`);
  process.exit(1);
}
console.log(
  `OK   Identity, exact versions, ${migrationFiles.length} immutable migrations, environment mirrors, and tiered qualification manifests are consistent.`,
);

function parseEnv(text) {
  return new Map(
    text
      .split(/\r?\n/u)
      .filter((line) => line.trim() !== "" && !line.trim().startsWith("#"))
      .map((line) => {
        const separator = line.indexOf("=");
        return [line.slice(0, separator), line.slice(separator + 1)];
      }),
  );
}
function tomlPackageValue(text, key) {
  const packageSection = text.split("[package]")[1]?.split(/\n\[/u)[0] ?? "";
  return packageSection.match(new RegExp(`^${key}\\s*=\\s*"([^"]+)"`, "mu"))?.[1];
}
