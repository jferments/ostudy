#!/usr/bin/env node

import { createHash } from "node:crypto";
import { mkdir, readFile, readdir, writeFile } from "node:fs/promises";
import { basename, dirname, join } from "node:path";

const ANSI_ESCAPE = /\u001b\[[0-?]*[ -/]*[@-~]/gu;
const VITEST_FAILED_FILE = /^\s*FAIL\s+([A-Za-z0-9_./-]+\.(?:test|spec)\.[cm]?[jt]sx?)(?=\s|$)/gmu;
const RUST_FAILED_TEST = /^test (.+) \.\.\. FAILED$/gmu;
const PYTHON_FAILED_TEST = /^FAIL:\s+([^\s(]+)(?:\s+\([^)]*\))?/gmu;
const options = parseArguments(process.argv.slice(2));

const logs = await readLogs(options.logFiles);
const contexts = await readContexts(options.diagnosticRoots);
const allText = logs.map(({ text }) => text).join("\n");
const failure = options.status === "success" ? null : buildFailure(logs, contexts, allText);
const result = {
  schemaVersion: 1,
  generatedAt: new Date().toISOString(),
  headSha: process.env.QUALIFIED_SHA ?? process.env.GITHUB_SHA ?? "unknown",
  gate: options.gate,
  status: options.status,
  runner: process.env.RUNNER_NAME ?? "unknown",
  workflow: {
    name: process.env.GITHUB_WORKFLOW ?? "unknown",
    event: process.env.GITHUB_EVENT_NAME ?? "unknown",
    runId: process.env.GITHUB_RUN_ID ?? null,
    runAttempt: process.env.GITHUB_RUN_ATTEMPT ?? null,
  },
  steps: options.steps,
  failure,
};

await mkdir(dirname(options.output), { recursive: true });
await writeFile(options.output, `${JSON.stringify(result, null, 2)}\n`, "utf8");

console.log("### Machine-readable qualification evidence");
console.log();
console.log(`- Head: \`${result.headSha}\``);
console.log(`- Gate: \`${result.gate}\``);
console.log(`- Result: **${result.status}**`);
console.log(`- Record: \`${options.output}\``);
if (failure) {
  console.log(`- Failure fingerprint: \`${failure.fingerprint}\``);
  console.log(`- Boundary: \`${failure.boundary}\``);
  console.log(`- Classification hint: \`${failure.classificationHint}\``);
  console.log();
  console.log("Focused reproduction:");
  console.log();
  console.log("```bash");
  console.log(`OSTUDY_FORCE_CHECK=1 \\
OSTUDY_KEEP_FAILURE_ARTIFACTS=1 \\
OSTUDY_LOG=${shellQuote(failure.logFilter)} \\
${failure.reproduction}`);
  console.log("```");
}

function parseArguments(argumentsList) {
  const parsed = {
    output: null,
    status: "unknown",
    gate: "unknown",
    steps: [],
    diagnosticRoots: [],
    logFiles: [],
  };
  for (let index = 0; index < argumentsList.length; index += 1) {
    const argument = argumentsList[index];
    if (argument === "--output") {
      parsed.output = requireValue(argumentsList, ++index, argument);
    } else if (argument === "--status") {
      parsed.status = requireValue(argumentsList, ++index, argument);
    } else if (argument === "--gate") {
      parsed.gate = requireValue(argumentsList, ++index, argument);
    } else if (argument === "--step") {
      const value = requireValue(argumentsList, ++index, argument);
      const separator = value.indexOf("=");
      if (separator <= 0 || separator === value.length - 1) {
        throw new Error("--step requires name=status");
      }
      parsed.steps.push({ name: value.slice(0, separator), status: value.slice(separator + 1) });
    } else if (argument === "--diagnostics") {
      parsed.diagnosticRoots.push(requireValue(argumentsList, ++index, argument));
    } else if (argument.startsWith("--")) {
      throw new Error(`Unknown argument: ${argument}`);
    } else {
      parsed.logFiles.push(argument);
    }
  }
  if (!parsed.output) throw new Error("--output is required");
  return parsed;
}

function requireValue(argumentsList, index, option) {
  const value = argumentsList[index];
  if (!value) throw new Error(`${option} requires a value`);
  return value;
}

async function readLogs(paths) {
  const output = [];
  for (const path of paths) {
    try {
      output.push({ path, text: (await readFile(path, "utf8")).replace(ANSI_ESCAPE, "") });
    } catch (error) {
      if (error?.code !== "ENOENT") throw error;
    }
  }
  return output;
}

async function readContexts(roots) {
  const files = [];
  for (const root of roots) await collectContextFiles(root, files);
  const contexts = [];
  for (const path of files.slice(0, 256)) {
    try {
      const text = await readFile(path, "utf8");
      contexts.push(parseContext(text));
    } catch (error) {
      if (error?.code !== "ENOENT") throw error;
    }
  }
  return contexts;
}

async function collectContextFiles(root, output, depth = 0) {
  if (depth > 6 || output.length >= 256) return;
  let entries;
  try {
    entries = await readdir(root, { withFileTypes: true });
  } catch (error) {
    if (error?.code === "ENOENT" || error?.code === "ENOTDIR") return;
    throw error;
  }
  entries.sort((left, right) => left.name.localeCompare(right.name));
  for (const entry of entries) {
    if (output.length >= 256) return;
    const path = join(root, entry.name);
    if (entry.isDirectory()) await collectContextFiles(path, output, depth + 1);
    else if (entry.isFile() && entry.name === "context.txt") output.push(path);
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

function buildFailure(logs, contexts, allText) {
  const detected = findFailure(logs, contexts);
  const failedTests = [
    ...uniqueMatches(allText, VITEST_FAILED_FILE, 1),
    ...uniqueMatches(allText, RUST_FAILED_TEST, 1),
    ...uniqueMatches(allText, PYTHON_FAILED_TEST, 1),
  ].slice(0, 20);
  const reproduction = focusedReproduction(detected.boundary, allText, contexts);
  const signatureParts = [normalizeSignature(detected.boundary), ...failedTests.map(normalizeSignature)];
  if (signatureParts.length === 1) {
    const errorLine = firstErrorLine(allText);
    if (errorLine) signatureParts.push(normalizeSignature(errorLine));
  }
  const fingerprint = `qf-${createHash("sha256").update(signatureParts.join("|")).digest("hex").slice(0, 16)}`;
  return {
    fingerprint,
    boundary: detected.boundary,
    summary: detected.line || "Qualification failed without a recognized wrapper boundary.",
    classificationHint: classifyFailure(allText),
    failedTests,
    reproduction: reproduction.command,
    logFilter: reproduction.filter,
    transientEvidence: contexts.some((context) => context.kind === "transient"),
    logFiles: logs.map(({ path }) => basename(path)),
  };
}

function findFailure(logs, contexts) {
  const lines = logs.flatMap(({ text }) => text.split("\n"));
  const candidates = lines
    .filter((line) => /(?:STATIC|FAST|FULL) CHECK FAILED during:|Android smoke case failed:/u.test(line))
    .map((line) => {
      const trimmed = line.trim();
      const scripted = trimmed.match(/CHECK FAILED during: (.+?) \(exit/u)?.[1];
      const android = trimmed.match(/Android smoke case failed: ([^ ]+)/u)?.[1];
      return {
        line: trimmed,
        boundary: scripted ?? (android ? `Android smoke ${android}` : null),
      };
    });
  const selected = [...candidates].reverse().find((entry) => entry.boundary !== "initialization")
    ?? candidates.at(-1);
  const context = preferredContext(contexts, selected?.boundary ?? null);
  return {
    line: selected?.line ?? "",
    boundary: selected?.boundary ?? context?.step ?? firstFailedStep(options.steps) ?? "qualification",
  };
}

function preferredContext(contexts, boundary) {
  const reversed = [...contexts].reverse();
  return reversed.find((context) => context.kind === "failure" && boundary && context.step === boundary)
    ?? reversed.find((context) => context.kind === "failure" && context.step !== "initialization")
    ?? reversed.find((context) => context.kind === "failure")
    ?? reversed.find((context) => context.kind === "transient")
    ?? reversed[0];
}

function firstFailedStep(steps) {
  return steps.find((step) => step.status === "failure" || step.status === "cancelled")?.name ?? null;
}

function focusedReproduction(boundary, text, contexts) {
  const context = preferredContext(contexts, boundary);
  const lower = boundary.toLowerCase();
  const filter = context?.replayFilter ?? recommendedFilter(lower);
  const vitestFiles = uniqueMatches(text, VITEST_FAILED_FILE, 1).slice(0, 8);
  if (lower.includes("react tests") || vitestFiles.length > 0) {
    return {
      filter,
      command: vitestFiles.length > 0
        ? `pnpm exec vitest run ${vitestFiles.map(shellQuote).join(" ")} --reporter=verbose`
        : "pnpm test",
    };
  }
  const rustTests = uniqueMatches(text, RUST_FAILED_TEST, 1);
  if (rustTests.length > 0) {
    return {
      filter,
      command: `cargo test --manifest-path src-tauri/Cargo.toml --locked --all-features ${shellQuote(rustTests[0])} -- --exact --nocapture`,
    };
  }
  const mappings = [
    [/pr-contract|pull-request execution contract/u, "node ./scripts/verify-pr-contract.mjs --event \"$GITHUB_EVENT_PATH\" --strict"],
    [/rust compile/u, "cargo check --manifest-path src-tauri/Cargo.toml --locked --all-targets --all-features"],
    [/rust warnings|rust lint/u, "cargo clippy --manifest-path src-tauri/Cargo.toml --locked --all-targets --all-features -- -D warnings"],
    [/external file/u, "cargo test --manifest-path src-tauri/Cargo.toml --locked --all-features --lib external_file_ -- --nocapture"],
    [/document domain/u, "cargo test --manifest-path src-tauri/Cargo.toml --locked --all-features --lib document_ -- --nocapture"],
    [/pdf extraction/u, "cargo test --manifest-path src-tauri/Cargo.toml --locked --all-features --lib pdf_ -- --nocapture"],
    [/frontend production build/u, "pnpm run build"],
    [/standalone tauri/u, "pnpm tauri build --debug --no-bundle --ci -- --locked"],
    [/webdriver/u, "./scripts/smoke-native.sh"],
    [/android.*smoke/u, "./scripts/android-emulator-smoke.sh"],
    [/android/u, "./scripts/android-test.sh"],
    [/static|environment and repository|frontend type|rust formatting/u, "./scripts/check-static.sh"],
  ];
  for (const [pattern, command] of mappings) {
    if (pattern.test(lower)) return { filter, command };
  }
  return { filter, command: context?.replayCommand ?? "./scripts/check.sh" };
}

function recommendedFilter(lower) {
  if (/database|migration|schema|sqlite/u.test(lower)) return "warn,ostudy.database=debug";
  if (/openrouter|provider|document|pdf|ocr|\bai\b/u.test(lower)) {
    return "warn,ostudy.ai=debug,ostudy.documents=debug";
  }
  if (/qr|scanner|device sync/u.test(lower)) {
    return "warn,ostudy.app=info,ostudy.qr=debug,ostudy.device_sync=debug";
  }
  if (/android/u.test(lower)) return "warn,ostudy.app=debug,ostudy.qr=debug,ostudy.filesystem=debug";
  if (/webdriver|frontend|react|tauri/u.test(lower)) return "warn,ostudy.app=debug,ostudy.frontend=debug";
  if (/external file|filesystem|asset|import|export/u.test(lower)) return "warn,ostudy.filesystem=debug";
  return "warn,ostudy=debug";
}

function classifyFailure(text) {
  if (/Resource not accessible by integration|GitHub.*(?:502|503|504)|github\.com.*timed? out/iu.test(text)) {
    return "githubControlPlane";
  }
  if (/No space left on device|KVM|emulator.*(?:boot|offline)|rustup.*(?:download|network)|connection reset.*(?:rust-lang|googleapis)/iu.test(text)) {
    return "runnerEnvironment";
  }
  if (/error TS\d+:|AssertionError|TestingLibraryElementError|\.\.\. FAILED$|panicked at |clippy|rustfmt|verification failed/imu.test(text)) {
    return "productOrTest";
  }
  return "unclassified";
}

function firstErrorLine(text) {
  return text
    .split("\n")
    .map((line) => line.trim())
    .find((line) => /error|failed|panic|exception/iu.test(line) && line.length > 0)
    ?.slice(0, 500) ?? "";
}

function normalizeSignature(value) {
  return String(value)
    .toLowerCase()
    .replaceAll(/[0-9a-f]{40}/gu, "<sha>")
    .replaceAll(/\b\d+\b/gu, "<n>")
    .replaceAll(/\s+/gu, " ")
    .trim();
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
