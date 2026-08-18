use std::{
    fs,
    path::{Path, PathBuf},
    time::Duration,
};

use rusqlite::{backup::Backup, params, Connection, OptionalExtension, TransactionBehavior};
use serde_json::json;

use crate::util::unix_time_millis;

const FINAL_PRE_PUBLIC_VERSION: i64 = 26;
const PUBLIC_SCHEMA_LINEAGE: &str = "ostudy-public-v1";
const PUBLIC_BASELINE_VERSION: i64 = 1;
const PUBLIC_BASELINE_NAME: &str = "public_baseline";
const PUBLIC_BASELINE_CHECKSUM: &str =
    "2d978f8be026ecb820e71ba77a8585e1de51cb08fc2820714526a33284167f85";

const PRE_PUBLIC_MIGRATIONS: &[(i64, &str, &str)] = &[
    (1, "create_decks", "771e157d6f7eb5e1c024862966718e46ef744298f49287636c75167065904fd6"),
    (2, "cards_assets_settings_practice", "af64ceea1bfb3f1c04fed33f4558fae051174b50b15873d9ddf5ab4b36e55c8b"),
    (3, "fsrs_reviews", "49dd1e24e463c9a08a59e243d90143e2874459dd2a4a8d5f61613d5df7ebcae3"),
    (4, "card_organization", "0e0e25b3de24c04614aa949fce883fb6cc5bf12d1aab6e08dcaab12c548a768d"),
    (5, "remove_legacy_duplicate_trigger", "e275254085414039a7cf5f3a5e08ae35a61ea8f7d3d1b8e016a8d1f087c03029"),
    (6, "card_types_and_templates", "c07b900b2119f6eae6a4de58aeb3af9ab5882ee21d5c29ac7a5c7fcee6b86be8"),
    (7, "rich_templates_single_cloze", "91d120cb0b72c72c40d499a2c8be9b090be96ca760b2bb5a71072e2b71ed45e1"),
    (8, "scheduling_controls", "862b5277d92154d5586b7924023fb64f7cfee08d566b52efbd625fcbab410b2e"),
    (9, "operation_boundaries", "1482b734fadc2e71720e53c397131916179a3e7966c8461bf9d4fe3d5e51dc15"),
    (10, "transactional_schedule_actions", "bff6ab81045823654725e1f022de7c2c9b1715c63205273a24fd7db03eb0a82a"),
    (11, "managed_asset_lifecycle", "fc9d0b9601cbb45a99bb93b8ed448e150013186b88cb87430c84193176ad7280"),
    (12, "versioned_data_repairs", "6776fd4d70ed8bac79dd01774d27d7dca97ece65742dabd69a55077dc6a721a3"),
    (13, "canonical_data_model", "2e56fa0b68049f8a4d2e937b1dbdb18ec45d9c9041b06f5994b3c847f1d2f049"),
    (14, "review_attribution_and_projection_indexes", "ed5b4f510d014b34f7c6b83c3fcf7914baaa3942e51345a74fb8a7179cc1e11a"),
    (15, "ai_provider_profiles", "241246c924f8b050138011088014f515bdb26145eecd4536320331d905ad783b"),
    (16, "ai_provider_probe_capabilities", "12b4ec2518478a9798a19e87577f440a89943d06abfe7cb725f56727a854b75a"),
    (17, "document_flashcard_generation", "6f503141d0a99e0e603fc465c18ae456f18a9c7c66d16b3289b4d5cd4ed7baaa"),
    (18, "document_semantics_and_outline", "06b0023711038b8f9ea2c90c17c5ffb9dda8b286c01e34c4881a0596e96a1c7e"),
    (19, "resumable_large_document_generation", "80dc940f7018b6a754d440cbf5c3b287150b2dfb4def096539c4ff1f14d5fc66"),
    (20, "document_platform_generalization", "ad25bb267ff0061d7dea31f1bac0d7376be5c5af225854b329c31ec6caf29e32"),
    (21, "document_ocr_fallback", "31466a2bec357f46ee1e05680f3b26d5a297445bf686bfc2afd5aef470206e5d"),
    (22, "document_processing_metrics", "92ed1082377e685570ce3fb79aacf1610c38cbea58e60f289104d7a96ce24c8c"),
    (23, "document_provenance_tag_cleanup", "ec70e81cd4f7c5db36793e1ced79d646ab1981ffd2eaee90244908fbc4b71892"),
    (24, "external_file_transfers", "392cfda8f689f8e6f08be650003cf58d283d113452efa6d6d3f905b959bcb2d8"),
    (25, "durable_editor_drafts", "842ab270202dd3e936216e57878cc2b3f503059d78fc1476b4fdeb9a05fbb54e"),
    (26, "deck_folders", "4361a6ea23c1fe910b078b5edfb98c1a44a5dd6e3c307df1f0436db6076bcc58"),
];

