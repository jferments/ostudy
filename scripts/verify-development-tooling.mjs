#!/usr/bin/env node

import { readFile } from "node:fs/promises";

const read = (path) => readFile(new URL(`../${path}`, import.meta.url), "utf8");
const failures = [];

const [
  developerCli,
  doctor,
  setup,
  staticCheck,
  fastCheck,
  fullCheck,
  ciPreflight,
  guestPreflight,
  checkWorkflow,
  contributing,
  runbook,
] = await Promise.all([
  read("scripts/ostudy-dev"),
  read("scripts/doctor.sh"),
  read("scripts/setup-ubuntu.sh"),
  read("scripts/check-static.sh"),
  read("scripts/check-fast.sh"),
  read("scripts/check.sh"),
  read("scripts/runner/ci-runtime-preflight.sh"),
  read("scripts/runner/guest-preflight.sh"),
  read(".github/workflows/check.yml"),
  read("CONTRIBUTING.md"),
  read("docs/agents/COMMAND-RUNBOOK.md"),
]);

function requireText(label, source, needle) {
  if (!source.includes(needle)) failures.push(`${label} missing ${JSON.stringify(needle)}`);
}

function rejectText(label, source, needle) {
  if (source.includes(needle)) failures.push(`${label} must not contain ${JSON.stringify(needle)}`);
}

for (const [label, command, target] of [
  ["setup", "setup)", 'exec "$SCRIPT_DIR/setup-ubuntu.sh" "$@"'],
  ["doctor", "doctor)", 'exec "$SCRIPT_DIR/doctor.sh" "$@"'],
  ["build", "build)", 'exec "$SCRIPT_DIR/build.sh" "$@"'],
  ["run", "run)", 'exec "$ROOT_DIR/run-app.sh" "$@"'],
  ["changed check", "check-changed)", 'exec "$SCRIPT_DIR/check-changed.sh" "$@"'],
  ["static check", "check-static)", 'exec "$SCRIPT_DIR/check-static.sh" "$@"'],
  ["fast check", "check-fast|test)", 'exec "$SCRIPT_DIR/check-fast.sh" "$@"'],
  ["full check", "check|test-full)", 'exec "$SCRIPT_DIR/check.sh" "$@"'],
]) {
  requireText(`developer CLI ${label}`, developerCli, command);
  requireText(`developer CLI ${label} target`, developerCli, target);
}

requireText("developer CLI help", developerCli, "doctor          Verify core local build requirements");
requireText("doctor core mode", doctor, "CHECK_NATIVE_SMOKE=0");
requireText("doctor native-smoke mode", doctor, "--native-smoke) CHECK_NATIVE_SMOKE=1");
requireText("doctor native-smoke tools", doctor, "WebKitWebDriver tauri-driver");
requireText("doctor script linters", doctor, "ruff shellcheck ty");
requireText("setup pins script linters", setup, "ostudy_ensure_script_linters");
requireText("static check owns pinned script linters", staticCheck, "ostudy_ensure_script_linters");
requireText("full check owns pinned script linters", fullCheck, "ostudy_ensure_script_linters");
requireText("static check runs script correctness lint", staticCheck, 'lint-scripts.sh');
requireText("setup links ty", setup, 'ln -sfn "$OSTUDY_TY" "$LOCAL_BIN/ty"');
requireText("static check owns local doctor", staticCheck, '"$SCRIPT_DIR/doctor.sh"');
requireText("fast check reuses static contract", fastCheck, 'check-static.sh');
requireText("full check owns native dependencies", fullCheck, 'install-native-deps.sh');

for (const [label, source] of [
  ["developer CLI", developerCli],
  ["doctor", doctor],
  ["setup", setup],
]) {
  for (const forbidden of [
    "ostudy-ci-unified",
    "RUNNER_NAME",
    "GITHUB_WORKSPACE",
    "RUNNER_WORKSPACE",
    "OSTUDY_EXPECT_RUNNER",
  ]) {
    rejectText(label, source, forbidden);
  }
}

requireText("CI-only runner identity", ciPreflight, "RUNNER_NAME");
requireText("CI-only workspace isolation", ciPreflight, "GITHUB_WORKSPACE");
requireText("runner-only resource sizing", guestPreflight, "OSTUDY_VM_RAM_GIB");
requireText("setup self-verifies native smoke", setup, '"$SCRIPT_DIR/doctor.sh" --native-smoke');
requireText("Actions uses local static wrapper", checkWorkflow, "bash ./scripts/check-static.sh");
requireText("Actions uses local fast wrapper", checkWorkflow, "bash ./scripts/check-fast.sh");
requireText("Actions uses local full wrapper", checkWorkflow, "bash ./scripts/check.sh");
requireText("Actions adds runner preflight separately", checkWorkflow, "bash ./scripts/runner/ci-runtime-preflight.sh");
requireText("Actions adds guest resource preflight separately", checkWorkflow, "bash ./scripts/runner/guest-preflight.sh");
requireText("contributor docs doctor", contributing, "./scripts/ostudy-dev doctor");
requireText("runbook doctor", runbook, "./scripts/ostudy-dev doctor");
requireText("runbook runner separation", runbook, "scripts/runner/");

if (failures.length > 0) {
  console.error("Development-tooling verification failed:");
  for (const failure of failures) console.error(`  - ${failure}`);
  process.exit(1);
}

console.log(
  "OK   Contributor setup/doctor/build/run/check commands are repository-owned and independent of private runner identity/resource policy.",
);
