import { createHash } from "node:crypto";
import { readFile } from "node:fs/promises";
import { dirname, join } from "node:path";
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
  scheduler,
  transfer,
  extensions,
  richText,
  decksScreen,
  editor,
  viewer,
  links,
  practice,
  analytics,
  settings,
  launcher,
  checkScript,
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
  read("src-tauri", "src", "scheduler.rs"),
  read("src-tauri", "src", "transfer.rs"),
  read("src", "editor", "extensions.ts"),
  read("src", "editor", "richText.ts"),
  read("src", "screens", "DecksScreen.tsx"),
  read("src", "editor", "RichTextEditor.tsx"),
  read("src", "editor", "RichTextView.tsx"),
  read("src", "editor", "links.ts"),
  read("src", "screens", "PracticeScreen.tsx"),
  read("src", "screens", "AnalyticsScreen.tsx"),
  read("src", "screens", "SettingsScreen.tsx"),
  read("run-app.sh"),
  read("scripts", "check.sh"),
]);

const env = parseEnv(envText);
const pkg = JSON.parse(packageText);
const tauri = JSON.parse(tauriText);
const capability = JSON.parse(capabilityText);
const failures = [];

function equal(label, actual, expected) {
  if (actual !== expected) {
    failures.push(`${label}: expected ${expected}, found ${String(actual)}`);
  }
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
};

for (const name of Object.keys(pkg.dependencies ?? {}).filter((name) => name.startsWith("@tiptap/"))) {
  versions[name] = env.get("TIPTAP_VERSION");
}

for (const [name, version] of Object.entries(versions)) {
  const actual = pkg.dependencies?.[name] ?? pkg.devDependencies?.[name];
  equal(`package ${name}`, actual, version);
  if (typeof actual === "string" && /[~^*<>]/u.test(actual)) {
    failures.push(`${name} is not exactly pinned`);
  }
}

equal("packageManager", pkg.packageManager, `pnpm@${env.get("PNPM_VERSION")}`);
equal("Node engine", pkg.engines?.node, env.get("NODE_VERSION"));
equal("pnpm engine", pkg.engines?.pnpm, env.get("PNPM_VERSION"));
equal("license", pkg.license, "MIT");
equal("Tauri identifier", tauri.identifier, "com.example.studyapp");
equal("Tauri version", tauri.version, pkg.version);

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
if (!(tauri.app?.security?.csp ?? "").includes("asset:")) {
  failures.push("CSP does not allow local asset images");
}
for (const permission of ["core:default", "dialog:default", "opener:default"]) {
  if (!capability.permissions?.includes(permission)) {
    failures.push(`capability missing ${permission}`);
  }
}

for (const [label, text] of [
  ["0001", migration1],
  ["0002", migration2],
  ["0003", migration3],
]) {
  const checksum = createHash("sha256").update(text).digest("hex");
  if (!database.includes(`"${checksum}"`)) {
    failures.push(`database.rs checksum mismatch for ${label}: ${checksum}`);
  }
}

for (const command of [
  "list_decks", "get_deck", "create_deck", "update_deck", "delete_deck",
  "list_cards", "get_card", "create_card", "update_card", "delete_card",
  "import_image", "get_settings", "save_settings", "get_app_info",
  "start_practice", "mark_practice_card_viewed", "mark_practice_card_revealed",
  "submit_practice_review", "complete_practice_session", "get_analytics",
  "export_deck", "import_deck",
]) {
  if (!rustCommands.includes(command)) failures.push(`Rust command not registered: ${command}`);
  if (!ipc.includes(`"${command}"`)) failures.push(`frontend command missing: ${command}`);
}

for (const label of ["Decks", "Practice", "Analytics", "Settings"]) {
  if (!app.includes(`label="${label}"`)) failures.push(`sidebar missing ${label}`);
}
for (const obsolete of ['label="Browse"', 'label="Statistics"', "Local library"]) {
  if (app.includes(obsolete)) failures.push(`obsolete sidebar text remains: ${obsolete}`);
}

