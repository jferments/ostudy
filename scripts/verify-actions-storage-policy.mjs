#!/usr/bin/env node

import { readdir, readFile } from "node:fs/promises";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const workflowDirectory = join(root, ".github", "workflows");
const failures = [];

const workflows = new Map();
for (const entry of await readdir(workflowDirectory, { withFileTypes: true })) {
  if (!entry.isFile() || !/\.ya?ml$/u.test(entry.name)) continue;
  const text = await readFile(join(workflowDirectory, entry.name), "utf8");
  workflows.set(entry.name, text);

  if (/runs-on:\s*(?:ubuntu|windows|macos)-/u.test(text)) {
    failures.push(`${entry.name}: GitHub-hosted runner is forbidden; use the self-hosted runner`);
  }
  if (/actions\/cache@/u.test(text)) {
    failures.push(`${entry.name}: actions/cache stores data in GitHub cloud storage; use runner-local caches`);
  }
  if (/^\s+cache:\s*\S+/mu.test(text)) {
    failures.push(`${entry.name}: action-managed cache input is forbidden; use runner-local cache directories`);
  }
  if (/actions\/upload-artifact@/u.test(text)) {
    failures.push(`${entry.name}: GitHub Actions artifact storage is forbidden; persist output on the self-hosted runner`);
  }
}

const check = workflows.get("check.yml") ?? "";
for (const required of [
  "persist-runner-output.sh",
  "Persist bounded diagnostic evidence on runner",
  "Persist bounded failure evidence on runner",
]) {
  if (!check.includes(required)) failures.push(`check.yml missing runner-local evidence contract: ${required}`);
}

const unified = workflows.get("unified-runner-qualification.yml") ?? "";
if (!unified.includes("Persist bounded failure evidence on runner")) {
  failures.push("unified-runner-qualification.yml must retain failure evidence on the runner");
}

const android = workflows.get("android-package.yml") ?? "";
if (!android.includes("Persist Android package on runner")) {
  failures.push("android-package.yml must retain packages on the runner");
}

const release = workflows.get("release.yml") ?? "";
if (!release.includes("Persist release artifacts on runner")) {
  failures.push("release.yml must retain release output on the runner");
}

const maintenance = workflows.get("actions-storage-maintenance.yml") ?? "";
for (const required of [
  "schedule:",
  "cron:",
  "actions: write",
  "runs-on: [self-hosted, linux, x64, ostudy-unified]",
  "cleanup-actions-storage.py",
  "--artifact-max-age-hours 0",
  "--delete-all-caches",
]) {
  if (!maintenance.includes(required)) failures.push(`actions-storage-maintenance.yml missing cleanup contract: ${required}`);
}

const persistence = await readFile(join(root, "scripts", "persist-runner-output.sh"), "utf8");
for (const required of [
  "OSTUDY_RUNNER_OUTPUT_RETENTION_DAYS:-7",
  "OSTUDY_RUNNER_OUTPUT_MAX_BYTES:-21474836480",
  "prune-runner-output.py",
  "--retention-days",
  "--max-bytes",
]) {
  if (!persistence.includes(required)) failures.push(`persist-runner-output.sh missing local retention contract: ${required}`);
}

const localPruner = await readFile(join(root, "scripts", "prune-runner-output.py"), "utf8");
for (const required of [
  'RUN_DIR = re.compile(r"^[0-9]+-[0-9]+$")',
  "item.name != current",
  "resolved.parent != resolved_root",
  "retention-days",
  "max-bytes",
]) {
  if (!localPruner.includes(required)) failures.push(`prune-runner-output.py missing safe-pruning contract: ${required}`);
}

if (failures.length > 0) {
  console.error("Actions storage policy verification failed:");
  for (const failure of failures) console.error(`  - ${failure}`);
  process.exit(1);
}

console.log(
  "OK   Actions storage policy: oStudy CI executes on the self-hosted runner, cloud Actions artifacts/caches are forbidden, outputs and diagnostics stay runner-local with bounded local retention, and scheduled cleanup purges residual GitHub Actions artifacts and caches.",
);
