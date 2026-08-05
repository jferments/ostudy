use super::*;
use crate::{
    cards::validate_content,
    decks::{create_deck_in_connection, CreateDeckInput},
    scheduler::insert_new_schedule,
    util::generate_id,
};
use rusqlite::params;
use serde_json::json;

fn temp_paths(label: &str) -> AppPaths {
    let root = std::env::temp_dir().join(format!(
        "ostudy-backup-extra-{label}-{}-{}",
        std::process::id(),
        unix_time_millis().expect("clock")
    ));
    AppPaths::new(root).expect("paths")
}

fn png_bytes() -> Vec<u8> {
    vec![0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a, 0, 0, 0, 0]
}

fn populate_complete_library(
    database: &Database,
    paths: &AppPaths,
) -> Result<(String, String), CommandError> {
    database.with_connection(|connection| {
        let deck = create_deck_in_connection(
            connection,
            CreateDeckInput {
                name: "Backed up deck".to_string(),
                description: Some("Complete history".to_string()),
            },
        )?;
        let note_id = generate_id(connection)?;
        let card_id = generate_id(connection)?;
        let asset_id = generate_id(connection)?;
        let asset_relative = AppPaths::image_relative_path(&asset_id, "png");
        let asset_path = paths.resolve_relative(&asset_relative);
        fs::write(&asset_path, png_bytes()).map_err(CommandError::io)?;
        let front_doc = json!({
            "type": "doc",
            "content": [
                {"type":"paragraph","content":[{"type":"text","text":"Question"}]},
                {"type":"image","attrs":{
                    "assetId": asset_id,
                    "src": format!("study-asset://{asset_id}"),
                    "alt": "diagram.png"
                }}
            ]
        });
        let back_doc = json!({
            "type": "doc",
            "content": [{"type":"paragraph","content":[{"type":"text","text":"Answer"}]}]
        });
        let content = validate_content(front_doc, back_doc, String::new(), String::new())?;
        connection
            .execute(
                "INSERT INTO assets (
                     id, original_filename, mime_type, byte_size,
                     relative_path, created_at, lifecycle_state,
                     ready_at, verified_at, updated_at
                 ) VALUES (
                     ?1, 'diagram.png', 'image/png', ?2,
                     ?3, 10, 'ready', 10, 10, 10
                 )",
                params![asset_id, png_bytes().len() as i64, asset_relative],
            )
            .map_err(CommandError::database)?;
        connection
            .execute(
                "INSERT INTO notes (
                     id, deck_id, note_type, content_format, content_schema_version,
                     front_doc, back_doc, front_text, back_text, duplicate_hash,
                     created_at, updated_at, revision
                 ) VALUES (
                     ?1, ?2, 'basic', 'tiptap-json', 1,
                     ?3, ?4, ?5, ?6, 'backup-hash', 10, 10, 1
                 )",
                params![
                    note_id,
                    deck.id,
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
                params![card_id, note_id, deck.id],
            )
            .map_err(CommandError::database)?;
        insert_new_schedule(connection, &card_id, 10)?;
        connection
            .execute(
                "UPDATE card_schedules
                 SET state = 2, due_at = 5000, reps = 7,
                     stability = 3.0, difficulty = 4.5, revision = 4
                 WHERE card_id = ?1",
                [card_id.as_str()],
            )
            .map_err(CommandError::database)?;
        connection
            .execute(
                "INSERT INTO note_assets (note_id, asset_id, side)
                 VALUES (?1, ?2, 'front')",
                params![note_id, asset_id],
            )
            .map_err(CommandError::database)?;

        let template_id = generate_id(connection)?;
        let field_id = generate_id(connection)?;
        let rule_id = generate_id(connection)?;
        connection
            .execute(
                "INSERT INTO card_templates (
                     id, name, normalized_name, description,
                     created_at, updated_at, revision
                 ) VALUES (?1, 'Reusable backup template',
                           'reusable backup template', '', 10, 10, 1)",
                [template_id.as_str()],
            )
            .map_err(CommandError::database)?;
        connection
            .execute(
                "INSERT INTO card_template_fields (
                     id, template_id, name, normalized_name, description, position
                 ) VALUES (?1, ?2, 'Term', 'term', '', 0)",
                params![field_id, template_id],
            )
            .map_err(CommandError::database)?;
        let template_doc = serde_json::to_string(&json!({
            "type": "doc",
            "content": [{
                "type": "paragraph",
                "content": [{
                    "type": "templateFieldInline",
                    "attrs": {"fieldId": field_id, "label": "Term"}
                }]
            }]
        }))
        .map_err(CommandError::serialization)?;
        connection
            .execute(
                "INSERT INTO card_template_rules (
                     id, template_id, name, front_doc, back_doc, position
                 ) VALUES (?1, ?2, 'Term card', ?3, ?3, 0)",
                params![rule_id, template_id, template_doc],
            )
            .map_err(CommandError::database)?;
        connection
            .execute(
                "INSERT INTO card_template_assets (template_id, asset_id)
                 VALUES (?1, ?2)",
                params![template_id, asset_id],
            )
            .map_err(CommandError::database)?;

        let session_id = generate_id(connection)?;
        connection
            .execute(
                "INSERT INTO practice_sessions (
                     id, selected_deck_ids_json, shuffle, card_count,
                     viewed_count, revealed_count, started_at, completed_at,
                     rated_count, correct_count, mode
                 ) VALUES (?1, ?2, 0, 1, 1, 1, 20, 30, 1, 1, 'scheduled')",
                params![
                    session_id,
                    serde_json::to_string(&vec![deck.id.clone()])
                        .map_err(CommandError::serialization)?,
                ],
            )
            .map_err(CommandError::database)?;
        connection
            .execute(
                "INSERT INTO practice_session_cards (
                     session_id, card_id, position, viewed_at, revealed_at,
                     rated_at, rating, response_time_ms, presentation_variant
                 ) VALUES (?1, ?2, 0, 21, 22, 23, 3, 1200, '')",
                params![session_id, card_id],
            )
            .map_err(CommandError::database)?;
        connection
            .execute(
                "INSERT INTO review_events (
                     id, submission_id, session_id, card_id, reviewed_at,
                     rating, answer_correct, confidence, response_time_ms,
                     state_before, state_after, due_before, due_after,
                     stability_before, stability_after,
                     difficulty_before, difficulty_after,
                     scheduled_days_before, scheduled_days_after,
                     scheduler_name, scheduler_version, created_at,
                     request_fingerprint, result_completed,
                     result_rated_count, result_correct_count
                 ) VALUES (
                     ?1, ?2, ?3, ?4, 23,
                     3, 1, 'somewhat_confident', 1200,
                     0, 2, 10, 5000,
                     0.0, 3.0, 0.0, 4.5,
                     0, 5, 'fsrs', 'test', 23,
                     ?5, 1, 1, 1
                 )",
                params![
                    generate_id(connection)?,
                    generate_id(connection)?,
                    session_id,
                    card_id,
                    "f".repeat(64),
                ],
            )
            .map_err(CommandError::database)?;
        connection
            .execute(
                "UPDATE app_settings SET value_json = '12', updated_at = 40
                 WHERE key = 'dailyNewLimit'",
                [],
            )
            .map_err(CommandError::database)?;
        Ok((deck.id, asset_relative))
    })
}

