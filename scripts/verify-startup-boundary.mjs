#!/usr/bin/env node

import { readFile } from "node:fs/promises";

const files = {
  lib: "src-tauri/src/lib.rs",
  startup: "src-tauri/src/startup.rs",
  transition: "src-tauri/src/startup/pre_public_transition.rs",
  ledger: "src-tauri/src/startup/pre_public_v26_ledger.json",
  database: "src-tauri/src/database.rs",
  fullCheck: "scripts/check.sh",
  smoke: "scripts/smoke-startup-compatibility.sh",
  reset: "scripts/reset-pre-public-data.sh",
  restore: "scripts/restore_pre_public_backup.py",
  checkState: "scripts/check-state.sh",
};

const source = Object.fromEntries(
  await Promise.all(
    Object.entries(files).map(async ([key, path]) => [key, await readFile(path, "utf8")]),
  ),
);
const ledger = JSON.parse(source.ledger);

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
    "startup attempts verified final-v26 adoption before presenting manual recovery",
    source.startup.includes("open_database_with_pre_public_adoption") &&
      source.startup.includes("adopt_final_pre_public_database(paths.database_path())") &&
      source.startup.includes("pre_public_database_requires_manual_recovery"),
  ],
  [
    "unsupported startup failures are presented rather than returned through Tauri setup",
    source.startup.includes("if let Err(error) = initialize(app)") &&
      source.startup.includes("present_startup_failure(app, &error.to_string())") &&
      source.startup.includes("application_startup_blocked"),
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
    "the immutable final pre-public contract matches the historical migration runner",
    ledger.schemaVersion === 26 &&
      Array.isArray(ledger.acceptedSqliteUserVersions) &&
      ledger.acceptedSqliteUserVersions.length === 2 &&
      ledger.acceptedSqliteUserVersions[0] === 0 &&
      ledger.acceptedSqliteUserVersions[1] === 26 &&
      Array.isArray(ledger.migrations) &&
      ledger.migrations.length === 26 &&
      ledger.migrations.at(-1)?.name === "deck_folders" &&
      Array.isArray(ledger.repairs) &&
      ledger.repairs.length === 3 &&
      ledger.repairs.at(-1)?.name === "review_event_historical_attribution",
  ],
  [
    "Rust and recovery tooling consume the same immutable lineage contract",
    source.transition.includes('include_str!("pre_public_v26_ledger.json")') &&
      source.transition.includes("accepted_sqlite_user_versions") &&
      source.restore.includes("pre_public_v26_ledger.json") &&
      source.restore.includes("ACCEPTED_SQLITE_USER_VERSIONS") &&
      source.restore.includes("HISTORICAL_SQLITE_USER_VERSION = 0"),
  ],
  [
    "final pre-public adoption is exact-ledger, backed up, transactional, and data preserving",
    source.transition.includes("Backup::new") &&
      source.transition.includes("TransactionBehavior::Immediate") &&
      source.transition.includes("preserved_data_snapshot") &&
      source.transition.includes("Learner data changed during adoption") &&
      source.transition.includes("pre_public_database_adoption_completed"),
  ],
  [
    "startup diagnostics are flushed after durable initialization",
    source.startup.includes("logging::flush(Duration::from_millis(500))") &&
      source.startup.indexOf('"application_started"') <
        source.startup.indexOf("logging::flush(Duration::from_millis(500))"),
  ],
  [
    "the built-binary adoption smoke is part of full qualification",
    source.fullCheck.includes('step "Native startup compatibility smoke"') &&
      source.fullCheck.includes('bash "$SCRIPT_DIR/smoke-startup-compatibility.sh"'),
  ],
  [
    "startup smoke verifies the historical marker, preserved decks, and analytics",
    source.smoke.includes("HISTORICAL_SQLITE_USER_VERSION") &&
      source.smoke.includes("pre_public_database_adoption_completed") &&
      source.smoke.includes("application_started") &&
      source.smoke.includes("before_decks") &&
      source.smoke.includes("before_reviews") &&
      source.smoke.includes("after_attributions") &&
      source.smoke.includes("library.sqlite3.pre-public-v26-"),
  ],
  [
    "archived pre-public data has a non-destructive verified restore path",
    source.restore.includes("validate_final_pre_public_backup") &&
      source.restore.includes("EXPECTED_MIGRATIONS") &&
      source.restore.includes("EXPECTED_REPAIRS") &&
      source.restore.includes("Archived the previous active directory") &&
      source.restore.includes("original timestamped pre-public backup was retained unchanged"),
  ],
  [
    "empty-library reset requires explicit destructive intent",
    source.reset.includes("--confirm-empty-library") &&
      source.reset.includes("INTENTIONALLY archives") &&
      source.reset.includes("restore_pre_public_backup.py") &&
      !source.reset.includes("--confirm) CONFIRMED=1"),
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
  "OK   Startup preserves the actual historical final-v26 library through a shared lineage contract, consistent SQLite backup/adoption, durable startup diagnostics, manual recovery for unsupported data, and built-binary preservation qualification.",
);