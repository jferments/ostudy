#!/usr/bin/env node

import { readFile } from "node:fs/promises";

const read = (path) => readFile(new URL(`../${path}`, import.meta.url), "utf8");
const failures = [];
const [
  database,
  release,
  releaseStaging,
  repairs,
  prePublicLedger,
  recovery,
  backup,
  studydeck,
  syncPackage,
  generatedImport,
  generatedPrepare,
  richContent,
  checkpoint,
  attempts,
  knowledge,
  canonicalDocument,
  audit,
] = await Promise.all([
  read("src-tauri/src/database.rs"),
  read("src-tauri/src/database/release.rs"),
  read("src-tauri/src/database/release/staging.rs"),
  read("src-tauri/src/data_repairs.rs"),
  read("scripts/history/pre-public-v26/ledger.json"),
  read("scripts/restore_pre_public_backup.py"),
  read("src-tauri/src/library_backup.rs"),
  read("src-tauri/src/studydeck.rs"),
  read("src-tauri/src/device_sync/package.rs"),
  read("src-tauri/src/authoring/card_types_and_templates/generated_import/mod.rs"),
  read("src-tauri/src/authoring/card_types_and_templates/generated_import/prepare.rs"),
  read("src-tauri/src/cards/rich_content.rs"),
  read("src-tauri/src/ui_checkpoint.rs"),
  read("src-tauri/src/cards/questions/attempt.rs"),
  read("src-tauri/src/knowledge/contracts.rs"),
  read("src-tauri/src/document_canonical.rs"),
  read("docs/development/post-104-data-contract-audit.md"),
]);

function requireText(label, source, needle) {
  if (!source.includes(needle)) failures.push(`${label} missing ${JSON.stringify(needle)}`);
}

requireText("legacy public lineage", database, 'const LEGACY_PUBLIC_SCHEMA_LINEAGE: &str = "ostudy-public-v1"');
requireText("legacy schema 30", database, "pub(crate) const LEGACY_PUBLIC_SCHEMA_VERSION: i64 = 30;");
requireText("release lineage", release, 'pub(crate) const SCHEMA_LINEAGE: &str = "ostudy-release-v1";');
requireText("release baseline schema 1", release, "pub(crate) const BASELINE_VERSION: i64 = 1;");
requireText("release current schema 2", release, "pub(crate) const SCHEMA_VERSION: i64 = 2;");
requireText("release baseline checksum", release, '"64553c98070ad58a0aa73563cf7e9972e9dcb6a27666895a78d8a53882fc26e8"');
requireText("selection-deck release migration", release, 'name: "selection_deck_operations"');
requireText("selection-deck migration checksum", release, '"c854f5d50338beec395399d723038fdfb6d4436af3305718d5a1c6a70ca65238"');
requireText("disk-backed legacy converter", release, "convert_legacy_current_with(connection, staging::Candidate::copy_from)");
requireText("bounded disk staging backup", releaseStaging, "Backup::new(source, staged)");
requireText("file-backed SQLite temp storage", releaseStaging, "PRAGMA temp_store = FILE");
requireText("low-space staging classification", releaseStaging, "rusqlite::ffi::SQLITE_FULL");
requireText("typed preservation receipt", release, "fn preservation_receipt(connection: &Connection)");
requireText("atomic live ledger conversion", release, "transaction_with_behavior(TransactionBehavior::Immediate)");

for (const [version, name] of [
  [1, "canonical_semantic_chunk_hashes"],
  [2, "canonical_block_hashes"],
  [3, "document_artifact_user_edits"],
]) {
  requireText(`legacy public repair ${version}`, repairs, `version: ${version},`);
  requireText(`legacy public repair ${version} name`, repairs, `name: "${name}"`);
}
const legacy = JSON.parse(prePublicLedger);
if (legacy.schemaVersion !== 26 || JSON.stringify(legacy.acceptedSqliteUserVersions) !== "[0,26]") {
  failures.push("pre-public transition contract must remain exact v26 with SQLite markers 0/26");
}
requireText("archived pre-public recovery", recovery, 'scripts" / "history" / "pre-public-v26" / "ledger.json');
requireText("explicit public-v1 conversion", recovery, 'PUBLIC_SCHEMA_LINEAGE = "ostudy-public-v1"');
requireText("backup format", backup, 'const BACKUP_VERSION: i64 = 4;');
requireText("backup package floor", backup, 'const MIN_SUPPORTED_BACKUP_VERSION: i64 = 1;');
requireText("release backup admission", backup, "lineage == CURRENT_SCHEMA_LINEAGE");
requireText("release backup schema window", backup, "(MIN_SUPPORTED_RELEASE_SCHEMA_VERSION..=CURRENT_SCHEMA_VERSION).contains(&version)");
requireText("legacy backup admission", backup, 'lineage == "ostudy-public-v1"');
requireText("legacy backup SQLite floor", backup, "(10..=LATEST_SCHEMA_VERSION).contains(&version)");
requireText("studydeck writer", studydeck, 'const CURRENT_PACKAGE_VERSION: i64 = 3;');
requireText("studydeck v2 reader", studydeck, 'const CORE_PACKAGE_VERSION: i64 = 2;');
requireText("studydeck v1 reader", studydeck, 'const LEGACY_PACKAGE_VERSION: i64 = 1;');
requireText("sync writer", syncPackage, 'const PACKAGE_VERSION: i64 = 3;');
requireText("sync legacy reader", syncPackage, 'const LEGACY_PACKAGE_VERSION: i64 = 2;');
requireText("generated import legacy v1 reader", generatedImport, 'const LEGACY_GENERATED_IMPORT_VERSION: i64 = 1;');
requireText("generated import current v2 writer", generatedImport, 'const GENERATED_IMPORT_VERSION: i64 = 2;');
requireText(
  "generated import explicit v1/v2 readers",
  generatedPrepare,
  '![LEGACY_GENERATED_IMPORT_VERSION, GENERATED_IMPORT_VERSION]',
);
requireText("rich schema 3 admission", richContent, 'if !(1..=3).contains(&version)');
requireText("rich schema 3 projection", richContent, 'if version == 3');
requireText("UI checkpoint writer", checkpoint, 'const CHECKPOINT_SCHEMA_VERSION: i64 = 2;');
requireText("UI checkpoint v1 reader", checkpoint, 'const CHECKPOINT_V1_FILENAME: &str = "ui-checkpoint-v1.json"');
requireText("question attempt v1", attempts, 'const ATTEMPT_VERSION: u32 = 1;');
requireText("knowledge schema v1", knowledge, 'pub(crate) const CONTENT_SCHEMA_VERSION: i64 = 1;');
requireText("source identity v3", knowledge, 'self.identity_version == 3');
requireText("source identity v2", knowledge, 'self.identity_version == 2');
requireText("canonical document schema v1", canonicalDocument, 'pub(crate) const CANONICAL_DOCUMENT_SCHEMA_VERSION: i64 = 1;');

for (const needle of [
  "ostudy-release-v1",
  "**baseline is schema 1**",
  "**schema 2**",
  "Legacy `ostudy-public-v1` schemas **10–30**",
  "repair versions **1–3**",
  "writer v3",
  "explicit source-checkout recovery",
]) requireText("audit document", audit, needle);

if (failures.length > 0) {
  console.error("Current-data-contract verification failed:");
  for (const failure of failures) console.error(`  - ${failure}`);
  process.exit(1);
}
console.log("OK   Release-v1 baseline, public-v1 compatibility, repairs, backups, interchange, and historical recovery match current source.");
