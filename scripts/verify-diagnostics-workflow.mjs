#!/usr/bin/env node

import { readFile } from "node:fs/promises";

const files = {
  workflow: ".github/workflows/check.yml",
  library: "scripts/lib/diagnostics.bash",
  nativeSmoke: "scripts/smoke-native.sh",
  summary: "scripts/summarize-check-failure.mjs",
  diagnosticsDoc: "docs/development/diagnostics.md",
  testingDoc: "docs/development/testing-and-ci.md",
};

const contents = Object.fromEntries(
  await Promise.all(
    Object.entries(files).map(async ([key, path]) => [key, await readFile(path, "utf8")]),
  ),
);

const contracts = [
  [
    "CI establishes a low-noise first-run diagnostic timeline",
    contents.library.includes("OSTUDY_CI_LOG_FILTER:-warn,ostudy=info"),
  ],
  [
    "CI retains bounded failure artifacts without a separate debug rerun",
    contents.library.includes('OSTUDY_KEEP_FAILURE_ARTIFACTS=1'),
  ],
  [
    "diagnostic artifacts support a runner-owned root",
    contents.library.includes("OSTUDY_DIAGNOSTIC_ARTIFACT_ROOT"),
  ],
  [
    "every captured boundary writes a focused replay script",
    contents.library.includes('reproduce.sh') && contents.library.includes("ostudy_replay_command_for_step"),
  ],
  [
    "the authoritative workflow exports the first-run baseline to every platform step",
    /OSTUDY_LOG:\s*warn,ostudy=info/u.test(contents.workflow) &&
      /OSTUDY_DIAGNOSTICS_AUTOMATIC:\s*"1"/u.test(contents.workflow),
  ],
  [
    "the authoritative workflow owns a runner-temp diagnostic directory",
    contents.workflow.includes(
      'echo "OSTUDY_DIAGNOSTIC_ARTIFACT_ROOT=$RUNNER_TEMP/ostudy-diagnostics" >> "$GITHUB_ENV"',
    ),
  ],
  [
    "the failure summary reads structured diagnostic artifacts",
    contents.workflow.includes('--diagnostics "$RUNNER_TEMP/ostudy-diagnostics"') &&
      contents.summary.includes("Structured application diagnostics"),
  ],
  [
    "recovered WebDriver transport flakes preserve their isolated app log",
    contents.nativeSmoke.includes("ostudy_capture_transient_artifacts") &&
      contents.nativeSmoke.includes("application_log"),
  ],
  [
    "final WebDriver failures preserve their isolated app log before cleanup",
    contents.nativeSmoke.includes("ostudy_capture_failure_artifacts") &&
      contents.nativeSmoke.indexOf("ostudy_capture_failure_artifacts") <
        contents.nativeSmoke.lastIndexOf('return "$status"'),
  ],
  [
    "bounded diagnostic evidence is uploaded even when a transient retry recovers",
    /name: Upload bounded diagnostic evidence[\s\S]*?if: always\(\)[\s\S]*?ostudy-diagnostics/u.test(
      contents.workflow,
    ),
  ],
  [
    "developer diagnostics documentation explains focused replay",
    contents.diagnosticsDoc.includes("Focused failure replay") &&
      contents.diagnosticsDoc.includes("reproduce.sh"),
  ],
  [
    "CI documentation explains the first-run diagnostic baseline",
    contents.testingDoc.includes("warn,ostudy=info") &&
      contents.testingDoc.includes("transient WebDriver"),
  ],
];

const failures = contracts.filter(([, passed]) => !passed).map(([description]) => description);
if (failures.length > 0) {
  console.error("Diagnostics workflow contract failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log(
  "OK   CI captures a low-noise structured timeline, preserves isolated failure state, emits focused replay commands, and uploads bounded transient/failure evidence.",
);
