#!/usr/bin/env node

import { access, readFile } from "node:fs/promises";

const read = (path) => readFile(new URL(`../${path}`, import.meta.url), "utf8");
const exists = async (path) => {
  try {
    await access(new URL(`../${path}`, import.meta.url));
    return true;
  } catch {
    return false;
  }
};
const failures = [];

const [
  lib,
  startup,
  ledgerText,
  recovery,
  recoveryEntrypoint,
  database,
  fullCheck,
  smoke,
  reset,
  checkState,
] = await Promise.all([
  read("src-tauri/src/lib.rs"),
  read("src-tauri/src/startup.rs"),
  read("scripts/history/pre-public-v26/ledger.json"),
  read("scripts/restore_pre_public_backup.py"),
  read("scripts/transition-pre-public-data.sh"),
  read("src-tauri/src/database.rs"),
  read("scripts/check.sh"),
  read("scripts/smoke-startup-compatibility.sh"),
  read("scripts/reset-pre-public-data.sh"),
  read("scripts/check-state.sh"),
]);
const ledger = JSON.parse(ledgerText);

function requireContract(label, passed) {
  if (!passed) failures.push(label);
}

const lineageIndex = database.indexOf("let lineage = inspect_lineage(connection)?;");
const configureIndex = database.indexOf("configure_connection(connection)?;");
const setup = startup.slice(startup.indexOf("pub(crate) fn setup"), startup.indexOf("fn initialize("));
const initialization = startup.slice(startup.indexOf("fn initialize("), startup.indexOf("fn create_linux_main_window"));

requireContract(
  "Tauri delegates setup to the non-panicking startup boundary",
  lib.includes("mod startup;") && lib.includes(".setup(startup::setup)"),
);
requireContract(
  "automatic pre-public adoption is absent from the runtime",
  !(await exists("src-tauri/src/startup/pre_public_transition.rs"))
    && !startup.includes("mod pre_public_transition")
    && !startup.includes("adopt_final_pre_public_database")
    && startup.includes("database::Database::open(paths.database_path())"),
);
requireContract(
  "non-public data still fails closed with explicit recovery guidance",
  startup.includes("pre_public_database_requires_manual_recovery")
    && startup.includes("transition-pre-public-data.sh")
    && startup.includes("Automatic pre-public conversion has been retired"),
);
requireContract(
  "unsupported startup failures are presented rather than returned through Tauri setup",
  setup.includes("present_startup_failure(app.handle(), &error.to_string())")
    && setup.includes("Ok(Err(error)) => present_startup_failure(&handle, &error)")
    && setup.includes("Err(error) => present_startup_failure(&handle, &error.to_string())")
    && setup.includes("Ok(())")
    && !setup.includes("initialize(app)?"),
);
requireContract(
  "native initialization releases the event loop and readiness follows all managed runtimes",
  setup.includes("tauri::async_runtime::spawn_blocking")
    && initialization.indexOf("app.manage(database)") > initialization.indexOf("app.manage(pdf_render_runtime)")
    && initialization.indexOf("app.manage(database)") > initialization.indexOf("logging::flush"),
);
requireContract(
  "database lineage is inspected before write-capable connection configuration",
  lineageIndex >= 0 && configureIndex >= 0 && lineageIndex < configureIndex,
);
requireContract(
  "archived final-v26 contract remains exact",
  ledger.schemaVersion === 26
    && JSON.stringify(ledger.acceptedSqliteUserVersions) === "[0,26]"
    && ledger.migrations?.length === 26
    && ledger.migrations.at(-1)?.name === "deck_folders"
    && ledger.repairs?.length === 3
    && ledger.repairs.at(-1)?.name === "review_event_historical_attribution",
);
requireContract(
  "explicit recovery consumes the archived contract and converts a staging copy",
  recovery.includes('scripts" / "history" / "pre-public-v26" / "ledger.json"')
    && recovery.includes("convert_staged_copy_to_public_v1")
    && recovery.includes("application_snapshot")
    && recovery.includes("Learner data changed during explicit pre-public recovery")
    && recovery.includes('PUBLIC_SCHEMA_LINEAGE = "ostudy-public-v1"'),
);
requireContract(
  "recovery entrypoint supports active or archived v26 sources",
  recoveryEntrypoint.includes("--data-dir")
    && recoveryEntrypoint.includes("--source-backup")
    && recoveryEntrypoint.includes("--confirm")
    && recoveryEntrypoint.includes("restore_pre_public_backup.py"),
);
requireContract(
  "full qualification exercises runtime refusal and explicit recovery",
  fullCheck.includes('step "Native startup compatibility smoke"')
    && smoke.includes("application_startup_blocked")
    && smoke.includes("transition-pre-public-data.sh")
    && smoke.includes("application_started")
    && smoke.includes("ostudy-public-v1"),
);
requireContract(
  "empty-library reset remains explicit",
  reset.includes("--confirm-empty-library")
    && reset.includes("INTENTIONALLY archives")
    && reset.includes("transition-pre-public-data.sh"),
);
requireContract(
  "startup diagnostics flush after durable initialization",
  startup.includes("logging::flush(Duration::from_millis(500))")
    && startup.indexOf('"application_started"') < startup.indexOf("logging::flush(Duration::from_millis(500))"),
);
requireContract(
  "qualification retains Git-visible worktree mutation guard",
  checkState.includes("ostudy_capture_worktree_state")
    && checkState.includes("ostudy_assert_worktree_unchanged")
    && checkState.includes("--untracked-files=all"),
);

if (failures.length > 0) {
  console.error("Startup boundary contract failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}
console.log(
  "OK   Runtime accepts only the public lineage; historical final-v26 recovery is explicit, staged, fail-closed, and covered by built-binary qualification.",
);