#[test]
fn complete_backup_restore_replaces_the_library_with_every_history_source() {
    let source_paths = temp_paths("source-roundtrip");
    let source = Database::open(source_paths.database_path()).expect("source database");
    let (source_deck_id, asset_relative) =
        populate_complete_library(&source, &source_paths).expect("complete fixture");
    let backup_path = source_paths.app_data_dir().join("complete.ostudybackup");
    let exported = export_backup(&source, &source_paths, &backup_path).expect("backup");
    assert_eq!(exported.deck_count, 1);
    assert_eq!(exported.template_count, 1);
    assert_eq!(exported.review_count, 1);
    assert_eq!(exported.asset_count, 1);
    drop(source);

    let target_paths = temp_paths("target-roundtrip");
    {
        let target = Database::open(target_paths.database_path()).expect("target database");
        target
            .with_connection(|connection| {
                create_deck_in_connection(
                    connection,
                    CreateDeckInput {
                        name: "Replaced library".to_string(),
                        description: None,
                    },
                )?;
                Ok(())
            })
            .expect("target fixture");
    }
    let pending = pending_restore_path(&target_paths);
    copy_and_sync(&backup_path, &pending).expect("pending backup");
    let guard = apply_pending_restore(&target_paths)
        .expect("restore should prepare")
        .expect("restore should exist");
    let restored = Database::open(target_paths.database_path()).expect("restored database");
    restored
        .with_connection(|connection| {
            let counts: (i64, i64, i64, i64, i64) = connection
                .query_row(
                    "SELECT
                         (SELECT COUNT(*) FROM decks WHERE deleted_at IS NULL),
                         (SELECT COUNT(*) FROM card_templates WHERE deleted_at IS NULL),
                         (SELECT COUNT(*) FROM review_events),
                         (SELECT COUNT(*) FROM practice_sessions),
                         (SELECT COUNT(*) FROM assets
                          WHERE lifecycle_state = 'ready' AND deleted_at IS NULL)",
                    [],
                    |row| {
                        Ok((
                            row.get(0)?,
                            row.get(1)?,
                            row.get(2)?,
                            row.get(3)?,
                            row.get(4)?,
                        ))
                    },
                )
                .map_err(CommandError::database)?;
            assert_eq!(counts, (1, 1, 1, 1, 1));
            let deck_name: String = connection
                .query_row(
                    "SELECT name FROM decks WHERE id = ?1",
                    [source_deck_id.as_str()],
                    |row| row.get(0),
                )
                .map_err(CommandError::database)?;
            assert_eq!(deck_name, "Backed up deck");
            let schedule: (i64, i64, i64) = connection
                .query_row(
                    "SELECT state, reps, revision FROM card_schedules LIMIT 1",
                    [],
                    |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
                )
                .map_err(CommandError::database)?;
            assert_eq!(schedule, (2, 7, 4));
            let daily_new: String = connection
                .query_row(
                    "SELECT value_json FROM app_settings WHERE key = 'dailyNewLimit'",
                    [],
                    |row| row.get(0),
                )
                .map_err(CommandError::database)?;
            assert_eq!(daily_new, "12");
            Ok(())
        })
        .expect("restored state should match backup");
    assert!(target_paths.resolve_relative(&asset_relative).is_file());
    drop(restored);
    guard.commit();
    assert!(!pending.exists());
    let _ = fs::remove_dir_all(source_paths.app_data_dir());
    let _ = fs::remove_dir_all(target_paths.app_data_dir());
}

