use std::{
    fs,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};

use rusqlite::{params, Connection, TransactionBehavior};

use super::*;
use crate::database::{Database, LATEST_SCHEMA_VERSION};

static NEXT_TEMP_ID: AtomicU64 = AtomicU64::new(1);

fn temp_database_path(name: &str) -> PathBuf {
    let id = NEXT_TEMP_ID.fetch_add(1, Ordering::Relaxed);
    let directory = std::env::temp_dir().join(format!("ostudy-{name}-{}-{id}", std::process::id()));
    fs::create_dir_all(&directory).expect("temporary database directory should exist");
    directory.join("library.sqlite3")
}

fn open_public_v1_fixture(path: &std::path::Path) -> Connection {
    let connection = Connection::open(path).expect("public-v1 fixture should open");
    connection
        .execute_batch(include_str!("../../../migrations/0001_public_baseline.sql"))
        .expect("immutable public-v1 schema should load");
    connection
        .execute(
            "INSERT OR REPLACE INTO schema_metadata(key, value)
             VALUES ('schema_lineage', ?1)",
            [PUBLIC_SCHEMA_LINEAGE],
        )
        .expect("public lineage should be recorded");
    connection
        .execute(
            "INSERT INTO schema_migrations(version, name, checksum, applied_at)
             VALUES (?1, ?2, ?3, 0)",
            params![
                PUBLIC_BASELINE_VERSION,
                PUBLIC_BASELINE_NAME,
                PUBLIC_BASELINE_CHECKSUM,
            ],
        )
        .expect("public baseline migration should be recorded");
    connection
        .pragma_update(None, "user_version", PUBLIC_BASELINE_VERSION)
        .expect("public-v1 SQLite marker should be set");
    connection
}

fn assert_preexisting_data_survives_schema_additions(
    before: &PreservedDataSnapshot,
    after: &PreservedDataSnapshot,
) {
    for (table, count) in &before.table_counts {
        assert_eq!(
            after.count(table),
            *count,
            "pre-existing table {table} must preserve its row count"
        );
    }
    assert_eq!(after.deck_ids, before.deck_ids);
    assert_eq!(after.note_ids, before.note_ids);
    assert_eq!(after.card_ids, before.card_ids);
    assert_eq!(after.review_ids, before.review_ids);
    assert_eq!(after.review_attribution_ids, before.review_attribution_ids);
    assert_eq!(after.ai_profile_ids, before.ai_profile_ids);
}

