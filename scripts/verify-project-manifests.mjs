#!/usr/bin/env node

import { execFileSync } from "node:child_process";
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
  maintenanceWorkflow,
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
  read(".github", "workflows", "maintain-generated-candidates.yml"),
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
  ["base64", `base64 = "=${env.get("BASE64_VERSION")}"`],
  ["rusqlite", `version = "=${env.get("RUSQLITE_VERSION")}"`],
  ["serde", `version = "=${env.get("SERDE_VERSION")}"`],
  ["serde_json", `serde_json = "=${env.get("SERDE_JSON_VERSION")}"`],
  ["chrono", `chrono = "=${env.get("CHRONO_VERSION")}"`],
  ["image", `image = { version = "=${env.get("IMAGE_VERSION")}"`],
  ["oxidize-pdf", `version = "=${env.get("OXIDIZE_PDF_VERSION")}"`],
  ["pdfium-render", `pdfium-render = { version = "=${env.get("PDFIUM_RENDER_VERSION")}"`],
  ["sysinfo", `sysinfo = { version = "=${env.get("SYSINFO_VERSION")}"`],
  ["keyring", `keyring = "=${env.get("KEYRING_VERSION")}"`],
  ["reqwest", `version = "=${env.get("REQWEST_VERSION")}"`],
  ["rs-fsrs", `rs-fsrs = "=${env.get("RS_FSRS_VERSION")}"`],
  ["url", `url = "=${env.get("URL_VERSION")}"`],
]) {
  const source = label === "Rust toolchain" ? toolchain : cargo;
  if (!source.includes(needle)) failures.push(`${label} exact pin missing`);
}
if (cargo.includes("pdfium-bundled")) failures.push("pdfium-bundled must not enter the dependency graph");
if (cargo.includes("justpdf")) failures.push("justpdf must not remain in the renderer dependency graph");
if (cargo.includes("rusty-tesseract")) failures.push("OCR/Tesseract wrapper must not be a mandatory dependency");
requireText("oxidize-pdf feature policy", cargo, [
  'oxidize-pdf = { version = "=',
  'default-features = false',
  'features = ["compression", "semantic"]',
]);
requireText("PDFium external renderer policy", cargo, [
  "target_os = \"android\"",
  "target_os = \"ios\"",
  `pdfium-render = { version = "=${env.get("PDFIUM_RENDER_VERSION")}"`,
  '"pdfium_7881"',
  '"thread_safe"',
]);

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

const trackedScriptEntries = execFileSync("git", ["ls-files", "-s", "scripts"], {
  cwd: root,
  encoding: "utf8",
})
  .trim()
  .split(/\r?\n/u)
  .filter(Boolean);
for (const entry of trackedScriptEntries) {
  const match = entry.match(/^(\d{6})\s+[0-9a-f]+\s+\d+\t(.+)$/u);
  if (!match) {
    failures.push(`Could not parse tracked script entry: ${entry}`);
    continue;
  }
  const [, mode, path] = match;
  if (path.endsWith(".sh") && mode !== "100755") {
    failures.push(`${path} must be committed executable (100755), found ${mode}`);
  }
}

const migrationDirectory = join(root, "src-tauri", "migrations");
const migrationFiles = (await readdir(migrationDirectory))
  .filter((name) => /^\d{4}_.+\.sql$/u.test(name))
  .sort();
const registry = new Map();
const migrationRegistrySource =
  database.split("const MIGRATIONS:")[1]?.split("pub(crate) const LATEST_SCHEMA_VERSION")[0] ?? "";
