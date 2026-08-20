#!/usr/bin/env node

import { execFileSync } from "node:child_process";
import { mkdir, readFile, writeFile } from "node:fs/promises";
import { dirname, resolve } from "node:path";

const SHA_PATTERN = /^[0-9a-f]{40}$/u;
const TERMINAL_ACCEPTANCE = new Set(["implemented", "humanRequired", "notApplicable"]);
const COMMAND_BY_STEP = new Map([
  ["pr-contract", "node ./scripts/verify-pr-contract.mjs --event \"$GITHUB_EVENT_PATH\""],
  ["static", "./scripts/check-static.sh"],
  ["preflight", "./scripts/runner/guest-preflight.sh"],
  ["dependency-graph", "cargo metadata --manifest-path src-tauri/Cargo.toml --locked"],
  ["fast", "./scripts/check-fast.sh"],
  ["linux-full", "./scripts/check.sh"],
  ["android-compile", "./scripts/android-test.sh"],
  ["android-smoke", "./scripts/android-emulator-smoke.sh"],
]);
const options = parseArguments(process.argv.slice(2));
const qualification = await readJson(options.qualification);
const contractArtifact = options.contract ? await readJson(options.contract) : null;
const actual = isRecord(contractArtifact?.actual) ? contractArtifact.actual : null;
const contract = isRecord(contractArtifact?.contract) ? contractArtifact.contract : null;
const integrityErrors = [];
const integrityWarnings = [];

validateQualification(qualification, integrityErrors);
validateContractArtifact(contractArtifact, actual, contract, integrityErrors, integrityWarnings);
validateIdentityAgreement(qualification, actual, contract, integrityErrors, integrityWarnings);
const changedFiles = readChangedFiles(
  options.repositoryRoot,
  actual?.baseSha ?? contract?.baseSha,
  qualification.headSha,
  integrityWarnings,
);

const lines = [];
lines.push("# ostudy evidence-derived handoff", "");
renderPublication(lines, qualification, actual, contract);
renderAcceptance(lines, contract);
renderChangedFiles(lines, changedFiles);
renderNarrative(lines, "Non-goals", contract?.nonGoals);
renderValidation(lines, contract);
renderQualification(lines, qualification);
renderCommands(lines, qualification);
renderManualAcceptance(lines, contract);
renderNarrative(lines, "Compatibility notes", contract?.compatibilityNotes);
renderNarrative(lines, "Rollback notes", contract?.rollbackNotes);
renderNarrative(lines, "Residual risks", contract?.residualRisks);
renderKnownFailures(lines, contract, qualification);
renderMerge(lines, contract);
renderIntegrity(lines, contractArtifact, integrityErrors, integrityWarnings);

await mkdir(dirname(options.output), { recursive: true });
await writeFile(options.output, `${lines.join("\n")}\n`, "utf8");
console.log(`Wrote evidence-derived handoff to ${options.output}`);
if (integrityErrors.length > 0) process.exitCode = 2;

function parseArguments(argumentsList) {
  const parsed = {
    contract: null,
    qualification: null,
    output: null,
    repositoryRoot: process.cwd(),
  };
  for (let index = 0; index < argumentsList.length; index += 1) {
    const argument = argumentsList[index];
    if (argument === "--contract") {
      parsed.contract = requireValue(argumentsList, ++index, argument);
    } else if (argument === "--qualification") {
      parsed.qualification = requireValue(argumentsList, ++index, argument);
    } else if (argument === "--output") {
      parsed.output = requireValue(argumentsList, ++index, argument);
    } else if (argument === "--repository-root") {
      parsed.repositoryRoot = resolve(requireValue(argumentsList, ++index, argument));
    } else {
      throw new Error(`Unknown argument: ${argument}`);
    }
  }
  if (!parsed.qualification) throw new Error("--qualification is required");
  if (!parsed.output) throw new Error("--output is required");
  return parsed;
}

function requireValue(argumentsList, index, option) {
  const value = argumentsList[index];
  if (!value) throw new Error(`${option} requires a value`);
  return value;
}

async function readJson(path) {
  const value = JSON.parse(await readFile(path, "utf8"));
  if (!isRecord(value)) throw new Error(`${path} must contain a JSON object`);
  return value;
}