fn seed_library(connection: &Connection) {
    connection
        .execute_batch(
            r#"
INSERT INTO decks (
    id, name, description, sort_order, created_at, updated_at,
    archived_at, deleted_at, revision, folder_id
) VALUES (
    '00000000000000000000000000000001', 'Preserved deck', '', 0,
    1000, 1000, NULL, NULL, 1, NULL
);

INSERT INTO notes (
    id, deck_id, note_type, content_format, content_schema_version,
    front_doc, back_doc, front_text, back_text, created_at, updated_at,
    deleted_at, revision, duplicate_hash, reverse_enabled, cloze_source, cloze_mode
) VALUES (
    '00000000000000000000000000000002',
    '00000000000000000000000000000001',
    'basic', 'tiptap-json', 1,
    '{"type":"doc","content":[{"type":"paragraph","content":[{"type":"text","text":"Question"}]}]}',
    '{"type":"doc","content":[{"type":"paragraph","content":[{"type":"text","text":"Answer"}]}]}',
    'Question', 'Answer', 1000, 1000, NULL, 1, '', 0, '', 'one_at_a_time'
);

INSERT INTO cards (
    id, note_id, deck_id, direction, template_ordinal,
    created_at, updated_at, suspended_at, deleted_at,
    buried_until, archive_suspended_at
) VALUES (
    '00000000000000000000000000000003',
    '00000000000000000000000000000002',
    '00000000000000000000000000000001',
    'forward', 0, 1000, 1000, NULL, NULL, NULL, NULL
);

INSERT INTO card_schedules (
    card_id, state, due_at, stability, difficulty, elapsed_days,
    scheduled_days, reps, lapses, last_review_at,
    scheduler_name, scheduler_version, updated_at, revision
) VALUES (
    '00000000000000000000000000000003',
    2, 2000, 1.0, 5.0, 1, 1, 1, 0, 1000,
    'fsrs', 'rs-fsrs-1.2.1', 1000, 1
);

INSERT INTO review_events (
    id, submission_id, session_id, card_id, reviewed_at, rating,
    answer_correct, confidence, response_time_ms, state_before, state_after,
    due_before, due_after, stability_before, stability_after,
    difficulty_before, difficulty_after, scheduled_days_before,
    scheduled_days_after, scheduler_name, scheduler_version, created_at
) VALUES (
    '00000000000000000000000000000004',
    '00000000000000000000000000000005',
    NULL,
    '00000000000000000000000000000003',
    1000, 3, 1, 'somewhat_confident', 1200, 1, 2,
    1000, 2000, 0.5, 1.0, 5.0, 5.0, 0, 1,
    'fsrs', 'rs-fsrs-1.2.1', 1000
);

INSERT INTO review_event_attributions (
    review_event_id, reviewed_at, review_local_date, time_zone_id,
    utc_offset_minutes, card_id, note_id, note_revision,
    owner_deck_id_at_review, captured_deck_name, direction, card_type,
    card_label, presentation_variant, question_excerpt, question_text_hash,
    question_was_truncated, attribution_source
) VALUES (
    '00000000000000000000000000000004', 1000, '2026-08-17', 'UTC', 0,
    '00000000000000000000000000000003',
    '00000000000000000000000000000002', 1,
    '00000000000000000000000000000001', 'Preserved deck',
    'forward', 'basic', 'Basic', '', 'Question',
    '0000000000000000000000000000000000000000000000000000000000000000',
    0, 'captured'
);
"#,
        )
        .expect("library fixture should be seeded");
}