const PRE_PUBLIC_REPAIRS: &[(i64, &str, &str)] = &[
    (1, "duplicate_fingerprints", "fb2f07be41d479048759ac7d754a59022f2c63c3f60082eaf9a40fd28bab7993"),
    (2, "template_and_generated_card_canonicalization", "ec486bad97c8c5495755bae160bf019d61c084444cc09d88ba8e1f0135b3e663"),
    (3, "review_event_historical_attribution", "97014e9b56c2608f9d83d3ba449d6f4308a1fcb3292cfef5c2640ed2cc5e7700"),
];

const REBUILD_AI_PROFILES_SQL: &str = r#"
CREATE TABLE ai_provider_profiles_public (
    id                      TEXT PRIMARY KEY CHECK(length(id) = 32),
    display_name            TEXT NOT NULL CHECK(length(trim(display_name)) BETWEEN 1 AND 100),
    normalized_name         TEXT NOT NULL CHECK(length(normalized_name) BETWEEN 1 AND 100),
    provider_kind           TEXT NOT NULL CHECK(length(provider_kind) BETWEEN 1 AND 80),
    base_url                TEXT NOT NULL CHECK(length(base_url) BETWEEN 1 AND 2048),
    model_id                TEXT NOT NULL DEFAULT '' CHECK(length(model_id) <= 300),
    auth_mode               TEXT NOT NULL CHECK(auth_mode IN ('none', 'bearer')),
    enabled                 INTEGER NOT NULL DEFAULT 1 CHECK(enabled IN (0, 1)),
    allow_insecure_http     INTEGER NOT NULL DEFAULT 0 CHECK(allow_insecure_http IN (0, 1)),
    require_zero_data_retention INTEGER NOT NULL DEFAULT 0
                                CHECK(require_zero_data_retention IN (0, 1)),
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

INSERT INTO ai_provider_profiles_public (
    id, display_name, normalized_name, provider_kind, base_url, model_id,
    auth_mode, enabled, allow_insecure_http, require_zero_data_retention,
    request_timeout_ms, credential_storage, credential_epoch, revision,
    created_at, updated_at, deleted_at
)
SELECT
    id, display_name, normalized_name, provider_kind, base_url, model_id,
    auth_mode, enabled, allow_insecure_http, 0,
    request_timeout_ms, credential_storage, credential_epoch, revision,
    created_at, updated_at, deleted_at
FROM ai_provider_profiles;

DROP TABLE ai_provider_profiles;
ALTER TABLE ai_provider_profiles_public RENAME TO ai_provider_profiles;

CREATE UNIQUE INDEX ai_provider_profiles_active_name_idx
    ON ai_provider_profiles(normalized_name)
    WHERE deleted_at IS NULL;

CREATE INDEX ai_provider_profiles_active_updated_idx
    ON ai_provider_profiles(updated_at DESC)
    WHERE deleted_at IS NULL;

CREATE TABLE schema_metadata (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL
) STRICT;

INSERT INTO schema_metadata(key, value)
VALUES ('schema_lineage', 'ostudy-public-v1');

DELETE FROM schema_migrations;
DELETE FROM data_repairs;
"#;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct PreservedCounts {
    decks: i64,
    notes: i64,
    cards: i64,
    reviews: i64,
    review_attributions: i64,
    ai_profiles: i64,
}

#[derive(Debug)]
pub(super) struct AdoptionReport {
    pub backup_path: PathBuf,
    pub deck_count: i64,
    pub card_count: i64,
    pub review_count: i64,
}

