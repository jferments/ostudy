#!/usr/bin/env node

import { readFile } from "node:fs/promises";
import { fileURLToPath } from "node:url";
import { dirname, join } from "node:path";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const workflow = await readFile(join(root, ".github", "workflows", "check.yml"), "utf8");
const unifiedRunnerWorkflow = await readFile(
  join(root, ".github", "workflows", "unified-runner-qualification.yml"),
  "utf8",
);
const checkStatic = await readFile(join(root, "scripts", "check-static.sh"), "utf8");
const checkFast = await readFile(join(root, "scripts", "check-fast.sh"), "utf8");
const checkFull = await readFile(join(root, "scripts", "check.sh"), "utf8");
const diagnostics = await readFile(join(root, "scripts", "lib", "diagnostics.bash"), "utf8");
const summary = await readFile(join(root, "scripts", "summarize-check-failure.mjs"), "utf8");
const resultWriter = await readFile(join(root, "scripts", "write-qualification-result.mjs"), "utf8");
const handoffRenderer = await readFile(join(root, "scripts", "render-agent-handoff.mjs"), "utf8");
const ciCapture = await readFile(join(root, "scripts", "run-ci-captured.sh"), "utf8");
const nativeSmoke = await readFile(join(root, "scripts", "smoke-native.sh"), "utf8");
const androidSmoke = await readFile(join(root, "scripts", "android-emulator-smoke.sh"), "utf8");

const failures = [];

for (const [path, content] of [
  ["scripts/check-static.sh", checkStatic],
  ["scripts/check-fast.sh", checkFast],
  ["scripts/check.sh", checkFull],
]) {
  for (const required of [
    "source \"$SCRIPT_DIR/lib/diagnostics.bash\"",
    "ostudy_parse_diagnostic_args",
    "OSTUDY_QUALIFICATION_COMMAND",
    "ostudy_capture_failure_artifacts",
  ]) {
    if (!content.includes(required)) failures.push(`${path} missing ${required}`);
  }
}

for (const required of [
  "OSTUDY_CI_LOG_FILTER: warn,ostudy=info",
  "OSTUDY_DIAGNOSTICS_ACTIVE: \"1\"",
  "OSTUDY_DIAGNOSTICS_AUTOMATIC: \"1\"",
  "OSTUDY_KEEP_FAILURE_ARTIFACTS: \"1\"",
  "run-ci-captured.sh",
  "ostudy-static-check.log",
  "ostudy-fast-check.log",
  "ostudy-full-check.log",
  "ostudy-android-test.log",
  "ostudy-android-smoke.log",
  "ostudy-failure-summary.md",
  "summarize-check-failure.mjs",
  "write-qualification-result.mjs",
  "render-agent-handoff.mjs",
  "ostudy-qualification-result.json",
  "ostudy-pr-contract.json",
  "ostudy-agent-handoff.md",
  "Enforce handoff evidence integrity",
  "export-diagnostics.sh",
  "Upload bounded diagnostic evidence",
  "Upload bounded failure evidence",
]) {
  if (!workflow.includes(required)) failures.push(`check.yml missing ${required}`);
}

if (!workflow.includes("cancel-in-progress: true")) {
  failures.push(
    "check.yml must cancel obsolete work within each qualification lane",
  );
}

if (!unifiedRunnerWorkflow.includes("cancel-in-progress: true")) {
  failures.push(
    "unified-runner-qualification.yml must cancel obsolete manual qualification for the same ref",
  );
}

const checkBranchConcurrency =
  "group: ${{ github.workflow }}-${{ github.event_name == 'workflow_dispatch' && 'manual' || 'automatic' }}-${{ github.event.pull_request.head.repo.full_name || github.repository }}-${{ github.event.pull_request.head.ref || github.ref_name }}";
if (!workflow.includes(checkBranchConcurrency)) {
  failures.push(
    "check.yml must isolate automatic and manual pending slots while keying each lane by head repository and branch",
  );
}

const manualCheckContext =
  "name: ${{ github.event_name == 'workflow_dispatch' && 'manual-check' || 'check' }}";
if (!workflow.includes(manualCheckContext)) {
  failures.push(
    "check.yml must keep manual diagnostic results from replacing the automatic PR check context",
  );
}

if (!workflow.includes("if: github.event.action != 'closed'")) {
  failures.push(
    "check.yml must use a cancellation-only closed event without starting qualification",
  );
}

