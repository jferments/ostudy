#!/usr/bin/env node

import { readFile } from "node:fs/promises";
import { resolve } from "node:path";
import { fileURLToPath } from "node:url";

const root = fileURLToPath(new URL("..", import.meta.url));
const failures = [];

const lib = await read("src-tauri/src/lib.rs");
const transfer = await read("src-tauri/src/external_file_streaming.rs");
const ipc = await read("src/ipc/externalFiles.ts");
const bridge = await read("src/testing/androidTestBridge.ts");
const smoke = await read(
  "src-tauri/gen/android/app/src/androidTest/java/io/github/jferments/ostudy/AndroidSmokeTest.kt",
);
const androidAdapter = await read("src-tauri/src/platform/android.rs");
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
  "fn acquire_import(",
  "fn publish_export(",
  "app.ostudy_platform().acquire_import",
  "app.ostudy_platform().publish_export",
]) {
  expectContains(androidAdapter, token, `Android streaming adapter invariant ${token}`);
}

for (const token of [
  "fileOperationSnapshot",
  "startImportPicker",
  "startDeckExportPicker",
  'phase: "pending"',
  'phase: "error"',
]) {
  expectContains(bridge, token, `Android bridge operation-state invariant ${token}`);
}
if (bridge.includes(".catch(() => undefined)")) {
  failures.push("Android smoke bridge must not discard external-file operation failures");
}

for (const token of [
  "waitForDocumentsUiOrFailure",
  "waitForFileOperationSettled",
  "fileOperationSnapshot",
  "startFileOperation",
]) {
  expectContains(smoke, token, `Android picker synchronization invariant ${token}`);
}
if (smoke.includes("Thread.sleep(250)")) {
  failures.push("Android picker sequencing must use acknowledged operation state, not a fixed delay");
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
  "OK   External file imports and exports use the streaming port, expose test failures, and synchronize picker cancellation by acknowledged operation state.",
);

function expectContains(text, value, label) {
  if (!text.includes(value)) failures.push(`Missing ${label}`);
}

async function read(path) {
  return readFile(resolve(root, path), "utf8");
}