pub(super) fn adopt_final_pre_public_database(
    database_path: &Path,
) -> Result<Option<AdoptionReport>, String> {
    let mut connection = Connection::open(database_path)
        .map_err(|error| format!("Could not reopen the pre-public database: {error}"))?;
    if !is_exact_final_pre_public_database(&connection)? {
        return Ok(None);
    }

    verify_integrity(&connection)?;
    let before = preserved_counts(&connection)?;
    let backup_path = create_database_backup(&connection, database_path)?;

    crate::logging::info_target(
        "database.migration",
        "pre_public_database_adoption_started",
        "The final pre-public database is being adopted into the public schema lineage.",
        json!({
            "legacyVersion": FINAL_PRE_PUBLIC_VERSION,
            "backupFile": backup_path.file_name().and_then(|value| value.to_str()).unwrap_or("pre-public.backup"),
            "deckCount": before.decks,
            "cardCount": before.cards,
            "reviewCount": before.reviews,
        }),
    );

    if let Err(error) = apply_adoption(&mut connection, before) {
        return Err(format!(
            "The pre-public database could not be adopted safely: {error} A consistent database backup remains at {}.",
            backup_path.display()
        ));
    }

    crate::logging::info_target(
        "database.migration",
        "pre_public_database_adoption_completed",
        "The final pre-public database was adopted without replacing learner data.",
        json!({
            "legacyVersion": FINAL_PRE_PUBLIC_VERSION,
            "schemaLineage": PUBLIC_SCHEMA_LINEAGE,
            "schemaVersion": PUBLIC_BASELINE_VERSION,
            "backupFile": backup_path.file_name().and_then(|value| value.to_str()).unwrap_or("pre-public.backup"),
            "deckCount": before.decks,
            "cardCount": before.cards,
            "reviewCount": before.reviews,
        }),
    );

    Ok(Some(AdoptionReport {
        backup_path,
        deck_count: before.decks,
        card_count: before.cards,
        review_count: before.reviews,
    }))
}

fn is_exact_final_pre_public_database(connection: &Connection) -> Result<bool, String> {
    if table_exists(connection, "schema_metadata")? {
        return Ok(false);
    }
    if !table_exists(connection, "schema_migrations")?
        || !table_exists(connection, "data_repairs")?
        || !table_exists(connection, "deck_folders")?
        || !table_exists(connection, "review_events")?
        || !table_exists(connection, "review_event_attributions")?
        || !table_exists(connection, "ai_provider_profiles")?
    {
        return Ok(false);
    }
    if column_exists(
        connection,
        "ai_provider_profiles",
        "require_zero_data_retention",
    )? {
        return Ok(false);
    }

    let user_version: i64 = connection
        .query_row("PRAGMA user_version", [], |row| row.get(0))
        .map_err(|error| format!("Could not read the pre-public SQLite user_version: {error}"))?;
    if user_version != FINAL_PRE_PUBLIC_VERSION {
        return Ok(false);
    }

    let migrations = read_ledger(connection, "schema_migrations")?;
    let repairs = read_ledger(connection, "data_repairs")?;
    Ok(migrations == PRE_PUBLIC_MIGRATIONS && repairs == PRE_PUBLIC_REPAIRS)
}

fn read_ledger(
    connection: &Connection,
    table: &str,
) -> Result<Vec<(i64, String, String)>, String> {
    let sql = format!("SELECT version, name, checksum FROM {table} ORDER BY version");
    let mut statement = connection
        .prepare(&sql)
        .map_err(|error| format!("Could not inspect {table}: {error}"))?;
    statement
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))
        .map_err(|error| format!("Could not read {table}: {error}"))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("Could not decode {table}: {error}"))
}

fn table_exists(connection: &Connection, table: &str) -> Result<bool, String> {
    connection
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM sqlite_schema WHERE type = 'table' AND name = ?1)",
            [table],
            |row| row.get::<_, i64>(0),
        )
        .map(|value| value != 0)
        .map_err(|error| format!("Could not inspect table {table}: {error}"))
}

fn column_exists(connection: &Connection, table: &str, column: &str) -> Result<bool, String> {
    let sql = format!("PRAGMA table_info({table})");
    let mut statement = connection
        .prepare(&sql)
        .map_err(|error| format!("Could not inspect columns for {table}: {error}"))?;
    let mut rows = statement
        .query([])
        .map_err(|error| format!("Could not read columns for {table}: {error}"))?;
    while let Some(row) = rows
        .next()
        .map_err(|error| format!("Could not decode columns for {table}: {error}"))?
    {
        let name: String = row
            .get(1)
            .map_err(|error| format!("Could not decode a column name for {table}: {error}"))?;
        if name == column {
            return Ok(true);
        }
    }
    Ok(false)
}

fn preserved_counts(connection: &Connection) -> Result<PreservedCounts, String> {
    Ok(PreservedCounts {
        decks: count_rows(connection, "decks")?,
        notes: count_rows(connection, "notes")?,
        cards: count_rows(connection, "cards")?,
        reviews: count_rows(connection, "review_events")?,
        review_attributions: count_rows(connection, "review_event_attributions")?,
        ai_profiles: count_rows(connection, "ai_provider_profiles")?,
    })
}