const readyOnlyContract =
  "if: github.event_name == 'pull_request' && github.event.pull_request.draft == false";
if (!workflow.includes(readyOnlyContract)) {
  failures.push(
    "check.yml must not let compatibility snapshot errors block draft source qualification",
  );
}

for (const forbidden of [
  "edited]",
  "PR_BODY:",
  "ostudy-check:all-full",
]) {
  if (workflow.includes(forbidden)) {
    failures.push(`check.yml must not use PR-body edits or markers to control product qualification: ${forbidden}`);
  }
}

for (const forbidden of ["id: ui_performance", "check-ui-performance"]) {
  if (workflow.includes(forbidden)) {
    failures.push(
      `check.yml must keep wall-clock UI performance outside required correctness qualification: ${forbidden}`,
    );
  }
}

for (const forbidden of [
  "pull_request:",
  "PR_BODY:",
  "unified-runner:preflight",
  "unified-runner:linux-fast",
  "unified-runner:linux-full",
  "unified-runner:android-full",
  "unified-runner:all-full",
]) {
  if (unifiedRunnerWorkflow.includes(forbidden)) {
    failures.push(`unified-runner-qualification.yml must remain manual-only and marker-free: ${forbidden}`);
  }
}

if (/2>&1\s*\|\s*tee\s+"\$RUNNER_TEMP\/ostudy-(?:static|fast|full|android)/u.test(workflow)) {
  failures.push(
    "check.yml must retain full qualification logs in files instead of streaming them through tee",
  );
}

for (const required of [
  "OSTUDY_CI_SUCCESS_TAIL_BYTES",
  "OSTUDY_CI_FAILURE_TAIL_BYTES",
  "OSTUDY_CI_HEARTBEAT_SECONDS",
  "Qualification still running",
  "tail -c",
  "full output retained",
  "exit \"$status\"",
]) {
  if (!ciCapture.includes(required)) failures.push(`CI log capture helper missing ${required}`);
}

if (!nativeSmoke.includes("timeout --signal=INT --kill-after=30s 300s")) {
  failures.push("native WebDriver smoke must retain a hard-kill fallback");
}
if (!androidSmoke.includes("timeout --signal=INT --kill-after=30s 180s")) {
  failures.push("Android instrumentation smoke must retain a per-case hard timeout");
}

for (const required of [
  "failure-claim",
  "reproduce.sh",
  "replayFilter",
  "OSTUDY_DIAGNOSTIC_ARTIFACT_ROOT",
  "OSTUDY_DIAGNOSTIC_CAPTURE_TOKEN",
  "ostudy_capture_transient_artifacts",
]) {
  if (!diagnostics.includes(required)) failures.push(`diagnostics library missing ${required}`);
}

for (const required of [
  "Fastest local reproduction",
  "OSTUDY_FORCE_CHECK=1",
  "OSTUDY_KEEP_FAILURE_ARTIFACTS=1",
  "renderStructuredDiagnostics",
]) {
  if (!summary.includes(required)) failures.push(`failure summarizer missing ${required}`);
}

for (const required of [
  "schemaVersion: 1",
  "fingerprint",
  "focusedReproduction",
  "classificationHint",
]) {
  if (!resultWriter.includes(required)) failures.push(`qualification result writer missing ${required}`);
}

for (const required of [
  "Evidence integrity",
  "Changed files",
  "Commands represented by this qualification",
  "Qualification head",
  "Requested method",
]) {
  if (!handoffRenderer.includes(required)) failures.push(`handoff renderer missing ${required}`);
}

if (workflow.includes("OSTUDY_LOG: debug")) {
  failures.push("check.yml must not default qualification to a repository-wide debug filter");
}
if (/OSTUDY_LOG:\s*\$\{\{/u.test(workflow)) {
  failures.push("check.yml must not accept an untrusted dynamic OSTUDY_LOG expression");
}

if (failures.length > 0) {
  console.error("Diagnostics workflow verification failed:");
  for (const failure of failures) console.error(`  - ${failure}`);
  process.exit(1);
}

console.log(
  "OK   Static/fast/full checks share bounded failure capture, CI retains full raw logs with bounded liveness heartbeats, exact-head JSON and Markdown handoff evidence are retained, source changes supersede obsolete qualification, wall-clock performance remains outside required correctness CI, GUI boundaries have hard timeout fallbacks, and summaries emit focused reproductions.",
);
