#!/usr/bin/env node

import { readFile } from "node:fs/promises";
import { join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const defaultRoot = fileURLToPath(new URL("..", import.meta.url));
const rootIndex = process.argv.indexOf("--root");
const root = rootIndex >= 0 ? resolve(process.argv[rootIndex + 1] ?? "") : defaultRoot;
if (rootIndex >= 0 && !process.argv[rootIndex + 1]) {
  console.error("ERROR: --root requires a path");
  process.exit(2);
}

const requirements = new Map([
  ["AGENTS.md", ["ANCHORLINE RESET", "never means", "Do not ask the human to repeat", "docs/agents/RECOVERY-PROTOCOL.md"]],
  ["docs/agents/RECOVERY-PROTOCOL.md", ["One bounded resumption read", "Keep an active checkpoint", "Tool and access discipline", "direct PR and branch-ref reads", "Progress and response discipline", "Workflow success and gate coverage are separate facts"]],
  ["docs/agents/GITHUB-DIRECT-DEVELOPMENT.md", ["Interruption-resilient execution", "Efficient GitHub reads and publication", "Known branch", "No mid-composition polling", "Blob proportionality", "ANCHORLINE RESET", "Do not ask the human to repeat", "substantive response"]],
  ["docs/agents/GITHUB-CONNECTOR.md", ["session-local action exposure", "does not revoke repository permission", "Do not rediscover a working action surface"]],
  ["docs/agents/COMMAND-RUNBOOK.md", ["verify-agent-recovery-policy.mjs"]],
  ["scripts/check-static.sh", ["verify-agent-recovery-policy.mjs"]],
]);

const failures = [];
for (const [path, needles] of requirements) {
  let text;
  try {
    text = await readFile(join(root, path), "utf8");
  } catch (error) {
    failures.push(`${path}: cannot read required file (${error instanceof Error ? error.message : String(error)})`);
    continue;
  }
  for (const needle of needles) {
    if (!text.includes(needle)) failures.push(`${path}: missing recovery contract text: ${needle}`);
  }
}

if (failures.length > 0) {
  console.error("Agent recovery policy verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}
console.log("Agent recovery policy verified.");