fn count_rows(connection: &Connection, table: &str) -> Result<i64, String> {
    let sql = format!("SELECT COUNT(*) FROM {table}");
    connection
        .query_row(&sql, [], |row| row.get(0))
        .map_err(|error| format!("Could not count rows in {table}: {error}"))
}

fn create_database_backup(
    source: &Connection,
    database_path: &Path,
) -> Result<PathBuf, String> {
    let parent = database_path
        .parent()
        .ok_or_else(|| "The database path has no parent directory.".to_string())?;
    let file_name = database_path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("library.sqlite3");
    let timestamp = unix_time_millis().map_err(|error| error.message)?;
    let mut backup_path = parent.join(format!(
        "{file_name}.pre-public-v{FINAL_PRE_PUBLIC_VERSION}-{timestamp}.backup"
    ));
    for suffix in 1..=100 {
        if !backup_path.exists() {
            break;
        }
        backup_path = parent.join(format!(
            "{file_name}.pre-public-v{FINAL_PRE_PUBLIC_VERSION}-{timestamp}-{suffix}.backup"
        ));
    }
    if backup_path.exists() {
        return Err("Could not allocate a unique pre-public database backup path.".to_string());
    }

    let backup_result = (|| {
        let mut destination = Connection::open(&backup_path)
            .map_err(|error| format!("Could not create the pre-public database backup: {error}"))?;
        {
            let backup = Backup::new(source, &mut destination)
                .map_err(|error| format!("Could not start the SQLite backup: {error}"))?;
            backup
                .run_to_completion(64, Duration::from_millis(10), None)
                .map_err(|error| format!("Could not finish the SQLite backup: {error}"))?;
        }
        verify_integrity(&destination)
    })();

    if let Err(error) = backup_result {
        let _ = fs::remove_file(&backup_path);
        return Err(error);
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&backup_path, fs::Permissions::from_mode(0o600)).map_err(|error| {
            format!("Could not restrict the pre-public database backup permissions: {error}")
        })?;
    }

    Ok(backup_path)
}

fn apply_adoption(
    connection: &mut Connection,
    expected_counts: PreservedCounts,
) -> Result<(), String> {
    connection
        .execute_batch("PRAGMA foreign_keys = OFF;")
        .map_err(|error| format!("Could not prepare the pre-public adoption: {error}"))?;

    let adoption_result = (|| {
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|error| format!("Could not begin the pre-public adoption: {error}"))?;
        transaction
            .execute_batch(REBUILD_AI_PROFILES_SQL)
            .map_err(|error| format!("Could not rebuild the provider-profile schema: {error}"))?;
        let applied_at = unix_time_millis().map_err(|error| error.message)?;
        transaction
            .execute(
                "INSERT INTO schema_migrations (version, name, checksum, applied_at)
                 VALUES (?1, ?2, ?3, ?4)",
                params![
                    PUBLIC_BASELINE_VERSION,
                    PUBLIC_BASELINE_NAME,
                    PUBLIC_BASELINE_CHECKSUM,
                    applied_at,
                ],
            )
            .map_err(|error| format!("Could not record the public baseline: {error}"))?;
        transaction
            .execute_batch("PRAGMA user_version = 1;")
            .map_err(|error| format!("Could not set the public SQLite user_version: {error}"))?;

        let after = preserved_counts(&transaction)?;
        if after != expected_counts {
            return Err(format!(
                "Learner-data counts changed during adoption (before {expected_counts:?}, after {after:?})."
            ));
        }
        verify_integrity(&transaction)?;
        transaction
            .commit()
            .map_err(|error| format!("Could not commit the pre-public adoption: {error}"))
    })();

    let enforcement_result = connection
        .execute_batch("PRAGMA foreign_keys = ON;")
        .map_err(|error| format!("Could not restore SQLite foreign-key enforcement: {error}"));

    match (adoption_result, enforcement_result) {
        (Err(error), _) => Err(error),
        (Ok(()), Err(error)) => Err(error),
        (Ok(()), Ok(())) => verify_integrity(connection),
    }
}

