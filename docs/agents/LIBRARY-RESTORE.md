# Backup/restore development and recovery rules

Read [restore ownership](../architecture/library-restore.md) before changing
backup, activation, database lifetime, IPC admission, or their tests. Follow the
repository GitHub-direct workflow; no personal libraries enter GitHub or CI.

## Required invariants

- Normal **Restore backup** completes in the running native process. Reloading
  the web interface is not restarting the application. Do not restore the old
  prepare-only success message or require a manual restart for normal activation.
- Validation, preparation, activation, verified completion and rollback are
  distinct states. An empty `public-current` database with integrity OK can be
  the untouched destination. Inspect the selected/pending package's embedded
  snapshot and the active destination separately before claiming lost data.
- Acquire process ownership before startup recovery. The lock file persists;
  never unlink it to bypass another owner. Close old binaries that do not honor
  the new lock. Isolated XDG roots are not isolated production keyrings.
- Drain request leases and exclude detached work before replacing connections or
  managed files. Async requests keep their original generation. Do not silently
  retry stale writes after refreshing their generation.
- Keep database/WAL/SHM/rollback journals together. Keep fixed-path restore journals,
  prior libraries and input backups. No `rm -rf`, journal deletion, schema stamping,
  rollback overwrite or automatic package removal is a recovery shortcut.
- Commit-marker evidence governs ambiguous results. A lost reply after commit
  must replay its receipt, not repeat replacement. Preserve files and fail closed
  when a coherent library cannot be established. Cleanup failure is not permission
  to undo a committed restore.
- Do not erase saved keys or log credential/content bytes. Only session credentials
  tied to the replaced/restored profiles are cleared during live activation.

## Tests and handoff

Use synthetic, isolated libraries. Preserve row identities/values, schedules,
attributions, question attempts, learner edits, durable generation receipts,
document ownership and managed-file hashes. Counts alone are not parity evidence.
Test same-process activation through an existing Database clone, post-restore
writes/reopen, failed reconciliation, receipt replay, cancellation cleanup,
interrupted rollback and real WAL recovery. Keep stale-generation and UI
confirmation/completion tests. Never weaken required assertions to obtain green CI.

Use maintained Static/Fast checks during repair, then freeze a complete candidate
for applicable Linux full, production Debian and Android qualification. Report
actual exact-SHA steps; skipped gates, live-provider tests, installed-copy checks
and physical-device tests are not implied by workflow success. Do not ask the
human to regenerate expensive document work merely to verify restore persistence.

Schema consolidation is not part of the backup repair. Keep the current lineage,
immutable migrations and supported backup readers until a separately implemented
baseline/converter has preservation evidence. No workstation schema-squashing
commands precede that implementation.
