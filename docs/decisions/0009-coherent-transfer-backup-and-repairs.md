# Decision 0009: Coherent studydeck transfer, complete backups, and versioned repairs

## Status

Accepted for Stages 3.4 and 3.5.

## Context

The released `.studydeck` format grew through three sequential writers: the core package, optional tags, and optional reverse/cloze metadata. Exporting each layer from a separate query window could combine different library states, and importing each extension after the core transaction could leave a committed partial deck. Startup also ran duplicate-hash and Stage 3 normalization across the complete library on every launch, even after all rows were canonical.

A content-sharing package and a disaster-recovery backup have different trust and identity requirements. A shared deck must not silently import another learner's schedules or review history. A complete local backup must restore every deck, template, schedule, review event, practice session, analytics source record, image, and setting exactly.

## Decision

### Current `.studydeck` format

New deck exports write package version 2 from one SQLite read transaction. Core notes, generated-card identities, tags, tag links, image links, and card-type metadata are captured from that same snapshot. Ready managed image files are immutable and are validated against snapshot metadata before the package transaction commits.

The package is written to a temporary SQLite file, checked with `PRAGMA integrity_check`, flushed, and atomically moved to the requested destination.

Deck packages remain content-sharing files:

- schedules and review events are not exported;
- imported cards receive fresh local schedules;
- deck names are preserved without implementation suffixes;
- internal ID collisions are remapped;
- tag names merge by canonical normalized name; and
- reusable templates are not implied by generated cards and are therefore not attached to a single-deck package.

### Legacy package adapter

Package version 1 remains readable through one explicit compatibility reader. It recognizes the released core tables and the optional tag and card-type tables, validates the complete graph, converts historical multi-row cloze presentations to one canonical forward card in memory, and then uses the same version-2 import transaction.

No current export writes version 1. The old layered runtime writers and post-import wrapper transactions are removed.

### Retry-safe import

Every import carries a stable operation ID and a SHA-256 package fingerprint. The operation ledger is written before asset staging. The complete deck, tags, notes, generated cards, fresh schedules, asset links, asset promotion, and committed result are then recorded in one immediate transaction.

A repeated operation ID with the same package returns the original committed result. Reusing it for another package returns `idempotency_conflict`. An interrupted pending operation may clean its own unreferenced draft and retry. It cannot create a second deck after an ambiguous successful response.

### Complete library backup

A separate `.ostudybackup` file contains:

- one SQLite online-backup snapshot of the complete library database; and
- every `ready` managed image with its exact asset identity, path, type, and size.

Because analytics is derived from review events, schedules, practice sessions, tags, and current card state, the database snapshot is also the authoritative analytics backup. No derived chart cache or second analytics format is introduced.

The backup command refuses to run while an editor or import owns an active asset draft. It validates the SQLite snapshot and every ready image before atomically replacing the destination file.

Restore is prepared while ostudy is running and applied before SQLite opens on the next application launch. The current database and asset directory are renamed to rollback paths, the validated restored pair is installed, and a guard restores the previous pair automatically if opening or migrating the restored database fails. The rollback copies are removed only after native initialization succeeds.

### Versioned repairs and canonical startup

Structural migrations remain immutable. Migration 12 introduces a separate `data_repairs` ledger. Repairs use the same validated Rust transformations as authoring and record their name, checksum, timestamp, and changed-row count in the transaction that performs the repair.

The initial repairs:

1. fill missing duplicate fingerprints;
2. convert legacy template patterns and explicit placeholders to canonical rich documents;
3. normalize historical cloze sibling rows to one active forward identity;
4. convert the abandoned image-occlusion enum values to ordinary Basic content while preserving the rich question and answer; and
5. retire extra generated rows without deleting their schedules or review history.

Migration 13 then removes the legacy text template columns and installs canonical guards that reject abandoned note types, card directions, and nullable rich template layouts.

After all migration and repair ledger rows are present, normal startup verifies them but performs no normalization writes. Startup does not update user timestamps, revisions, FTS rows, or the WAL merely because the application opened.

## Compatibility classification

The following remain because they represent released data or external formats:

- migrations 1 through 11 and released database fixtures;
- package-version-1 reading;
- stable soft-deleted generated-card IDs that retain schedules and review history; and
- data-only template placeholder conversion in the one-time repair.

The following are removed as implementation residue:

- the layered core/tag/card-type export writers;
- post-core tag and card-type import transactions;
- Stage-named transfer command wrappers;
- the startup-wide duplicate backfill;
- the startup-wide template/cloze normalizer;
- legacy `front_pattern` and `back_pattern` columns in the active schema; and
- active acceptance of `image_occlusion` note/card enum values.

## Consequences

A complete backup may be substantially larger than a shared deck because it includes the database history and all ready images. Restoring intentionally replaces the current local library and requires closing and reopening ostudy. Single-deck packages remain suitable for sharing because they do not transfer learner history.

Future data changes must add a migration for structure, a repair version for validated row transformation, or both. They must not reintroduce unconditional startup rewrites.