for (const match of migrationRegistrySource.matchAll(/Migration\s*\{([\s\S]*?)\}/gu)) {
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

const publicBaselineManifest = JSON.parse(
  await read("src-tauri", "tests", "fixtures", "public-baseline-schema.json"),
);
equal("public schema lineage", publicBaselineManifest.schemaLineage, "ostudy-public-v1");
equal("public schema fixture version", publicBaselineManifest.schemaVersion, 1);
equal(
  "public baseline fixture checksum",
  publicBaselineManifest.baselineSha256,
  registry.get(1)?.checksum,
);

requireText("check-static.sh", checkStatic, [
  "verify-project-manifests.mjs",
  "verify-ipc-contract.mjs",
  "verify-repository-policy.mjs",
  "verify-agent-workflow-policy.mjs",
  "verify-test-policy.mjs",
  "python3 -m unittest",
  "pnpm install --frozen-lockfile",
  "pnpm exec tsc",
  "cargo fmt",
]);
requireText("check-fast.sh", checkFast, ["check-static.sh", "install-native-deps.sh", "pnpm test", "cargo check"]);
requireText("check.sh", checkFull, ["install-native-deps.sh", "pnpm run build", "cargo clippy", "cargo test", "smoke-native.sh"]);
requireText("check-changed.sh", checkChanged, ["vitest run --changed", "cargo test", "check-fast.sh", "iteration aid"]);
requireText("read-only check workflow", checkWorkflow, [
  "permissions:\n  contents: read",
  "runs-on: [self-hosted, linux, x64, ostudy-unified]",
  "persist-credentials: false",
  "OSTUDY_CARGO_TARGET_DIR",
  "select-check-gate.mjs",
  "verify-pr-contract.mjs",
  "write-qualification-result.mjs",
  "cargo metadata",
  "check-fast.sh",
  "check.sh",
]);
requireText("manual generated-candidate maintenance workflow", maintenanceWorkflow, [
  "workflow_dispatch:",
  "contents: write",
  'if [[ "$GITHUB_REF_NAME" == "main" ]]',
  "git add -- src-tauri/Cargo.lock src-tauri/src src-tauri/build.rs",
  'git push origin "HEAD:$GITHUB_REF_NAME"',
]);
requireText("release workflow", releaseWorkflow, ["runs-on: [self-hosted, linux, x64, ostudy-unified]", "./scripts/release.sh"]);
requireText("pull request template", pullRequestTemplate, [
  "Acceptance criteria",
  "<!-- ostudy-agent-state:start -->",
  "./scripts/check-static.sh",
  "./scripts/check-fast.sh",
  "./scripts/check.sh",
]);

for (const [name, expected] of [
  ["check:static", "./scripts/check-static.sh"],
  ["check:fast", "./scripts/check-fast.sh"],
  ["check:changed", "./scripts/check-changed.sh"],
  ["check", "./scripts/check.sh"],
  ["verify:contracts", "node ./scripts/verify-ipc-contract.mjs && node ./scripts/verify-repository-policy.mjs && node ./scripts/verify-platform-boundaries.mjs"],
]) {
  equal(`package script ${name}`, pkg.scripts?.[name], expected);
}

for (const requiredPath of [
  ".github/workflows/maintain-generated-candidates.yml",
  "scripts/build.sh",
  "scripts/install-native-deps.sh",
  "scripts/install-native-deps.ps1",
  "scripts/native-deps-manifest.json",
  "scripts/check-static.sh",
  "scripts/check-fast.sh",
  "scripts/check.sh",
  "scripts/check-changed.sh",
  "scripts/select-check-gate.mjs",
  "scripts/verify-pr-contract.mjs",
  "scripts/verify-agent-workflow-policy.mjs",
  "scripts/write-qualification-result.mjs",
  "scripts/verify-ipc-contract.mjs",
  "scripts/verify-repository-policy.mjs",
  "scripts/log-run.py",
  "tests/scripts/test_log_run.py",
  "tests/scripts/test_check_gate_selection.py",
  "tests/scripts/test_pr_contract.py",
  "tests/scripts/test_agent_workflow_policy.py",
  "tests/scripts/test_qualification_result.py",
  "docs/agents/EXECUTION-STATE.md",
  "docs/agents/FAILURE-TRIAGE.md",
  "src/ipc/diagnostics.ts",
  "src/ipc/diagnostics.test.ts",
  "src/ipc/documents.ts",
  "src/screens/DocumentsScreen.tsx",
  "src-tauri/src/diagnostics.rs",
  "src-tauri/src/documents.rs",
  "src-tauri/src/document_semantics.rs",
  "src-tauri/src/pdf_backend.rs",
  "src-tauri/src/pdf_render_backend.rs",
  "src-tauri/src/system_resources.rs",
  "src-tauri/src/document_backup.rs",
  "src-tauri/src/ai/document_intelligence.rs",
  "src-tauri/tests/fixtures/public-baseline-schema.json",
  "scripts/reset-pre-public-data.sh",
  "src-tauri/tests/fixtures/document-generation/simple-text.pdf",
  "docs/decisions/0017-managed-pdf-flashcard-generation.md",
  "docs/decisions/0018-structured-document-intelligence.md",
  "docs/decisions/0025-cross-platform-ocr-runtime.md",
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
  `OK   Identity, exact versions, ${migrationFiles.length} immutable migrations, environment mirrors, audited native PDFium provisioning, resource-aware document concurrency, direct-source workflow policy, and tiered qualification manifests are consistent.`,
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
