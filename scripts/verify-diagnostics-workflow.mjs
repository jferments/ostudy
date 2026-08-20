#!/usr/bin/env node

import { readFile } from "node:fs/promises";
import { fileURLToPath } from "node:url";
import { dirname, join } from "node:path";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const workflow = await readFile(join(root, ".github", "workflows", "check.yml"), "utf8");
const checkStatic = await readFile(join(root, "scripts", "check-static.sh"), "utf8");
const checkFast = await readFile(join(root, "scripts", "check-fast.sh"), "utf8");
const checkFull = await readFile(join(root, "scripts", "check.sh"), "utf8");
const diagnostics = await readFile(join(root, "scripts", "lib", "diagnostics.bash"), "utf8");
const summary = await readFile(join(root, "scripts", "summarize-check-failure.mjs"), "utf8");
const resultWriter = await readFile(join(root, "scripts", "write-qualification-result.mjs"), "utf8");
const handoffRenderer = await readFile(join(root, "scripts", "render-agent-handoff.mjs"), "utf8");

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
  "ostudy-static-check.log",
  "ostudy-fast-check.log",
  "ostudy-full-check.log",
  "ostudy-android-test.log",
  "ostudy-android-smoke.log",
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

for (const required of [
  "OSTUDY_DIAGNOSTIC_CAPTURED",
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
  "OK   Static/fast/full checks share bounded failure capture, CI starts with low-noise structured diagnostics, exact-head JSON and Markdown handoff evidence are retained, and summaries emit focused reproductions.",
);
