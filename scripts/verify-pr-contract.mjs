#!/usr/bin/env node

import { readFile, writeFile } from "node:fs/promises";

const START_MARKER = "<!-- ostudy-agent-state:start -->";
const END_MARKER = "<!-- ostudy-agent-state:end -->";
const SHA_PATTERN = /^[0-9a-f]{40}$/u;
const PLACEHOLDERS = new Set(["unrecorded", "unqualified"]);
const ACCEPTANCE_STATUSES = new Set([
  "notStarted",
  "inProgress",
  "implemented",
  "humanRequired",
  "notApplicable",
]);
const TERMINAL_ACCEPTANCE_STATUSES = new Set([
  "implemented",
  "humanRequired",
  "notApplicable",
]);
const EVIDENCE_STATUSES = new Set([
  "pending",
  "passed",
  "failed",
  "blocked",
  "notRun",
  "notRequired",
  "humanRequired",
]);
const TERMINAL_EVIDENCE_STATUSES = new Set([
  "passed",
  "notRequired",
  "humanRequired",
]);
const REQUIRED_READY_GATES = ["static", "fast", "full"];
const CURRENT_STATES = new Set([
  "planning",
  "scaffolding",
  "implementation",
  "validation",
  "manualAcceptance",
  "readyForReview",
]);
const EXECUTION_MODES = new Set([
  "localCheckout",
  "githubConnector",
  "githubConnectorWithExecutionWorkspace",
]);
const FAILURE_CLASSIFICATIONS = new Set([
  "product",
  "testContract",
  "runnerEnvironment",
  "connector",
  "githubIncident",
  "externalManualBoundary",
  "unknown",
]);

const options = parseArguments(process.argv.slice(2));
const source = await readSource(options);
if (source.skip) {
  console.log(source.message);
  process.exit(0);
}

const errors = [];
const warnings = [];
let contract = null;
try {
  contract = extractContract(source.body);
} catch (error) {
  errors.push(error instanceof Error ? error.message : String(error));
}

const actual = source.pullRequest
  ? {
      draft: Boolean(source.pullRequest.draft),
      baseSha: source.pullRequest.base?.sha ?? "",
      headSha: source.pullRequest.head?.sha ?? "",
      branch: source.pullRequest.head?.ref ?? "",
      number: source.pullRequest.number ?? null,
    }
  : null;

if (contract) validateContract(contract, actual, options.strict, errors, warnings);

const result = {
  schemaVersion: 1,
  valid: errors.length === 0,
  strict: options.strict || actual?.draft === false,
  actual,
  contract,
  warnings,
  errors,
};

if (options.output) {
  await writeFile(options.output, `${JSON.stringify(result, null, 2)}\n`, "utf8");
}

for (const warning of warnings) console.warn(`PR contract warning: ${warning}`);
if (errors.length > 0) {
  console.error("Pull-request execution contract verification failed:");
  for (const error of errors) console.error(`  - ${error}`);
  process.exit(1);
}

const acceptanceCount = Object.keys(contract.acceptance).length;
const terminalCount = Object.values(contract.acceptance).filter((entry) =>
  TERMINAL_ACCEPTANCE_STATUSES.has(entry.status),
).length;
console.log(
  `OK   Pull-request execution contract: ${terminalCount}/${acceptanceCount} acceptance criteria are terminal; current state is ${contract.currentState}.`,
);

function parseArguments(argumentsList) {
  const parsed = {
    eventPath: process.env.GITHUB_EVENT_PATH ?? null,
    bodyFile: null,
    output: null,
    strict: false,
  };
  for (let index = 0; index < argumentsList.length; index += 1) {
    const argument = argumentsList[index];
    if (argument === "--event") {
      parsed.eventPath = requireValue(argumentsList, ++index, argument);
    } else if (argument === "--body-file") {
      parsed.bodyFile = requireValue(argumentsList, ++index, argument);
    } else if (argument === "--output") {
      parsed.output = requireValue(argumentsList, ++index, argument);
    } else if (argument === "--strict") {
      parsed.strict = true;
    } else {
      throw new Error(`Unknown argument: ${argument}`);
    }
  }
  return parsed;
}

function requireValue(argumentsList, index, option) {
  const value = argumentsList[index];
  if (!value) throw new Error(`${option} requires a value`);
  return value;
}

async function readSource(parsed) {
  if (parsed.bodyFile) {
    return {
      body: await readFile(parsed.bodyFile, "utf8"),
      pullRequest: null,
      skip: false,
    };
  }
  if (!parsed.eventPath) {
    return {
      skip: true,
      message: "SKIP Pull-request execution contract: no event or body file was supplied.",
    };
  }
  const event = JSON.parse(await readFile(parsed.eventPath, "utf8"));
  if (!event.pull_request) {
    return {
      skip: true,
      message: "SKIP Pull-request execution contract: the current event is not a pull request.",
    };
  }
  return {
    body: event.pull_request.body ?? "",
    pullRequest: event.pull_request,
    skip: false,
  };
}

