# Complete-library backup and restore ownership

`library_backup.rs` owns the package contract. Its `package` module owns format-4
SQLite containers, coherent SQLite snapshots, managed-file validation and isolated
extraction. `activation` owns private candidate preparation and journaled filesystem
replacement; `live` owns same-process activation and durable completion receipts.
The asynchronous `library_backup_commands.rs` adapter keeps this work off the native
event loop. Package preparation is never reported as completed restoration.
Startup schema upgrades use `activation::checkpoint` and the same activation owner,
not an independent updater file-swap implementation. See the [library update guide](../user-guide/library-upgrades.md).

## Runtime admission

Startup acquires `library-owner.lock` before opening or recovering the library.
The persistent file is never unlinked; the OS releases ownership when the process
ends. This is a cooperating-process lock, not protection against old binaries or
arbitrary programs that ignore it. Separate production/dev directories can coexist.

`library_session` holds synchronous IPC leases through dispatch. Async forwarding
adapters retain a lease through their whole future, stamped with the original
page's generation. The typed IPC transport obtains one generation per loaded
page; it never automatically retries a stale mutation using a new generation.
Duplicate Rust macro symbols and duplicate registered wire names are separately
forbidden. The asset protocol holds a lease across ledger lookup and file reads;
responses are not cached across library replacement.

Restore first obtains the existing maintenance guard and an ordinary request lease
for the original page generation. It resolves the staged input and prepares a
private validated candidate while ordinary IPC admission remains open. Invalid
input therefore fails without closing the active connection or ordinary admission.
Before exclusive activation, it releases its own preparation lease and rechecks the
original generation while closing admission and draining the remaining leases.

Document, collection, Tutor, preparation and synchronization workers retain shared
activity guards until their actual work ends, including canceled transports whose
cleanup has not finished. Maintenance admission and close protection consult these
common activity facts. Domain registries still own their cancellation/progress
policies. No database/file IO happens while the session-state mutex is held. The
stable managed `Database` object and its clones see the replacement connection only
after activation and reconciliation succeed.

## Private preparation and durable activation

The source package is copied into a unique `restore-transactions/<operation-id>`
directory. Extraction, schema upgrade and validation operate only on its candidate.
The native `PreparedRestore` value binds the library root, copied source digest and
a length-framed digest of the candidate tree. It is not a client-supplied path or
serialized authorization token. An abandoned preparation retains its files but has
no active journal, so startup does not silently install it.

Exclusive activation verifies those same private bytes before recording a journal
or moving any active file. Changing the external input afterward does not change
the selected private snapshot; changing the retained source or candidate refuses
activation. Persist `library-restore-journal.json` before the first active-file move.
The journal contains an operation identity, input hash and original-entry presence
bits, never arbitrary filesystem paths. Paths are application-owned constants.

The learner database, WAL/SHM/rollback journal, managed assets/documents, and
semantic cache sidecars move as a coordinated operation. Each move synchronizes
both parent directories. Never unlink a database underneath open connections or
discard its WAL. Startup and live activation use the same reconciliation owner.

Commit is recorded only after reopen, integrity/foreign-key verification and
reconciliation. A committed operation is not rolled back merely because evidence
cleanup failed. An indeterminate commit fsync keeps runtime admission closed until
startup resolves the on-disk decision. The previous library and selected package
remain in the transaction directory. Normal startup, installers and subsequent
restores do not purge them.

Before commit, failures restore the previous coherent entries; an abrupt process
exit is recovered from the journal on startup. Recovery is idempotent when itself
interrupted. Prepared inputs are preserved outside the automatic startup slot
rather than reapplied at every launch. Matching duplicate copies are retired only
after the retained copy is verified and synced. Unknown `.pre-restore` remnants,
malformed journals and symlinked recovery paths are preserved and refused.
Rust Drop handles ordinary errors; disk records are the crash-recovery authority.

## Startup library upgrades

