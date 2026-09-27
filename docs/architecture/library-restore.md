# Complete-library backup and restore ownership

`library_backup.rs` owns the package contract. Its `package` module owns format-4
SQLite containers, coherent SQLite snapshots, managed-file validation and isolated
extraction. `activation` owns private candidate preparation and journaled filesystem
replacement; `live` owns same-process activation and durable completion receipts.
The asynchronous `library_backup_commands.rs` adapter keeps this work off the native
event loop. Package preparation is never reported as completed restoration.

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

## Completion, UI and credentials

Completion receipts live outside the replaced learner database and are trusted
only when their transaction has a valid commit marker. A lost acknowledgement can
replay its completed receipt without replacing the library a second time. Results
include deck, card and document counts and distinguish completed from prepared.

Successful activation advances the library generation and emits completion. The
interface reloads to discard old controllers, selected IDs, drafts and caches;
the native process does not restart and its XDG environment does not change.
A completion observer also reads the last durable receipt after the refresh.
A failed activation that reopened the old library likewise invalidates stale UI
requests. A failure that cannot prove a coherent connection leaves IPC blocked.

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

These contracts do not rewrite the immutable public-v1 schema-30 migration bytes or the backup version. Historical backup readers remain. PR #106's release-baseline boundary converts supported public-v1 libraries to the immutable `ostudy-release-v1` schema-1 baseline only after staged schema/value preservation proof; ordinary release migrations then advance that restored library to the current schema. Restore must use that database owner rather than deleting ledgers or manually relabelling a library.
See [agent restore rules](../agents/LIBRARY-RESTORE.md) and the
[user backup guide](../user-guide/backups-imports-and-sharing.md).
