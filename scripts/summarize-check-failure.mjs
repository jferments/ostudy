#!/usr/bin/env node

import { readFile } from "node:fs/promises";

const ANSI_ESCAPE = /\u001b\[[0-?]*[ -/]*[@-~]/gu;
const files = process.argv.slice(2);
const logs = [];

for (const path of files) {
  if (!path) continue;
  try {
    logs.push({ path, text: (await readFile(path, "utf8")).replace(ANSI_ESCAPE, "") });
  } catch (error) {
    if (error?.code !== "ENOENT") throw error;
  }
}

console.log("## ostudy qualification failure");
console.log();
console.log(`- Commit: \`${process.env.GITHUB_SHA ?? "unknown"}\``);
console.log(`- Runner: \`${process.env.RUNNER_NAME ?? "unknown"}\``);

const failureLines = logs
  .flatMap(({ text }) => text.split("\n"))
  .filter((line) => /(?:STATIC|FAST|FULL) CHECK FAILED during:/u.test(line));
if (failureLines.length > 0) {
  console.log(`- ${failureLines.at(-1).trim()}`);
}

const highlights = [];
for (const { path, text } of logs) {
  const lines = text.split("\n");
  const selected = new Set();

  for (let index = 0; index < lines.length; index += 1) {
    const line = lines[index];
    const trigger =
      /(?:STATIC|FAST|FULL) CHECK FAILED during:/u.test(line) ||
      /^Diff in /u.test(line) ||
      /\berror TS\d+:/u.test(line) ||
      /^FAIL\s{2}/u.test(line) ||
      /TestingLibraryElementError|AssertionError/u.test(line) ||
      /^error(?:\[[^\]]+\])?:/u.test(line) ||
      /panicked at /u.test(line) ||
      /---- .* stdout ----/u.test(line) ||
      /\.\.\. FAILED$/u.test(line) ||
      /test result: FAILED/u.test(line) ||
      /^ERROR:/u.test(line) ||
      /Native smoke test failed|Traceback \(most recent call last\)/u.test(line);

    if (!trigger) continue;
    const before = /^error(?:\[[^\]]+\])?:/u.test(line) ? 1 : 0;
    const after = /^Diff in /u.test(line) ? 10 : 5;
    for (let cursor = Math.max(0, index - before); cursor <= Math.min(lines.length - 1, index + after); cursor += 1) {
      selected.add(cursor);
    }
  }

  const excerpt = [...selected]
    .sort((left, right) => left - right)
    .map((index) => lines[index].trimEnd())
    .filter((line, index, values) => line !== "" || values[index - 1] !== "")
    .slice(-120);

  if (excerpt.length > 0) {
    highlights.push({ path, excerpt });
  }
}

if (highlights.length === 0) {
  console.log();
  console.log("No concise diagnostic pattern was detected. Inspect the uploaded full logs.");
  process.exit(0);
}

for (const { path, excerpt } of highlights) {
  console.log();
  console.log(`### ${path.split("/").at(-1)}`);
  console.log();
  console.log("```text");
  console.log(excerpt.join("\n"));
  console.log("```");
}
