#!/usr/bin/env node

const VALID_GATES = new Set([
  "preflight",
  "static",
  "fast",
  "linux-full",
  "android-full",
  "all-full",
]);

const eventName = process.env.EVENT_NAME ?? "";
const dispatchGate = process.env.DISPATCH_GATE ?? "all-full";
const pullRequestDraft = (process.env.PR_DRAFT ?? "false") === "true";

let gate;
if (eventName === "workflow_dispatch") {
  gate = dispatchGate;
} else if (eventName === "pull_request" && pullRequestDraft) {
  gate = "fast";
} else {
  gate = "all-full";
}

if (!VALID_GATES.has(gate)) {
  console.error(`Invalid qualification gate: ${gate}`);
  process.exit(2);
}

process.stdout.write(`${gate}\n`);
