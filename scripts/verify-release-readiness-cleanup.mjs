#!/usr/bin/env node

import { readFile } from "node:fs/promises";
import { verifyReleaseReadinessCleanup } from "./lib/release-readiness-cleanup.mjs";

const read = (path) => readFile(new URL(`../${path}`, import.meta.url), "utf8");
const [audit, startup, database, projectManifests] = await Promise.all([
  read("docs/development/release-readiness-cleanup-audit.md"),
  read("src-tauri/src/startup.rs"),
  read("src-tauri/src/database.rs"),
  read("scripts/verify-project-manifests.mjs"),
]);

const failures = verifyReleaseReadinessCleanup({
  audit,
  startup,
  database,
  projectManifests,
});

if (failures.length > 0) {
  console.error("Release-readiness cleanup verification failed:");
  for (const failure of failures) console.error(`  - ${failure}`);
  process.exit(1);
}
console.log(
  "OK   Release-readiness cleanup retains release-v1 lineage, compatibility evidence, and explicit historical recovery.",
);
