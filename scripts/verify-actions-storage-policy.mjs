#!/usr/bin/env node

import { readdir, readFile } from "node:fs/promises";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const workflowDirectory = join(root, ".github", "workflows");
const failures = [];
const cloudArtifactAllowlist = new Set([
  "android-package.yml",
  "maintain-generated-candidates.yml",
  "release.yml",
]);

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

  const uploadsArtifacts = /actions\/upload-artifact@/u.test(text);
  if (uploadsArtifacts && !cloudArtifactAllowlist.has(entry.name)) {
    failures.push(`${entry.name}: routine CI may not upload GitHub Actions artifacts`);
  }
  if (uploadsArtifacts && !/^\s+retention-days:\s*1\s*$/mu.test(text)) {
    failures.push(`${entry.name}: exceptional cloud artifacts must use retention-days: 1`);
  }
  if (uploadsArtifacts && /^\s+retention-days:\s*(?!1\s*$)\d+/mu.test(text)) {
    failures.push(`${entry.name}: cloud artifact retention may not exceed one day`);
  }
}

for (const name of ["check.yml", "unified-runner-qualification.yml", "pr-contract.yml"]) {
  const text = workflows.get(name) ?? "";
  if (/actions\/upload-artifact@/u.test(text)) {
    failures.push(`${name}: qualification/metadata evidence must remain runner-local`);
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
for (const required of [
  "publish_to_github:",
  "if: inputs.publish_to_github",
  "Persist Android package on runner",
  "retention-days: 1",
]) {
  if (!android.includes(required)) failures.push(`android-package.yml missing bounded publication contract: ${required}`);
}

const auxiliary = workflows.get("maintain-generated-candidates.yml") ?? "";
for (const required of [
  "github.event_name == 'workflow_dispatch' && inputs.export_auxiliary_bundle",
  "retention-days: 1",
]) {
  if (!auxiliary.includes(required)) failures.push(`maintain-generated-candidates.yml missing opt-in artifact contract: ${required}`);
}

const release = workflows.get("release.yml") ?? "";
for (const required of ["Persist release artifacts on runner", "retention-days: 1"]) {
  if (!release.includes(required)) failures.push(`release.yml missing bounded release-storage contract: ${required}`);
}

const maintenance = workflows.get("actions-storage-maintenance.yml") ?? "";
for (const required of [
  "schedule:",
  "cron:",
  "actions: write",
  "runs-on: [self-hosted, linux, x64, ostudy-unified]",
  "cleanup-actions-storage.py",
  "artifact_age_hours=24",
  "--delete-all-caches",
]) {
  if (!maintenance.includes(required)) failures.push(`actions-storage-maintenance.yml missing cleanup contract: ${required}`);
}

if (failures.length > 0) {
  console.error("Actions storage policy verification failed:");
  for (const failure of failures) console.error(`  - ${failure}`);
  process.exit(1);
}

console.log(
  "OK   Actions storage policy: CI executes on the self-hosted runner, reusable caches and diagnostics stay runner-local, exceptional cloud artifacts are opt-in/bounded to one day, and scheduled cleanup purges stale artifacts and GitHub caches.",
);