if (!cards.includes('const CONTENT_FORMAT: &str = "tiptap-json"')) {
  failures.push("canonical card format is not pinned");
}
if (!cards.includes('"width", "height"') || !richText.includes('["width", "height"]')) {
  failures.push("Tiptap image width/height normalization is incomplete");
}
if (!cards.includes("insert_new_schedule(&transaction, &card_id, now)")) {
  failures.push("new cards are not assigned an initial schedule");
}
if (!scheduler.includes("FSRS::new(parameters).next")) {
  failures.push("FSRS adapter is not wired to the scheduling crate");
}
if (transfer.includes(" (Imported)") || transfer.includes("imported_deck_name")) {
  failures.push("deck import still rewrites display names");
}
if (!transfer.includes("insert_new_schedule(&transaction, &card_id, imported_at)")) {
  failures.push("imported cards do not receive fresh local schedules");
}
if (!extensions.includes("TextStyleKit") || !extensions.includes("StudyImage")) {
  failures.push("rich editor extensions incomplete");
}
if (app.includes("dangerouslySetInnerHTML") || ipc.includes("dangerouslySetInnerHTML")) {
  failures.push("unsafe raw HTML rendering found");
}

if (!editor.includes("normalizeExternalLink") || !editor.includes("setEditable(!disabled, false)")) {
  failures.push("rich editor link validation or save-time locking is missing");
}
if (!viewer.includes("openUrl(href).catch")) {
  failures.push("card link opening errors are not handled");
}
for (const scheme of ["http://", "https://", "mailto:", "tel:"]) {
  if (!links.includes(scheme)) failures.push(`link validator missing ${scheme}`);
}

if (!app.includes("autoStart: true") || !practice.includes("launchRequest.autoStart")) {
  failures.push("deck-level Practice does not auto-start a single-deck session");
}
if (!practice.includes("const nextChecked = event.currentTarget.checked")) {
  failures.push("practice checkbox still retains React's event inside a state updater");
}
for (const outcome of ["wrong", "not_confident", "somewhat_confident", "very_confident"]) {
  if (!practice.includes(outcome) || !scheduler.includes(outcome)) {
    failures.push(`review outcome is not wired end to end: ${outcome}`);
  }
}
if (!analytics.includes("ratingDistribution") || !analytics.includes("dueNow")) {
  failures.push("analytics UI is not based on graded reviews and due counts");
}
if (settings.includes("Privacy and network") || settings.includes("Card content security")) {
  failures.push("obsolete settings filler remains");
}
if (decksScreen.includes("deck-summary-strip") || decksScreen.includes(">Edit deck<")) {
  failures.push("deck detail still contains the obsolete summary strip or large edit action");
}
if (editor.includes("FLASHCARD EDITOR") || editor.includes("Rich text · Markdown shortcuts")) {
  failures.push("card editor still contains redundant labels");
}

if (!launcher.includes("pnpm run build") || !launcher.includes("cargo build --manifest-path src-tauri/Cargo.toml --locked")) {
  failures.push("launcher is missing the TypeScript or locked Rust build gate");
}
if (launcher.includes("cargo fmt") || launcher.includes("cargo clippy") || launcher.includes("cargo test")) {
  failures.push("normal application startup is blocked by developer-only quality checks");
}
for (const requiredCheck of [
  "cargo fmt --manifest-path src-tauri/Cargo.toml --all -- --check",
  "cargo clippy --manifest-path src-tauri/Cargo.toml --locked --all-targets --all-features -- -D warnings",
  "cargo test --manifest-path src-tauri/Cargo.toml --locked --all-features",
  "cargo build --manifest-path src-tauri/Cargo.toml --locked",
]) {
  if (!checkScript.includes(requiredCheck)) {
    failures.push(`strict project check is missing: ${requiredCheck}`);
  }
}

for (const requiredPath of [
  "docs/GIT-WORKFLOW.md",
  "docs/DEVELOPMENT-AND-QUALITY.md",
  "docs/ARCHITECTURE.md",
  "docs/decisions/0003-fsrs-review-ledger.md",
  "scripts/release-check.sh",
  "CONTRIBUTING.md",
]) {
  try {
    await readFile(join(root, requiredPath), "utf8");
  } catch {
    failures.push(`required project document is missing: ${requiredPath}`);
  }
}

if (failures.length > 0) {
  console.error("Project verification failed:");
  for (const failure of failures) console.error(`  - ${failure}`);
  process.exit(1);
}

console.log(
  "OK   Project pins, migrations, commands, FSRS reviews, image schema, import behavior, navigation, and quality gates are internally consistent.",
);
