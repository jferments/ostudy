use super::*;
use rusqlite::types::Value as SqlValue;

fn id(value: u32) -> String {
    format!("{value:032x}")
}

fn previous_database() -> Connection {
    let mut connection = public_baseline_connection().unwrap();
    for migration in &MIGRATIONS[1..14] {
        apply_transactional_migration(&mut connection, migration).unwrap();
    }
    connection
}

fn populate_legacy_graph(connection: &Connection) {
    connection
        .execute(
            "INSERT INTO decks(id, name, created_at, updated_at) VALUES (?1, 'Retained deck', 1, 1)",
            [id(1)],
        )
        .unwrap();
    connection
        .execute(
            "INSERT INTO notes (
                 rowid, id, deck_id, note_type, front_doc, back_doc, front_text, back_text,
                 created_at, updated_at, revision, duplicate_hash, reverse_enabled
             ) VALUES (97, ?1, ?2, 'basic', ?3, ?4, 'Stored question', 'Stored answer',
                       10, 20, 7, 'original duplicate identity', 1)",
            params![
                id(2),
                id(1),
                r#"{"type":"doc","content":[{"type":"paragraph","content":[{"type":"text","text":"Stored question"}]}]}"#,
                r#"{"type":"doc","content":[{"type":"paragraph","content":[{"type":"text","text":"Stored answer"}]}]}"#,
            ],
        )
        .unwrap();
    connection
        .execute(
            "INSERT INTO notes (
                 id, deck_id, note_type, front_doc, back_doc, front_text, back_text,
                 created_at, updated_at, revision, cloze_source, cloze_mode
             ) SELECT ?1, deck_id, 'cloze', front_doc, back_doc, front_text, back_text,
                      11, 21, 3, 'Preserve {{c1::this source}} exactly.', 'all_together'
               FROM notes WHERE id = ?2",
            params![id(15), id(2)],
        )
        .unwrap();
    connection
        .execute(
            "INSERT INTO cards(id, note_id, deck_id, direction, created_at, updated_at)
             VALUES (?1, ?2, ?3, 'forward', 11, 21)",
            params![id(16), id(15), id(1)],
        )
        .unwrap();
    crate::scheduler::insert_new_schedule(connection, &id(16), 11).unwrap();
    for (card, direction) in [(3, "forward"), (8, "reverse")] {
        connection
            .execute(
                "INSERT INTO cards(id, note_id, deck_id, direction, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, 10, 20)",
                params![id(card), id(2), id(1), direction],
            )
            .unwrap();
        connection
            .execute(
                "INSERT INTO card_schedules (
                     card_id, state, due_at, stability, difficulty, elapsed_days, scheduled_days,
                     reps, lapses, last_review_at, scheduler_name, scheduler_version, updated_at
                 ) VALUES (?1, 2, 86401000, 2.5, 4.1, 3, 1, 6, 2, 1000, 'fsrs', 'retained', 1000)",
                [id(card)],
            )
            .unwrap();
    }
    connection
        .execute(
            "INSERT INTO practice_sessions (
                 id, selected_deck_ids_json, shuffle, card_count, started_at,
                 viewed_count, revealed_count, rated_count, correct_count
             ) VALUES (?1, ?2, 0, 2, 500, 1, 1, 1, 1)",
            params![id(4), serde_json::to_string(&vec![id(1)]).unwrap()],
        )
        .unwrap();
    connection
        .execute(
            "INSERT INTO practice_session_cards (
                 session_id, card_id, position, viewed_at, revealed_at, rated_at, rating, response_time_ms
             ) VALUES (?1, ?2, 0, 500, 700, 1000, 3, 500)",
            params![id(4), id(3)],
        )
        .unwrap();
    connection
        .execute(
            "INSERT INTO practice_session_card_snapshots (
                 session_id, card_id, note_id, note_revision, owner_deck_id_at_capture,
                 captured_deck_name, direction, card_type, card_label, presentation_variant,
                 front_doc_json, back_doc_json, front_text, back_text, question_excerpt,
                 question_text_hash, question_was_truncated, captured_at
             ) SELECT ?1, ?2, n.id, n.revision, n.deck_id, 'Old deck name', 'forward', 'basic',
                      'Basic', '', n.front_doc, n.back_doc, n.front_text, n.back_text,
                      n.front_text, ?3, 0, 500 FROM notes n WHERE n.id = ?4",
            params![id(4), id(3), "a".repeat(64), id(2)],
        )
        .unwrap();
    connection
        .execute(
            "INSERT INTO review_events (
                 id, submission_id, session_id, card_id, reviewed_at, rating, answer_correct,
                 confidence, response_time_ms, state_before, state_after, due_before, due_after,
                 stability_before, stability_after, difficulty_before, difficulty_after,
                 scheduled_days_before, scheduled_days_after, scheduler_name, scheduler_version,
                 created_at, request_fingerprint, result_completed, result_rated_count, result_correct_count
             ) VALUES (?1, ?2, ?3, ?4, 1000, 3, 1, 'somewhat_confident', 500, 0, 2, 0, 86401000,
                       0, 2.5, 0, 4.1, 0, 1, 'fsrs', 'retained', 1000, 'retained request', 0, 1, 1)",
            params![id(5), id(6), id(4), id(3)],
        )
        .unwrap();
    connection
        .execute(
            "INSERT INTO review_event_attributions (
                 review_event_id, reviewed_at, review_local_date, time_zone_id, utc_offset_minutes,
                 card_id, note_id, note_revision, owner_deck_id_at_review, captured_deck_name,
                 direction, card_type, card_label, presentation_variant, question_excerpt,
                 question_text_hash, question_was_truncated, attribution_source
             ) VALUES (?1, 1000, '2026-09-15', 'America/Los_Angeles', -420, ?2, ?3, 7, ?4,
                       'Old deck name', 'forward', 'basic', 'Basic', '', 'Stored question', ?5, 0, 'captured')",
            params![id(5), id(3), id(2), id(1), "a".repeat(64)],
        )
        .unwrap();
    connection
        .execute(
            "INSERT INTO tags(id, name, normalized_name, created_at, updated_at)
             VALUES (?1, 'Retained tag', 'retained tag', 1, 1)",
            [id(9)],
        )
        .unwrap();
    connection.execute("INSERT INTO note_tags VALUES (?1, ?2)", params![id(2), id(9)]).unwrap();
    connection.execute("INSERT INTO practice_session_card_tag_snapshots VALUES (?1, ?2, ?3, 'Old tag name')", params![id(4), id(3), id(9)]).unwrap();
    connection.execute("INSERT INTO review_event_tag_attributions VALUES (?1, ?2, 'Old tag name')", params![id(5), id(9)]).unwrap();
    connection
        .execute(
            "INSERT INTO assets(id, original_filename, mime_type, byte_size, relative_path, created_at)
             VALUES (?1, 'retained.png', 'image/png', 1, 'assets/images/retained.png', 1)",
            [id(40)],
        )
        .unwrap();
    connection.execute("INSERT INTO note_assets VALUES (?1, ?2, 'back')", params![id(2), id(40)]).unwrap();
    connection
        .execute(
            "INSERT INTO editor_drafts (
                 id, open_operation_id, draft_kind, target_entity_id, context_entity_id,
                 base_revision, payload_schema_version, payload_json, payload_sha256,
                 revision, state, created_at, updated_at, last_opened_at
             ) VALUES (?1, ?2, 'card', ?3, ?4, 7, 1, '{\"retained\":true}', ?5, 3, 'active', 1, 2, 2)",
            params![id(42), id(41), id(2), id(1), "b".repeat(64)],
        )
        .unwrap();
    connection.execute("INSERT INTO editor_draft_assets VALUES (?1, ?2)", params![id(42), id(40)]).unwrap();
    connection
        .execute(
            "INSERT INTO knowledge_sources (
                 id, kind, owner_id, segment_key, parent_key, owner_revision, content_hash,
                 record_json, active, note_id, card_id, extraction_contract, extracted_hash,
                 created_at, updated_at
             ) VALUES (?1, 'card', ?2, 'front', 'front', '7', ?3, '{}', 1, ?4, ?2, ?3, ?3, 1, 1)",
            params![id(10), id(3), "c".repeat(64), id(2)],
        )
        .unwrap();
    connection
        .execute(
            "INSERT INTO knowledge_units (
                 id, source_id, source_hash, content_hash, record_json, extraction_contract, created_at, updated_at
             ) VALUES (?1, ?2, ?3, ?3, '{}', ?3, 1, 1)",
            params![id(11), id(10), "c".repeat(64)],
        )
        .unwrap();
}

