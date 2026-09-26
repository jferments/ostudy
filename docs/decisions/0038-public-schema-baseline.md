# ADR 0038: Public and release database baselines

## Decision

oStudy now has two explicit SQLite lineages with different responsibilities.

The historical public compatibility lineage is `ostudy-public-v1`. Its immutable
migrations `0001_public_baseline.sql` through `0030_generation_operation_outcomes.sql`
and repair records 1–3 remain supported input contracts. They are not the normal
fresh-install path for the 0.1 release.

The 0.1 runtime lineage is `ostudy-release-v1`. Its immutable **baseline is
schema 1**. Fresh libraries execute that canonical baseline first:

```text
src-tauri/schema/0001_release_baseline.sql
```

That file is generated from the exact qualified result of public-v1 schema 30 plus
the required legacy repairs. Its canonical SHA-256 is:

```text
64553c98070ad58a0aa73563cf7e9972e9dcb6a27666895a78d8a53882fc26e8
```

The baseline ledger starts with one row named `release_baseline`; the release
repair ledger starts empty. Forward release migrations append from this baseline
rather than replaying the thirty pre-release migrations on every new library.

The first forward release migration is schema **2**,
`selection_deck_operations`. It adds only the durable replay receipt for the
atomic **Save selection as deck** mutation. Current libraries therefore open as
`ostudy-release-v1` schema 2, while schema-1 release libraries and backups remain
supported upgrade inputs.

## Existing public-v1 libraries

An existing `ostudy-public-v1` library remains a supported compatibility input.
Normal startup:

1. validates its exact known migration prefix without writing;
2. validates every already-recorded legacy repair before applying any missing
   prerequisite repair;
3. migrates only through the immutable public-v1 registry until schema 30;
4. applies and verifies legacy repairs 1–3;
5. proves that the resulting SQLite schema matches the canonical release
   baseline;
6. creates an isolated SQLite Backup API copy;
7. converts that isolated candidate to release-v1 metadata;
8. compares a typed preservation receipt for every non-ledger SQLite value; and
9. only after that proof, atomically replaces the live lineage, migration ledger,
   repair ledger, and `PRAGMA user_version`.

The lineage collapse does **not** rebuild learner-owned tables merely to make the
migration history shorter. Decks, notes, cards, schedules, reviews, documents,
study guides, Tutor history, provider metadata, and other retained application
rows stay in place. Managed document/image files are not rewritten by the schema
lineage conversion.

The live metadata transaction changes only:

```text
schema_metadata.schema_lineage
schema_migrations
data_repairs
PRAGMA user_version
```

After preservation-proven lineage conversion, the schema-1 baseline is upgraded
through the ordinary release migration registry to the current release schema.
Reopening an already-current release library does not replay completed migrations.

## Backup compatibility

Whole-library backup remains distinct from runtime schema initialization.

Current backups may contain `ostudy-release-v1` / schema 1. Supported historical
whole-library backups may contain `ostudy-public-v1` schemas 10–30. Historical
public-v1 inputs are validated with their immutable migration and repair evidence,
then normalized and converted through the same release boundary during restore.

No backup reader is allowed to treat an arbitrary schema number or relabelled
SQLite file as a supported library.

## Pre-public recovery

The exact final development lineage from before the public baseline remains
recoverable, but automatic pre-public adoption is not part of ordinary runtime
startup.

The archived explicit recovery utility:

1. verifies the complete embedded final-v26 contract;
2. runs SQLite integrity and foreign-key checks;
3. creates a consistent staging backup;
4. converts only a staging copy into the historical public-v1 baseline;
5. verifies retained identities/counts; and
6. lets normal public-v1 compatibility handling take over afterward.

Older, partial, checksum-mismatched, damaged, or otherwise unrecognized
development databases are not guessed at or silently reset.

`scripts/reset-pre-public-data.sh` remains an explicit empty-library recovery
operation. It requires `--confirm-empty-library` and archives rather than silently
deleting the previous directory.

## Immutability

Released compatibility inputs are evidence.

- Do not rewrite public-v1 migration bytes or checksums.
- Do not rewrite historical repair contracts merely to match a new implementation.
- Do not manually stamp `PRAGMA user_version` on a learner library.
- Do not delete migration/repair ledgers on the workstation as a migration shortcut.
- Do not remove historical readers while supported backups still depend on them.

Future release schema changes are ordinary new release migrations/repairs from the
release-v1 baseline unless another explicitly reviewed baseline consolidation is
later justified and independently proven.