#[test]
fn dropping_the_restore_guard_rolls_back_the_previous_library() {
    let source_paths = temp_paths("source-rollback");
    let source = Database::open(source_paths.database_path()).expect("source database");
    populate_complete_library(&source, &source_paths).expect("source fixture");
    let backup_path = source_paths.app_data_dir().join("rollback.ostudybackup");
    export_backup(&source, &source_paths, &backup_path).expect("backup");
    drop(source);

    let target_paths = temp_paths("target-rollback");
    let original_deck_id = {
        let target = Database::open(target_paths.database_path()).expect("target database");
        let id = target
            .with_connection(|connection| {
                Ok(create_deck_in_connection(
                    connection,
                    CreateDeckInput {
                        name: "Original library".to_string(),
                        description: None,
                    },
                )?
                .id)
            })
            .expect("target fixture");
        drop(target);
        id
    };
    let pending = pending_restore_path(&target_paths);
    copy_and_sync(&backup_path, &pending).expect("pending backup");
    let guard = apply_pending_restore(&target_paths)
        .expect("restore swap")
        .expect("restore guard");

    // Model a failure while opening or reconciling the restored database.
    drop(guard);

    let rolled_back = Database::open(target_paths.database_path()).expect("rolled back database");
    rolled_back
        .with_connection(|connection| {
            let name: String = connection
                .query_row(
                    "SELECT name FROM decks WHERE id = ?1",
                    [original_deck_id.as_str()],
                    |row| row.get(0),
                )
                .map_err(CommandError::database)?;
            assert_eq!(name, "Original library");
            let count: i64 = connection
                .query_row(
                    "SELECT COUNT(*) FROM decks WHERE deleted_at IS NULL",
                    [],
                    |row| row.get(0),
                )
                .map_err(CommandError::database)?;
            assert_eq!(count, 1);
            Ok(())
        })
        .expect("prior library should be restored");
    let _ = fs::remove_dir_all(source_paths.app_data_dir());
    let _ = fs::remove_dir_all(target_paths.app_data_dir());
}
