# Pre-public database schema evolution

Before ostudy's first public release, the local schema evolved through 26 development migrations. No public installation depended on those intermediate states, so PR #54 folded their final result into `0001_public_baseline.sql` and began the supported migration lineage at `ostudy-public-v1`.

The original SQL remains permanently recoverable from Git history immediately before this baseline commit. It is not copied into a runtime archive because duplicate migration trees invite accidental registration and maintenance drift.

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

A database without the `ostudy-public-v1` lineage marker is never modified or deleted. Startup returns an explicit pre-public-reset error, and `scripts/reset-pre-public-data.sh` moves the complete old data directory to a timestamped sibling backup before creating an empty directory.

## Policy after public release

Every public migration is immutable, checksum-protected, append-only, and tested from the immediately previous public schema. Data repairs remain independently versioned and transactional.
