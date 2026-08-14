#!/usr/bin/env node

import { mkdir, readFile, writeFile } from "node:fs/promises";
import { dirname, resolve } from "node:path";

const args = process.argv.slice(2);
let resultsPath = "";
let outputDir = "";
let profile = "unknown";

for (let index = 0; index < args.length; index += 1) {
  const arg = args[index];
  if (arg === "--results") resultsPath = args[++index] ?? "";
  else if (arg === "--output-dir") outputDir = args[++index] ?? "";
  else if (arg === "--profile") profile = args[++index] ?? "unknown";
  else throw new Error(`Unknown argument: ${arg}`);
}

if (!resultsPath || !outputDir) {
  throw new Error("Usage: summarize-diagnostics.mjs --results PATH --output-dir PATH [--profile NAME]");
}

const absoluteResults = resolve(resultsPath);
const absoluteOutput = resolve(outputDir);
await mkdir(absoluteOutput, { recursive: true });

const raw = await readFile(absoluteResults, "utf8");
const lines = raw.split(/\r?\n/u).filter(Boolean);
const [headerLine, ...rows] = lines;
const expectedHeader = "name\tprofile\tstatus\texit_code\tdependencies\tlog";
if (headerLine !== expectedHeader) {
  throw new Error(`Unexpected diagnostic results header: ${headerLine ?? "<missing>"}`);
}

const gates = [];
for (const row of rows) {
  const [name, gateProfile, status, exitCode, dependencies, logPath] = row.split("\t");
  if (!name || !gateProfile || !status || !logPath) {
    throw new Error(`Malformed diagnostic result row: ${row}`);
  }

  let logText = "";
  try {
    logText = await readFile(resolve(dirname(absoluteResults), logPath), "utf8");
  } catch (error) {
    if (error?.code !== "ENOENT") throw error;
  }

  gates.push({
    name,
    profile: gateProfile,
    status,
    exitCode: exitCode === "" ? null : Number(exitCode),
    dependencies: dependencies ? dependencies.split(/\s+/u).filter(Boolean) : [],
    log: logPath,
    highlights: extractHighlights(logText),
  });
}

const counts = Object.fromEntries(
  ["PASS", "FAIL", "BLOCKED"].map((status) => [
    status.toLowerCase(),
    gates.filter((gate) => gate.status === status).length,
  ]),
);

const payload = {
  schemaVersion: 1,
  generatedAt: new Date().toISOString(),
  profile,
  commit: process.env.GITHUB_SHA ?? null,
  pullRequest: process.env.GITHUB_EVENT_NAME === "pull_request"
    ? Number(process.env.OSTUDY_DIAGNOSTIC_PR ?? 0) || null
    : null,
  runner: process.env.RUNNER_NAME ?? null,
  counts,
  gates,
};

await writeFile(
  resolve(absoluteOutput, "diagnostics.json"),
  `${JSON.stringify(payload, null, 2)}\n`,
  "utf8",
);

const summary = [];
summary.push("# ostudy agent diagnostic sweep", "");
summary.push(`- Profile: \`${profile}\``);
summary.push(`- Commit: \`${payload.commit ?? "unknown"}\``);
summary.push(`- Runner: \`${payload.runner ?? "unknown"}\``);
summary.push(`- Results: **${counts.pass} passed**, **${counts.fail} failed**, **${counts.blocked} blocked**`, "");
summary.push("| Gate | Profile | Result | Exit | Dependencies |");
summary.push("| --- | --- | --- | ---: | --- |");
for (const gate of gates) {
  const dependencies = gate.dependencies.length > 0 ? gate.dependencies.map((item) => `\`${item}\``).join(", ") : "—";
  summary.push(`| \`${gate.name}\` | ${gate.profile} | **${gate.status}** | ${gate.exitCode ?? "—"} | ${dependencies} |`);
}

const failures = gates.filter((gate) => gate.status === "FAIL");
if (failures.length > 0) {
  summary.push("", "## Failure highlights");
  for (const gate of failures) {
    summary.push("", `### ${gate.name}`, "");
    if (gate.highlights.length === 0) {
      summary.push(`No concise error pattern was detected. Inspect \`${gate.log}\`.`);
      continue;
    }
    summary.push("```text", ...gate.highlights, "```");
  }
}

const blocked = gates.filter((gate) => gate.status === "BLOCKED");
if (blocked.length > 0) {
  summary.push("", "## Blocked gates", "");
  for (const gate of blocked) {
    summary.push(`- \`${gate.name}\` waited on ${gate.dependencies.map((item) => `\`${item}\``).join(", ")}.`);
  }
}

summary.push(
  "",
  "This sweep is diagnostic only. A merge-ready head must still pass the repository's authoritative qualification workflows.",
  "",
);
await writeFile(resolve(absoluteOutput, "summary.md"), summary.join("\n"), "utf8");

function extractHighlights(text) {
  const clean = text.replace(/\u001b\[[0-?]*[ -/]*[@-~]/gu, "");
  const source = clean.split(/\r?\n/u);
  const selected = new Set();

  for (let index = 0; index < source.length; index += 1) {
    const line = source[index];
    const trigger =
      /\berror TS\d+:/u.test(line) ||
      /^FAIL\s{2}/u.test(line) ||
      /TestingLibraryElementError|AssertionError/u.test(line) ||
      /^error(?:\[[^\]]+\])?:/u.test(line) ||
      /panicked at /u.test(line) ||
      /\.\.\. FAILED$/u.test(line) ||
      /test result: FAILED/u.test(line) ||
      /^ERROR:/u.test(line) ||
      /^FAILURE:/u.test(line) ||
      /^Diff in /u.test(line) ||
      /BUILD FAILED/u.test(line) ||
      /Execution failed for task/u.test(line) ||
      /Compilation error/u.test(line) ||
      /Exception|Traceback \(most recent call last\)/u.test(line);

    if (!trigger) continue;
    for (let cursor = Math.max(0, index - 2); cursor <= Math.min(source.length - 1, index + 6); cursor += 1) {
      selected.add(cursor);
    }
  }

  if (selected.size === 0) {
    return source.filter(Boolean).slice(-25);
  }

  return [...selected]
    .sort((left, right) => left - right)
    .map((index) => source[index].trimEnd())
    .filter((line, index, values) => line !== "" || values[index - 1] !== "")
    .slice(-100);
}
