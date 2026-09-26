#!/usr/bin/env node

import { readFile } from "node:fs/promises";

const read = (path) => readFile(new URL(`../${path}`, import.meta.url), "utf8");
const failures = [];
const [successors, audit, startup, database, projectManifests] = await Promise.all([
  read("docs/development/shared-knowledge-lessons/README.md"),
  read("docs/development/release-readiness-cleanup-audit.md"),
  read("src-tauri/src/startup.rs"),
  read("src-tauri/src/database.rs"),
  read("scripts/verify-project-manifests.mjs"),
]);

function requireText(label, source, needle) {
  if (!source.includes(needle)) failures.push(`${label} missing ${JSON.stringify(needle)}`);
}
function rejectText(label, source, needle) {
  if (source.includes(needle)) failures.push(`${label} still contains stale current claim ${JSON.stringify(needle)}`);
}

rejectText("successor README", successors, "PR #104 is the active successor");
rejectText("successor README", successors, "PR #105 and PR #106 remain successor scopes");
requireText("successor README", successors, "PR #105 is intentionally deferred");
requireText("successor README", successors, "PR #106 is the active post-#104 stabilization, data-safety, packaging, and qualification tranche");

for (const path of [
  "scripts/restore_pre_public_backup.py",
  "scripts/reset-pre-public-data.sh",
]) requireText("cleanup audit", audit, path);
requireText("retired v26 runtime decision", audit, "retire from runtime");
rejectText("runtime startup", startup, "pre_public_transition::adopt_final_pre_public_database");
rejectText("runtime startup module", startup, "mod pre_public_transition");
requireText("pre-public fail-closed marker", database, "PRE_PUBLIC_RESET_PREFIX");
requireText("reset helper manifest", projectManifests, '"scripts/reset-pre-public-data.sh"');
requireText("completed retirement gate", audit, "human lineage gate confirmed");
requireText("explicit historical recovery", audit, "explicit staged recovery tool");
requireText("release baseline", audit, "`ostudy-release-v1` / schema 1");
requireText("migration retention", audit, "public-v1 migrations 1–30");
requireText("repair retention", audit, "public-v1 repairs 1–3");
requireText("legacy compatibility disposition", audit, "retain as compatibility evidence");

if (failures.length > 0) {
  console.error("Release-readiness cleanup verification failed:");
  for (const failure of failures) console.error(`  - ${failure}`);
  process.exit(1);
}
console.log("OK   Release-readiness cleanup records the release-v1 baseline while retaining public-v1 compatibility evidence and explicit historical recovery.");
