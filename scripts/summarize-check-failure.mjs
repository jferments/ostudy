#!/usr/bin/env node

import { readdir, readFile } from "node:fs/promises";
import { basename, join } from "node:path";

const ANSI_ESCAPE = /\u001b\[[0-?]*[ -/]*[@-~]/gu;
const TEST_FILE_PATTERN = /(?:^|\s)([A-Za-z0-9_./-]+\.(?:test|spec)\.[cm]?[jt]sx?)(?=\s|$)/gu;
const VITEST_FAILED_FILE_PATTERN = /^\s*FAIL\s+([A-Za-z0-9_./-]+\.(?:test|spec)\.[cm]?[jt]sx?)(?=\s|$)/gmu;
const RUST_FAILED_TEST_PATTERN = /^test (.+) \.\.\. FAILED$/gmu;
const diagnosticRoots = [];
const files = [];

for (let index = 2; index < process.argv.length; index += 1) {
  const argument = process.argv[index];
  if (argument === "--diagnostics") {
    const root = process.argv[index + 1];
    if (!root) throw new Error("--diagnostics requires a directory");
    diagnosticRoots.push(root);
    index += 1;
  } else {
    files.push(argument);
  }
}

const logs = [];
for (const path of files) {
  if (!path) continue;
  try {
    logs.push({ path, text: (await readFile(path, "utf8")).replace(ANSI_ESCAPE, "") });
  } catch (error) {
    if (error?.code !== "ENOENT") throw error;
  }
}

const diagnosticFiles = [];
for (const root of diagnosticRoots) {
  await collectDiagnosticFiles(root, diagnosticFiles);
}

const contexts = [];
const events = [];
for (const path of diagnosticFiles.slice(0, 256)) {
  let text;
  try {
    text = await readFile(path, "utf8");
  } catch (error) {
    if (error?.code === "ENOENT") continue;
    throw error;
  }
  if (basename(path) === "context.txt") {
    contexts.push(parseContext(text));
    continue;
  }
  if (!path.endsWith(".jsonl") && !path.endsWith("logcat.txt")) continue;
  for (const line of text.split("\n")) {
    if (line.length === 0 || line.length > 65_536) continue;
    try {
      const event = parseDiagnosticLine(line);
      if (
        event?.application === "ostudy" &&
        typeof event.target === "string" &&
        event.target.startsWith("ostudy") &&
        typeof event.event === "string" &&
        typeof event.message === "string"
      ) {
        events.push({ ...event, source: path });
      }
    } catch {
      // Wrapper and Logcat artifacts can be colocated with JSONL files. A
      // malformed line is ignored instead of hiding the primary failure.
    }
  }
}

function parseDiagnosticLine(line) {
  if (line.startsWith("{")) return JSON.parse(line);
  const start = line.indexOf("{\"schemaVersion\"");
  const end = line.lastIndexOf("}");
  if (start < 0 || end <= start) throw new Error("No structured event");
  return JSON.parse(line.slice(start, end + 1));
}

const allText = logs.map(({ text }) => text).join("\n");
const failure = findFailure(logs, contexts);
const reproduction = focusedReproduction(failure.step, allText, contexts);

console.log("## ostudy qualification failure");
console.log();
console.log(`- Commit: \`${process.env.QUALIFIED_SHA ?? process.env.GITHUB_SHA ?? "unknown"}\``);
console.log(`- Runner: \`${process.env.RUNNER_NAME ?? "unknown"}\``);
if (failure.line) console.log(`- ${failure.line}`);
if (contexts.some((context) => context.kind === "transient")) {
  console.log("- A bounded diagnostic record also captured a recovered transient failure.");
}

console.log();
console.log("### Fastest local reproduction");
console.log();
console.log("Run the smallest failed boundary first; do not rerun the complete Linux and Android matrix unless that focused command passes:");
console.log();
console.log("```bash");
console.log(`OSTUDY_FORCE_CHECK=1 \\
OSTUDY_KEEP_FAILURE_ARTIFACTS=1 \\
OSTUDY_LOG=${shellQuote(reproduction.filter)} \\
${reproduction.command}`);
console.log("```");

renderStructuredDiagnostics(events);
renderLogHighlights(logs);

async function collectDiagnosticFiles(root, output, depth = 0) {
  if (depth > 6 || output.length >= 256) return;
  let entries;
  try {
    entries = await readdir(root, { withFileTypes: true });
  } catch (error) {
    if (error?.code === "ENOENT" || error?.code === "ENOTDIR") return;
    throw error;
  }
  for (const entry of entries) {
    if (output.length >= 256) return;
    const path = join(root, entry.name);
    if (entry.isDirectory()) {
      await collectDiagnosticFiles(path, output, depth + 1);
    } else if (
      entry.isFile() &&
      (entry.name === "context.txt" || entry.name.endsWith(".jsonl") || entry.name.endsWith("logcat.txt"))
    ) {
      output.push(path);
    }
  }
}

function parseContext(text) {
  return Object.fromEntries(
    text
      .split("\n")
      .map((line) => line.split(/=(.*)/su))
      .filter(([key, value]) => key && value !== undefined),
  );
}

function preferredContext(diagnosticContexts, step = null) {
  const reversed = [...diagnosticContexts].reverse();
  return (
    reversed.find((context) => context.kind === "failure" && step && context.step === step) ??
    reversed.find(
      (context) => context.kind === "failure" && context.step && context.step !== "initialization",
    ) ??
    reversed.find((context) => context.kind === "failure") ??
    reversed.find((context) => context.kind === "transient") ??
    reversed[0]
  );
}

function findFailure(logEntries, diagnosticContexts) {
  const lines = logEntries.flatMap(({ text }) => text.split("\n"));
  const failures = lines
    .filter((line) => /(?:STATIC|FAST|FULL) CHECK FAILED during:|Android smoke case failed:/u.test(line))
    .map((line) => {
      const trimmed = line.trim();
      const scriptedStep = trimmed.match(/CHECK FAILED during: (.+?) \(exit/u)?.[1];
      const androidCase = trimmed.match(/Android smoke case failed: ([^ ]+)/u)?.[1];
      return {
        line: trimmed,
        step: scriptedStep ?? (androidCase ? `Android smoke ${androidCase}` : null),
      };
    });
  const failure =
    [...failures].reverse().find((entry) => entry.step && entry.step !== "initialization") ??
    failures.at(-1);
  const context = preferredContext(diagnosticContexts, failure?.step ?? null);
  return {
    line: failure?.line ?? "",
    step: failure?.step ?? context?.step ?? "qualification",
  };
}

function focusedReproduction(step, text, diagnosticContexts) {
  const context = preferredContext(diagnosticContexts, step);
  const lower = step.toLowerCase();
  const filter = context?.replayFilter ?? recommendedFilter(lower);

  if (lower.includes("react tests")) {
    const failedTestFiles = uniqueMatches(text, VITEST_FAILED_FILE_PATTERN, 1).slice(0, 8);
    const testFiles =
      failedTestFiles.length > 0
        ? failedTestFiles
        : uniqueMatches(text, TEST_FILE_PATTERN, 1).slice(0, 8);
    return {
      filter,
      command:
        testFiles.length > 0
          ? `pnpm vitest run ${testFiles.map(shellQuote).join(" ")} --reporter=verbose`
          : "pnpm test",
    };
  }

  if (lower.includes("rust unit") || lower.includes("rust tests")) {
    const testNames = uniqueMatches(text, RUST_FAILED_TEST_PATTERN, 1);
    if (testNames.length > 0) {
      return {
        filter,
        command: `cargo test --manifest-path src-tauri/Cargo.toml --locked --all-features ${shellQuote(testNames[0])} -- --exact --nocapture`,
      };
    }
  }

  const mappings = [
    [/rust compile/u, "cargo check --manifest-path src-tauri/Cargo.toml --locked --all-targets --all-features"],
    [/rust warnings|rust lint/u, "cargo clippy --manifest-path src-tauri/Cargo.toml --locked --all-targets --all-features -- -D warnings"],
    [/external file/u, "cargo test --manifest-path src-tauri/Cargo.toml --locked --all-features --lib external_file_ -- --nocapture"],
    [/document domain/u, "cargo test --manifest-path src-tauri/Cargo.toml --locked --all-features --lib document_ -- --nocapture"],
    [/pdf extraction/u, "cargo test --manifest-path src-tauri/Cargo.toml --locked --all-features --lib pdf_ -- --nocapture"],
    [/latest-schema/u, "cargo test --manifest-path src-tauri/Cargo.toml --locked --all-features --lib database::tests::public_baseline_is_idempotent_and_matches_manifest -- --exact --nocapture"],
    [/migration immutability/u, "cargo test --manifest-path src-tauri/Cargo.toml --locked --all-features --lib database::tests::public_migration_checksum_and_future_version_are_enforced -- --exact --nocapture"],
    [/frontend production build/u, "pnpm run build"],
    [/standalone tauri/u, "pnpm tauri build --debug --no-bundle --ci -- --locked"],
    [/webdriver/u, "./scripts/smoke-native.sh"],
    [/android.*smoke/u, "./scripts/android-emulator-smoke.sh"],
    [/android/u, "./scripts/android-test.sh"],
    [/environment and repository|behavior-test policy|frozen javascript|frontend type|rust formatting/u, "./scripts/check-static.sh"],
  ];
  for (const [pattern, command] of mappings) {
    if (pattern.test(lower)) return { filter, command };
  }
  return { filter, command: context?.replayCommand ?? "./scripts/check.sh" };
}

function recommendedFilter(lowerStep) {
  if (/database|migration|schema|sqlite/u.test(lowerStep)) return "warn,ostudy.database=debug";
  if (/openrouter|provider|document|pdf|ocr|\bai\b/u.test(lowerStep)) {
    return "warn,ostudy.ai=debug,ostudy.documents=debug";
  }
  if (/qr|scanner/u.test(lowerStep)) {
    return "warn,ostudy.app=info,ostudy.qr=debug,ostudy.device_sync=debug";
  }
  if (/android/u.test(lowerStep)) {
    return "warn,ostudy.app=debug,ostudy.qr=debug,ostudy.filesystem=debug";
  }
  if (/webdriver|frontend|react|tauri/u.test(lowerStep)) {
    return "warn,ostudy.app=debug,ostudy.frontend=debug";
  }
  if (/external file|filesystem|asset|import|export/u.test(lowerStep)) {
    return "warn,ostudy.filesystem=debug";
  }
  return "warn,ostudy=debug";
}

function uniqueMatches(text, pattern, group) {
  const values = [];
  const seen = new Set();
  for (const match of text.matchAll(pattern)) {
    const value = match[group];
    if (!value || seen.has(value)) continue;
    seen.add(value);
    values.push(value);
  }
  return values;
}

function shellQuote(value) {
  return `'${String(value).replaceAll("'", `'"'"'`)}'`;
}

function latestApplicationSession(events) {
  const sessions = new Map();
  for (let index = 0; index < events.length; index += 1) {
    const event = events[index];
    if (typeof event.sessionId !== "string" || event.sessionId.length === 0) continue;
    const timestamp = Number(event.timestampMs ?? 0);
    const existing = sessions.get(event.sessionId);
    if (!existing) {
      sessions.set(event.sessionId, {
        id: event.sessionId,
        firstTimestamp: timestamp,
        lastTimestamp: timestamp,
        lastIndex: index,
      });
      continue;
    }
    existing.firstTimestamp = Math.min(existing.firstTimestamp, timestamp);
    if (timestamp > existing.lastTimestamp || (timestamp === existing.lastTimestamp && index > existing.lastIndex)) {
      existing.lastTimestamp = timestamp;
      existing.lastIndex = index;
    }
  }

  if (sessions.size === 0) return events;

  const latest = [...sessions.values()].sort(
    (left, right) => left.lastTimestamp - right.lastTimestamp || left.lastIndex - right.lastIndex,
  ).at(-1);
  if (!latest) return events;

  return events.filter((event) => {
    if (event.sessionId === latest.id) return true;
    if (typeof event.sessionId === "string" && event.sessionId.length > 0) return false;
    return Number(event.timestampMs ?? 0) >= latest.firstTimestamp;
  });
}

function renderStructuredDiagnostics(rawEvents) {
  if (rawEvents.length === 0) return;
  const deduplicated = new Map();
  for (const event of rawEvents) {
    const key = [event.sessionId, event.timestampMs, event.level, event.target, event.event, event.message].join("|");
    deduplicated.set(key, event);
  }
  const ordered = [...deduplicated.values()].sort(
    (left, right) => Number(left.timestampMs ?? 0) - Number(right.timestampMs ?? 0),
  );
  const scoped = latestApplicationSession(ordered);
  const notable = scoped.filter((event) => event.level === "error" || event.level === "warn");
  const selected = (notable.length > 0 ? notable : scoped).slice(-20);
  if (selected.length === 0) return;

  console.log();
  console.log("### Structured application diagnostics");
  console.log();
  console.log(
    notable.length > 0
      ? `Parsed ${ordered.length} bounded event(s); showing the last ${selected.length} warning/error event(s) from the latest application session.`
      : `Parsed ${ordered.length} bounded event(s); no warning/error record was emitted in the latest application session, so the last ${selected.length} timeline event(s) are shown.`,
  );
  console.log();
  for (const event of selected) {
    const fields = event.fields && Object.keys(event.fields).length > 0
      ? ` — \`${markdownInline(JSON.stringify(event.fields).slice(0, 500))}\``
      : "";
    console.log(
      `- **${markdownInline(event.level ?? "info")}** \`${markdownInline(event.target)}\` / \`${markdownInline(event.event)}\`: ${markdownInline(event.message)}${fields}`,
    );
  }
}

function markdownInline(value) {
  return String(value).replaceAll("`", "\\`").replaceAll("\n", " ").replaceAll("\r", " ");
}

function renderLogHighlights(logEntries) {
  const highlights = [];
  for (const { path, text } of logEntries) {
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
        /Android smoke case failed|Native smoke test failed|Traceback \(most recent call last\)/u.test(line);

      if (!trigger) continue;
      const before = /^error(?:\[[^\]]+\])?:/u.test(line) ? 1 : 0;
      const after = /^Diff in /u.test(line) ? 10 : 5;
      for (
        let cursor = Math.max(0, index - before);
        cursor <= Math.min(lines.length - 1, index + after);
        cursor += 1
      ) {
        selected.add(cursor);
      }
    }

    const excerpt = [...selected]
      .sort((left, right) => left - right)
      .map((index) => lines[index].trimEnd())
      .filter((line, index, values) => line !== "" || values[index - 1] !== "")
      .slice(-120);

    if (excerpt.length > 0) highlights.push({ path, excerpt });
  }

  if (highlights.length === 0) {
    console.log();
    console.log("No concise wrapper-log pattern was detected. Inspect the uploaded bounded evidence.");
    return;
  }

  for (const { path, excerpt } of highlights) {
    console.log();
    console.log(`### ${basename(path)}`);
    console.log();
    console.log("```text");
    console.log(excerpt.join("\n"));
    console.log("```");
  }
}