fn verify_integrity(connection: &Connection) -> Result<(), String> {
    let foreign_key_violation: Option<(String, i64)> = connection
        .query_row(
            "SELECT [table], COALESCE(rowid, -1) FROM pragma_foreign_key_check LIMIT 1",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()
        .map_err(|error| format!("Could not validate pre-public foreign keys: {error}"))?;
    if let Some((table, rowid)) = foreign_key_violation {
        return Err(format!(
            "The pre-public database contains an invalid foreign-key reference in {table} at row {rowid}."
        ));
    }
    let result: String = connection
        .query_row("PRAGMA integrity_check", [], |row| row.get(0))
        .map_err(|error| format!("Could not run SQLite integrity_check: {error}"))?;
    if result != "ok" {
        return Err(format!("SQLite integrity_check failed: {result}"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicU64, Ordering};

    use super::*;
    use crate::database::Database;

    static NEXT_TEMP_ID: AtomicU64 = AtomicU64::new(1);

    fn temp_database_path(name: &str) -> PathBuf {
        let id = NEXT_TEMP_ID.fetch_add(1, Ordering::Relaxed);
        let directory = std::env::temp_dir().join(format!(
            "ostudy-{name}-{}-{id}",
            std::process::id()
        ));
        fs::create_dir_all(&directory).expect("temporary database directory should exist");
        directory.join("library.sqlite3")
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

    fn convert_public_database_to_final_pre_public(connection: &mut Connection) {
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
        for (version, name, checksum) in PRE_PUBLIC_MIGRATIONS {
            transaction
                .execute(
                    "INSERT INTO schema_migrations(version, name, checksum, applied_at)
                     VALUES (?1, ?2, ?3, 0)",
                    params![version, name, checksum],
                )
                .expect("legacy migration should be recorded");
        }
        for (version, name, checksum) in PRE_PUBLIC_REPAIRS {
            transaction
                .execute(
                    "INSERT INTO data_repairs(version, name, checksum, applied_at, changed_rows)
                     VALUES (?1, ?2, ?3, 0, 0)",
                    params![version, name, checksum],
                )
                .expect("legacy repair should be recorded");
        }
        transaction
            .execute_batch("PRAGMA user_version = 26;")
            .expect("legacy user_version should be set");
        transaction.commit().expect("legacy conversion should commit");
        connection
            .execute_batch("PRAGMA foreign_keys = ON;")
            .expect("foreign keys should re-enable");
    }

    #[test]
    fn exact_final_pre_public_database_preserves_library_and_analytics() {
        let path = temp_database_path("pre-public-adoption");
        let database = Database::open(&path).expect("public fixture should open");
        database
            .with_connection(|connection| {
                seed_library(connection);
                Ok(())
            })
            .expect("library fixture should seed");
        drop(database);

        let mut connection = Connection::open(&path).expect("fixture database should reopen");
        convert_public_database_to_final_pre_public(&mut connection);
        drop(connection);

        let report = adopt_final_pre_public_database(&path)
            .expect("adoption should succeed")
            .expect("exact v26 database should be adoptable");
        assert_eq!(report.deck_count, 1);
        assert_eq!(report.card_count, 1);
        assert_eq!(report.review_count, 1);
        assert!(report.backup_path.is_file());

        let database = Database::open(&path).expect("adopted database should open");
        database
            .with_connection(|connection| {
                assert_eq!(count_rows(connection, "decks").unwrap(), 1);
                assert_eq!(count_rows(connection, "cards").unwrap(), 1);
                assert_eq!(count_rows(connection, "review_events").unwrap(), 1);
                assert_eq!(
                    count_rows(connection, "review_event_attributions").unwrap(),
                    1
                );
                let lineage: String = connection
                    .query_row(
                        "SELECT value FROM schema_metadata WHERE key = 'schema_lineage'",
                        [],
                        |row| row.get(0),
                    )
                    .unwrap();
                assert_eq!(lineage, PUBLIC_SCHEMA_LINEAGE);
                let migration: (i64, String, String) = connection
                    .query_row(
                        "SELECT version, name, checksum FROM schema_migrations",
                        [],
                        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
                    )
                    .unwrap();
                assert_eq!(
                    migration,
                    (
                        PUBLIC_BASELINE_VERSION,
                        PUBLIC_BASELINE_NAME.to_string(),
                        PUBLIC_BASELINE_CHECKSUM.to_string(),
                    )
                );
                assert!(column_exists(
                    connection,
                    "ai_provider_profiles",
                    "require_zero_data_retention"
                )
                .unwrap());
                Ok(())
            })
            .expect("adopted data should remain queryable");
        drop(database);
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
                 PRAGMA user_version = 26;",
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
            .filter(|entry| entry.file_name().to_string_lossy().contains("pre-public-v26"))
            .count();
        assert_eq!(backup_count, 0);
        fs::remove_dir_all(path.parent().unwrap()).expect("temporary fixture should clean up");
    }
}
