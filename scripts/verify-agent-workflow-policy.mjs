#!/usr/bin/env node

import { access, readdir, readFile } from "node:fs/promises";
import { basename, join, relative, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const defaultRoot = fileURLToPath(new URL("..", import.meta.url));
const root = resolve(parseRoot(process.argv.slice(2)) ?? defaultRoot);
const failures = [];
const workflowDirectory = join(root, ".github", "workflows");
const scriptsDirectory = join(root, "scripts");

if (!(await exists(workflowDirectory))) {
  failures.push(".github/workflows is missing.");
} else {
  for (const entry of await readdir(workflowDirectory, { withFileTypes: true })) {
    if (!entry.isFile() || !/\.ya?ml$/u.test(entry.name)) continue;
    const path = join(workflowDirectory, entry.name);
    const text = await readFile(path, "utf8");
    validateWorkflow(entry.name, text, failures);
  }
}

if (await exists(scriptsDirectory)) {
  for (const path of await collectFiles(scriptsDirectory)) {
    const name = basename(path);
    if (isTaskSpecificArtifact(name)) {
      failures.push(
        `${relative(root, path)}: task-specific applicator/finalizer artifacts are forbidden; publish the reviewed product source directly.`,
      );
    }
  }
}

if (failures.length > 0) {
  console.error("Agent workflow policy verification failed:");
  for (const failure of failures) console.error(`  - ${failure}`);
  process.exit(1);
}

console.log(
  "OK   Agent workflow policy: automatic PR qualification is read-only, source is published directly, and task-specific applicators are absent.",
);

function parseRoot(argumentsList) {
  let selected = null;
  for (let index = 0; index < argumentsList.length; index += 1) {
    const argument = argumentsList[index];
    if (argument !== "--root") throw new Error(`Unknown argument: ${argument}`);
    selected = argumentsList[index + 1];
    if (!selected) throw new Error("--root requires a directory");
    index += 1;
  }
  return selected;
}

function validateWorkflow(name, text, output) {
  if (isTaskSpecificArtifact(name)) {
    output.push(`${name}: task-specific mutation workflow is committed.`);
  }

  const pullRequestTriggered = /^\s*pull_request\s*:/mu.test(text);
  const pullRequestTargetTriggered = /^\s*pull_request_target\s*:/mu.test(text);
  if (pullRequestTargetTriggered) {
    output.push(`${name}: pull_request_target is forbidden for repository-code qualification.`);
  }
  if (!pullRequestTriggered && !pullRequestTargetTriggered) return;

  const prohibited = [
    [
      /^\s*contents\s*:\s*write\s*$/mu,
      "automatic pull-request workflows must use read-only repository contents permission",
    ],
    [
      /^\s*persist-credentials\s*:\s*true\s*$/mu,
      "automatic pull-request checkout may not persist write credentials",
    ],
    [
      /\bgit\s+push\b/iu,
      "automatic pull-request qualification may not push source or refs",
    ],
    [
      /\bgit\s+(?:commit|commit-tree|write-tree|update-index)\b/iu,
      "automatic pull-request qualification may not author commits or Git trees",
    ],
    [
      /\bgit\s+add\s+(?:-A|--all|\.)(?:\s|$)/imu,
      "automatic pull-request qualification may not stage a generated product tree",
    ],
    [
      /\bgh\s+pr\s+(?:ready|edit)\b|markPullRequestReadyForReview|convertPullRequestToDraft/iu,
      "automatic pull-request qualification may not mutate draft/readiness state",
    ],
    [
      /\b(?:apply|generate|rewrite|materialize)[-_ ](?:product|source|tree)\b/iu,
      "automatic pull-request qualification may not generate the reviewed product source",
    ],
  ];

  for (const [pattern, message] of prohibited) {
    if (pattern.test(text)) output.push(`${name}: ${message}.`);
  }
}

function isTaskSpecificArtifact(name) {
  return /^(?:agent[-_]|pr\d+[-_]|one[-_]?off[-_])|(?:applicator|finalizer)/iu.test(name);
}

async function collectFiles(directory) {
  const entries = await readdir(directory, { withFileTypes: true });
  const output = [];
  for (const entry of entries) {
    const path = join(directory, entry.name);
    if (entry.isDirectory()) output.push(...(await collectFiles(path)));
    else if (entry.isFile()) output.push(path);
  }
  return output;
}

async function exists(path) {
  try {
    await access(path);
    return true;
  } catch {
    return false;
  }
}
