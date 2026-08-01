import { createHash } from "node:crypto";
import { readdir, readFile } from "node:fs/promises";
import { dirname, join, relative } from "node:path";
import { fileURLToPath } from "node:url";

const scriptDirectory = dirname(fileURLToPath(import.meta.url));
const root = join(scriptDirectory, "..");
const read = (...parts) => readFile(join(root, ...parts), "utf8");

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

function tomlPackageValue(cargo, key) {
  const packageSection = cargo.split("[package]")[1]?.split(/\n\[/u)[0] ?? "";
  return packageSection.match(new RegExp(`^${key}\\s*=\\s*"([^"]+)"`, "mu"))?.[1];
}

async function collectOwnedText(directory = root) {
  const output = [];
  for (const entry of await readdir(directory, { withFileTypes: true })) {
    if ([".git", "node_modules", "dist", "target", "artifacts"].includes(entry.name)) continue;
    const path = join(directory, entry.name);
    if (entry.isDirectory()) {
      output.push(...(await collectOwnedText(path)));
      continue;
    }
    const rel = relative(root, path);
    if ([
      "pnpm-lock.yaml",
      "src-tauri/Cargo.lock",
      "scripts/verify-project-manifests.mjs",
    ].includes(rel)) continue;
    if (!/\.(?:json|mjs|js|ts|tsx|toml|yaml|yml|md|sql|sh|py|html|env)$/u.test(rel)) continue;
    output.push([rel, await readFile(path, "utf8")]);
  }
  return output;
}

const [
  envText,
  packageText,
  workspace,
  cargo,
  toolchain,
  tauriText,
  capabilityText,
  migration1,
  migration2,
  migration3,
  database,
  rustCommands,
  app,
  ipc,
  cards,
  authoringWrites,
  scheduler,
  transfer,
  launcher,
  buildScript,
  checkScript,
  setupScript,
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
  read("src-tauri", "migrations", "0001_decks.sql"),
  read("src-tauri", "migrations", "0002_cards_assets_settings_practice.sql"),
  read("src-tauri", "migrations", "0003_fsrs_reviews.sql"),
  read("src-tauri", "src", "database.rs"),
  read("src-tauri", "src", "lib.rs"),
  read("src", "App.tsx"),
  read("src", "ipc.ts"),
  read("src-tauri", "src", "cards.rs"),
  read("src-tauri", "src", "authoring", "writes_and_transfer.rs"),
  read("src-tauri", "src", "scheduler.rs"),
  read("src-tauri", "src", "transfer.rs"),
  read("run-app.sh"),
  read("scripts", "build.sh"),
  read("scripts", "check.sh"),
  read("scripts", "setup-ubuntu.sh"),
  read(".github", "workflows", "check.yml"),
  read(".github", "workflows", "release.yml"),
  read(".github", "pull_request_template.md"),
]);

const env = parseEnv(envText);
const pkg = JSON.parse(packageText);
const tauri = JSON.parse(tauriText);
const capability = JSON.parse(capabilityText);
const failures = [];

function equal(label, actual, expected) {
  if (actual !== expected) failures.push(`${label}: expected ${expected}, found ${String(actual)}`);
}

const versions = {
  "@tauri-apps/api": env.get("TAURI_API_VERSION"),
  "@tauri-apps/cli": env.get("TAURI_CLI_VERSION"),
  "@tauri-apps/plugin-dialog": env.get("TAURI_DIALOG_VERSION"),
  "@tauri-apps/plugin-opener": env.get("TAURI_OPENER_VERSION"),
  react: env.get("REACT_VERSION"),
  "react-dom": env.get("REACT_VERSION"),
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
  versions[name] = env.get("TIPTAP_VERSION");
}
for (const [name, version] of Object.entries(versions)) {
  const actual = pkg.dependencies?.[name] ?? pkg.devDependencies?.[name];
  equal(`package ${name}`, actual, version);
  if (typeof actual === "string" && /[~^*<>]/u.test(actual)) failures.push(`${name} is not exactly pinned`);
}

