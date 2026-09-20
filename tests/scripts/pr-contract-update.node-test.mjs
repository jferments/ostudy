import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { existsSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import test from "node:test";

import { preparePrContractUpdate } from "../../scripts/prepare-pr-contract-update.mjs";

const HEAD = "a".repeat(40);
const BASE = "b".repeat(40);
const NARRATIVES = ["nonGoals", "compatibilityNotes", "rollbackNotes", "residualRisks"];

function fixture(edit = () => {}) {
  const contract = {
    schemaVersion: 1, baseSha: BASE, headSha: HEAD, branch: "agent/fixture",
    executionMode: "githubConnector", currentState: "implementation", mergeMethod: "squash",
    ...Object.fromEntries(NARRATIVES.map((key) => [key, [`Preserve ${key}.`]])),
    acceptance: {
      "AC-01": { status: "implemented", evidence: ["Published implementation"], tests: ["Existing test"] },
      "AC-02": { status: "notStarted", evidence: [], tests: [] },
    },
    validation: {
      static: { status: "passed", sha: HEAD, evidence: ["Exact-head static run"] },
      fast: { status: "pending" }, full: { status: "pending" }, android: { status: "pending" },
    },
    manualAcceptance: { realLibrary: { status: "humanRequired", evidence: ["Local adoption is separate."] } },
    knownFailures: [],
  };
  edit(contract);
  return { pull_request: {
    number: 103, draft: true, base: { sha: BASE }, head: { sha: HEAD, ref: "agent/fixture" },
    body: `Keep this introduction.\n<!-- ostudy-agent-state:start -->\n\`\`\`json\n${JSON.stringify(contract)}\n\`\`\`\n<!-- ostudy-agent-state:end -->\nKeep this handoff.\n`,
  } };
}

test("a partial progress update preserves narratives, sibling acceptance and exact evidence", () => {
  const event = fixture();
  const original = structuredClone(event);
  const { body, contract } = preparePrContractUpdate(event, { acceptance: { "AC-02": { status: "inProgress" } } });
  for (const key of NARRATIVES) assert.deepEqual(contract[key], [`Preserve ${key}.`]);
  assert.equal(contract.acceptance["AC-01"].status, "implemented");
  assert.deepEqual(contract.acceptance["AC-01"].tests, ["Existing test"]);
  assert.equal(contract.acceptance["AC-02"].status, "inProgress");
  assert.deepEqual(contract.acceptance["AC-02"].evidence, []);
  assert.equal(contract.validation.static.sha, HEAD);
  assert.equal(contract.manualAcceptance.realLibrary.status, "humanRequired");
  assert.ok(body.startsWith("Keep this introduction.\n"));
  assert.ok(body.endsWith("Keep this handoff.\n"));
  assert.deepEqual(event, original, "preparation must not mutate the source snapshot");
});

for (const key of NARRATIVES) {
  test(`missing ${key} is rejected rather than silently filled`, () => {
    assert.throws(() => preparePrContractUpdate(fixture((contract) => { delete contract[key]; }), {}),
      new RegExp(`${key} must be an array`, "u"));
    assert.throws(() => preparePrContractUpdate(fixture(), { [key]: null }),
      new RegExp(`${key} must be an array`, "u"));
  });
}

test("head movement cannot relabel ancestor qualification as passed", () => {
  const event = fixture();
  event.pull_request.head.sha = "c".repeat(40);
  assert.throws(() => preparePrContractUpdate(event, {}), /identity does not match/u);
  assert.throws(() => preparePrContractUpdate(event, { headSha: event.pull_request.head.sha }), /sha must match the ledger headSha/u);
  const result = preparePrContractUpdate(event, {
    headSha: event.pull_request.head.sha,
    validation: { static: { status: "notRun", evidence: ["New source has not been qualified."] } },
  });
  assert.equal(result.contract.validation.static.status, "notRun");
});

test("malformed ledgers and destructive or unsafe patches fail closed", () => {
  assert.throws(() => preparePrContractUpdate({}, {}), /fresh pull_request snapshot/u);
  assert.throws(() => preparePrContractUpdate(fixture(), []), /JSON object/u);
  assert.throws(() => preparePrContractUpdate(fixture(), { acceptance: null }), /acceptance must be/u);
  assert.throws(() => preparePrContractUpdate(fixture(), JSON.parse('{"__proto__":{"polluted":true}}')), /Unsupported patch key/u);
  assert.equal({}.polluted, undefined);
  const event = fixture();
  event.pull_request.body += "<!-- ostudy-agent-state:end -->";
  assert.throws(() => preparePrContractUpdate(event, {}), /exactly one complete/u);
});

test("the CLI writes only validated bodies and never overwrites an existing output", () => {
  const root = mkdtempSync(join(tmpdir(), "ostudy-pr-update-test-"));
  try {
    const eventPath = join(root, "event.json");
    const patchPath = join(root, "patch.json");
    const outputPath = join(root, "body.md");
    writeFileSync(eventPath, JSON.stringify(fixture()));
    writeFileSync(patchPath, JSON.stringify({ residualRisks: null }));
    const args = [fileURLToPath(new URL("../../scripts/prepare-pr-contract-update.mjs", import.meta.url)),
      "--event", eventPath, "--patch", patchPath, "--output", outputPath];
    const invoke = () => spawnSync(process.execPath, args, { encoding: "utf8" });
    assert.equal(invoke().status, 1);
    assert.equal(existsSync(outputPath), false);
    writeFileSync(patchPath, "{}");
    assert.equal(invoke().status, 0);
    const bytes = readFileSync(outputPath);
    assert.equal(invoke().status, 1);
    assert.deepEqual(readFileSync(outputPath), bytes);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});