fn rows(connection: &Connection, table: &str) -> Vec<Vec<SqlValue>> {
    // This helper is private to fixed test fixtures; no runtime query input.
    let mut statement = connection.prepare(&format!("SELECT * FROM {table} ORDER BY 1")).unwrap();
    let columns = statement.column_count();
    let result = statement
        .query_map([], |row| (0..columns).map(|index| row.get(index)).collect())
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    result
}

fn apply_question_migration(connection: &mut Connection) {
    assert_eq!(MIGRATIONS[14].name, "interactive_questions");
    apply_transactional_migration(connection, &MIGRATIONS[14]).unwrap();
    verify_integrity(connection).unwrap();
}

#[test]
fn question_migration_preserves_note_rowids_schedules_history_drafts_and_semantic_children() {
    let mut connection = previous_database();
    populate_legacy_graph(&connection);
    let tables = [
        "notes", "cards", "card_schedules", "practice_sessions", "practice_session_cards",
        "practice_session_card_snapshots", "practice_session_card_tag_snapshots",
        "review_events", "review_event_attributions", "review_event_tag_attributions",
        "note_tags", "note_assets", "assets", "editor_drafts", "editor_draft_assets",
        "knowledge_sources", "knowledge_units", "card_search_fts",
    ];
    let before: Vec<_> = tables.iter().map(|name| rows(&connection, name)).collect();
    apply_question_migration(&mut connection);
    for (index, table) in tables.iter().enumerate() {
        let mut after = rows(&connection, table);
        if matches!(*table, "notes" | "practice_session_card_snapshots") {
            for row in &mut after {
                assert_eq!(row.pop(), Some(SqlValue::Null));
            }
        }
        assert_eq!(after, before[index], "migration changed retained {table}");
    }
    let rowid: i64 = connection.query_row("SELECT rowid FROM notes WHERE id = ?1", [id(2)], |row| row.get(0)).unwrap();
    assert_eq!(rowid, 97);
    let indexed: i64 = connection.query_row("SELECT rowid FROM card_search_fts WHERE note_id = ?1", [id(2)], |row| row.get(0)).unwrap();
    assert_eq!(indexed, rowid);
    let foreign_keys: bool = connection.query_row("PRAGMA foreign_keys", [], |row| row.get(0)).unwrap();
    assert!(foreign_keys);
    let before_reopen = connection.total_changes();
    assert!(!apply_migrations(&mut connection, false).unwrap());
    assert_eq!(connection.total_changes(), before_reopen);
}

