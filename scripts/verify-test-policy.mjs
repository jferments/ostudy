#!/usr/bin/env node

import { readdir, readFile, stat } from "node:fs/promises";
import { extname, join, relative } from "node:path";
import { fileURLToPath } from "node:url";

const root = join(fileURLToPath(new URL("..", import.meta.url)));
const excludedDirectories = new Set([
  ".git",
  "dist",
  "node_modules",
  "target",
  "gen",
]);

const sourceFiles = await collectFiles(root);
const testFiles = sourceFiles.filter(isTestFile);
const failures = [];
let frontendTests = 0;
let rustTests = 0;
let pythonTests = 0;

for (const path of testFiles) {
  const text = await readFile(path, "utf8");
  const displayPath = relative(root, path);
  if (text.trim() === "") {
    failures.push(`${displayPath}: test file is empty`);
    continue;
  }

  const extension = extname(path);
  if ([".ts", ".tsx", ".js", ".jsx", ".mjs", ".cjs"].includes(extension)) {
    checkForbidden(
      text,
      displayPath,
      [
        {
          pattern: /\b(?:describe|it|test)\s*\.\s*(?:only|skip|todo|skipIf|runIf)\s*\(/gu,
          message: "focused, skipped, conditional, or placeholder JavaScript test",
        },
        {
          pattern: /\b(?:fdescribe|fit|xdescribe|xit|xtest)\s*\(/gu,
          message: "focused or disabled JavaScript test alias",
        },
        {
          pattern: /\.toBeInTheDocument\s*\(/gu,
          message:
            "unsupported jest-dom matcher toBeInTheDocument; use repository-native DOM assertions unless jest-dom is explicitly adopted",
        },
        {
          pattern: /\.toHaveAttribute\s*\(/gu,
          message:
            "unsupported jest-dom matcher toHaveAttribute; assert getAttribute() directly unless jest-dom is explicitly adopted",
        },
      ],
      failures,
    );
    frontendTests += countMatches(text, /\b(?:it|test)\s*\(/gu);
    continue;
  }

  if (extension === ".rs") {
    checkForbidden(
      text,
      displayPath,
      [
        {
          pattern: /#\s*\[\s*ignore(?:\s*=\s*[^\]]+)?\s*\]/gu,
          message: "ignored Rust test",
        },
      ],
      failures,
    );
    rustTests += countMatches(text, /#\s*\[\s*test\s*\]/gu);
    continue;
  }

  if (extension === ".py") {
    checkForbidden(
      text,
      displayPath,
      [
        {
          pattern: /@(?:unittest\.)?skip(?:If|Unless)?\s*\(/gu,
          message: "skipped Python test",
        },
        {
          pattern: /@pytest\.mark\.(?:skip|skipif)\b/gu,
          message: "skipped pytest test",
        },
      ],
      failures,
    );
    pythonTests += countMatches(text, /^\s*def\s+test_[A-Za-z0-9_]*\s*\(/gmu);
  }
}

if (frontendTests === 0) failures.push("No active frontend tests were found.");
if (rustTests === 0) failures.push("No active Rust tests were found.");
if (pythonTests === 0) {
  // The native WebDriver workflow is intentionally a script rather than a
  // unittest module, so its executable main path is the active smoke check.
  const smokePath = join(root, "tests", "webdriver", "smoke.py");
  const smoke = sourceFiles.includes(smokePath)
    ? await readFile(smokePath, "utf8")
    : "";
  if (!/def\s+main\s*\(/u.test(smoke) || !/raise\s+SystemExit\(main\(\)\)/u.test(smoke)) {
    failures.push("tests/webdriver/smoke.py has no executable smoke-test entry point.");
  }
}

if (failures.length > 0) {
  console.error("ERROR: test policy verification failed:\n");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log(
  `OK   Test policy: ${frontendTests} frontend tests, ${rustTests} Rust tests, ` +
    `${pythonTests > 0 ? `${pythonTests} Python tests` : "1 native Python smoke workflow"}; ` +
    "no focused, skipped, ignored, conditional, placeholder, or unconfigured jest-dom tests.",
);

async function collectFiles(directory) {
  const entries = await readdir(directory, { withFileTypes: true });
  const files = [];
  for (const entry of entries) {
    if (excludedDirectories.has(entry.name)) continue;
    const path = join(directory, entry.name);
    if (entry.isDirectory()) {
      files.push(...(await collectFiles(path)));
    } else if (entry.isFile() && (await stat(path)).size >= 0) {
      files.push(path);
    }
  }
  return files;
}

function isTestFile(path) {
  const displayPath = relative(root, path).replaceAll("\\", "/");
  if (/\.(?:test|spec)\.[cm]?[jt]sx?$/u.test(displayPath)) return true;
  if (displayPath.startsWith("src-tauri/") && displayPath.endsWith(".rs")) {
    return true;
  }
  return displayPath.startsWith("tests/") && displayPath.endsWith(".py");
}

function checkForbidden(text, path, rules, output) {
  for (const { pattern, message } of rules) {
    pattern.lastIndex = 0;
    for (const match of text.matchAll(pattern)) {
      output.push(`${path}:${lineNumber(text, match.index ?? 0)}: ${message}`);
    }
  }
}

function countMatches(text, pattern) {
  pattern.lastIndex = 0;
  return [...text.matchAll(pattern)].length;
}

function lineNumber(text, index) {
  return text.slice(0, index).split("\n").length;
}
