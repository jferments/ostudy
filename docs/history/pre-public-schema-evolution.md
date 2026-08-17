# Pre-public database schema evolution

Before ostudy's first public release, the local schema evolved through 26 development migrations. No public installation depended on those intermediate states, so PR #54 folded their final result into `0001_public_baseline.sql` and began the supported migration lineage at `ostudy-public-v1`.

The original SQL remains permanently recoverable from Git history at cutoff commit `61b50d91170ce8bd00653af9f7e5f1cec686341f`, the `main` head immediately after PR #52 and before the public-baseline branch began. It is not copied into a runtime archive because duplicate migration trees invite accidental registration and maintenance drift.

## Durable architectural eras

| Era | Durable result preserved in the public baseline |
| --- | --- |
| Initial study library | Decks, cards, settings, practice sessions, and managed images |
| Scheduling and review history | FSRS schedules, immutable review events, workload controls, suspend/bury/reset semantics |
| Organization and authoring | Tags, search/FTS, card types, cloze cards, rich templates, duplicate fingerprints |
| Transaction boundaries | Operation identities, idempotent writes, transactional schedule actions, deletion undo |
| Canonical ownership and analytics | Card-owned review history, stable review attribution, bounded projections |
| AI provider platform | Provider profiles, OS-backed credentials, model discovery, capability probes |
| Document intelligence | Managed sources, OCR metadata, semantic windows, resumable generation, usage calibration |
| Platform storage | Opaque external transfers, durable editor drafts, deck folders, library ordering |

## Intentionally retired transition mechanisms

- table-rebuild and backfill steps whose result is already represented by the baseline;
- pre-release migration-number special cases;
- historical data repairs for synthetic development data;
- compatibility with developer-only schema versions 1 through 26.

A database without the `ostudy-public-v1` lineage marker is never silently migrated, deleted, or reset. Lineage inspection occurs before write-capable SQLite durability configuration; the on-disk regression requires a rejected legacy database to remain byte-for-byte identical and to create no WAL/shared-memory sidecars. Native startup converts the expected compatibility condition into a visible **One-time data reset required** dialog and the structured `application_startup_blocked` diagnostic rather than allowing the Tauri setup hook to panic. `scripts/reset-pre-public-data.sh` remains the explicit transition: it moves the complete old data directory to a timestamped sibling backup before creating an empty directory.

The full Linux qualification gate launches the exact built binary against a synthetic pre-public on-disk database. That smoke must remain alive behind the reset dialog, emit the expected structured reason code, avoid `panic_captured`, and leave the seeded database unchanged before the candidate can qualify.

## Policy after public release

Every public migration is immutable, checksum-protected, append-only, and tested from the immediately previous public schema. Data repairs remain independently versioned and transactional.
