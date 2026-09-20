#!/usr/bin/env node

// Prepare metadata only. Publication still belongs to the authenticated connector.
import { spawnSync } from "node:child_process";
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const START = "<!-- ostudy-agent-state:start -->";
const END = "<!-- ostudy-agent-state:end -->";
const SHA = /^[0-9a-f]{40}$/u;
const UNSAFE_KEYS = new Set(["__proto__", "prototype", "constructor"]);

const isRecord = (value) => value !== null && typeof value === "object" && !Array.isArray(value);

function mergePatch(current, patch) {
  const merged = { ...current };
  for (const [key, value] of Object.entries(patch)) {
    if (UNSAFE_KEYS.has(key)) throw new Error(`Unsupported patch key: ${key}`);
    // Objects merge; arrays are deliberately replaced, never guessed or defaulted.
    merged[key] = isRecord(value)
      ? mergePatch(isRecord(current[key]) ? current[key] : {}, value)
      : structuredClone(value);
  }
  return merged;
}

/** Merge a progress patch into a fresh GitHub PR event and preflight the whole body. */
export function preparePrContractUpdate(event, patch) {
  const pr = event?.pull_request;
  if (!isRecord(pr) || typeof pr.body !== "string" || typeof pr.draft !== "boolean"
    || !SHA.test(pr.base?.sha ?? "") || !SHA.test(pr.head?.sha ?? "")
    || typeof pr.head?.ref !== "string" || pr.head.ref.length === 0) {
    throw new Error("A fresh pull_request snapshot with body, draft, base.sha, head.sha and head.ref is required.");
  }
  if (!isRecord(patch)) throw new Error("The progress patch must be a JSON object.");
  const start = pr.body.indexOf(START);
  const end = pr.body.indexOf(END);
  if (start < 0 || end < start
    || pr.body.indexOf(START, start + START.length) !== -1
    || pr.body.indexOf(END, end + END.length) !== -1) {
    throw new Error("The source body must contain exactly one complete agent-state ledger.");
  }
  const segment = pr.body.slice(start + START.length, end);
  const match = segment.match(/^\s*```json\s*([\s\S]*?)```\s*$/u);
  if (!match) throw new Error("The source ledger must contain exactly one fenced JSON object.");
  const previous = JSON.parse(match[1]);
  if (!isRecord(previous)) throw new Error("The source ledger must be an object.");
  const contract = mergePatch(previous, patch);
  // Draft CI tolerates synchronization lag; an intentional update must not publish it.
  if (contract.baseSha !== pr.base.sha || contract.headSha !== pr.head.sha
    || contract.branch !== pr.head.ref) {
    throw new Error("The proposed ledger identity does not match the fresh GitHub snapshot.");
  }
  const body = `${pr.body.slice(0, start + START.length)}\n\`\`\`json\n${JSON.stringify(contract, null, 2)}\n\`\`\`\n${pr.body.slice(end)}`;
  const directory = mkdtempSync(join(tmpdir(), "ostudy-pr-contract-"));
  try {
    const eventPath = join(directory, "event.json");
    const reportPath = join(directory, "report.json");
    writeFileSync(eventPath, JSON.stringify({ pull_request: { ...pr, body } }), { mode: 0o600 });
    const result = spawnSync(process.execPath, [
      fileURLToPath(new URL("./verify-pr-contract.mjs", import.meta.url)),
      "--event", eventPath, "--output", reportPath,
    ], { encoding: "utf8", timeout: 30_000, maxBuffer: 1024 * 1024 });
    if (result.error) throw result.error;
    if (result.status !== 0) {
      throw new Error(`PR body preflight failed:\n${result.stderr.trim()}`);
    }
    const report = JSON.parse(readFileSync(reportPath, "utf8"));
    if (report.valid !== true || !isRecord(report.contract) || report.actual?.headSha !== pr.head.sha) {
      throw new Error("The contract verifier did not validate this PR snapshot.");
    }
    return { body, contract };
  } finally {
    rmSync(directory, { recursive: true, force: true });
  }
}

function main(args) {
  const options = new Map();
  for (let index = 0; index < args.length; index += 2) {
    const key = args[index];
    if (!["--event", "--patch", "--output"].includes(key) || options.has(key)
      || !args[index + 1] || args[index + 1].startsWith("--")) {
      throw new Error("Usage: node scripts/prepare-pr-contract-update.mjs --event <fresh-pr.json> --patch <progress.json> --output <new-body.md>");
    }
    options.set(key, args[index + 1]);
  }
  if (options.size !== 3) throw new Error("--event, --patch and --output are required.");
  const event = JSON.parse(readFileSync(options.get("--event"), "utf8"));
  const patch = JSON.parse(readFileSync(options.get("--patch"), "utf8"));
  const { body } = preparePrContractUpdate(event, patch);
  // Exclusive creation prevents an unsuccessful run being mistaken for a refreshed file.
  writeFileSync(options.get("--output"), body, { encoding: "utf8", flag: "wx", mode: 0o600 });
  console.log("OK   Complete PR body prepared and validated; no GitHub mutation performed.");
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  try {
    main(process.argv.slice(2));
  } catch (error) {
    console.error(error instanceof Error ? error.message : String(error));
    process.exitCode = 1;
  }
}
