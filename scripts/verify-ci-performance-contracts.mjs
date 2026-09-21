#!/usr/bin/env node

import { readFile } from "node:fs/promises";
import { join } from "node:path";
import { fileURLToPath } from "node:url";

const root = fileURLToPath(new URL("..", import.meta.url));
const read = (...parts) => readFile(join(root, ...parts), "utf8");

const [checkStatic, checkFast, timing, nativeContracts, installer, guestPreflight, storageReport] =
  await Promise.all([
    read("scripts", "check-static.sh"),
    read("scripts", "check-fast.sh"),
    read("scripts", "lib", "qualification-timing.bash"),
    read("scripts", "test-native-contracts.sh"),
    read("scripts", "install-native-deps.sh"),
    read("scripts", "runner", "guest-preflight.sh"),
    read("scripts", "runner", "report-storage.sh"),
  ]);

const failures = [];
function requireText(label, text, needles) {
  for (const needle of needles) {
    if (!text.includes(needle)) failures.push(`${label} missing: ${needle}`);
  }
}

requireText("static timing", checkStatic, [
  "qualification-timing.bash",
  "ostudy_timing_init static",
  "ostudy_timing_finish_total passed",
]);
requireText("fast timing", checkFast, [
  "qualification-timing.bash",
  "ostudy_timing_init fast",
  'step "Rust compile and warnings-as-errors lint gate"',
  "cargo clippy --manifest-path src-tauri/Cargo.toml --locked --all-targets --all-features -- -D warnings",
]);
if (
  checkFast.includes(
    "cargo check --manifest-path src-tauri/Cargo.toml --locked --all-targets --all-features",
  )
) {
  failures.push("fast qualification must not duplicate the all-target/all-feature Clippy compile pass with cargo check");
}
requireText("timing helper", timing, [
  "TIMING scope=%s status=%s elapsed_seconds=%s phase=%s",
  "GITHUB_STEP_SUMMARY",
  "ostudy_timing_finish_step",
  "ostudy_timing_finish_total",
]);
requireText("native contract helper", nativeContracts, [
  "test result: ok",
  "matched no executed tests",
  "did not execute every selected test",
]);
if (nativeContracts.includes("-- --list")) {
  failures.push("native contract helper must not invoke Cargo a second time only to list matching tests");
}
requireText("PDFium cache", installer, [
  "OSTUDY_NATIVE_DEPS_CACHE_DIR",
  "cached_library",
  "archive_sha256=$EXPECTED_SHA",
  "library_sha256=$cached_sha",
  "restored from verified runner cache",
]);
requireText("runner storage report", guestPreflight, [
  "OSTUDY_REPORT_STORAGE",
  "report-storage.sh",
  "root_free_gib",
]);
requireText("bounded storage inventory", storageReport, [
  "OSTUDY_STORAGE_DU_TIMEOUT_SECONDS",
  "Cargo target",
  "Gradle active",
  "Android SDK",
  "Actions work",
  "Runner evidence",
]);

if (failures.length > 0) {
  console.error("CI performance contract verification failed:");
  for (const failure of failures) console.error(`  - ${failure}`);
  process.exit(1);
}
console.log("OK   CI timing, cache, Cargo-call, and storage-report contracts are consistent.");