equal("package name", pkg.name, "ostudy");
equal("package version", pkg.version, "0.1.0");
equal("packageManager", pkg.packageManager, `pnpm@${env.get("PNPM_VERSION")}`);
equal("Node engine", pkg.engines?.node, env.get("NODE_VERSION"));
equal("pnpm engine", pkg.engines?.pnpm, env.get("PNPM_VERSION"));
equal("license", pkg.license, "MIT");
equal("Cargo package name", tomlPackageValue(cargo, "name"), "ostudy");
equal("Cargo version", tomlPackageValue(cargo, "version"), pkg.version);
equal("Tauri product name", tauri.productName, "ostudy");
equal("Tauri identifier", tauri.identifier, "io.github.jferments.ostudy");
equal("Tauri version", tauri.version, pkg.version);
if (!cargo.includes('name = "ostudy_lib"')) failures.push("Cargo library is not named ostudy_lib");
if (!rustCommands.includes("mod logging;")) failures.push("Rust structured logging module is not registered");

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
if ((allowSection.match(/:\s*true/gmu) ?? []).length !== 1) {
  failures.push("allowBuilds must permit exactly one package");
}

for (const [label, needle] of [
  ["Rust toolchain", `channel = "${env.get("RUST_VERSION")}"`],
  ["tauri", `version = "=${env.get("TAURI_RUST_VERSION")}"`],
  ["tauri-build", `version = "=${env.get("TAURI_BUILD_VERSION")}"`],
  ["rusqlite", `version = "=${env.get("RUSQLITE_VERSION")}"`],
  ["serde", `version = "=${env.get("SERDE_VERSION")}"`],
  ["serde_json", `serde_json = "=${env.get("SERDE_JSON_VERSION")}"`],
  ["chrono", `chrono = "=${env.get("CHRONO_VERSION")}"`],
  ["rs-fsrs", `rs-fsrs = "=${env.get("RS_FSRS_VERSION")}"`],
]) {
  const source = label === "Rust toolchain" ? toolchain : cargo;
  if (!source.includes(needle)) failures.push(`${label} exact pin missing`);
}

if (!tauri.app?.security?.assetProtocol?.enable) failures.push("asset protocol not enabled");
if (!(tauri.app?.security?.csp ?? "").includes("asset:")) failures.push("CSP does not allow local asset images");
for (const permission of ["core:default", "dialog:default", "opener:default"]) {
  if (!capability.permissions?.includes(permission)) failures.push(`capability missing ${permission}`);
}

for (const [label, text] of [["0001", migration1], ["0002", migration2], ["0003", migration3]]) {
  const checksum = createHash("sha256").update(text).digest("hex");
  if (!database.includes(`"${checksum}"`)) failures.push(`database.rs checksum mismatch for ${label}: ${checksum}`);
}
if (!database.includes('release: "0.1.0"') || !database.includes("v0.1.0.sql")) {
  failures.push("0.1.0 release schema fixture is not registered");
}
const releaseFixture = await read("src-tauri", "tests", "fixtures", "v0.1.0.sql");
if (!releaseFixture.includes("Immutable database fixture captured for the ostudy 0.1.0 release") ||
    !releaseFixture.includes("CREATE TABLE schema_migrations") ||
    !releaseFixture.includes("PRAGMA user_version = 3")) {
  failures.push("0.1.0 migration fixture is not a complete historical schema snapshot");
}

for (const command of [
  "list_decks", "get_deck", "create_deck", "update_deck", "delete_deck",
  "list_cards", "delete_card",
  "list_tags", "search_cards", "list_matching_cards",
  "get_organized_card", "create_organized_card", "update_organized_card",
  "find_duplicate_cards", "analyze_bulk_cards", "bulk_create_cards",
  "transfer_cards", "bulk_tag_cards", "delete_cards",
  "import_image", "get_settings", "save_settings", "get_app_info",
  "start_selected_practice", "start_card_practice",
  "mark_practice_card_viewed", "mark_practice_card_revealed",
  "submit_practice_review", "complete_practice_session", "get_analytics",
  "export_deck", "import_deck",
]) {
  if (!rustCommands.includes(command)) failures.push(`Rust command not registered: ${command}`);
  if (!ipc.includes(`"${command}"`)) failures.push(`frontend command missing: ${command}`);
}

