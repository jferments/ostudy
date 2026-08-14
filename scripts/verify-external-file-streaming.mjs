#!/usr/bin/env node

import { readFile } from "node:fs/promises";
import { resolve } from "node:path";
import { fileURLToPath } from "node:url";

const root = fileURLToPath(new URL("..", import.meta.url));
const failures = [];

const lib = await read("src-tauri/src/lib.rs");
const transfer = await read("src-tauri/src/external_file_streaming.rs");
const recovery = await read("src-tauri/src/external_file_recovery.rs");
const platformPort = await read("src-tauri/src/platform/mod.rs");
const ipc = await read("src/ipc/externalFiles.ts");
const bridge = await read("src/testing/androidTestBridge.ts");
const smoke = await read(
  "src-tauri/gen/android/app/src/androidTest/java/io/github/jferments/ostudy/AndroidSmokeTest.kt",
);
const smokeRunner = await read("scripts/android-emulator-smoke.sh");
const androidAdapter = await read("src-tauri/src/platform/android.rs");
const mobileStub = await read("src-tauri/src/platform/mobile_stub.rs");
const desktopAdapter = await read("src-tauri/src/platform/desktop/external_files.rs");
const checkStatic = await read("scripts/check-static.sh");
const androidTest = await read("scripts/android-test.sh");
const androidBuild = await read("scripts/android-build.sh");

expectContains(
  lib,
  "external_file_streaming::stream_select_and_stage_external_import",
  "canonical streaming import command registration",
);
expectContains(
  lib,
  "external_file_streaming::stream_publish_prepared_export",
  "canonical streaming export command registration",
);
for (const legacy of [
  "external_files::select_and_stage_external_import",
  "external_files::publish_prepared_export",
]) {
  if (lib.includes(legacy)) {
    failures.push(`Legacy path-based command remains registered: ${legacy}`);
  }
}

const recoveryIndex = lib.indexOf("external_file_recovery::reconcile_interrupted_staging");
const generalReconcileIndex = lib.indexOf("external_files::reconcile_startup");
if (recoveryIndex < 0) {
  failures.push("External-file crash recovery is not registered at startup");
} else if (generalReconcileIndex < 0 || recoveryIndex > generalReconcileIndex) {
  failures.push("Interrupted external-file staging must recover before general transfer reconciliation");
}
for (const token of [
  "StagingRecoveryAction::Ready",
  "StagingRecoveryAction::PromotePartial",
  "StagingRecoveryAction::Missing",
  "file_matches",
  "sync_file(&ready)",
  "state = 'staging'",
]) {
  expectContains(recovery, token, `external-file crash-recovery invariant ${token}`);
}

for (const token of [
  ".acquire_import(",
  "ExternalImportTarget",
  ".publish_export(",
  "ExternalExportSource",
  "validate_acquired_file",
  "validate_publish_result",
  "ExternalImportPurpose::Image",
  "ExternalImportPurpose::PdfDocument",
  "ExternalImportPurpose::StudyContent",
  "ExternalImportPurpose::LibraryBackup",
]) {
  expectContains(transfer, token, `streaming transfer invariant ${token}`);
}
for (const forbidden of [".select_import(", ".select_export("]) {
  if (transfer.includes(forbidden)) {
    failures.push(`Canonical transfer command uses forbidden legacy API ${forbidden}`);
  }
}

for (const [method, resultType] of [
  ["acquire_import", "ExternalAcquireResult"],
  ["publish_export", "ExternalPublishResult"],
]) {
  const requiredMethod = new RegExp(
    `fn\\s+${method}\\s*\\([\\s\\S]*?\\)\\s*->\\s*PlatformFuture<Result<Option<${resultType}>,\\s*CommandError>>\\s*;`,
    "u",
  );
  if (!requiredMethod.test(platformPort)) {
    failures.push(`ExternalFilePort.${method} must be a required trait method with no default body`);
  }
}
if (platformPort.includes("external_file_streaming_unavailable")) {
  failures.push("ExternalFilePort must not provide a default unavailable streaming implementation");
}
for (const [name, adapter] of [
  ["Android", androidAdapter],
  ["desktop", desktopAdapter],
  ["mobile fallback", mobileStub],
]) {
  for (const method of ["fn acquire_import(", "fn publish_export("]) {
    expectContains(adapter, method, `${name} required streaming adapter ${method}`);
  }
}
for (const token of [
  "app.ostudy_platform().acquire_import",
  "app.ostudy_platform().publish_export",
]) {
  expectContains(androidAdapter, token, `Android streaming adapter invariant ${token}`);
}

