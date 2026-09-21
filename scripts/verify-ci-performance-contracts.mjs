#!/usr/bin/env node

import { readFile } from "node:fs/promises";
import { join } from "node:path";
import { fileURLToPath } from "node:url";

const root = fileURLToPath(new URL("..", import.meta.url));
const read = (...parts) => readFile(join(root, ...parts), "utf8");

const [
  checkStatic,
  checkFast,
  timing,
  nativeContracts,
  fastNativeInventory,
  ciTestTmpdir,
  ciCapture,
  checkFull,
  checkWorkflow,
  androidTest,
  androidSmokeSetup,
  installer,
  guestPreflight,
  storageReport,
  createVm,
  hostPreflight,
  bootstrapGuest,
] = await Promise.all([
    read("scripts", "check-static.sh"),
    read("scripts", "check-fast.sh"),
    read("scripts", "lib", "qualification-timing.bash"),
    read("scripts", "test-native-contracts.sh"),
    read("scripts", "verify-fast-native-contracts.sh"),
    read("scripts", "lib", "ci-test-tmpdir.bash"),
    read("scripts", "run-ci-captured.sh"),
    read("scripts", "check.sh"),
    read(".github", "workflows", "check.yml"),
    read("scripts", "android-test.sh"),
    read("scripts", "lib", "android-emulator-smoke-setup.bash"),
    read("scripts", "install-native-deps.sh"),
    read("scripts", "runner", "guest-preflight.sh"),
    read("scripts", "runner", "report-storage.sh"),
    read("scripts", "runner", "create-unified-runner-vm.sh"),
    read("scripts", "runner", "host-preflight.sh"),
    read("scripts", "runner", "bootstrap-unified-guest.sh"),
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
  'step "Rust warnings-as-errors lint gate"',
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
  "-- --list --color never",
  "matched no tests",
  "did not execute every selected test",
]);

requireText("fast native inventory", fastNativeInventory, [
  "--list",
  "database::entry_tests::",
  "knowledge::",
  "library_backup::image_set_backup_tests::",
  "public_migration_checksum_and_future_version_are_enforced",
]);
requireText("CI tmpfs test workspace", ciTestTmpdir, [
  "/dev/shm",
  "OSTUDY_CI_USE_TMPFS",
  "4 * 1024 * 1024",
  "export TMPDIR",
]);
requireText("fast tmpfs and consolidated Rust execution", checkFast, [
  "ostudy_prepare_ci_test_tmpdir fast",
  'step "Critical native contract inventory"',
  'step "Rust library contract suite"',
  '--test-threads "$CARGO_BUILD_JOBS"',
]);
const fastCargoTestCalls = checkFast.match(/\bcargo test\b/gu) ?? [];
if (fastCargoTestCalls.length !== 2) {
  failures.push(`fast qualification must contain exactly two cargo test commands after inventory, found ${fastCargoTestCalls.length}`);
}
requireText("full tmpfs and timing", checkFull, [
  "ostudy_prepare_ci_test_tmpdir full",
  "ostudy_timing_init full",
  "ostudy_timing_finish_total passed",
]);
requireText("captured timing transport", ciCapture, [
  "qualification phase timings",
  "grep '^TIMING '",
]);
requireText("full avoids duplicate library suite", checkFull, [
  'step "Rust integration and migration-fixture tests"',
  "--test document_artifact_metadata_migration7",
  "--test document_citation_migration6",
  "--test study_guide_migration5",
  'step "Rust binary tests"',
  'step "Rust documentation tests"',
]);
if (checkFull.includes('step "Rust unit, integration, and migration-fixture tests"')) {
  failures.push("full qualification must not rerun the complete Rust library suite after fast qualification");
}
requireText("same-job all-full reuse markers", checkWorkflow, [
  "OSTUDY_LINUX_FULL_PASSED",
  "OSTUDY_ANDROID_COMPILE_PASSED",
]);
requireText("Android shared-check reuse", androidTest, [
  "OSTUDY_LINUX_FULL_PASSED",
  "REUSE Shared TypeScript, Vitest, host Rust compile/lint, and Rust test coverage",
]);
requireText("Android smoke build reuse", androidSmokeSetup, [
  "OSTUDY_ANDROID_COMPILE_PASSED",
  "REUSE x86_64 Android smoke APK and instrumentation package",
]);

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
requireText("32-vCPU VM creation default", createVm, ['OSTUDY_VM_VCPUS:-32', "default: 32"]);
requireText("32-vCPU host preflight default", hostPreflight, ['OSTUDY_VM_VCPUS:-32']);
requireText("32-vCPU guest preflight default", guestPreflight, ['OSTUDY_VM_VCPUS:-32']);
requireText("unified Gradle cache", bootstrapGuest, ["$RUNNER_HOME/.cache/ostudy/gradle"]);
if (bootstrapGuest.includes("$RUNNER_HOME/.cache/gradle")) {
  failures.push("runner bootstrap must not create or advertise the legacy duplicate Gradle cache");
}

requireText("bounded storage inventory", storageReport, [
  "OSTUDY_STORAGE_DU_TIMEOUT_SECONDS",
  "Cargo target",
  "Cargo host incremental",
  "x86_64-linux-android",
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
