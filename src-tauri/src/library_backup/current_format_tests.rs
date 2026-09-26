//! Synthetic preservation receipts. No personal database or provider is used.

use std::collections::BTreeMap;

use rusqlite::types::ValueRef;
use sha2::{Digest, Sha256};

use super::current_format_questions::{id, rich};
use super::*;
use crate::cards::questions::{QuestionAttempt, ValidatedQuestion};

#[path = "current_format_tests/collections.rs"]
mod collections;
#[path = "current_format_tests/images.rs"]
mod images;
#[path = "current_format_tests/knowledge.rs"]
mod knowledge;

// Version the receipt encoding separately from the application schema. Preserve
// SQLite types, IEEE float bits, lengths and duplicates, not just row counts.
fn rows(connection: &Connection) -> BTreeMap<String, (usize, String)> {
    let tables = connection
        .prepare(
            "SELECT name FROM pragma_table_list WHERE schema = 'main'
         AND type = 'table' AND name NOT LIKE 'sqlite_%' ORDER BY name",
        )
        .unwrap()
        .query_map([], |row| row.get::<_, String>(0))
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    let mut manifest = BTreeMap::new();
    for table in tables {
        let quoted = table.replace('"', "\"\"");
        let columns = if table == "notes" { "rowid, *" } else { "*" };
        let mut statement = connection
            .prepare(&format!("SELECT {columns} FROM \"{quoted}\""))
            .unwrap();
        let width = statement.column_count();
        let mut query = statement.query([]).unwrap();
        let mut encoded_rows = Vec::new();
        while let Some(row) = query.next().unwrap() {
            let mut encoded = Vec::new();
            for index in 0..width {
                let (tag, bytes) = match row.get_ref(index).unwrap() {
                    ValueRef::Null => (0_u8, Vec::new()),
                    ValueRef::Integer(value) => (1, value.to_be_bytes().to_vec()),
                    ValueRef::Real(value) => (2, value.to_bits().to_be_bytes().to_vec()),
                    ValueRef::Text(value) => (3, value.to_vec()),
                    ValueRef::Blob(value) => (4, value.to_vec()),
                };
                encoded.push(tag);
                encoded.extend_from_slice(&(bytes.len() as u64).to_be_bytes());
                encoded.extend_from_slice(&bytes);
            }
            encoded_rows.push(encoded);
        }
        encoded_rows.sort();
        let mut digest = Sha256::new();
        digest.update(b"ostudy-preservation-rows-v1");
        digest.update((width as u64).to_be_bytes());
        for row in &encoded_rows {
            digest.update((row.len() as u64).to_be_bytes());
            digest.update(row);
        }
        manifest.insert(
            table,
            (encoded_rows.len(), format!("{:x}", digest.finalize())),
        );
    }
    manifest
}

fn manifest(database: &Database) -> BTreeMap<String, (usize, String)> {
    database
        .with_connection(|connection| {
            let problems: i64 = connection
                .query_row("SELECT COUNT(*) FROM pragma_foreign_key_check", [], |row| {
                    row.get(0)
                })
                .map_err(CommandError::database)?;
            assert_eq!(problems, 0);
            let integrity: String = connection
                .query_row("PRAGMA integrity_check", [], |row| row.get(0))
                .map_err(CommandError::database)?;
            assert_eq!(integrity, "ok");
            let mut definitions = connection
                .prepare("SELECT question_definition FROM notes WHERE note_type = 'interactive'")
                .map_err(CommandError::database)?;
            for row in definitions
                .query_map([], |row| row.get::<_, String>(0))
                .map_err(CommandError::database)?
            {
                ValidatedQuestion::parse(&row.map_err(CommandError::database)?)?;
            }
            let mut attempts = connection
                .prepare("SELECT checkpoint_json FROM question_attempts")
                .map_err(CommandError::database)?;
            for row in attempts
                .query_map([], |row| row.get::<_, String>(0))
                .map_err(CommandError::database)?
            {
                QuestionAttempt::restore(&row.map_err(CommandError::database)?)?;
            }
            knowledge::verify(connection)?;
            collections::verify(connection)?;
            images::verify(connection)?;
            Ok(rows(connection))
        })
        .unwrap()
}

fn files(paths: &AppPaths, relative: &[String]) -> BTreeMap<String, (usize, String)> {
    relative
        .iter()
        .map(|path| {
            let bytes = fs::read(paths.resolve_relative(path)).unwrap();
            (
                path.clone(),
                (bytes.len(), format!("{:x}", Sha256::digest(bytes))),
            )
        })
        .collect()
}