function extractContract(body) {
  const start = body.indexOf(START_MARKER);
  const end = body.indexOf(END_MARKER, start + START_MARKER.length);
  if (start < 0 || end < 0 || end <= start) {
    throw new Error(`Pull-request body must contain one ${START_MARKER} / ${END_MARKER} state ledger.`);
  }
  if (body.indexOf(START_MARKER, start + START_MARKER.length) >= 0) {
    throw new Error("Pull-request body contains more than one agent-state ledger.");
  }
  const segment = body.slice(start + START_MARKER.length, end);
  const match = segment.match(/```json\s*([\s\S]*?)```/u);
  if (!match) throw new Error("Agent-state ledger must contain one fenced JSON object.");
  let parsed;
  try {
    parsed = JSON.parse(match[1]);
  } catch (error) {
    throw new Error(`Agent-state ledger is not valid JSON: ${error instanceof Error ? error.message : String(error)}`);
  }
  if (!isRecord(parsed)) throw new Error("Agent-state ledger must be a JSON object.");
  return parsed;
}

function validateContract(contract, actual, forceStrict, errors, warnings) {
  requireInteger(contract, "schemaVersion", errors, 1);
  requireString(contract, "baseSha", errors);
  requireString(contract, "headSha", errors);
  requireString(contract, "branch", errors);
  requireEnum(contract, "executionMode", EXECUTION_MODES, errors);
  requireEnum(contract, "currentState", CURRENT_STATES, errors);

  const strict = forceStrict || actual?.draft === false;
  validateIdentity("baseSha", contract.baseSha, actual?.baseSha, strict, errors, warnings);
  validateIdentity("headSha", contract.headSha, actual?.headSha, strict, errors, warnings);
  validateBranch(contract.branch, actual?.branch, strict, errors, warnings);

  if (!isRecord(contract.acceptance) || Object.keys(contract.acceptance).length === 0) {
    errors.push("acceptance must be a non-empty object keyed by stable AC-## identifiers.");
  } else {
    for (const [identifier, entry] of Object.entries(contract.acceptance)) {
      if (!/^AC-\d{2,}$/u.test(identifier)) {
        errors.push(`acceptance key ${identifier} must match AC-##.`);
      }
      if (!isRecord(entry)) {
        errors.push(`${identifier} must be an object.`);
        continue;
      }
      if (!ACCEPTANCE_STATUSES.has(entry.status)) {
        errors.push(`${identifier}.status must be one of ${[...ACCEPTANCE_STATUSES].join(", ")}.`);
      }
      validateStringArray(entry.evidence, `${identifier}.evidence`, errors);
      validateStringArray(entry.tests, `${identifier}.tests`, errors);
      if (
        TERMINAL_ACCEPTANCE_STATUSES.has(entry.status)
        && (!Array.isArray(entry.evidence) || entry.evidence.length === 0)
      ) {
        errors.push(`${identifier} is terminal but has no implementation or disposition evidence.`);
      }
    }
  }

  if (!isRecord(contract.validation)) {
    errors.push("validation must be an object.");
  } else {
    for (const required of ["static", "fast", "full", "android"]) {
      if (!(required in contract.validation)) errors.push(`validation.${required} is required.`);
    }
    for (const [name, entry] of Object.entries(contract.validation)) {
      validateEvidenceEntry(entry, `validation.${name}`, contract.headSha, errors);
    }
  }

  if (!isRecord(contract.manualAcceptance) || Object.keys(contract.manualAcceptance).length === 0) {
    errors.push("manualAcceptance must contain at least one named workflow or an explicit notRequired entry.");
  } else {
    for (const [name, entry] of Object.entries(contract.manualAcceptance)) {
      validateEvidenceEntry(entry, `manualAcceptance.${name}`, contract.headSha, errors, false);
    }
  }

  if (!Array.isArray(contract.knownFailures)) {
    errors.push("knownFailures must be an array.");
  } else {
    for (const [index, failure] of contract.knownFailures.entries()) {
      validateFailure(failure, index, errors);
    }
  }

  const hasFailedOrBlockedValidation = isRecord(contract.validation)
    && Object.values(contract.validation).some(
      (entry) => isRecord(entry) && (entry.status === "failed" || entry.status === "blocked"),
    );
  if (hasFailedOrBlockedValidation && contract.knownFailures?.length === 0) {
    errors.push("failed or blocked validation requires a corresponding knownFailures record.");
  }

  if (strict) validateReadyState(contract, actual, errors);
}

function validateIdentity(label, recorded, actual, strict, errors, warnings) {
  if (!SHA_PATTERN.test(recorded) && !(PLACEHOLDERS.has(recorded) && !strict)) {
    errors.push(`${label} must be a 40-character lowercase commit SHA${strict ? "" : " or a draft placeholder"}.`);
    return;
  }
  if (!actual || PLACEHOLDERS.has(recorded)) return;
  if (recorded !== actual) {
    const message = `${label} ${recorded} does not match GitHub ${actual}.`;
    if (strict) errors.push(message);
    else warnings.push(`${message} Draft ledgers may lag a synchronize event, but must be reconciled before readiness.`);
  }
}