function validateQualification(value, errors) {
  if (value.schemaVersion !== 1) errors.push("Qualification evidence has an unsupported schema version.");
  if (!isSha(value.headSha)) errors.push("Qualification evidence does not identify a valid exact head SHA.");
  if (typeof value.status !== "string" || value.status.length === 0) {
    errors.push("Qualification evidence does not identify a status.");
  }
  if (!Array.isArray(value.steps)) errors.push("Qualification evidence does not contain step outcomes.");
}

function validateContractArtifact(artifact, actualValue, contractValue, errors, warnings) {
  if (!artifact) {
    warnings.push("No pull-request contract artifact was supplied; publication and acceptance state are limited to qualification evidence.");
    return;
  }
  if (artifact.schemaVersion !== 1) errors.push("Pull-request contract artifact has an unsupported schema version.");
  if (!actualValue) warnings.push("Pull-request contract artifact has no GitHub event identity.");
  if (!contractValue) errors.push("Pull-request contract artifact has no parsed state ledger.");
  if (artifact.valid === false) errors.push("Pull-request contract verification reported an invalid ledger.");
  for (const warning of Array.isArray(artifact.warnings) ? artifact.warnings : []) {
    warnings.push(sanitize(warning));
  }
}

function validateIdentityAgreement(qualificationValue, actualValue, contractValue, errors, warnings) {
  const qualificationHead = qualificationValue.headSha;
  const actualHead = actualValue?.headSha;
  const ledgerHead = contractValue?.headSha;

  if (isSha(actualHead) && isSha(qualificationHead) && actualHead !== qualificationHead) {
    errors.push(`Qualification head ${qualificationHead} does not match GitHub pull-request head ${actualHead}.`);
  }
  if (!isSha(actualHead) && isSha(ledgerHead) && isSha(qualificationHead) && ledgerHead !== qualificationHead) {
    errors.push(`Qualification head ${qualificationHead} does not match ledger head ${ledgerHead}.`);
  }
  if (isSha(actualHead) && isSha(ledgerHead) && actualHead !== ledgerHead) {
    warnings.push(`Draft ledger head ${ledgerHead} differs from GitHub pull-request head ${actualHead}.`);
  }
}

function readChangedFiles(repositoryRoot, baseSha, headSha, warnings) {
  if (!isSha(baseSha) || !isSha(headSha)) {
    warnings.push("Changed-file evidence was unavailable because the base or head SHA was not exact.");
    return [];
  }
  try {
    const text = execFileSync("git", ["diff", "--name-status", `${baseSha}...${headSha}`], {
      cwd: repositoryRoot,
      encoding: "utf8",
      stdio: ["ignore", "pipe", "pipe"],
    });
    return text
      .split(/\r?\n/u)
      .filter(Boolean)
      .map((line) => {
        const fields = line.split("\t");
        return {
          status: fields[0] ?? "?",
          paths: fields.slice(1),
          group: changedFileGroup(fields.at(-1) ?? ""),
        };
      });
  } catch (error) {
    warnings.push(`Changed-file evidence could not be derived from Git: ${error instanceof Error ? error.message : String(error)}`);
    return [];
  }
}

function changedFileGroup(path) {
  if (path.startsWith(".github/workflows/")) return "CI workflows";
  if (path.startsWith("tests/") || /(?:^|\/)(?:test|spec)[^/]*\.[^/]+$/u.test(path)) return "Tests";
  if (path.startsWith("docs/") || path.endsWith(".md")) return "Documentation";
  if (path.startsWith("scripts/")) return "Development scripts";
  if (path.startsWith("src-tauri/")) return "Native and Rust";
  if (path.startsWith("src/")) return "Frontend";
  return "Repository configuration";
}

function renderPublication(output, qualificationValue, actualValue, contractValue) {
  output.push("## Publication", "");
  if (actualValue?.number) output.push(`- Pull request: #${actualValue.number}`);
  if (actualValue) output.push(`- Pull-request state: ${actualValue.draft ? "draft" : "ready/non-draft"}`);
  output.push(`- Base SHA: ${plain(actualValue?.baseSha ?? contractValue?.baseSha ?? "unknown")}`);
  output.push(`- Branch: ${plain(actualValue?.branch ?? contractValue?.branch ?? "unknown")}`);
  output.push(`- Published head: ${plain(actualValue?.headSha ?? contractValue?.headSha ?? qualificationValue.headSha ?? "unknown")}`);
  if (contractValue?.executionMode) output.push(`- Execution mode: ${plain(contractValue.executionMode)}`);
  if (contractValue?.currentState) output.push(`- Recorded current state: ${plain(contractValue.currentState)}`);
  output.push("");
}