fn seed_recall_and_draft(connection: &Connection, deck: &str) {
    let note: String = connection
        .query_row(
            "SELECT id FROM notes WHERE deck_id = ?1 ORDER BY id LIMIT 1",
            [deck],
            |row| row.get(0),
        )
        .unwrap();
    connection
        .execute(
            "UPDATE notes SET reverse_enabled = 1 WHERE id = ?1",
            [&note],
        )
        .unwrap();
    connection
        .execute(
            "INSERT INTO cards (id, note_id, deck_id, direction, template_ordinal,
            created_at, updated_at) VALUES (?1, ?2, ?3, 'reverse', 1, 10, 10)",
            params![id(700), note, deck],
        )
        .unwrap();
    insert_new_schedule(connection, &id(700), 10).unwrap();
    for (offset, mode) in [(0, "one_at_a_time"), (1, "all_together")] {
        let note = id(710 + offset);
        let card = id(720 + offset);
        let content = validate_content(
            rich("First: […]"),
            rich("First: one"),
            "First: […]".to_string(),
            "First: one".to_string(),
        )
        .unwrap();
        connection
            .execute(
                "INSERT INTO notes (id, deck_id, note_type, content_format,
                content_schema_version, front_doc, back_doc, front_text, back_text,
                cloze_source, cloze_mode, created_at, updated_at)
             VALUES (?1, ?2, 'cloze', 'tiptap-json', 1, ?3, ?4, ?5, ?6,
                'First: {{c1::one}}', ?7, 10, 10)",
                params![
                    note,
                    deck,
                    content.front_doc_json,
                    content.back_doc_json,
                    content.front_text,
                    content.back_text,
                    mode
                ],
            )
            .unwrap();
        connection
            .execute(
                "INSERT INTO cards (id, note_id, deck_id, direction, template_ordinal,
                created_at, updated_at) VALUES (?1, ?2, ?3, 'forward', 0, 10, 10)",
                params![card, note, deck],
            )
            .unwrap();
        insert_new_schedule(connection, &card, 10).unwrap();
    }
    // A durable receipt survives removal of its authored note and suppresses
    // resurrection on replay. This is the current boundary, not LK-07 bundles.
    connection
        .execute(
            "INSERT INTO question_note_writes VALUES (?1, ?2, ?3, 1, '[]', 40)",
            params![id(900), "e".repeat(64), id(901)],
        )
        .unwrap();
    connection
        .execute(
            "INSERT INTO decks (id, name, created_at, updated_at, deleted_at)
         VALUES (?1, 'Deliberately deleted material', 10, 40, 40)",
            [id(902)],
        )
        .unwrap();
    let payload = json!({"rows":[{"front":"Unfinished","back":""}]}).to_string();
    connection
        .execute(
            "INSERT INTO editor_drafts (id, open_operation_id, draft_kind,
            context_entity_id, payload_schema_version, payload_json, payload_sha256,
            revision, state, created_at, updated_at, last_opened_at)
         VALUES (?1, ?1, 'bulk_cards', ?2, 1, ?3, ?4, 1, 'active', 10, 20, 20)",
            params![
                id(800),
                deck,
                payload,
                format!("{:x}", Sha256::digest(payload.as_bytes()))
            ],
        )
        .unwrap();
}

