# Complete-library backup and restore ownership

`library_backup.rs` owns the application contract and DTOs. Its `package` module
owns format-4 SQLite containers, coherent SQLite snapshots, managed-file
validation and isolated extraction. Its `activation` module owns recoverable
filesystem replacement. `library_backup_commands.rs` is the non-blocking native
command adapter. Never treat package preparation as completed activation.

## Durable activation

The source package is retained in a unique `restore-transactions/<operation-id>`
directory. Validate and upgrade an isolated candidate before touching the active
library. Persist the fixed-path journal `library-restore-journal.json` before
moving any active entry. The journal contains an operation identity, input hash
and original-entry presence bits, never arbitrary filesystem paths.

The learner database, its WAL/SHM/rollback journal, managed assets/documents, and
semantic cache sidecars move as a coordinated operation. Each move synchronizes
both parent directories. Do not unlink a live SQLite database or discard its WAL.
All runtime users must be stopped/drained before activation or recovery.

A commit marker is written only after the replacement reopens and reconciliation
succeeds. A committed operation is never rolled back merely because evidence
cleanup failed. Its prior library and source package stay under the transaction
directory. This intentionally consumes additional disk space; normal installer
removal, startup and subsequent restores must not purge these recovery copies.

If the process dies before commit, the next startup uses the journal to finish
restoring the previous entries. Recovery is idempotent even when interrupted
again. The pending input is retained with the transaction rather than silently
reapplied at every launch. A retry is an explicit new selection/preparation.
Malformed journals, symbolic links, and pre-journal `.pre-restore` artifacts are
preserved and reported; they must never be deleted to make a new attempt pass.
A Rust destructor handles ordinary errors, but on-disk evidence—not Drop—is the
crash-recovery authority.

## Acceptance and interpretation

A populated `pending-library-restore.ostudybackup` plus an empty active database
proves preparation, not completed restoration. A `public-current` schema report
and SQLite integrity success can describe a perfectly valid empty database.
Verify active content after activation, not merely the package summary.

Current-format preservation tests compare typed row digests, stable identities,
schedules, attempts, reviews, drafts and managed-file hashes. Additional tests
in `activation_tests.rs` exit real child processes at every install/rollback
boundary and after the durable commit marker, and exercise a real WAL containing
committed rows. These tests run in the existing current-format fast gate.

This recovery increment does not change the schema-30 lineage or any immutable
migration bytes. In-app activation and its UI/session fencing are the next
acceptance increment; the old prepare-and-restart UI is not yet a completed
in-app restore. Migration consolidation is a separate retained-data conversion,
not an uninstall, ledger deletion or manual `PRAGMA user_version` assignment.