`startup/upgrade.rs` performs read-only planning after process ownership and prior
journal recovery. It rejects missing databases with orphan journals, nonregular
sources/sidecars, unsupported lineages and invalid migration prefixes. A current
header avoids redundant planning scans, but `Database::open` still validates the
current payload before ordinary use. Empty initialization and explicit pre-public
recovery remain separate. Released SQL and the native migration metadata owner are
unchanged. Later forward migrations (schema 3 deck-merge receipts, schema 4
StudyTarget generation guidance) use this same checkpointed upgrade path.

`library_backup/checkpoint.rs` writes a durable upgrade intent before preparation.
It creates a private SQLite Backup API snapshot, including committed WAL rows, and
copies complete assets, documents, transfers and cache trees plus an available
build receipt. Regular-file/path checks, file/tree hashes, schema admission,
integrity and foreign-key checks, and filesystem synchronization verify the
checkpoint before candidate migration starts. Copy errors and low-space failures
stop the operation; they never authorize unprotected migration. The source SQLite
data version and managed-root presence/hashes are rechecked before preparation
returns. These checks supplement, rather than replace, cooperating-process ownership.

All startup migration and lineage-conversion writes operate on the candidate.
Native `Database::open` retains its existing per-migration transaction, checksum,
ledger, payload and legacy-converter checks. `checkpoint_preservation.rs` adds a
version-specific retained-value proof for release schema 1 to 2: typed values,
column identities, multiplicity and notes rowids remain equal, while the newly
added receipt table must be empty. A future release upgrade requires an explicit
preservation contract rather than silently adopting count-only comparisons.
Historical public migrations/repairs retain their existing database-owner proofs;
there is no claim that every historical transformation is byte-identical.

The existing activation journal keeps its immutable version-1 path mapping for
package restores. Version 2 coordinates the entire upgrade-managed trees, build
receipt and database sidecars. The source digest identifies `checkpoint.json`, and
the prepared value also binds the candidate tree. Rollback retains replaced
candidate entries under `failed` instead of discarding them. Prior entries and the
checkpoint remain available. Paths are fixed by code, not accepted from a writable
manifest. Hashes detect inconsistent bytes; these local records are not signatures
or protection against arbitrary hostile same-account filesystem mutation.

Startup holds ordinary IPC readiness closed throughout activation and
reconciliation. The database is declared after the activation guard so error
unwinding closes it before filesystem rollback. Upgrade commit validates the
post-reconciliation database again before recording its decision. A successful
commit cannot be turned into a rollback by an intent-archival failure.

A failed or abandoned preparation retains its intent and requires an explicit
recovery choice. No blind startup retries occur. Restoration copies the verified
checkpoint into a new activation transaction, retains the replaced library and
records a Revert intent. The newer executable cannot immediately migrate that
restored older schema again. A compatible executable with the restored target
schema can admit it after validation; an explicit retry authorizes one new attempt.
An unresolved activation journal must be recovered before installing an older
binary that might not understand its layout.

`startup/progress.rs` stores and replays advisory progress snapshots.
`src/ipc/libraryUpgrade.ts` subscribes before requesting replay, ignores stale or
malformed messages, and cleans up subscriptions. `NativeStartupGate` renders the
phase and a dismissible committed-update notice, but only database-backed
readiness admits product children. Native recovery dialogs are available before
that product gate and bind confirmations to the outstanding operation identity.
They distinguish library restoration from installing a compatible application.

`library-build.json` records the last successfully opened build when possible.
Checkpoint metadata includes available old/new build identity, application profile,
lineage/schema, operation identity, timestamp and checkpoint digest. Missing old
build information stays unknown; commit SHAs are recorded only when supplied by
the build environment. No keys or source text belong in this metadata. Installer
updates, package versioning and artifact distribution remain separate from this
library transaction. Same-schema updates do not create migration checkpoints.

There is no automatic pruning of recovery copies, no historical-checkpoint Settings
browser, and no automatic executable downgrade. Disk usage can grow across failed
attempts. Internal checkpoints do not replace external portable backups.

