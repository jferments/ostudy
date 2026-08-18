# ADR 0038: Public database schema baseline

## Decision

The first supported public schema lineage is `ostudy-public-v1`, represented by the immutable migration `0001_public_baseline.sql`. The final pre-release schema was built from all development migrations, validated, converted into canonical DDL plus seed rows, and checked against a committed semantic schema manifest.

The exact final development lineage—SQLite `user_version` 26 with the immutable 26-migration and 3-repair ledgers—is adopted automatically rather than discarded. Before changing that database, ostudy:

1. verifies the complete embedded pre-public lineage contract;
2. runs SQLite integrity and foreign-key checks;
3. creates a consistent SQLite backup in the application-data directory;
4. transactionally changes only the schema elements that differ from the public baseline;
5. replaces the development ledgers with `ostudy-public-v1` / migration 1;
6. verifies all application-table row counts and critical deck, note, card, review, attribution, and AI-profile identities are unchanged;
7. reopens the database through the normal public migration verifier.

Older, partial, checksum-mismatched, damaged, or otherwise unrecognized development databases are not guessed at and are not reset automatically. They remain unchanged and produce a manual-recovery dialog. An archived exact-v26 application-data directory can be copied back non-destructively with `scripts/restore_pre_public_backup.py`; the existing active directory and the original archive are both retained.

`scripts/reset-pre-public-data.sh` is no longer the normal transition. It is an explicit empty-library operation and requires `--confirm-empty-library`, wording that makes the loss of active visibility for decks, cards, reviews, analytics, documents, and settings unmistakable. It still archives rather than deletes the previous directory.

Historical repair results are folded into the baseline. The independent repair framework remains with an empty public registry, ready for future versioned repairs. Future public migrations begin at version 2 and are append-only and checksum-protected.