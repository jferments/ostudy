# Decision 0008: Managed asset lifecycle, crash reconciliation, and deletion undo

## Status

Accepted for Stage 3.3 implementation.

## Context

Images previously became durable filesystem files and `assets` rows as soon as the editor imported them. A canceled editor could therefore leave an unowned asset. A crash between copy, rename, database insertion, note/template persistence, or deck import could also leave the SQLite ledger and managed asset directory disagreeing.

Card and deck deletion used soft tombstones, but the UI had no short recovery path for an accidental deletion.

## Decision

### Asset states

Every editor-managed asset has one explicit lifecycle state:

- `staged`: copied into a managed staging path and owned by an editor draft;
- `ready`: promoted to its canonical managed path and referenced by durable content;
- `missing`: referenced by SQLite but absent or invalid at its managed path;
- `quarantined`: an unreferenced or conflicting file retained outside the ready namespace for conservative recovery;
- `deleted`: unreferenced, past its recovery grace period, and no longer rendered.

The lifecycle ledger stores final and staged relative paths, draft owner, timestamps, and reconciliation reason. Existing assets migrate as `ready`.

### Staged editor ingest

Image ingest is draft-scoped. It validates the source, copies to a unique managed temporary file, flushes the file, flushes its parent directory, and records the staged asset. The editor receives the staged asset for immediate local preview.

Card writes reference `staged` assets only when the request carries the matching draft ID. Promotion is recorded in the card write transaction. The filesystem rename occurs before commit, and ordinary failures compensate completed renames immediately. The durable ledger makes process-interruption boundaries recoverable:

- a database rollback after rename leaves a detectable unreferenced canonical file;
- a crash before rename leaves a detectable staged row and file;
- a crash after rename but before final state update is resolved from both paths and durable references.

Some historical template helpers own and commit their content transaction internally. Their asset finalization therefore runs as a separately journaled post-commit step. Failure in that step is logged and reconciled instead of incorrectly reporting that the already-durable template or generated cards failed to save.

After successful promotion, the asset becomes `ready`, its draft ownership is cleared, and the draft is committed. Removing a staged image before save or canceling the editor abandons only unreferenced assets owned by that draft.

### Startup reconciliation

Startup reconciliation is conservative and idempotent. It:

1. repairs staged rows whose final rename completed;
2. promotes referenced staged assets when one valid staged file exists;
3. marks referenced assets missing when their file is absent or fails validation, without deleting or quarantining the referenced file;
4. quarantines unreferenced canonical files and unknown files in managed directories;
5. removes expired, unreferenced staged assets;
6. marks unreferenced ready assets deleted only after the recovery grace period;
7. never deletes or quarantines a file that has a live note or template reference; and
8. records each repair decision for diagnostics.

Repeated reconciliation produces the same stable state and does not duplicate repair events.

### Transfers

Deck-package import retains the established package compatibility path: each image is written to a unique temporary file, flushed, and renamed into the managed namespace around the package database transaction. Stage 3.3 adds a durable import-operation journal and runs the same conservative reconciliation policy after success or failure. An interrupted import therefore leaves identifiable, recoverable state rather than an anonymous filesystem orphan.

A package transaction that has already committed remains a successful import even if subsequent journal finalization or display refresh fails; the failure is logged and startup reconciliation completes the remaining asset bookkeeping. Existing `.studydeck` formats are unchanged.

Exports include only durable assets referenced by the deck and validate their managed files before copying them into the package.

### Deletion undo

Deleting one card, multiple selected cards, or a deck creates a deletion operation containing the exact rows tombstoned by that operation. The command returns a short-lived undo token. The toast displays an **Undo** action while that token is valid.

Undo restores only rows whose tombstone still matches the deletion operation, so it cannot overwrite a later independent change. Deck undo restores the deck and only the notes/cards newly tombstoned by that deck deletion. Card undo preserves tags, assets, schedules, and review history because deletion remains soft and relationship rows are retained during the recovery window.

Expired undo operations remain audit records but cannot be replayed. Relationship cleanup occurs only after the recovery window expires.

## Compatibility

- Historical migrations remain immutable.
- Existing asset IDs and relative paths remain valid.
- Existing `.studydeck` packages remain importable.
- Existing note/template image references remain authoritative.
- Review history, schedule identity, tags, and shared asset references are preserved.

## Required validation

Tests cover copy, flush, rename, database, ordinary partial-promotion compensation, and post-rename crash failures; draft cancellation; removing an image before save; shared references; missing, corrupt, and orphan files; import journaling; repeated reconciliation; referenced-file protection; exact card/deck restoration; expired/conflicting undo; and frontend toast action behavior.