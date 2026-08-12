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
    "paths.resolve_relative(AppPaths::staged_asset_relative_path(",
    "paths.resolve_relative(&AppPaths::staged_asset_relative_path(",
  )
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

let templates = await read(
  "src-tauri/src/authoring/card_types_and_templates/templates.rs",
);
templates = templates.replace(
  /\n\s*file_path:\s*paths[\s\S]*?\.into_owned\(\),/gu,
  "",
);
await write(
  "src-tauri/src/authoring/card_types_and_templates/templates.rs",
  templates,
);

let generatedMod = await read(
  "src-tauri/src/authoring/card_types_and_templates/generated_import/mod.rs",
);
generatedMod = generatedMod.replace(
  "use apply::{import_prepared_flashcards, import_prepared_flashcards_with_commit};",
  "#[cfg(test)]\nuse apply::import_prepared_flashcards;\nuse apply::import_prepared_flashcards_with_commit;",
);
if (!generatedMod.includes("#[cfg(test)]\nuse apply::import_prepared_flashcards;")) {
  throw new Error("generated import apply imports were not migrated");
}
await write(
  "src-tauri/src/authoring/card_types_and_templates/generated_import/mod.rs",
  generatedMod,
);

let generatedPrepare = await read(
  "src-tauri/src/authoring/card_types_and_templates/generated_import/prepare.rs",
);
if (!generatedPrepare.includes("pub(super) fn prepare_flashcard_import(")) {
  generatedPrepare = `#[cfg(test)]
pub(super) fn prepare_flashcard_import(source_path: &str) -> Result<PreparedImport, CommandError> {
    let source = std::fs::canonicalize(source_path).map_err(CommandError::io)?;
    if !source.is_file() {
        return Err(CommandError::new(
            "invalid_flashcard_import",
            "Select an ostudy generated-flashcard file.",
        ));
    }
    let bytes = std::fs::read(source).map_err(CommandError::io)?;
    prepare_flashcard_import_bytes(&bytes)
}

${generatedPrepare}`;
}
await write(
  "src-tauri/src/authoring/card_types_and_templates/generated_import/prepare.rs",
  generatedPrepare,
);

let decks = await read("src-tauri/src/decks.rs");
decks = decks.replace(
  "#[derive(Clone, Debug, Serialize)]\n#[serde(rename_all = \"camelCase\")]\npub struct DeckSummaryDto",
  "#[derive(Clone, Debug, Deserialize, Serialize)]\n#[serde(rename_all = \"camelCase\")]\npub struct DeckSummaryDto",
);
if (!decks.includes("#[derive(Clone, Debug, Deserialize, Serialize)]\n#[serde(rename_all = \"camelCase\")]\npub struct DeckSummaryDto")) {
  throw new Error("DeckSummaryDto Deserialize derive was not added");
}
await write("src-tauri/src/decks.rs", decks);

let studydeck = await read("src-tauri/src/studydeck.rs");
const normalizeStart = studydeck.indexOf("fn normalize_export_path(");
const temporaryStart = studydeck.indexOf("fn temporary_export_path", normalizeStart);
if (normalizeStart >= 0 && temporaryStart >= 0) {
  studydeck = studydeck.slice(0, normalizeStart) + studydeck.slice(temporaryStart);
}
await write("src-tauri/src/studydeck.rs", studydeck);

await rm(join(root, "scripts", "run-platform-p2-pathfree-native.mjs"), { force: true });
console.log("Applied corrected path-free native migration.");