fn insert_interactive_note(connection: &Connection) {
    let definition = serde_json::json!({
        "schemaVersion": 1,
        "question": {
            "kind": "true_false",
            "statement": {"type":"doc","content":[{"type":"paragraph","content":[{"type":"text","text":"A proposition."}]}]},
            "correct": false,
            "explanation": null,
            "correctedStatement": null
        }
    });
    let validated = crate::cards::questions::ValidatedQuestion::parse(&definition.to_string()).unwrap();
    connection
        .execute(
            "INSERT INTO notes (
                 id, deck_id, note_type, content_format, front_text, back_text,
                 question_definition, created_at, updated_at
             ) VALUES (?1, ?2, 'interactive', 'question-json', 'A proposition.', '', ?3, 1, 1)",
            params![id(20), id(1), validated.to_json().unwrap()],
        )
        .unwrap();
}

#[test]
fn typed_notes_have_one_content_owner_and_cannot_be_saved_as_basic_front_back() {
    let mut connection = previous_database();
    populate_legacy_graph(&connection);
    apply_question_migration(&mut connection);
    insert_interactive_note(&connection);
    for update in [
        "UPDATE notes SET front_doc = '{}' WHERE note_type = 'interactive'",
        "UPDATE notes SET back_doc = '{}' WHERE note_type = 'interactive'",
        "UPDATE notes SET question_definition = NULL WHERE note_type = 'interactive'",
        "UPDATE notes SET content_format = 'tiptap-json' WHERE note_type = 'interactive'",
        "UPDATE notes SET reverse_enabled = 1 WHERE note_type = 'interactive'",
        "UPDATE notes SET question_definition = '{}' WHERE note_type = 'basic'",
        "UPDATE notes SET note_type = 'unknown' WHERE note_type = 'interactive'",
    ] {
        assert!(connection.execute(update, []).is_err(), "invalid owner shape was accepted");
    }
    verify_integrity(&connection).unwrap();
}

