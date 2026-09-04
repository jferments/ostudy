#!/usr/bin/env node

import fs from "node:fs";

const VALID_GATES = new Set([
  "preflight",
  "static",
  "fast",
  "linux-full",
  "android-full",
  "all-full",
]);
const FULL_QUALIFICATION_LABEL = "qualification:full";

const eventName = process.env.EVENT_NAME ?? "";
const dispatchGate = process.env.DISPATCH_GATE ?? "all-full";
const pullRequestDraft = (process.env.PR_DRAFT ?? "false") === "true";

function draftRequestsFullQualification() {
  if (eventName !== "pull_request" || !pullRequestDraft) return false;
  const eventPath = process.env.GITHUB_EVENT_PATH;
  if (!eventPath) return false;

  let event;
  try {
    event = JSON.parse(fs.readFileSync(eventPath, "utf8"));
  } catch (error) {
    console.error(`Could not read GitHub event payload: ${error instanceof Error ? error.message : String(error)}`);
    process.exit(2);
  }

  const labels = event?.pull_request?.labels;
  return Array.isArray(labels)
    && labels.some((label) => label && typeof label === "object" && label.name === FULL_QUALIFICATION_LABEL);
}

let gate;
if (eventName === "workflow_dispatch") {
  gate = dispatchGate;
} else if (eventName === "pull_request" && pullRequestDraft) {
  gate = draftRequestsFullQualification() ? "all-full" : "fast";
} else {
  gate = "all-full";
}

if (!VALID_GATES.has(gate)) {
  console.error(`Invalid qualification gate: ${gate}`);
  process.exit(2);
}

process.stdout.write(`${gate}\n`);
