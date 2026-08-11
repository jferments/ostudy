use std::{fs, path::PathBuf};

use rusqlite::{params, Connection};
use serde_json::json;

use super::*;
use crate::{
    cards::validate_content,
    database::Database,
    decks::{create_deck_in_connection, CreateDeckInput},
    scheduler::insert_new_schedule,
    util::generate_id,
};

struct TestPaths {
    root: PathBuf,
    paths: AppPaths,
}

impl TestPaths {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "ostudy-operation-test-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("clock should be valid")
                .as_nanos()
        ));
        let paths = AppPaths::new(root.clone()).expect("test paths should initialize");
        Self { root, paths }
    }
}

impl Drop for TestPaths {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

fn paragraph(text: &str) -> serde_json::Value {
    json!({
        "type": "doc",
        "content": [{
            "type": "paragraph",
            "content": [{"type": "text", "text": text}]
        }]
    })
}

fn create_card(
    connection: &mut Connection,
    deck_id: &str,
) -> Result<(String, String), CommandError> {
    let note_id = generate_id(connection)?;
    let card_id = generate_id(connection)?;
    let content = validate_content(
        paragraph("Question"),
        paragraph("Answer"),
        "Question".to_string(),
        "Answer".to_string(),
    )?;
    connection
        .execute(
            "INSERT INTO notes (
                 id, deck_id, note_type, content_format, content_schema_version,
                 front_doc, back_doc, front_text, back_text, duplicate_hash,
                 created_at, updated_at, revision
             ) VALUES (
                 ?1, ?2, 'basic', 'tiptap-json', 1,
                 ?3, ?4, ?5, ?6, 'practice-start-test', 10, 10, 1
             )",
            params![
                note_id,
                deck_id,
                content.front_doc_json,
                content.back_doc_json,
                content.front_text,
                content.back_text,
            ],
        )
        .map_err(CommandError::database)?;
    connection
        .execute(
            "INSERT INTO cards (
                 id, note_id, deck_id, direction, template_ordinal,
                 created_at, updated_at
             ) VALUES (?1, ?2, ?3, 'forward', 0, 10, 10)",
            params![card_id, note_id, deck_id],
        )
        .map_err(CommandError::database)?;
    insert_new_schedule(connection, &card_id, 10)?;
    Ok((note_id, card_id))
}

#[test]
fn equivalent_practice_start_retry_returns_one_committed_session() {
    let paths = TestPaths::new();
    let database = Database::open_in_memory().expect("database should open");
    database
        .with_connection(|connection| {
            let deck_id = create_deck_in_connection(
                connection,
                CreateDeckInput {
                    name: "Start retry".to_string(),
                    description: None,
                },
            )?
            .id;
            let (_, card_id) = create_card(connection, &deck_id)?;
            let operation_id = "a".repeat(32);

            let first = start_selected_practice_in_connection(
                connection,
                &paths.paths,
                operation_id.clone(),
                vec![deck_id.clone()],
                Vec::new(),
                false,
                "extra",
            )?;
            let replay = start_selected_practice_in_connection(
                connection,
                &paths.paths,
                operation_id.clone(),
                vec![deck_id.clone()],
                Vec::new(),
                false,
                "extra",
            )?;

            assert_eq!(replay.id, first.id);
            assert_eq!(replay.card_count, 1);
            assert_eq!(replay.first_window.cards.len(), 1);
            assert_eq!(replay.first_window.cards[0].presentation.id, card_id);
            let count: i64 = connection
                .query_row(
                    "SELECT COUNT(*) FROM practice_sessions
                     WHERE start_operation_id = ?1",
                    [operation_id.as_str()],
                    |row| row.get(0),
                )
                .map_err(CommandError::database)?;
            assert_eq!(count, 1);

            let conflict = start_selected_practice_in_connection(
                connection,
                &paths.paths,
                operation_id,
                vec![deck_id],
                Vec::new(),
                true,
                "extra",
            )
            .expect_err("a reused operation must describe the same start request");
            assert_eq!(conflict.code, "idempotency_conflict");
            Ok(())
        })
        .expect("practice start retries should be idempotent");
}

#[test]
fn response_hydration_failure_occurs_before_session_commit() {
    let paths = TestPaths::new();
    let database = Database::open_in_memory().expect("database should open");
    database
        .with_connection(|connection| {
            let deck_id = create_deck_in_connection(
                connection,
                CreateDeckInput {
                    name: "Hydration boundary".to_string(),
                    description: None,
                },
            )?
            .id;
            let (note_id, _) = create_card(connection, &deck_id)?;
            connection
                .execute(
                    "UPDATE notes SET front_doc = 'not-json' WHERE id = ?1",
                    [note_id.as_str()],
                )
                .map_err(CommandError::database)?;

            let error = start_selected_practice_in_connection(
                connection,
                &paths.paths,
                "b".repeat(32),
                vec![deck_id],
                Vec::new(),
                false,
                "extra",
            )
            .expect_err("invalid response content should fail before commit");
            assert_eq!(error.code, "serialization_error");

            let session_count: i64 = connection
                .query_row(
                    "SELECT COUNT(*) FROM practice_sessions
                     WHERE start_operation_id = ?1",
                    ["b".repeat(32)],
                    |row| row.get(0),
                )
                .map_err(CommandError::database)?;
            assert_eq!(session_count, 0);
            Ok(())
        })
        .expect("failed hydration must not leave an invisible session");
}