#[test]
fn current_library_preserves_every_value_identity_and_asset_through_reopen_and_restore() {
    let paths = temp_paths("lk02-current-source");
    let database = Database::open(paths.database_path()).unwrap();
    let (deck, asset_path) = populate_complete_library(&database, &paths).unwrap();
    let image: &[u8] = &[
        137, 80, 78, 71, 13, 10, 26, 10, 0, 0, 0, 13, 73, 72, 68, 82, 0, 0, 0, 8, 0, 0, 0, 8, 8, 2,
        0, 0, 0, 75, 109, 41, 220, 0, 0, 0, 21, 73, 68, 65, 84, 120, 156, 99, 156, 54, 109, 26, 3,
        54, 192, 132, 85, 116, 208, 74, 0, 0, 99, 35, 1, 210, 131, 71, 152, 47, 0, 0, 0, 0, 73, 69,
        78, 68, 174, 66, 96, 130,
    ];
    fs::write(paths.resolve_relative(&asset_path), image).unwrap();
    database
        .with_connection(|connection| {
            connection
                .execute(
                    "UPDATE assets SET byte_size = ?1 WHERE relative_path = ?2",
                    params![image.len() as i64, asset_path],
                )
                .map_err(CommandError::database)?;
            seed_recall_and_draft(connection, &deck);
            let asset: String = connection
                .query_row(
                    "SELECT id FROM assets WHERE relative_path = ?1",
                    [&asset_path],
                    |row| row.get(0),
                )
                .map_err(CommandError::database)?;
            super::current_format_questions::seed(connection, &deck, &asset);
            Ok(())
        })
        .unwrap();
    let document_path = super::current_format_sources::seed(&database, &paths, &deck);
    knowledge::seed(&database);
    collections::seed(&database);
    let mut relative = vec![asset_path, document_path];
    relative.extend(images::seed(&database, &paths, image));
    let before = manifest(&database);
    let before_files = files(&paths, &relative);
    for (table, count) in [
        ("notes", 10),
        ("cards", 11),
        ("card_schedules", 11),
        ("question_attempts", 7),
        ("review_events", 1),
        ("document_study_guide_overlays", 1),
        ("tutor_turns", 1),
        ("editor_drafts", 1),
        ("question_note_writes", 1),
        ("knowledge_units", 1),
        ("knowledge_analysis_windows", 1),
        ("document_evidence_payloads", 1),
        ("document_evidence_occurrences", 1),
        ("document_collections", 1),
        ("document_collection_memberships", 1),
        ("document_collection_ranges", 1),
        ("knowledge_collection_views", 1),
        ("knowledge_collection_view_dependencies", 4),
        ("document_image_sets", 1),
        ("document_image_members", 2),
        ("document_image_imports", 1),
    ] {
        assert_eq!(before[table].0, count, "fixture coverage: {table}");
    }
    drop(database);
    let database = Database::open(paths.database_path()).unwrap();
    assert_eq!(manifest(&database), before);
    assert_eq!(files(&paths, &relative), before_files);
    let backup = paths.app_data_dir().join("preservation.ostudybackup");
    export_backup(&database, &paths, &backup).unwrap();
    assert_eq!(
        manifest(&database),
        before,
        "backup must not modify source rows"
    );
    let target = temp_paths("lk02-current-target");
    let original = Database::open(target.database_path()).unwrap();
    let original_rows = manifest(&original);
    drop(original);
    copy_and_sync(&backup, &pending_restore_path(&target)).unwrap();
    let guard = apply_pending_restore(&target).unwrap().unwrap();
    let restored = Database::open(target.database_path()).unwrap();
    assert_eq!(manifest(&restored), before);
    assert_eq!(files(&target, &relative), before_files);
    drop(restored);
    drop(guard);
    let rolled_back = Database::open(target.database_path()).unwrap();
    assert_eq!(manifest(&rolled_back), original_rows);
    drop(rolled_back);
    for path in &relative {
        assert!(!target.resolve_relative(path).exists());
    }
    assert!(
        !pending_restore_path(&target).exists(),
        "rollback must not auto-repeat a failed restore"
    );
    copy_and_sync(&backup, &pending_restore_path(&target)).unwrap();
    let guard = apply_pending_restore(&target).unwrap().unwrap();
    drop(Database::open(target.database_path()).unwrap());
    guard.commit();
    let reopened = Database::open(target.database_path()).unwrap();
    assert_eq!(manifest(&reopened), before);
    assert_eq!(files(&target, &relative), before_files);
    drop(reopened);
    drop(database);
    fs::remove_dir_all(paths.app_data_dir()).unwrap();
    fs::remove_dir_all(target.app_data_dir()).unwrap();
}

#[test]
fn canonical_receipt_detects_equal_count_value_identity_and_type_changes() {
    let connection = Connection::open_in_memory().unwrap();
    connection
        .execute_batch("CREATE TABLE probe(id, value); INSERT INTO probe VALUES (1, '2');")
        .unwrap();
    let before = rows(&connection);
    for update in [
        "UPDATE probe SET value = '3'",
        "UPDATE probe SET value = 2",
        "UPDATE probe SET id = 2, value = '2'",
    ] {
        connection
            .execute_batch("UPDATE probe SET id = 1, value = '2'")
            .unwrap();
        connection.execute_batch(update).unwrap();
        let after = rows(&connection);
        assert_eq!(before["probe"].0, after["probe"].0);
        assert_ne!(before["probe"].1, after["probe"].1);
    }
}

#[test]
fn corrupt_or_future_study_payloads_are_rejected_before_any_conversion_write() {
    let paths = temp_paths("lk02-payload-refusal");
    let database = Database::open(paths.database_path()).unwrap();
    let (deck, asset_path) = populate_complete_library(&database, &paths).unwrap();
    database.with_connection(|connection| {
        let asset: String = connection.query_row(
            "SELECT id FROM assets WHERE relative_path = ?1", [&asset_path], |row| row.get(0),
        ).map_err(CommandError::database)?;
        super::current_format_questions::seed(connection, &deck, &asset);
        for corruption in [
            "UPDATE notes SET content_schema_version = 999 WHERE note_type = 'basic'",
            "UPDATE notes SET front_doc = '{\"type\":\"script\"}' WHERE note_type = 'basic'",
            "UPDATE notes SET question_definition = json_set(question_definition, '$.question.kind', 'future') WHERE note_type = 'interactive'",
            "UPDATE question_attempts SET checkpoint_json = json_set(checkpoint_json, '$.attemptVersion', 999)",
            "UPDATE question_attempts SET instance_hash = 'bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb'",
        ] {
            let transaction = connection.transaction().map_err(CommandError::database)?;
            transaction.execute_batch(corruption).map_err(CommandError::database)?;
            let changes = transaction.total_changes();
            assert!(crate::database::validate_backup_lineage(&transaction).is_err());
            assert_eq!(transaction.total_changes(), changes);
            transaction.rollback().map_err(CommandError::database)?;
        }
        Ok(())
    }).unwrap();
    drop(database);
    fs::remove_dir_all(paths.app_data_dir()).unwrap();
}