function renderAcceptance(output, contractValue) {
  output.push("## Acceptance criteria", "");
  const entries = isRecord(contractValue?.acceptance) ? Object.entries(contractValue.acceptance) : [];
  if (entries.length === 0) {
    output.push("- No acceptance ledger was available.", "");
    return;
  }
  const terminal = entries.filter(([, entry]) => TERMINAL_ACCEPTANCE.has(entry?.status)).length;
  output.push(`- Terminal criteria: ${terminal}/${entries.length}`);
  for (const [identifier, entry] of entries) {
    const evidence = formatList(entry?.evidence);
    const tests = formatList(entry?.tests);
    output.push(`- ${plain(identifier)} — ${plain(entry?.status ?? "unknown")}`);
    if (evidence) output.push(`  - Evidence: ${evidence}`);
    if (tests) output.push(`  - Tests: ${tests}`);
  }
  output.push("");
}

function renderChangedFiles(output, changedFiles) {
  output.push("## Changed files", "");
  if (changedFiles.length === 0) {
    output.push("- No changed-file evidence was available.", "");
    return;
  }
  const groups = new Map();
  for (const entry of changedFiles) {
    const display = `${entry.status} ${entry.paths.map((path) => plain(path)).join(" → ")}`;
    const values = groups.get(entry.group) ?? [];
    values.push(display);
    groups.set(entry.group, values);
  }
  for (const [group, entries] of [...groups.entries()].sort(([left], [right]) => left.localeCompare(right))) {
    output.push(`- ${plain(group)}:`);
    for (const entry of entries) output.push(`  - ${entry}`);
  }
  output.push("");
}

function renderNarrative(output, title, values) {
  output.push(`## ${title}`, "");
  if (!Array.isArray(values) || values.length === 0) {
    output.push("- No machine-readable entry was recorded.", "");
    return;
  }
  for (const value of values) output.push(`- ${plain(value)}`);
  output.push("");
}

function renderValidation(output, contractValue) {
  output.push("## Validation ledger", "");
  const entries = isRecord(contractValue?.validation) ? Object.entries(contractValue.validation) : [];
  if (entries.length === 0) {
    output.push("- No validation ledger was available.", "");
    return;
  }
  for (const [name, entry] of entries) {
    let line = `- ${plain(name)}: ${plain(entry?.status ?? "unknown")}`;
    if (entry?.sha) line += ` on ${plain(entry.sha)}`;
    output.push(line);
    const evidence = formatList(entry?.evidence);
    if (evidence) output.push(`  - Evidence: ${evidence}`);
  }
  output.push("");
}

function renderQualification(output, qualificationValue) {
  output.push("## Exact-head qualification", "");
  output.push(`- Head: ${plain(qualificationValue.headSha ?? "unknown")}`);
  output.push(`- Gate: ${plain(qualificationValue.gate ?? "unknown")}`);
  output.push(`- Status: ${plain(qualificationValue.status ?? "unknown")}`);
  output.push(`- Runner: ${plain(qualificationValue.runner ?? "unknown")}`);
  if (qualificationValue.workflow?.name) output.push(`- Workflow: ${plain(qualificationValue.workflow.name)}`);
  if (qualificationValue.workflow?.runId) output.push(`- Run ID: ${plain(qualificationValue.workflow.runId)}`);
  output.push("- Steps:");
  for (const step of Array.isArray(qualificationValue.steps) ? qualificationValue.steps : []) {
    output.push(`  - ${plain(step?.name ?? "unknown")}: ${plain(step?.status ?? "unknown")}`);
  }
  output.push("");
}