if (!authoringWrites.includes('const CONTENT_FORMAT: &str = "tiptap-json"') ||
    !authoringWrites.includes("const CONTENT_SCHEMA_VERSION: i64 = 1")) {
  failures.push("canonical card format and schema version are not pinned in the active authoring path");
}
if (!scheduler.includes("FSRS::new(parameters).next")) failures.push("FSRS adapter is not wired to the scheduling crate");
if (transfer.includes(" (Imported)") || transfer.includes("imported_deck_name")) failures.push("deck import rewrites display names");
if (!app.includes("<strong>ostudy</strong>") || !app.includes("Opening ostudy")) failures.push("frontend branding is incomplete");

if (/pnpm run build|cargo build|pnpm tauri dev/u.test(launcher)) {
  failures.push("run-app.sh must launch the existing binary without compiling or starting a dev server");
}
for (const required of [
  "pnpm tauri build --debug --no-bundle --ci",
  "ostudy_write_build_stamp",
  "CARGO_BUILD_JOBS",
]) {
  if (!buildScript.includes(required)) failures.push(`build.sh missing: ${required}`);
}
for (const required of [
  "pnpm test",
  "cargo fmt",
  "cargo clippy",
  "cargo test",
  "pnpm tauri build --debug --no-bundle --ci",
  "smoke-native.sh",
]) {
  if (!checkScript.includes(required)) failures.push(`check.sh missing: ${required}`);
}
if (!setupScript.includes('"$SCRIPT_DIR/build.sh"')) failures.push("setup does not create the initial build");
if (!checkWorkflow.includes("./scripts/check.sh")) failures.push("GitHub check workflow does not run the local gate");
if (!releaseWorkflow.includes("./scripts/release.sh")) failures.push("release workflow does not use the release script");
if (!pullRequestTemplate.includes("Acceptance criteria") || !pullRequestTemplate.includes("./scripts/check.sh")) {
  failures.push("pull request template is missing acceptance criteria or validation");
}

for (const requiredPath of [
  "scripts/build.sh",
  "scripts/build-state.sh",
  "scripts/cargo-target.sh",
  "scripts/export-diagnostics.sh",
  "scripts/log-run.py",
  "scripts/smoke-native.sh",
  "scripts/release.sh",
  "tests/webdriver/smoke.py",
  "src/test/setup.ts",
  "src/components/Toast.test.tsx",
  "src/screens/SettingsScreen.test.tsx",
  "src-tauri/src/logging.rs",
  "src-tauri/tests/fixtures/v0.1.0.sql",
  ".github/workflows/check.yml",
  ".github/workflows/release.yml",
  ".github/pull_request_template.md",
]) {
  try {
    await readFile(join(root, requiredPath), "utf8");
  } catch {
    failures.push(`required Phase 1 file is missing: ${requiredPath}`);
  }
}

const ownedText = await collectOwnedText();
for (const [path, text] of ownedText) {
  for (const [label, pattern] of [
    ["archive revision 5.4", /\b(?:revision|version|release)\s+(?:v)?5\.4(?:\.0)?\b/iu],
    ["old pre-Git release version 0.3", /\b(?:ostudy\s+|revision\s+|version\s+|release\s+|v)0\.3(?:\.0)?\b/iu],
    ["example Tauri identifier", /com\.example\.studyapp/u],
    ["obsolete PROJECT_REVISION", /PROJECT_REVISION/u],
  ]) {
    if (pattern.test(text)) failures.push(`${path} still contains ${label}`);
  }
}

if (failures.length > 0) {
  console.error("Project verification failed:");
  for (const failure of failures) console.error(`  - ${failure}`);
  process.exit(1);
}

console.log("OK   ostudy identity, versions, migrations, tests, build scripts, CI, diagnostics, and release files are internally consistent.");