expectContains(
  ipc,
  '"stream_select_and_stage_external_import"',
  "frontend streaming import command",
);
expectContains(
  ipc,
  '"stream_publish_prepared_export"',
  "frontend streaming export command",
);
for (const legacy of ['"select_and_stage_external_import"', '"publish_prepared_export"']) {
  if (ipc.includes(legacy)) {
    failures.push(`Frontend still invokes legacy external-file command ${legacy}`);
  }
}

for (const token of [
  "fileOperationSnapshot",
  "startImportPicker",
  "startDeckExportPicker",
  'phase: "pending"',
  'phase: "error"',
  "cancelled: boolean | null",
  "=== null",
  '"image_import"',
  '"pdf_document_import"',
  '"study_content_import"',
  '"library_backup_import"',
]) {
  expectContains(bridge, token, `Android bridge operation-state invariant ${token}`);
}
if (bridge.includes(".catch(() => undefined)")) {
  failures.push("Android smoke bridge must not discard external-file operation failures");
}

for (const token of [
  "waitForPickerOrFailure",
  "waitForFileOperationCancelled",
  "fileOperationSnapshot",
  "startFileOperation",
  "documentPickerPackages",
  "Intent.ACTION_OPEN_DOCUMENT",
  "Intent.ACTION_CREATE_DOCUMENT",
  'getBoolean("cancelled")',
  "f_studyContentImportPickerCancelsCleanly",
  "f_imageImportPickerCancelsCleanly",
  "f_pdfImportPickerCancelsCleanly",
  "f_libraryBackupImportPickerCancelsCleanly",
  "f_deckExportPickerCancelsCleanly",
  "g_sequentialPickerCancellationReleasesNativeOperationSlot",
]) {
  expectContains(smoke, token, `Android picker synchronization invariant ${token}`);
}
if (smoke.includes("Thread.sleep(250)")) {
  failures.push("Android picker sequencing must use acknowledged operation state, not a fixed delay");
}
if (smoke.includes("f_systemImportImageAndExportPickersCancelCleanly")) {
  failures.push("Independent Android picker checks must not remain hidden inside one fail-fast test");
}

for (const token of [
  "failed_cases=()",
  "skipped_cases=()",
  "record_case",
  "f_studyContentImportPickerCancelsCleanly",
  "f_imageImportPickerCancelsCleanly",
  "f_pdfImportPickerCancelsCleanly",
  "f_libraryBackupImportPickerCancelsCleanly",
  "f_deckExportPickerCancelsCleanly",
  "g_sequentialPickerCancellationReleasesNativeOperationSlot",
]) {
  expectContains(smokeRunner, token, `aggregated Android smoke-runner invariant ${token}`);
}

for (const [path, text] of [
  ["scripts/check-static.sh", checkStatic],
  ["scripts/android-test.sh", androidTest],
  ["scripts/android-build.sh", androidBuild],
]) {
  expectContains(
    text,
    "verify-external-file-streaming.mjs",
    `external-file streaming verifier wiring in ${path}`,
  );
}

if (failures.length > 0) {
  console.error("External-file streaming verification failed:");
  for (const failure of failures) console.error(`  - ${failure}`);
  process.exit(1);
}

console.log(
  "OK   External file imports and exports use mandatory streaming ports, recover interrupted promotion, expose cancellation outcomes, cover every import purpose, and aggregate independent picker checks.",
);

function expectContains(text, value, label) {
  if (!text.includes(value)) failures.push(`Missing ${label}`);
}

async function read(path) {
  return readFile(resolve(root, path), "utf8");
}