function renderCommands(output, qualificationValue) {
  output.push("## Commands represented by this qualification", "");
  const steps = Array.isArray(qualificationValue.steps) ? qualificationValue.steps : [];
  const linuxFullSucceeded = steps.some((step) => step?.name === "linux-full" && step?.status === "success");
  const executed = [];
  const notRun = [];
  for (const step of steps) {
    const command = COMMAND_BY_STEP.get(step?.name);
    if (!command) continue;
    if (step?.name === "fast" && step?.status === "skipped" && linuxFullSucceeded) continue;
    const rendered = `${command} — ${step?.status ?? "unknown"}`;
    if (step?.status === "skipped" || step?.status === "pending") notRun.push(rendered);
    else executed.push(rendered);
  }
  if (linuxFullSucceeded) executed.push("./scripts/check-fast.sh — included by successful ./scripts/check.sh");
  output.push("- Executed or attempted:");
  if (executed.length === 0) output.push("  - None recorded.");
  for (const entry of executed) output.push(`  - ${plain(entry)}`);
  output.push("- Not run in this qualification:");
  if (notRun.length === 0) output.push("  - None recorded.");
  for (const entry of notRun) output.push(`  - ${plain(entry)}`);
  output.push("");
}

function renderManualAcceptance(output, contractValue) {
  output.push("## Manual acceptance", "");
  const entries = isRecord(contractValue?.manualAcceptance)
    ? Object.entries(contractValue.manualAcceptance)
    : [];
  if (entries.length === 0) {
    output.push("- No manual-acceptance ledger was available.", "");
    return;
  }
  for (const [name, entry] of entries) {
    output.push(`- ${plain(name)}: ${plain(entry?.status ?? "unknown")}`);
    const evidence = formatList(entry?.evidence);
    if (evidence) output.push(`  - Evidence: ${evidence}`);
  }
  output.push("");
}

function renderKnownFailures(output, contractValue, qualificationValue) {
  output.push("## Known failures", "");
  const failures = Array.isArray(contractValue?.knownFailures) ? contractValue.knownFailures : [];
  if (failures.length === 0) output.push("- Pull-request ledger: none recorded.");
  for (const failure of failures) {
    output.push(`- ${plain(failure?.fingerprint ?? "unfingerprinted")} — ${plain(failure?.classification ?? "unknown")}`);
    if (failure?.boundary) output.push(`  - Boundary: ${plain(failure.boundary)}`);
    if (failure?.reproduction) output.push(`  - Reproduction: ${plain(failure.reproduction)}`);
    if (failure?.attempts) output.push(`  - Attempts: ${plain(failure.attempts)}`);
  }
  if (qualificationValue.failure) {
    output.push(`- Qualification failure: ${plain(qualificationValue.failure.fingerprint ?? "unfingerprinted")}`);
    output.push(`  - Boundary: ${plain(qualificationValue.failure.boundary ?? "unknown")}`);
    output.push(`  - Classification hint: ${plain(qualificationValue.failure.classificationHint ?? "unclassified")}`);
    output.push(`  - Focused reproduction: ${plain(qualificationValue.failure.reproduction ?? "unavailable")}`);
  } else {
    output.push("- Qualification artifact: no failure recorded.");
  }
  output.push("");
}

function renderMerge(output, contractValue) {
  output.push("## Merge", "");
  output.push(`- Requested method: ${plain(contractValue?.mergeMethod ?? "not recorded")}`, "");
}

function renderIntegrity(output, contractArtifactValue, errors, warnings) {
  output.push("## Evidence integrity", "");
  output.push(`- Contract artifact: ${contractArtifactValue ? "present" : "not supplied"}`);
  output.push(`- Integrity status: ${errors.length === 0 ? "valid" : "invalid"}`);
  if (warnings.length > 0) {
    output.push("- Warnings:");
    for (const warning of warnings) output.push(`  - ${plain(warning)}`);
  }
  if (errors.length > 0) {
    output.push("- Errors:");
    for (const error of errors) output.push(`  - ${plain(error)}`);
  }
  output.push("");
}

function formatList(value) {
  if (!Array.isArray(value) || value.length === 0) return "";
  return value.map((entry) => plain(entry)).join("; ");
}

function plain(value) {
  return sanitize(value).replaceAll(/([\\`*_{}\[\]()#+.!|<>])/gu, "\\$1");
}

function sanitize(value) {
  return String(value ?? "unknown").replaceAll(/\s+/gu, " ").trim();
}

function isSha(value) {
  return typeof value === "string" && SHA_PATTERN.test(value);
}

function isRecord(value) {
  return value !== null && typeof value === "object" && !Array.isArray(value);
}
