# Decision 0008: Managed asset lifecycle, crash reconciliation, and deletion undo

## Status

Accepted for Stage 3.3 implementation.

## Context

Images currently become durable filesystem files and `assets` rows as soon as the editor imports them. A canceled editor can therefore leave an unowned asset. A crash between copy, rename, database insertion, note/template persistence, or deck import can also leave the SQLite ledger and managed asset directory disagreeing.

Card and deck deletion already uses soft tombstones, but the UI has no short recovery path for an accidental deletion.

## Decision

### Asset states

Every asset has one explicit lifecycle state:

- `staged`: copied into a managed staging path and owned by an editor/import draft;
- `ready`: promoted to its canonical managed path and referenced by durable content;
- `missing`: referenced by SQLite but absent from both managed paths;
- `quarantined`: an unreferenced or conflicting file retained outside the ready namespace for conservative recovery;
- `deleted`: unreferenced, past its recovery grace period, and no longer rendered.

The lifecycle ledger stores the final and staged relative paths, draft owner, timestamps, and reconciliation reason. Existing assets migrate as `ready`.

### Staged ingest

Image ingest is draft-scoped. It validates the source, copies to a unique managed temporary file, flushes the file, flushes its parent directory, and records the staged asset. The editor receives the staged asset for immediate local preview.

Card/template writes may reference `staged` assets only when the request carries the matching draft ID. Promotion is planned and recorded in the same SQLite transaction that writes the note or template. The filesystem rename occurs before commit, and the durable ledger makes every crash boundary recoverable:

- a database rollback after rename leaves a detectable unreferenced canonical file;
- a crash before rename leaves a detectable staged row and file;
- a crash after rename but before final state update is resolved from both paths and durable references.

After commit, the asset becomes `ready`, its draft ownership is cleared, and the draft is committed. Removing a staged image before save or canceling the editor abandons only unreferenced assets owned by that draft.

### Startup reconciliation

Startup reconciliation is conservative and idempotent. It:

1. repairs staged rows whose final rename completed;
2. promotes referenced staged assets when one valid staged file exists;
3. marks referenced assets missing instead of deleting metadata;
4. quarantines unreferenced canonical files and unknown files in managed directories;
5. removes expired, unreferenced staged assets;
6. marks unreferenced ready assets deleted only after the recovery grace period;
7. never deletes or quarantines a file that has a live note or template reference; and
8. records each repair decision for diagnostics.

Repeated reconciliation produces the same stable state and does not duplicate events.

### Transfers

Deck-package import uses the same staging and promotion primitives as editor ingest. Imported asset files are fully staged and flushed before the content transaction, and promoted with the imported notes. Import failure cleans only its own unreferenced draft.

Exports include only `ready` assets whose managed files pass size and signature validation.

### Deletion undo

Deleting one card, multiple selected cards, or a deck creates a deletion operation containing the exact rows tombstoned by that operation. The command returns a short-lived undo token. The toast displays an **Undo** action while that token is valid.

Undo restores only rows whose tombstone still matches the deletion operation, so it cannot overwrite a later independent change. Deck undo restores the deck and only the notes/cards newly tombstoned by that deck deletion. Card undo preserves tags, assets, schedules, and review history because deletion remains soft and relationship rows are retained.

Expired undo operations remain audit records but cannot be replayed.

## Compatibility

- Historical migrations remain immutable.
- Existing asset IDs and relative paths remain valid.
- Existing `.studydeck` packages remain importable.
- Existing note/template image references remain authoritative.
- Review history, schedule identity, tags, and shared asset references are preserved.

## Required validation

Tests cover copy, flush, rename, database, and post-rename failures; draft cancellation; removing an image before save; shared references; missing and orphan files; transfer staging; repeated reconciliation; referenced-file protection; exact card/deck restoration; expired/conflicting undo; and frontend toast action behavior.