## Completion, UI and credentials

Completion receipts live outside the replaced learner database and are trusted
only when their transaction has a valid commit marker. A lost acknowledgement can
replay its completed receipt without replacing the library a second time. Results
include deck, card and document counts and distinguish completed from prepared.

Successful live restore activation advances the library generation and emits completion.
The interface reloads to discard old controllers, selected IDs, drafts and caches;
the native process does not restart and its XDG environment does not change.
A completion observer also reads the last durable receipt after the refresh.
A failed activation that reopened the old library likewise invalidates stale UI
requests. A failure that cannot prove a coherent connection leaves IPC blocked.
Startup upgrades take place before product consumers are admitted.

The OS keyring remains untouched. Live replacement clears the native process's
entire session-only credential map before reopening admission, including orphaned
entries that are not present in either library. Backups contain no credential bytes.
A restored profile may still find its existing OS keyring entry; this does not mean
a backup carried a key.

## Qualification

Current-format preservation tests compare typed rows, stable identities,
schedules, attempts, reviews, drafts and managed-file hashes. Activation tests
terminate child processes at install/rollback/commit boundaries and retain actual
committed WAL rows. Live tests exercise stable database clones, rollback, receipt
replay and pending-input preservation. Preparation tests cover invalid backups,
continued writes to the current library, abandoned candidates, changed input paths
and tampered private bytes. Shared activity tests cover both admission orders,
duplicate registration and cancellation cleanup. Session/transport tests reject
stale work and verify that preparation alone cannot produce completion. UI tests
exercise explicit destructive confirmation and same-process completion.

Upgrade-specific tests add low-space snapshot failure, lossy equal-count migration,
populated schedules and review evidence, managed-root drift, post-reconciliation
commit refusal, stale recovery choices, compatible-schema recovery and actual
process termination during preparation/activation/rollback. Progress tests prove
that advisory completion cannot bypass startup readiness. Follow the [disposable
upgrade acceptance procedure](../development/library-upgrade-acceptance.md) for
local checks and remaining native-dialog/package/manual boundaries.

These contracts do not rewrite the immutable public-v1 schema-30 migration bytes or the backup version. Historical backup readers remain. PR #106's release-baseline boundary converts supported public-v1 libraries to the immutable `ostudy-release-v1` schema-1 baseline only after staged schema/value preservation proof; ordinary release migrations then advance that restored library to the current schema. Restore must use that database owner rather than deleting ledgers or manually relabelling a library.
See the [user backup guide](../user-guide/backups-imports-and-sharing.md) for
the learner-facing behavior.

## Rules for changing backup and restore code

Read this document before changing backup, activation, database lifetime, IPC
admission, or their tests. Personal libraries never enter GitHub or CI.

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
- Do not erase saved keys or log credential/content bytes. Live library replacement
  clears **all** session-only credentials held by the native process, including
  orphaned entries; it does not read, delete, or probe saved OS-keyring credentials.

Use synthetic, isolated libraries. Preserve row identities/values, schedules,
attributions, question attempts, learner edits, durable generation receipts,
document ownership and managed-file hashes. Counts alone are not parity evidence.
Test same-process activation through an existing Database clone, post-restore
writes/reopen, failed reconciliation, receipt replay, cancellation cleanup,
interrupted rollback and real WAL recovery. Keep stale-generation and UI
confirmation/completion tests. Never weaken required assertions to obtain green CI.

Run the static and fast checks while iterating, then the Linux full, Debian
package and Android gates on the final commit. Report the checks that actually
ran on that commit; skipped gates, live-provider tests, installed-copy checks and
physical-device tests are not implied by workflow success. Do not ask the
maintainer to regenerate expensive document work merely to verify restore
persistence.

Schema consolidation is a separate database-owner boundary from backup
activation. The release-v1 design keeps immutable public-v1 migrations and
supported backup readers as compatibility inputs, and uses the release-v1
baseline/converter only after staged preservation evidence.