function validateBranch(recorded, actual, strict, errors, warnings) {
  if (typeof recorded !== "string" || recorded.length === 0) {
    errors.push("branch must be a non-empty string.");
    return;
  }
  if (!actual || recorded === actual) return;
  const message = `branch ${recorded} does not match GitHub ${actual}.`;
  if (strict) errors.push(message);
  else warnings.push(`${message} Draft ledgers must be reconciled before readiness.`);
}

function validateEvidenceEntry(entry, label, recordedHead, errors, requireSha = true) {
  if (!isRecord(entry)) {
    errors.push(`${label} must be an object.`);
    return;
  }
  if (!EVIDENCE_STATUSES.has(entry.status)) {
    errors.push(`${label}.status must be one of ${[...EVIDENCE_STATUSES].join(", ")}.`);
    return;
  }
  if (entry.evidence !== undefined) validateStringArray(entry.evidence, `${label}.evidence`, errors);
  if (entry.status === "passed") {
    if (requireSha && !SHA_PATTERN.test(entry.sha ?? "")) {
      errors.push(`${label}.sha must record the exact passed commit.`);
    }
    if (requireSha && SHA_PATTERN.test(recordedHead ?? "") && entry.sha !== recordedHead) {
      errors.push(`${label}.sha must match the ledger headSha.`);
    }
    if (!Array.isArray(entry.evidence) || entry.evidence.length === 0) {
      errors.push(`${label} passed without a command, run, or artifact reference.`);
    }
  }
  if (["failed", "blocked", "notRun", "humanRequired"].includes(entry.status)) {
    if (!Array.isArray(entry.evidence) || entry.evidence.length === 0) {
      errors.push(`${label}.${entry.status} requires explanatory evidence.`);
    }
  }
}

function validateFailure(failure, index, errors) {
  const label = `knownFailures[${index}]`;
  if (!isRecord(failure)) {
    errors.push(`${label} must be an object.`);
    return;
  }
  for (const key of ["fingerprint", "boundary", "reproduction"]) {
    if (typeof failure[key] !== "string" || failure[key].length === 0) {
      errors.push(`${label}.${key} must be a non-empty string.`);
    }
  }
  if (!FAILURE_CLASSIFICATIONS.has(failure.classification)) {
    errors.push(`${label}.classification must be one of ${[...FAILURE_CLASSIFICATIONS].join(", ")}.`);
  }
  if (!Number.isInteger(failure.attempts) || failure.attempts < 1) {
    errors.push(`${label}.attempts must be an integer greater than zero.`);
  }
}

function validateReadyState(contract, actual, errors) {
  if (contract.currentState !== "readyForReview") {
    errors.push("A non-draft or strict pull request must record currentState readyForReview.");
  }
  for (const [identifier, entry] of Object.entries(contract.acceptance ?? {})) {
    if (!TERMINAL_ACCEPTANCE_STATUSES.has(entry.status)) {
      errors.push(`${identifier} is not terminal for review readiness.`);
    }
  }
  for (const gate of REQUIRED_READY_GATES) {
    if (contract.validation?.[gate]?.status !== "passed") {
      errors.push(`validation.${gate} must pass on the exact current head before review readiness.`);
    }
  }
  for (const [name, entry] of Object.entries(contract.validation ?? {})) {
    if (!TERMINAL_EVIDENCE_STATUSES.has(entry.status)) {
      errors.push(`validation.${name} is not terminal for review readiness.`);
    }
    if (entry.status === "passed" && actual?.headSha && entry.sha !== actual.headSha) {
      errors.push(`validation.${name} passed a commit other than the current GitHub head.`);
    }
  }
  for (const [name, entry] of Object.entries(contract.manualAcceptance ?? {})) {
    if (!TERMINAL_EVIDENCE_STATUSES.has(entry.status)) {
      errors.push(`manualAcceptance.${name} is not terminal for review readiness.`);
    }
  }
  if (contract.knownFailures?.length > 0) {
    errors.push("knownFailures must be empty for review readiness.");
  }
}

function requireString(object, key, errors) {
  if (typeof object[key] !== "string" || object[key].length === 0) {
    errors.push(`${key} must be a non-empty string.`);
  }
}

function requireInteger(object, key, errors, expected) {
  if (!Number.isInteger(object[key]) || object[key] !== expected) {
    errors.push(`${key} must equal ${expected}.`);
  }
}

function requireEnum(object, key, values, errors) {
  if (!values.has(object[key])) {
    errors.push(`${key} must be one of ${[...values].join(", ")}.`);
  }
}

function validateStringArray(value, label, errors) {
  if (!Array.isArray(value) || value.some((entry) => typeof entry !== "string" || entry.length === 0)) {
    errors.push(`${label} must be an array of non-empty strings.`);
  }
}

function isRecord(value) {
  return value !== null && typeof value === "object" && !Array.isArray(value);
}
