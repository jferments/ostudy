import assert from "node:assert/strict";
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { spawnSync } from "node:child_process";
import { test } from "node:test";
import { rustfmtDiagnostics } from "../../scripts/report-rustfmt-diffs.mjs";

const failure = "\u001b[1;31mSTATIC CHECK FAILED during: Rust formatting (exit 1)\u001b[0m";
const hunk = (name, content = "-old\n+new") => `Diff in /source/${name}.rs:7:\n${content}\n`;

test("reports the first and last complete formatting hunks beyond the ordinary tail", () => {
  const input = "unrelated setup output\n" + hunk("first") + hunk("large", "-" + "x".repeat(140_000) + "\n+fixed") + hunk("last") + failure;
  const output = rustfmtDiagnostics(input);
  assert.match(output, /Complete hunks: 3\/3/);
  assert.match(output, /first\.rs:7/);
  assert.match(output, /last\.rs:7/);
  assert.doesNotMatch(output, /unrelated setup output|\x1b/);
});

test("does not emit unrelated failure logs or manufacture absent diagnostics", () => {
  assert.equal(rustfmtDiagnostics(hunk("source") + "different failure"), "");
  assert.match(rustfmtDiagnostics(failure), /no complete diff headers/);
  assert.doesNotMatch(rustfmtDiagnostics(hunk("source") + "private unrelated footer\n" + failure), /private unrelated footer/);
});

test("deduplicates repeated formatter hunk streams", () => {
  const repeated = hunk("same", "-before\n+after");
  const output = rustfmtDiagnostics(repeated + repeated + failure);
  assert.match(output, /Complete hunks: 1\/1/);
  assert.equal((output.match(/Diff in \/source\/same\.rs:7:/g) ?? []).length, 1);
});

test("bounds output by whole hunks and explicitly reports omissions", () => {
  const first = hunk("first");
  const output = rustfmtDiagnostics(first + hunk("second") + failure, Buffer.byteLength(first));
  assert.match(output, /Complete hunks: 1\/2/);
  assert.match(output, /INCOMPLETE/);
  assert.doesNotMatch(output, /second\.rs/);
});

test("captured command keeps its exact nonzero status and source bytes", () => {
  const root = mkdtempSync(join(tmpdir(), "ostudy-format-log-"));
  try {
    const source = join(root, "source.rs");
    const log = join(root, "check.log");
    writeFileSync(source, "fn main( ) {}\n");
    const before = readFileSync(source);
    const run = spawnSync("bash", ["scripts/run-ci-captured.sh", log, process.execPath, "-e", `console.log(${JSON.stringify(hunk("source") + failure)}); process.exit(7);`], { encoding: "utf8" });
    assert.equal(run.status, 7);
    const output = run.stdout + run.stderr;
    assert.match(output, /All captured Rust formatting hunks/);
    assert.match(output, /generic failure tail suppressed/);
    assert.doesNotMatch(output, /--- bounded failure tail ---/);
    assert.equal((output.match(/Diff in \/source\/source\.rs:7:/g) ?? []).length, 1);
    assert.deepEqual(readFileSync(source), before);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});
