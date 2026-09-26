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
- pre-release migration-number special cases in the normal public migration runner;
- historical data repairs whose effects are already present in the final schema;
- general compatibility with arbitrary developer-only schema versions 1 through 25.

## Final-v26 recovery and runtime retirement

The exact final development lineage remains historically recoverable because it can contain old decks, cards, review history, analytics attribution, documents, settings, and managed files. Its immutable contract now lives under `scripts/history/pre-public-v26/ledger.json`.

During pre-release development, normal startup temporarily auto-adopted an exact v26 database. PR #106 retired that runtime path after the local human gate established that the retained production library was already `ostudy-public-v1` schema 30 with valid integrity/foreign keys and that no development-profile library required v26 adoption.

Current application startup no longer contains the v26 converter. A non-public database is left unchanged and startup blocks with manual-recovery guidance.

Historical recovery remains explicit and tested:

```bash
bash ./scripts/transition-pre-public-data.sh \
  --data-dir "/absolute/path/to/active-data" \
  --confirm
```

The helper creates/uses an immutable v26 source archive, converts only a staging copy to public baseline migration 1, verifies learner-data preservation and SQLite integrity, archives the previous active directory, and installs the converted copy. Current oStudy then advances that public-v1 copy through the ordinary immutable migration chain.

For a separate archived v26 directory, supply `--source-backup "/absolute/path/to/archive"`. The archived source is copied before SQLite opens anything and remains untouched.

The full Linux gate verifies that the current runtime refuses v26 without modifying it, the explicit recovery path succeeds on an isolated copy, and the recovered copy subsequently reaches the current public schema with representative learner data preserved.

## Policy after public release

Every public migration is immutable, checksum-protected, append-only, and tested from the immediately previous public schema. Data repairs remain independently versioned and transactional.