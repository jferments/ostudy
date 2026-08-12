#!/usr/bin/env node

import { mkdir, readFile, rm, writeFile } from "node:fs/promises";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const root = fileURLToPath(new URL("..", import.meta.url));
const read = (...parts) => readFile(join(root, ...parts), "utf8");
const write = async (path, content) => {
  const target = join(root, path);
  await mkdir(dirname(target), { recursive: true });
  await writeFile(target, content, "utf8");
};

const applicatorPath = "scripts/apply-platform-p2-pathfree-native.mjs";
let applicator = await read(applicatorPath);
const originalStudydeckRange = `await replaceRange(
  "src-tauri/src/studydeck.rs",
  "#[derive(Debug, Deserialize)]\\n#[serde(rename_all = \\\"camelCase\\\", deny_unknown_fields)]\\npub struct ExportDeckInput",
  "fn normalize_export_path",
  studydeckBlock,
);`;
const correctedStudydeckRange = `await replaceRange(
  "src-tauri/src/studydeck.rs",
  "#[derive(Debug, Deserialize)]\\n#[serde(rename_all = \\\"camelCase\\\", deny_unknown_fields)]\\npub struct ExportDeckInput",
  "#[derive(Clone)]\\nstruct PackageDeck",
  studydeckBlock,
);
await replaceRange(
  "src-tauri/src/studydeck.rs",
  "#[tauri::command]\\npub fn export_deck(\\n",
  "fn temporary_export_path",
  "",
);`;
if (!applicator.includes(originalStudydeckRange)) {
  throw new Error("The studydeck replacement range was not found in the native applicator.");
}
applicator = applicator.replace(originalStudydeckRange, correctedStudydeckRange);
applicator = applicator
  .replace(
    `struct ExportResultDto {
    destination_path: String,
    card_count: i64,
    asset_count: i64,
}`,
    `pub(crate) struct ExportResultDto {
    pub(crate) destination_path: String,
    pub(crate) card_count: i64,
    pub(crate) asset_count: i64,
}`,
  )
  .replace(
    `    let suggested_filename = format!(
        "ostudy-library-{}.ostudybackup",
        chrono::Utc::now().format("%Y-%m-%d")
    );`,
    `    let created_at = unix_time_millis()?;
    let created_date = chrono::DateTime::<chrono::Utc>::from_timestamp_millis(created_at)
        .map(|value| value.format("%Y-%m-%d").to_string())
        .unwrap_or_else(|| "backup".to_string());
    let suggested_filename = format!("ostudy-library-{created_date}.ostudybackup");`,
  );
await write(applicatorPath, applicator);

await import(new URL("./apply-platform-p2-pathfree-native.mjs", import.meta.url));

let assetLifecycle = await read("src-tauri/src/asset_lifecycle.rs");
assetLifecycle = assetLifecycle
  .replace(
    `                let retained = BTreeSet::from([kept.id.clone()]);`,
    `                let kept_path = paths.resolve_relative(&AppPaths::staged_asset_relative_path(
                    &draft.id,
                    &kept.id,
                    "png",
                ));
                let removed_path = paths.resolve_relative(&AppPaths::staged_asset_relative_path(
                    &draft.id,
                    &removed.id,
                    "png",
                ));
                let retained = BTreeSet::from([kept.id.clone()]);`,
  )
  .replace("assert!(Path::new(&kept.file_path).exists());", "assert!(kept_path.exists());")
  .replace("assert!(!Path::new(&removed.file_path).exists());", "assert!(!removed_path.exists());")
  .replace("assert!(!Path::new(&kept.file_path).exists());", "assert!(!kept_path.exists());");
await write("src-tauri/src/asset_lifecycle.rs", assetLifecycle);

let studydeck = await read("src-tauri/src/studydeck.rs");
studydeck = studydeck.replace(
  `fn normalize_export_path(value: &str) -> Result<PathBuf, CommandError> {
    const PLACEHOLDER: &str = "";
`,
  `fn normalize_export_path(value: &str) -> Result<PathBuf, CommandError> {
    const PLACEHOLDER: &str = "";
`,
);
await write("src-tauri/src/studydeck.rs", studydeck);

await rm(join(root, "scripts", "run-platform-p2-pathfree-native.mjs"), { force: true });
console.log("Applied corrected path-free native migration.");
