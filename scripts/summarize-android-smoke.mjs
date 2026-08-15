#!/usr/bin/env node

import { readdir, readFile, writeFile } from "node:fs/promises";
import { basename, resolve } from "node:path";

const options = new Map();
for (let index = 2; index < process.argv.length; index += 2) {
  const key = process.argv[index];
  const value = process.argv[index + 1];
  if (!key?.startsWith("--") || value === undefined) {
    throw new Error("Android smoke summarizer arguments must be --key value pairs.");
  }
  options.set(key.slice(2), value);
}

const resultsPath = options.get("results");
const artifactDir = options.get("artifact-dir");
const outputPath = options.get("output");
if (!resultsPath || !artifactDir || !outputPath) {
  throw new Error(
    "Usage: summarize-android-smoke.mjs --results PATH --artifact-dir DIR --output PATH --exit-code N [--serial S --api-level N --page-size N]",
  );
}

const raw = await readFile(resolve(resultsPath), "utf8");
const [header, ...rows] = raw.split(/\r?\n/u).filter(Boolean);
if (header !== "case\tstatus\tprerequisite\tduration_ms\tlog") {
  throw new Error(`Unexpected Android smoke result header: ${header ?? "<missing>"}`);
}

const cases = rows.map((row) => {
  const [name, status, prerequisite, durationMs, log] = row.split("\t");
  if (!name || !status) throw new Error(`Malformed Android smoke result: ${row}`);
  return {
    name,
    status,
    prerequisite: prerequisite || null,
    durationMs: durationMs === "" ? null : Number(durationMs),
    log: log || null,
  };
});

const artifacts = (await readdir(resolve(artifactDir), { withFileTypes: true }))
  .filter((entry) => entry.isFile() && entry.name !== basename(outputPath))
  .map((entry) => entry.name)
  .sort();

const payload = {
  schemaVersion: 1,
  generatedAt: new Date().toISOString(),
  commit: process.env.GITHUB_SHA ?? null,
  exitCode: Number(options.get("exit-code") ?? 0),
  serial: options.get("serial") || null,
  apiLevel: Number(options.get("api-level") ?? 0) || null,
  pageSize: Number(options.get("page-size") ?? 0) || null,
  counts: {
    pass: cases.filter((entry) => entry.status === "PASS").length,
    fail: cases.filter((entry) => entry.status === "FAIL").length,
    skip: cases.filter((entry) => entry.status === "SKIP").length,
  },
  cases,
  artifacts,
};

await writeFile(resolve(outputPath), `${JSON.stringify(payload, null, 2)}\n`, "utf8");
console.log(
  `Android smoke results: ${payload.counts.pass} passed, ${payload.counts.fail} failed, ${payload.counts.skip} skipped.`,
);