#[test]
fn migration_failure_restores_old_schema_ledger_and_foreign_key_enforcement() {
    let mut connection = previous_database();
    populate_legacy_graph(&connection);
    // A deliberately corrupt inherited relationship forces the pre-commit
    // integrity check to reject AFTER the table-copy work has executed.
    connection.execute_batch("PRAGMA foreign_keys = OFF;").unwrap();
    connection.execute("INSERT INTO note_tags VALUES (?1, ?2)", params![id(99), id(9)]).unwrap();
    connection.execute_batch("PRAGMA foreign_keys = ON;").unwrap();
    let before_notes = rows(&connection, "notes");
    let before_schema = schema_objects(&connection).unwrap();
    let result = apply_transactional_migration(&mut connection, &MIGRATIONS[14]);
    assert!(result.is_err());
    assert_eq!(rows(&connection, "notes"), before_notes);
    assert_eq!(schema_objects(&connection).unwrap(), before_schema);
    let version: i64 = connection.query_row("PRAGMA user_version", [], |row| row.get(0)).unwrap();
    assert_eq!(version, 14);
    let newest: i64 = connection.query_row("SELECT max(version) FROM schema_migrations", [], |row| row.get(0)).unwrap();
    assert_eq!(newest, 14);
    let enabled: bool = connection.query_row("PRAGMA foreign_keys", [], |row| row.get(0)).unwrap();
    assert!(enabled);
    assert!(connection.is_autocommit());
}

#[test]
fn question_keys_cannot_transfer_an_existing_schedule_to_another_target() {
    let mut connection = previous_database();
    populate_legacy_graph(&connection);
    apply_question_migration(&mut connection);
    insert_interactive_note(&connection);
    connection.execute("INSERT INTO cards(id, note_id, deck_id, created_at, updated_at) VALUES (?1, ?2, ?3, 1, 1)", params![id(21), id(20), id(1)]).unwrap();
    connection.execute("INSERT INTO card_question_presentations VALUES (?1, ?2, 'main')", params![id(21), id(20)]).unwrap();
    assert!(connection.execute("UPDATE card_question_presentations SET question_key = 'other'", []).is_err());
    assert!(connection.execute("INSERT INTO card_question_presentations VALUES (?1, ?2, 'main')", params![id(3), id(2)]).is_err());
    verify_integrity(&connection).unwrap();
}

#[test]
fn supported_previous_backups_remain_readable_without_mutating_the_snapshot() {
    let mut connection = public_baseline_connection().unwrap();
    for migration in &MIGRATIONS[1..] {
        apply_transactional_migration(&mut connection, migration).unwrap();
        if migration.version >= 10 {
            let before = connection.total_changes();
            crate::library_backup::validate_snapshot_database(&connection).unwrap();
            assert_eq!(connection.total_changes(), before);
        }
    }
    connection
        .execute(
            "UPDATE schema_migrations SET checksum = 'corrupted' WHERE version = 14",
            [],
        )
        .unwrap();
    assert!(crate::library_backup::validate_snapshot_database(&connection).is_err());
}
