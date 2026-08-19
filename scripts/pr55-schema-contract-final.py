#!/usr/bin/env python3
from __future__ import annotations

import pathlib

ROOT = pathlib.Path(__file__).resolve().parents[1]
SELF = pathlib.Path(__file__).resolve()


def replace_once(path: pathlib.Path, old: str, new: str, label: str) -> None:
    source = path.read_text()
    if old in source:
        if source.count(old) != 1:
            raise RuntimeError(f"{label}: ambiguous replacement anchor")
        path.write_text(source.replace(old, new, 1))
    elif new not in source:
        raise RuntimeError(f"{label}: replacement anchor missing")


# The committed public-schema fixture must track the latest migration rather
# than permanently assuming the original v1 baseline.
replace_once(
    ROOT / "scripts/verify-project-manifests.mjs",
    'equal("public schema fixture version", publicBaselineManifest.schemaVersion, 1);',
    'equal("public schema fixture version", publicBaselineManifest.schemaVersion, latest);',
    "scripts/verify-project-manifests.mjs public schema fixture version contract",
)

# Stage 3.6-3.8 regression coverage still needs to prove that the original
# baseline migration is present, but the ledger now legitimately contains one
# entry per public migration. Compare the count to the canonical latest schema
# version instead of hard-coding the historical one-migration state.
replace_once(
    ROOT / "src-tauri/src/stage36_38_regression_tests.rs",
    "            assert_eq!(migration_count, 1);",
    "            assert_eq!(migration_count, LATEST_SCHEMA_VERSION);",
    "stage36_38_regression_tests migration-count contract",
)

# These startup tests synthesize an exact final-v26 database by opening a
# current public database and rewinding it. Once migration v2 exists, rewinding
# must also remove every v2-only table; otherwise the fixture claims user_version
# 1 while still containing v2 tables and the legitimate v2 migration collides.
startup_tests = ROOT / "src-tauri/src/startup/pre_public_transition/tests.rs"
replace_once(
    startup_tests,
    '''CREATE INDEX ai_provider_profiles_active_updated_idx
    ON ai_provider_profiles(updated_at DESC) WHERE deleted_at IS NULL;
DROP TABLE schema_metadata;''',
    '''CREATE INDEX ai_provider_profiles_active_updated_idx
    ON ai_provider_profiles(updated_at DESC) WHERE deleted_at IS NULL;

DROP TABLE IF EXISTS document_learning_target_coverage;
DROP TABLE IF EXISTS document_learning_targets;
DROP TABLE IF EXISTS ai_task_policies;
DROP TABLE IF EXISTS ai_prompt_presets;
DROP TABLE schema_metadata;''',
    "pre-public test legacy-schema rewind",
)

# After adoption, reopening the database correctly applies migrations newer
# than the public baseline. Verify every legacy table count and critical learner
# identity survived, while allowing newly introduced migration tables to exist.
replace_once(
    startup_tests,
    '''            assert_eq!(
                preserved_data_snapshot(connection).unwrap(),
                before,
                "all learner-table counts and critical entity identities must survive"
            );''',
    '''            let after = preserved_data_snapshot(connection).unwrap();
            for (table, expected_count) in &before.table_counts {
                assert_eq!(
                    after.table_counts.get(table),
                    Some(expected_count),
                    "legacy table {table} must preserve its row count"
                );
            }
            assert_eq!(after.deck_ids, before.deck_ids);
            assert_eq!(after.note_ids, before.note_ids);
            assert_eq!(after.card_ids, before.card_ids);
            assert_eq!(after.review_ids, before.review_ids);
            assert_eq!(after.review_attribution_ids, before.review_attribution_ids);
            assert_eq!(after.ai_profile_ids, before.ai_profile_ids);''',
    "pre-public test learner-data preservation assertion",
)
replace_once(
    startup_tests,
    '                    "SELECT version, name, checksum FROM schema_migrations",',
    '                    "SELECT version, name, checksum FROM schema_migrations WHERE version = 1",',
    "pre-public test baseline-ledger query",
)

# The built-binary startup smoke has the same invariant: adoption must preserve
# the v1 baseline ledger entry, but a current application is expected to apply
# all later migrations before startup completes. Query the baseline row directly
# instead of requiring it to be the only row in the ledger.
replace_once(
    ROOT / "scripts/smoke-startup-compatibility.sh",
    '''migration="$(sqlite3 -separator '|' "$DATABASE" 'SELECT version, name, checksum FROM schema_migrations;')"''',
    '''migration="$(sqlite3 -separator '|' "$DATABASE" 'SELECT version, name, checksum FROM schema_migrations WHERE version = 1;')"''',
    "startup compatibility baseline-ledger query",
)

SELF.unlink()
