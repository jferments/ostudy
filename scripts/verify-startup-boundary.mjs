#!/usr/bin/env node

import { readFile } from "node:fs/promises";

const files = {
  lib: "src-tauri/src/lib.rs",
  startup: "src-tauri/src/startup.rs",
  database: "src-tauri/src/database.rs",
  fullCheck: "scripts/check.sh",
  smoke: "scripts/smoke-startup-compatibility.sh",
  checkState: "scripts/check-state.sh",
};

const source = Object.fromEntries(
  await Promise.all(
    Object.entries(files).map(async ([key, path]) => [key, await readFile(path, "utf8")]),
  ),
);

const lineageIndex = source.database.indexOf("let lineage = inspect_lineage(connection)?;");
const configureIndex = source.database.indexOf("configure_connection(connection)?;");

const contracts = [
  [
    "Tauri delegates setup to the non-panicking startup boundary",
    source.lib.includes("mod startup;") && source.lib.includes(".setup(startup::setup)"),
  ],
  [
    "blocked-startup window shutdown does not require state that was never managed",
    source.lib.includes("window.try_state::<platform::PlatformRuntime>()") &&
      !source.lib.includes("window.state::<platform::PlatformRuntime>()"),
  ],
  [
    "startup compatibility errors are presented rather than returned through the Tauri setup hook",
    source.startup.includes("if let Err(error) = initialize(app)") &&
      source.startup.includes("present_startup_failure(app, &error.to_string())") &&
      source.startup.includes("application_startup_blocked") &&
      source.startup.includes("pre_public_database_requires_reset"),
  ],
  [
    "startup failure presentation uses the nonblocking dialog API",
    source.startup.includes(".show(move |_| handle.exit(STARTUP_CONFIGURATION_EXIT_CODE))") &&
      !source.startup.includes("blocking_show"),
  ],
  [
    "database lineage is inspected before write-capable connection configuration",
    lineageIndex >= 0 && configureIndex >= 0 && lineageIndex < configureIndex,
  ],
  [
    "the built-binary legacy-data startup smoke is part of full qualification",
    source.fullCheck.includes('step "Native startup compatibility smoke"') &&
      source.fullCheck.includes('bash "$SCRIPT_DIR/smoke-startup-compatibility.sh"'),
  ],
  [
    "startup smoke rejects panic paths and verifies byte-for-byte database stability",
    source.smoke.includes("application_startup_blocked") &&
      source.smoke.includes("panic_captured") &&
      source.smoke.includes('before_sha="$(sha256sum "$DATABASE"') &&
      source.smoke.includes('after_sha="$(sha256sum "$DATABASE"') &&
      source.smoke.includes('[[ ! -e "$DATABASE-wal" && ! -e "$DATABASE-shm" ]]'),
  ],
  [
    "qualification has a Git-visible worktree mutation guard",
    source.checkState.includes("ostudy_capture_worktree_state") &&
      source.checkState.includes("ostudy_assert_worktree_unchanged") &&
      source.checkState.includes("--untracked-files=all"),
  ],
];

const failures = contracts.filter(([, passed]) => !passed).map(([description]) => description);
if (failures.length > 0) {
  console.error("Startup boundary contract failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log(
  "OK   Startup failures are presented without Tauri setup panics, legacy database rejection is write-free, and full qualification exercises the built-binary compatibility boundary.",
);
