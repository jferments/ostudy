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

## Final-v26 adoption

The exact final development lineage remains valuable because it contains the developer's real decks, cards, review history, analytics attribution, documents, settings, and managed files. It is therefore a narrow, one-time adoption input rather than an unsupported database to discard.

The immutable contract is stored in `src-tauri/src/startup/pre_public_v26_ledger.json` and contains all 26 migration identities/checksums and all 3 data-repair identities/checksums. Rust startup, the archived-data recovery helper, and the built-binary test fixture consume that same file.

When startup sees a database without `ostudy-public-v1`, it first inspects lineage before enabling write-capable SQLite durability settings. An exact final-v26 database is handled as follows:

1. verify the embedded migration and repair ledgers, expected terminal schema, `user_version`, foreign keys, and SQLite integrity;
2. create a consistent SQLite backup named like `library.sqlite3.pre-public-v26-<timestamp>.backup`;
3. transactionally rebuild only the AI provider-profile table required for the public OpenRouter shape, add `schema_metadata`, replace the development ledgers with public baseline migration 1, and set `user_version` to 1;
4. compare all application-table row counts plus the identities of decks, notes, cards, review events, review attributions, and AI profiles;
5. reopen through the normal public migration verifier before completing startup.

If any precondition or preservation check fails, the transaction rolls back and the backup remains available. Older, partial, checksum-mismatched, or damaged development databases are left unchanged and produce a **Pre-public data needs manual recovery** dialog instead of a Tauri setup panic.

`scripts/restore_pre_public_backup.py` supports users who already ran the earlier directory-reset command. It validates an archived exact-v26 directory, copies it back into the active location, archives the currently active directory, and keeps the original backup untouched. The next normal startup performs the verified adoption above.

`scripts/reset-pre-public-data.sh` is retained only for intentionally starting a blank library. It requires the explicit flag `--confirm-empty-library`; the former ambiguous `--confirm` option is rejected.

The full Linux qualification gate launches the exact built binary against a seeded final-v26 database. The application must create the consistent backup, preserve deck/card/review/analytics rows, adopt the public lineage, emit `pre_public_database_adoption_completed` and `application_started`, remain usable, and avoid both `application_startup_blocked` and panic paths.

## Policy after public release

Every public migration is immutable, checksum-protected, append-only, and tested from the immediately previous public schema. Data repairs remain independently versioned and transactional.