fn convert_public_database_to_final_pre_public(
    connection: &mut Connection,
    sqlite_user_version: i64,
) {
    let contract = pre_public_contract().expect("embedded contract should parse");
    connection
        .execute_batch("PRAGMA foreign_keys = OFF;")
        .expect("foreign keys should disable");
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .expect("legacy conversion should start");
    transaction
        .execute_batch(
            r#"
CREATE TABLE ai_provider_profiles_legacy (
    id                      TEXT PRIMARY KEY CHECK(length(id) = 32),
    display_name            TEXT NOT NULL CHECK(length(trim(display_name)) BETWEEN 1 AND 100),
    normalized_name         TEXT NOT NULL CHECK(length(normalized_name) BETWEEN 1 AND 100),
    provider_kind           TEXT NOT NULL
                            CHECK(provider_kind IN (
                                'llama_cpp', 'deepseek', 'ollama',
                                'openai', 'gemini', 'claude'
                            )),
    base_url                TEXT NOT NULL CHECK(length(base_url) BETWEEN 1 AND 2048),
    model_id                TEXT NOT NULL DEFAULT '' CHECK(length(model_id) <= 300),
    auth_mode               TEXT NOT NULL CHECK(auth_mode IN ('none', 'bearer')),
    enabled                 INTEGER NOT NULL DEFAULT 1 CHECK(enabled IN (0, 1)),
    allow_insecure_http     INTEGER NOT NULL DEFAULT 0 CHECK(allow_insecure_http IN (0, 1)),
    request_timeout_ms      INTEGER NOT NULL DEFAULT 60000
                            CHECK(request_timeout_ms BETWEEN 1000 AND 300000),
    credential_storage      TEXT NOT NULL DEFAULT 'none'
                            CHECK(credential_storage IN ('none', 'secure', 'session')),
    credential_epoch        INTEGER NOT NULL DEFAULT 0 CHECK(credential_epoch >= 0),
    revision                INTEGER NOT NULL DEFAULT 1 CHECK(revision >= 1),
    created_at              INTEGER NOT NULL,
    updated_at              INTEGER NOT NULL,
    deleted_at              INTEGER
) STRICT;

INSERT INTO ai_provider_profiles_legacy (
    id, display_name, normalized_name, provider_kind, base_url, model_id,
    auth_mode, enabled, allow_insecure_http, request_timeout_ms,
    credential_storage, credential_epoch, revision, created_at, updated_at, deleted_at
)
SELECT
    id, display_name, normalized_name, provider_kind, base_url, model_id,
    auth_mode, enabled, allow_insecure_http, request_timeout_ms,
    credential_storage, credential_epoch, revision, created_at, updated_at, deleted_at
FROM ai_provider_profiles;

DROP TABLE ai_provider_profiles;
ALTER TABLE ai_provider_profiles_legacy RENAME TO ai_provider_profiles;
CREATE UNIQUE INDEX ai_provider_profiles_active_name_idx
    ON ai_provider_profiles(normalized_name) WHERE deleted_at IS NULL;
CREATE INDEX ai_provider_profiles_active_updated_idx
    ON ai_provider_profiles(updated_at DESC) WHERE deleted_at IS NULL;
DROP TABLE schema_metadata;
DELETE FROM schema_migrations;
DELETE FROM data_repairs;
"#,
        )
        .expect("legacy provider schema should be restored");

    for entry in &contract.migrations {
        transaction
            .execute(
                "INSERT INTO schema_migrations(version, name, checksum, applied_at)
                 VALUES (?1, ?2, ?3, 0)",
                params![entry.version, entry.name, entry.checksum],
            )
            .expect("legacy migration should be recorded");
    }
    for entry in &contract.repairs {
        transaction
            .execute(
                "INSERT INTO data_repairs(version, name, checksum, applied_at, changed_rows)
                 VALUES (?1, ?2, ?3, 0, 0)",
                params![entry.version, entry.name, entry.checksum],
            )
            .expect("legacy repair should be recorded");
    }
    transaction
        .execute_batch(&format!("PRAGMA user_version = {sqlite_user_version};"))
        .expect("legacy user_version should be set");
    transaction
        .commit()
        .expect("legacy conversion should commit");
    connection
        .execute_batch("PRAGMA foreign_keys = ON;")
        .expect("foreign keys should re-enable");
}

#[test]
fn historical_zero_marker_preserves_library_and_analytics() {
    let path = temp_database_path("pre-public-adoption");
    let mut connection = open_public_v1_fixture(&path);
    seed_library(&connection);
    convert_public_database_to_final_pre_public(&mut connection, 0);
    let before = preserved_data_snapshot(&connection).expect("legacy snapshot should load");
    assert_eq!(read_sqlite_user_version(&connection).unwrap(), 0);
    drop(connection);

    let report = adopt_final_pre_public_database(&path)
        .expect("adoption should succeed")
        .expect("exact v26 database should be adoptable");
    assert_eq!(report.deck_count, 1);
    assert_eq!(report.card_count, 1);
    assert_eq!(report.review_count, 1);
    assert!(report.backup_path.is_file());

    let database = Database::open(&path).expect("adopted database should open and migrate");
    database
        .with_connection(|connection| {
            let after = preserved_data_snapshot(connection).unwrap();
            assert_preexisting_data_survives_schema_additions(&before, &after);
            let lineage: String = connection
                .query_row(
                    "SELECT value FROM schema_metadata WHERE key = 'schema_lineage'",
                    [],
                    |row| row.get(0),
                )
                .unwrap();
            assert_eq!(lineage, PUBLIC_SCHEMA_LINEAGE);

            let migrations = read_ledger(connection, "schema_migrations").unwrap();
            assert_eq!(
                migrations.len(),
                usize::try_from(LATEST_SCHEMA_VERSION).unwrap()
            );
            assert_eq!(
                migrations[0],
                LedgerEntry {
                    version: PUBLIC_BASELINE_VERSION,
                    name: PUBLIC_BASELINE_NAME.to_string(),
                    checksum: PUBLIC_BASELINE_CHECKSUM.to_string(),
                }
            );
            assert_eq!(migrations[1].version, 2);
            assert_eq!(migrations[1].name, "canonical_documents");

            let repairs = read_ledger(connection, "data_repairs").unwrap();
            assert_eq!(repairs.len(), 2);
            assert_eq!(repairs[0].name, "canonical_semantic_chunk_hashes");
            assert_eq!(repairs[1].name, "canonical_block_hashes");

            assert!(column_exists(
                connection,
                "ai_provider_profiles",
                "require_zero_data_retention"
            )
            .unwrap());
            assert!(column_exists(connection, "documents", "canonical_schema_version").unwrap());
            let profile_sql: String = connection
                .query_row(
                    "SELECT sql FROM sqlite_schema
                     WHERE type = 'table' AND name = 'ai_provider_profiles'",
                    [],
                    |row| row.get(0),
                )
                .unwrap();
            assert!(profile_sql.contains("length(provider_kind) BETWEEN 1 AND 80"));
            assert!(!profile_sql.contains("provider_kind IN"));
            Ok(())
        })
        .expect("adopted data should remain queryable after current migrations");
    drop(database);
    fs::remove_dir_all(path.parent().unwrap()).expect("temporary fixture should clean up");
}

