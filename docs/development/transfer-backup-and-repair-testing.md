# Transfer, backup, and restore local acceptance

This guide maintains the transfer checks originally introduced in Stages 3.4 and
3.5 and the current same-process restore checks. Historical schema-13 repair fixtures below are not the current schema or instructions for modifying a real library. The current runtime schema is `ostudy-release-v1`, version 5, built from the immutable release baseline at schema 1 plus forward release migrations. The immutable `ostudy-public-v1` chain through schema 30 remains compatibility/recovery input and is converted only through the staged preservation boundary.

## Safety first

Use disposable content. Close every process using a library before making a cold
copy of its complete application-data directory, including SQLite sidecars. For
the isolated acceptance flow below, the explicit `TEST_ROOT`/XDG directories are
the operational storage boundary; do not depend on a product help tile to locate
test data. Never delete a WAL, recovery journal, pending backup or ownership lock
to bypass a failure.

Build the maintained development profile, then launch an isolated library:

```bash
./scripts/ostudy-dev build
TEST_ROOT="$(mktemp -d)"
mkdir -p \
  "$TEST_ROOT/data" \
  "$TEST_ROOT/config" \
  "$TEST_ROOT/cache" \
  "$TEST_ROOT/state"

XDG_DATA_HOME="$TEST_ROOT/data" \
XDG_CONFIG_HOME="$TEST_ROOT/config" \
XDG_CACHE_HOME="$TEST_ROOT/cache" \
XDG_STATE_HOME="$TEST_ROOT/state" \
./scripts/ostudy-dev run
```

Keep this directory for the whole test. A desktop-menu launch does not inherit
these per-command XDG settings. Production and development identities are
separate; two production test roots still share the production keyring namespace.

## Automated gates

```bash
./scripts/ostudy-dev check-fast
./scripts/ostudy-dev check
```

The complete gate covers frontend tests, TypeScript, production build, rustfmt,
warning-free Clippy, Rust/migration tests, standalone Tauri build, dependency
verification and native persistence workflows. Use exact-SHA Actions evidence for
qualification and report skipped package/Android/manual boundaries separately.
See [restore architecture](../architecture/library-restore.md), including its
[rules for changing backup and restore code](../architecture/library-restore.md#rules-for-changing-backup-and-restore-code).

## Current `.studydeck` package

Create a deck containing an ordinary Basic note, a reverse-enabled Basic note,
a one-at-a-time cloze note, overlapping tags, and a managed image. Include typed
interactive questions when testing current format-3 behavior.

Export the deck, then verify:

1. the destination is one `.studydeck` SQLite file;
2. an existing destination is replaced only after the new file validates;
3. the original library can still edit and practice the deck;
4. importing creates one deck with matching content;
5. Basic, reverse, cloze and supported interactive presentation counts match;
6. tags and images render correctly;
7. imported schedules are fresh New schedules rather than donor review history;
8. importing again as a new user action creates an independent deck; and
9. retrying the same native operation ID does not create another deck.

Also import the supported package-version-1 fixture containing tags and historical
multi-row cloze cards. It should become one active cloze schedule.

## Failure and atomicity checks

With disposable packages or automated failure injection, verify damaged SQLite,
missing metadata, unsupported versions, invalid rich content/links/file signatures
are rejected before mutation. An image staging/rename failure must leave no
partial deck; a database failure cleans only that import's unreferenced draft.
Reusing an operation ID with different input returns `idempotency_conflict`, while
an ambiguous retry of the same committed operation returns its original result.

## Complete library backup

Populate the isolated library with decks, reusable templates, images, reviewed
cards, practice sessions, manual schedule actions and non-default settings. Include
a processed document, its guide/learner edits, a collection and Tutor history for
current full-library coverage. Existing completed document work can be reused;
there is no reason to repeat paid generation just to check persistence.

Open **Settings → Backup/Sync → Back up library** and save outside application
data. Inspect the selected package's embedded snapshot and managed-file inventory,
not only the UI summary. An empty destination that passes SQLite integrity is not
proof of a completed restore.

Transient asset work and active processing must block maintenance; supported
persisted editor drafts and their staged images must survive a complete backup.
Canceling the save dialog must not be reported as a saved external backup.

## Same-process restore

1. Back up the disposable library; record content, row identities, schedules,
   reviews, learner edits and managed-file hashes.
2. Make obvious disposable changes, then choose **Restore backup**.
3. Verify a whole-library destructive confirmation appears. Cancel once and
   confirm the library stays unchanged and the unneeded staged import is released.
4. Select the backup again and confirm **Restore library**.
5. Verify progress appears, library operations are fenced, and the interface
   refreshes with restored content **without restarting the native process**.
6. Verify completion counts include decks, cards and documents, and changes made
   after the backup are absent. Compare reviews/analytics, settings, drafts,
   documents/guides, question attempts and managed files against the backup.
7. Create a new card after restoration. Replaying the original completed restore
   receipt must not erase it or activate the package a second time.
8. Close and reopen solely as a persistence check; the post-restore card remains.

A document-only backup with zero decks is not empty. The saved source backup and
previous library must remain recoverable. Restoring must not delete OS-keyring
credentials; session-only credentials must not leak into the replacement library.
Android shared backups require the same explicit replacement confirmation.

## Interrupted activation and concurrency

Invalid input must leave the active library intact. Inject reconciliation, disk,
rename and commit failures. Exercise real committed WAL rows and process death
between the recorded rename/rollback/commit boundaries. Recovery itself must be
idempotent if interrupted. A lost reply after a durable commit replays a receipt,
not a destructive operation; indeterminate durability keeps commands closed.

Verify old-generation asynchronous requests cannot mutate the new library, active
sync workers retain maintenance exclusion until their IO cleanup ends, and a
second cooperating process cannot own the same library. A stale lock file after
process death must be reusable without deleting it. Close older binaries that do
not implement the ownership lock before testing.

## Historical Stage-3.5 repair fixtures

The following assertions apply only to explicitly constructed historical fixtures:
missing duplicate fingerprints are populated; legacy template patterns become rich
documents with canonical placeholders; multi-row cloze has one active forward card;
sibling rows remain soft-deleted so schedules/history survive; abandoned legacy
occlusion values become Basic content; the old template schema no longer exposes
`front_pattern`/`back_pattern`; unsupported legacy occlusion values and nullable
template documents are rejected; repair-ledger checksums stay stable. These are not
instructions to reject current native image-occlusion question definitions.

Repeated no-op initialization must not rewrite application identities, timestamps,
revisions or repair results merely because startup ran. Do not manually stamp a
schema version or erase migration rows on the workstation.

## Acceptance record

Record the exact source SHA and actual Static/Fast/Linux/package/Android results,
current/legacy transfer round trips, same-process restore, interrupted recovery,
post-restore write/reopen, and isolated retained-data preservation. Keep human and
physical-device acceptance separate from automated workflow success. Never merge
or trust the protected school library based only on a prepared restore message.
