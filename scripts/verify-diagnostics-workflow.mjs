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

const staticIndex = contents.workflow.indexOf("- name: Run static qualification first");
const fastIndex = contents.workflow.indexOf("- name: Run fast qualification");
const linuxIndex = contents.workflow.indexOf("- name: Run authoritative Linux qualification");
const androidIndex = contents.workflow.indexOf(
  "- name: Run authoritative Android compile and package qualification",
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
    "nested qualification wrappers share one failure-capture token",
    contents.library.includes("OSTUDY_DIAGNOSTIC_CAPTURE_TOKEN") &&
      contents.library.includes("failure-claim"),
  ],
  [
    "every captured boundary writes a focused replay script",
    contents.library.includes("reproduce.sh") &&
      contents.library.includes("ostudy_replay_command_for_step"),
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
    "static qualification runs before every expensive product gate",
    staticIndex >= 0 &&
      fastIndex > staticIndex &&
      linuxIndex > staticIndex &&
      androidIndex > staticIndex &&
      contents.workflow.includes("if: steps.gate.outputs.gate != 'preflight'") &&
      contents.workflow.includes('bash ./scripts/check-static.sh 2>&1 | tee "$RUNNER_TEMP/ostudy-static-check.log"'),
  ],
  [
    "connector-visible exact-head statuses expose static and overall qualification",
    /permissions:[\s\S]*?contents:\s*read[\s\S]*?statuses:\s*write/u.test(contents.workflow) &&
      contents.workflow.includes("--arg context 'ostudy/static'") &&
      contents.workflow.includes("--arg context 'ostudy/qualification'") &&
      contents.workflow.includes("statuses/$QUALIFIED_SHA"),
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
    "CI documentation explains the first-run diagnostic baseline and observable static gate",
    contents.testingDoc.includes("warn,ostudy=info") &&
      contents.testingDoc.includes("transient WebDriver") &&
      contents.testingDoc.includes("failure-capture token") &&
      contents.testingDoc.includes("ostudy/static") &&
      contents.testingDoc.includes("static qualification first"),
  ],
];

const failures = contracts.filter(([, passed]) => !passed).map(([description]) => description);
if (failures.length > 0) {
  console.error("Diagnostics workflow contract failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log(
  "OK   CI runs static qualification first, publishes connector-visible exact-head status, captures a low-noise structured timeline, preserves isolated failure state, emits focused replay commands, and uploads bounded transient/failure evidence.",
);