#[test]
fn explicitly_stamped_v26_marker_is_also_adoptable() {
    let path = temp_database_path("pre-public-stamped");
    let mut connection = open_public_v1_fixture(&path);
    convert_public_database_to_final_pre_public(&mut connection, 26);
    drop(connection);

    assert!(adopt_final_pre_public_database(&path)
        .expect("stamped adoption should succeed")
        .is_some());
    fs::remove_dir_all(path.parent().unwrap()).expect("temporary fixture should clean up");
}

#[test]
fn public_reopen_does_not_repeat_pre_public_adoption() {
    let path = temp_database_path("pre-public-idempotence");
    let database = Database::open(&path).expect("public fixture should open");
    drop(database);

    assert!(adopt_final_pre_public_database(&path)
        .expect("public inspection should succeed")
        .is_none());
    let backup_count = fs::read_dir(path.parent().unwrap())
        .unwrap()
        .filter_map(Result::ok)
        .filter(|entry| {
            entry
                .file_name()
                .to_string_lossy()
                .contains("pre-public-v26")
        })
        .count();
    assert_eq!(backup_count, 0);
    fs::remove_dir_all(path.parent().unwrap()).expect("temporary fixture should clean up");
}

#[test]
fn unrecognized_pre_public_database_is_not_modified_or_backed_up() {
    let path = temp_database_path("unsupported-pre-public");
    let connection = Connection::open(&path).expect("unsupported fixture should open");
    connection
        .execute_batch(
            "CREATE TABLE schema_migrations (
                 version INTEGER PRIMARY KEY,
                 name TEXT NOT NULL,
                 checksum TEXT NOT NULL,
                 applied_at INTEGER NOT NULL
             ) STRICT;
             INSERT INTO schema_migrations VALUES (26, 'deck_folders', 'wrong', 0);
             CREATE TABLE decks (id TEXT PRIMARY KEY) STRICT;
             PRAGMA user_version = 0;",
        )
        .expect("unsupported fixture should be created");
    drop(connection);
    let before = fs::read(&path).expect("unsupported fixture bytes should read");

    assert!(adopt_final_pre_public_database(&path)
        .expect("unsupported inspection should not error")
        .is_none());
    assert_eq!(
        before,
        fs::read(&path).expect("unsupported fixture bytes should remain")
    );
    let backup_count = fs::read_dir(path.parent().unwrap())
        .unwrap()
        .filter_map(Result::ok)
        .filter(|entry| {
            entry
                .file_name()
                .to_string_lossy()
                .contains("pre-public-v26")
        })
        .count();
    assert_eq!(backup_count, 0);
    fs::remove_dir_all(path.parent().unwrap()).expect("temporary fixture should clean up");
}
