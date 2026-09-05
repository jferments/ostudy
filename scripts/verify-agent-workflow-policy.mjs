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
  "OK   Agent workflow policy: Actions do not author product source or mutate pull-request metadata, product source is published directly with an expected parent, and task-specific applicators are absent.",
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

function validateWorkflow(name, rawText, output) {
  if (isTaskSpecificArtifact(name)) {
    output.push(`${name}: task-specific mutation workflow is committed.`);
  }

  const text = stripYamlComments(rawText);
  const pullRequestTriggered = /^\s*pull_request\s*:/mu.test(text);
  const pullRequestTargetTriggered = /^\s*pull_request_target\s*:/mu.test(text);

  if (pullRequestTargetTriggered) {
    output.push(`${name}: pull_request_target is forbidden for repository-code qualification.`);
  }
  if (hasContentsWritePermissions(text)) {
    output.push(`${name}: workflows may not request contents write permission; Actions are not source writers.`);
  }
  if (hasPullRequestWritePermissions(text)) {
    output.push(
      `${name}: workflows may not request pull-requests write permission; pull-request metadata is maintained explicitly by the interactive publisher, not an automatic projector.`,
    );
  }
  if (pullRequestTriggered && hasAnyWritePermissions(text)) {
    output.push(`${name}: automatic pull-request workflows may not request write permissions.`);
  }

  const prohibitedForEveryWorkflow = [
    [
      /^\s*persist-credentials\s*:\s*true\s*$/mu,
      "workflow checkout may not persist source-write credentials",
    ],
    [
      /\bgit\s+push\b/iu,
      "workflows may not push product source or refs",
    ],
    [
      /\bgit\s+(?:commit|commit-tree|write-tree|update-index)\b/iu,
      "workflows may not author commits or Git trees",
    ],
    [
      /\bgit\s+add\b/iu,
      "workflows may not stage a generated product tree",
    ],
    [
      /\bgh\s+api\b[^\n]*(?:--method|-X)\s+(?:POST|PUT|PATCH|DELETE)\b[^\n]*(?:\/contents\b|\/git\/(?:refs|blobs|trees|commits)\b)/iu,
      "workflows may not write repository contents or Git data through the GitHub API",
    ],
    [
      /(?:stefanzweifel\/git-auto-commit-action|peter-evans\/create-pull-request|EndBug\/add-and-commit)@/iu,
      "source-authoring and auto-commit actions are forbidden",
    ],
    [
      /(?:^|[\/._-])(?:agent|pr\d+|one[-_]?off)[-_](?:apply|applicator|finalize|finalizer|materialize|migrate|patch|qualif)/imu,
      "task-specific source applicators may not be invoked from workflows",
    ],
  ];

  for (const [pattern, message] of prohibitedForEveryWorkflow) {
    if (pattern.test(text)) output.push(`${name}: ${message}.`);
  }

  if (hasWriteModeFormatter(text)) {
    output.push(
      `${name}: workflows may not rewrite maintained source; use check-only formatter modes.`,
    );
  }

  if (
    pullRequestTriggered &&
    /\bgh\s+pr\s+(?:ready|edit)\b|markPullRequestReadyForReview|convertPullRequestToDraft/iu.test(text)
  ) {
    output.push(`${name}: automatic pull-request qualification may not mutate draft/readiness state.`);
  }
}

function stripYamlComments(text) {
  return text
    .split(/\r?\n/u)
    .map((line) => {
      let singleQuoted = false;
      let doubleQuoted = false;
      let escaped = false;
      for (let index = 0; index < line.length; index += 1) {
        const character = line[index];
        if (escaped) {
          escaped = false;
          continue;
        }
        if (character === "\\" && doubleQuoted) {
          escaped = true;
          continue;
        }
        if (character === "'" && !doubleQuoted) singleQuoted = !singleQuoted;
        else if (character === '"' && !singleQuoted) doubleQuoted = !doubleQuoted;
        else if (
          character === "#" &&
          !singleQuoted &&
          !doubleQuoted &&
          (index === 0 || /\s/u.test(line[index - 1]))
        ) {
          return line.slice(0, index);
        }
      }
      return line;
    })
    .join("\n");
}

function hasWriteModeFormatter(text) {
  const lines = text.split(/\r?\n/u);
  for (let index = 0; index < lines.length; index += 1) {
    const line = lines[index];
    if (!/\b(?:cargo\s+(?:fmt|fix)|eslint\b|prettier\b)/iu.test(line)) continue;

    let command = line.trim();
    while (/\\\s*$/u.test(lines[index]) && index + 1 < lines.length) {
      index += 1;
      command += ` ${lines[index].trim()}`;
    }

    if (/\bcargo\s+fix\b/iu.test(command)) return true;
    if (/\bcargo\s+fmt\b/iu.test(command) && !/--check\b/iu.test(command)) return true;
    if (/\beslint\b/iu.test(command) && /--fix\b/iu.test(command)) return true;
    if (/\bprettier\b/iu.test(command) && /--write\b/iu.test(command)) return true;
  }
  return false;
}

function hasContentsWritePermissions(text) {
  const permissions = permissionEntries(text);
  return permissions.some(
    ({ name, value }) =>
      value === "write-all" ||
      (name === "contents" && value === "write") ||
      (name === "permissions" && value === "write-all"),
  );
}

function hasPullRequestWritePermissions(text) {
  const permissions = permissionEntries(text);
  return permissions.some(
    ({ name, value }) =>
      value === "write-all" ||
      (name === "pull-requests" && value === "write") ||
      (name === "permissions" && value === "write-all"),
  );
}

function hasAnyWritePermissions(text) {
  return permissionEntries(text).some(({ value }) => value === "write" || value === "write-all");
}

function permissionEntries(text) {
  const entries = [];
  const lines = text.split(/\r?\n/u);
  for (let index = 0; index < lines.length; index += 1) {
    const match = lines[index].match(/^(\s*)permissions\s*:\s*(.*?)\s*$/u);
    if (!match) continue;

    const indentation = match[1].length;
    const value = match[2].trim();
    if (value !== "") {
      entries.push({ name: "permissions", value });
      if (value.startsWith("{")) {
        for (const inline of value.matchAll(/(?:^|[{,]\s*)([a-z][a-z-]*)\s*:\s*([a-z-]+)/giu)) {
          entries.push({ name: inline[1].toLowerCase(), value: inline[2].toLowerCase() });
        }
      }
      continue;
    }

    for (let nested = index + 1; nested < lines.length; nested += 1) {
      const line = lines[nested];
      if (/^\s*$/u.test(line)) continue;
      const nestedIndentation = line.match(/^\s*/u)?.[0].length ?? 0;
      if (nestedIndentation <= indentation) break;
      const nestedMatch = line.match(/^\s*([a-z][a-z-]*)\s*:\s*([a-z-]+)\s*$/iu);
      if (nestedMatch) {
        entries.push({ name: nestedMatch[1].toLowerCase(), value: nestedMatch[2].toLowerCase() });
      }
    }
  }
  return entries;
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