#[test]
fn operation_identifiers_cannot_cross_practice_start_kinds() {
    let paths = TestPaths::new();
    let database = Database::open_in_memory().expect("database should open");
    database
        .with_connection(|connection| {
            let deck_id = create_deck_in_connection(
                connection,
                CreateDeckInput {
                    name: "Operation kind".to_string(),
                    description: None,
                },
            )?
            .id;
            let (_, card_id) = create_card(connection, &deck_id)?;
            let operation_id = "c".repeat(32);
            start_selected_practice_in_connection(
                connection,
                &paths.paths,
                operation_id.clone(),
                vec![deck_id],
                Vec::new(),
                false,
                "extra",
            )?;

            let error = start_card_practice_in_connection(
                connection,
                &paths.paths,
                operation_id,
                vec![card_id],
                false,
            )
            .expect_err("selected and custom starts are different operations");
            assert_eq!(error.code, "idempotency_conflict");
            Ok(())
        })
        .expect("operation kind conflicts should be visible");
}

#[test]
fn consecutive_shuffled_sessions_avoid_repeating_the_same_equal_priority_first_card() {
    let paths = TestPaths::new();
    let database = Database::open_in_memory().expect("database should open");
    database
        .with_connection(|connection| {
            let deck_id = create_deck_in_connection(
                connection,
                CreateDeckInput {
                    name: "Shuffle fairness".to_string(),
                    description: None,
                },
            )?
            .id;
            for _ in 0..6 {
                create_card(connection, &deck_id)?;
            }
            let first = start_selected_practice_in_connection(
                connection,
                &paths.paths,
                "1".repeat(32),
                vec![deck_id.clone()],
                Vec::new(),
                true,
                "extra",
            )?;
            let second = start_selected_practice_in_connection(
                connection,
                &paths.paths,
                "2".repeat(32),
                vec![deck_id],
                Vec::new(),
                true,
                "extra",
            )?;
            assert_ne!(
                first.first_window.cards[0].presentation.id,
                second.first_window.cards[0].presentation.id
            );
            Ok(())
        })
        .expect("equivalent shuffled sessions should rotate their first card");
}

#[test]
fn practice_edit_refresh_removes_deleted_reverse_variant_and_updates_snapshot() {
    let paths = TestPaths::new();
    let database = Database::open_in_memory().expect("database should open");
    database
        .with_connection(|connection| {
            let deck_id = create_deck_in_connection(
                connection,
                CreateDeckInput {
                    name: "Edit during practice".to_string(),
                    description: None,
                },
            )?
            .id;
            let (note_id, forward_id) = create_card(connection, &deck_id)?;
            let reverse_id = generate_id(connection)?;
            connection
                .execute(
                    "INSERT INTO cards (
                     id, note_id, deck_id, direction, template_ordinal, created_at, updated_at
                 ) VALUES (?1, ?2, ?3, 'reverse', 0, 10, 10)",
                    params![reverse_id, note_id, deck_id],
                )
                .map_err(CommandError::database)?;
            insert_new_schedule(connection, &reverse_id, 10)?;
            connection
                .execute(
                    "UPDATE notes SET reverse_enabled = 1 WHERE id = ?1",
                    [note_id.as_str()],
                )
                .map_err(CommandError::database)?;

            let session = start_card_practice_in_connection(
                connection,
                &paths.paths,
                "3".repeat(32),
                vec![forward_id.clone(), reverse_id.clone()],
                false,
            )?;
            let edited = validate_content(
                paragraph("Updated question"),
                paragraph("Updated answer"),
                "Updated question".to_string(),
                "Updated answer".to_string(),
            )?;
            connection
                .execute(
                    "UPDATE notes
                 SET front_doc = ?1, back_doc = ?2, front_text = ?3, back_text = ?4,
                     reverse_enabled = 0, revision = revision + 1, updated_at = 20
                 WHERE id = ?5",
                    params![
                        edited.front_doc_json,
                        edited.back_doc_json,
                        edited.front_text,
                        edited.back_text,
                        note_id,
                    ],
                )
                .map_err(CommandError::database)?;
            connection
                .execute(
                    "UPDATE cards SET deleted_at = 20, updated_at = 20 WHERE id = ?1",
                    [reverse_id.as_str()],
                )
                .map_err(CommandError::database)?;

            let refreshed = refresh_practice_after_card_edit_in_connection(
                connection,
                &paths.paths,
                RefreshPracticeAfterCardEditInput {
                    session_id: session.id,
                    note_id,
                    start_position: 0,
                },
            )?;
            assert_eq!(refreshed.card_count, 1);
            assert!(!refreshed.completed);
            assert_eq!(refreshed.window.cards.len(), 1);
            assert_eq!(refreshed.window.cards[0].presentation.id, forward_id);
            assert_eq!(
                refreshed.window.cards[0].presentation.front_text,
                "Updated question"
            );
            Ok(())
        })
        .expect("practice session should refresh after editing its note");
}

#[test]
fn practice_start_persists_snapshot_and_returns_a_bounded_first_window() {
    let paths = TestPaths::new();
    let database = Database::open_in_memory().expect("database should open");
    database
        .with_connection(|connection| {
            let deck_id = create_deck_in_connection(
                connection,
                CreateDeckInput {
                    name: "Snapshots".to_string(),
                    description: None,
                },
            )?
            .id;
            let (_, card_id) = create_card(connection, &deck_id)?;
            let session = start_card_practice_in_connection(
                connection,
                &paths.paths,
                "d".repeat(32),
                vec![card_id.clone()],
                false,
            )?;
            assert_eq!(session.first_window.cards.len(), 1);
            let snapshot_count: i64 = connection
                .query_row(
                    "SELECT COUNT(*) FROM practice_session_card_snapshots
                     WHERE session_id = ?1 AND card_id = ?2",
                    params![session.id, card_id],
                    |row| row.get(0),
                )
                .map_err(CommandError::database)?;
            assert_eq!(snapshot_count, 1);
            Ok(())
        })
        .expect("practice snapshot should be durable");
}
