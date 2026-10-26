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
  externalFileStreaming,
  externalFileCommands,
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
  read("src-tauri/src/external_file_streaming.rs"),
  read("src-tauri/src/external_file_commands.rs"),
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
requireText("release current schema 24", release, "pub(crate) const SCHEMA_VERSION: i64 = 24;");
requireText("release baseline checksum", release, '"64553c98070ad58a0aa73563cf7e9972e9dcb6a27666895a78d8a53882fc26e8"');
requireText("selection-deck release migration", release, 'name: "selection_deck_operations"');
requireText("selection-deck migration checksum", release, '"c854f5d50338beec395399d723038fdfb6d4436af3305718d5a1c6a70ca65238"');
requireText("deck-merge release migration", release, 'name: "deck_merge_operations"');
requireText("deck-merge migration checksum", release, '"696295167cef3c20d750f93373b673dca78a829aca43379d7792ce3bd9be3705"');
requireText("generation-instruction release migration", release, 'name: "study_target_generation_instructions"');
requireText("generation-instruction migration checksum", release, '"4708634c4fc8d757d57561b008861f86d42e9b1974656bd9b5f23f1d87827e1d"');
requireText("review scheduler-context release migration", release, 'name: "review_event_scheduler_context"');
requireText("review scheduler-context migration checksum", release, '"ffb3788f79fcb4b3fe2092bcb6207f7ff7e2742b694fa845f175862737863158"');
requireText("subjects release migration", release, 'name: "subjects"');
requireText("subjects migration checksum", release, '"6b072b0f86efab3a628647edb8d5712e852dc03313e859e1e25aa996a3066a06"');
requireText("study activity release migration", release, 'name: "study_activity"');
requireText("release migration 7 checksum", release, '"01614ecab4fca2e86a8c7f307c430615a781cc473c901b4e465fa39ba08cc5ce"');
requireText("document annotations release migration", release, 'name: "document_annotations"');
requireText("release migration 8 checksum", release, '"aef233f14b0d9830abc51dfecb31d4d627a6cc39917659eef4d85090854c7254"');
requireText("lessons release migration", release, 'name: "lessons"');
requireText("release migration 9 checksum", release, '"781c7040dd19f12cdff7d740338dc55b149293d6dcaf7323aa03b916dd1b591e"');
requireText("card mark links release migration", release, 'name: "note_annotation_sources"');
requireText("release migration 10 checksum", release, '"44fe80681ce06adb81bc9afcf9213e2e6f2235c1bfa6d720329b571c35e24f45"');
requireText("lesson narration release migration", release, 'name: "lesson_narration"');
requireText("release migration 11 checksum", release, '"4c8db63fdc61a16c6a7008eeff26576b96e593ed82c20092e652ab8385e02f9f"');
requireText("study guide reading release migration", release, 'name: "study_guide_reading"');
requireText("release migration 12 checksum", release, '"b4dec3afded959aeaf1df5267d81790bb5a9c7de7b0bfd84bf2679ae5a3e8b4e"');
requireText("lecture exports release migration", release, 'name: "lecture_exports"');
requireText("release migration 13 checksum", release, '"b759a0e5e7347557a1f287a43b8d1e07cba3f1ebd347727ebe7d2191740d82ce"');
requireText("annotated PDF exports release migration", release, 'name: "annotated_pdf_exports"');
requireText("release migration 14 checksum", release, '"689d37008214b79efc99d7954c242cac9c9c1850a0e0ee20ed6319aa86a7b0be"');
requireText("lecture scripts release migration", release, 'name: "lecture_scripts"');
requireText("release migration 15 checksum", release, '"61892d73c101e80515f4df96b1f53ba2bbbce3d2e9c8a530c7d0655bbd362e1f"');
requireText("speech voices release migration", release, 'name: "speech_voices"');
requireText("release migration 16 checksum", release, '"2ff607022faa39f8db7876ba62233cd92cd2d138e953c1e1a189c0dff7c5070e"');
requireText("podcasts release migration", release, 'name: "podcasts"');
requireText("release migration 17 checksum", release, '"23abf0c9879caf011c3bb542c8f43274beae8d320fc3d6d914b87a907379e4cd"');
requireText("notebooks release migration", release, 'name: "notebooks"');
requireText("release migration 18 checksum", release, '"cd1eb1fc6917b5a599c5625f04e2034a8c725ca51984ceb5269a116f1f4a3032"');
requireText("document figures release migration", release, 'name: "document_figures"');
requireText("release migration 19 checksum", release, '"fa59b76d33f446d932b0a68d72b2f3a53feab456ff0e318c385e0c99b7a9bf75"');
requireText("notebook assets release migration", release, 'name: "notebook_assets"');
requireText("release migration 20 checksum", release, '"b79396a63983ef4d33da7d02202ce2616e52d76cc0613037bee83c0ea2755b02"');
requireText("material ink release migration", release, 'name: "material_ink"');
requireText("release migration 21 checksum", release, '"338768544f533b4d2960518cad610fdd0ed3f881f5bbf0277ceb74a3a1a21c08"');
requireText("document study exclusions release migration", release, 'name: "document_study_exclusions"');
requireText("release migration 22 checksum", release, '"ff47f442f9206e1610795cf14ad8827b1c513e35dc5452274cc6c211f574b480"');
requireText("note folders release migration", release, 'name: "note_folders"');
requireText("release migration 23 checksum", release, '"838c81189b2782597bb1220783fcd72145e810bbd307e48c123a591c841f2333"');
requireText("Tutor answer versions release migration", release, 'name: "tutor_turn_versions"');
requireText("release migration 24 checksum", release, '"6878229f81f274587f78ebef159e3496237e70ee7b32606c65dacd77b321781d"');
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
  generatedImport,
  "SUPPORTED_GENERATED_IMPORT_VERSIONS",
);
requireText(
  "generated importer uses shared identity policy",
  generatedPrepare,
  "is_supported_generated_import_identity",
);
// rustfmt can wrap the qualified call and add a trailing argument comma.
// Require the same owner, helper, and argument without prescribing whitespace.
const generatedEnvelopeCall = /crate::authoring::card_types_and_templates::generated_import::\s*is_generated_import_envelope\s*\(\s*&value\s*,?\s*\)/;
for (const [label, source] of [
  ["streaming generated JSON staging", externalFileStreaming],
  ["Android generated JSON staging", externalFileCommands],
]) {
  if (!generatedEnvelopeCall.test(source)) {
    failures.push(`${label} must call the importer-owned envelope helper with &value`);
  }
}
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
  "**schema 24**",
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
