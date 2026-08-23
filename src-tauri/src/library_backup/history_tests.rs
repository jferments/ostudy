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
        let tag_id = generate_id(connection)?;
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
        connection
            .execute(
                "INSERT INTO tags (id, name, normalized_name, created_at, updated_at)
                 VALUES (?1, 'Important', 'important', 10, 10)",
                [tag_id.as_str()],
            )
            .map_err(CommandError::database)?;
        connection
            .execute(
                "INSERT INTO note_tags (note_id, tag_id) VALUES (?1, ?2)",
                params![note_id, tag_id],
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
                     rated_count, correct_count, mode, selected_tag_ids_json
                 ) VALUES (?1, ?2, 0, 1, 1, 1, 20, 30, 1, 1, 'scheduled', ?3)",
                params![
                    session_id,
                    serde_json::to_string(&vec![deck.id.clone()])
                        .map_err(CommandError::serialization)?,
                    serde_json::to_string(&vec![tag_id.clone()])
                        .map_err(CommandError::serialization)?,
                ],
            )
            .map_err(CommandError::database)?;
        connection
            .execute(
                "INSERT INTO practice_session_cards (
                     session_id, card_id, position, viewed_at, revealed_at,
                     rated_at, rating, response_time_ms, presentation_variant
                 ) VALUES (?1, ?2, 0, 21, 22, 23, 3, 1200, 'forward')",
                params![session_id, card_id],
            )
            .map_err(CommandError::database)?;
        connection
            .execute(
                "INSERT INTO practice_session_card_snapshots (
                     session_id, card_id, note_id, note_revision,
                     owner_deck_id_at_capture, captured_deck_name,
                     direction, card_type, card_label, presentation_variant,
                     front_doc_json, back_doc_json, front_text, back_text,
                     question_excerpt, question_text_hash,
                     question_was_truncated, captured_at
                 ) VALUES (
                     ?1, ?2, ?3, 1,
                     ?4, 'Backed up deck',
                     'forward', 'basic', 'Basic', 'forward',
                     ?5, ?6, 'Question', 'Answer',
                     'Question', ?7, 0, 20
                 )",
                params![
                    session_id,
                    card_id,
                    note_id,
                    deck.id,
                    content.front_doc_json,
                    content.back_doc_json,
                    "a".repeat(64),
                ],
            )
            .map_err(CommandError::database)?;
        connection
            .execute(
                "INSERT INTO practice_session_card_tag_snapshots (
                     session_id, card_id, tag_id, tag_name
                 ) VALUES (?1, ?2, ?3, 'Important')",
                params![session_id, card_id, tag_id],
            )
            .map_err(CommandError::database)?;

        let review_id = generate_id(connection)?;
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
                    review_id,
                    generate_id(connection)?,
                    session_id,
                    card_id,
                    "f".repeat(64),
                ],
            )
            .map_err(CommandError::database)?;
        connection
            .execute(
                "INSERT INTO review_event_attributions (
                     review_event_id, reviewed_at, review_local_date,
                     time_zone_id, utc_offset_minutes,
                     card_id, note_id, note_revision,
                     owner_deck_id_at_review, captured_deck_name,
                     direction, card_type, card_label, presentation_variant,
                     question_excerpt, question_text_hash,
                     question_was_truncated, attribution_source
                 ) VALUES (
                     ?1, 23, '2026-08-06',
                     'America/Los_Angeles', -420,
                     ?2, ?3, 1,
                     ?4, 'Backed up deck',
                     'forward', 'basic', 'Basic', 'forward',
                     'Question', ?5, 0, 'captured'
                 )",
                params![review_id, card_id, note_id, deck.id, "b".repeat(64)],
            )
            .map_err(CommandError::database)?;
        connection
            .execute(
                "INSERT INTO review_event_tag_attributions (
                     review_event_id, tag_id, tag_name
                 ) VALUES (?1, ?2, 'Important')",
                params![review_id, tag_id],
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
            let counts: (i64, i64, i64, i64, i64, i64, i64, i64, i64) = connection
                .query_row(
                    "SELECT
                         (SELECT COUNT(*) FROM decks WHERE deleted_at IS NULL),
                         (SELECT COUNT(*) FROM card_templates WHERE deleted_at IS NULL),
                         (SELECT COUNT(*) FROM review_events),
                         (SELECT COUNT(*) FROM practice_sessions),
                         (SELECT COUNT(*) FROM assets
                          WHERE lifecycle_state = 'ready' AND deleted_at IS NULL),
                         (SELECT COUNT(*) FROM practice_session_card_snapshots),
                         (SELECT COUNT(*) FROM practice_session_card_tag_snapshots),
                         (SELECT COUNT(*) FROM review_event_attributions),
                         (SELECT COUNT(*) FROM review_event_tag_attributions)",
                    [],
                    |row| {
                        Ok((
                            row.get(0)?,
                            row.get(1)?,
                            row.get(2)?,
                            row.get(3)?,
                            row.get(4)?,
                            row.get(5)?,
                            row.get(6)?,
                            row.get(7)?,
                            row.get(8)?,
                        ))
                    },
                )
                .map_err(CommandError::database)?;
            assert_eq!(counts, (1, 1, 1, 1, 1, 1, 1, 1, 1));
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
            let attribution: (String, String, String, String) = connection
                .query_row(
                    "SELECT a.review_local_date, a.time_zone_id,
                            a.question_excerpt, t.tag_name
                     FROM review_event_attributions a
                     JOIN review_event_tag_attributions t
                       ON t.review_event_id = a.review_event_id",
                    [],
                    |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
                )
                .map_err(CommandError::database)?;
            assert_eq!(
                attribution,
                (
                    "2026-08-06".to_string(),
                    "America/Los_Angeles".to_string(),
                    "Question".to_string(),
                    "Important".to_string(),
                )
            );
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

#[test]
fn current_backup_restores_active_editor_draft_and_its_staged_image() {
    let source_paths = temp_paths("draft-source");
    let source = Database::open(source_paths.database_path()).expect("source database");
    let (draft_id, asset_draft_id, asset_id, staged_relative, payload_json) = source
        .with_connection(|connection| {
            let now = unix_time_millis()?;
            let draft_id = generate_id(connection)?;
            let asset_draft_id = generate_id(connection)?;
            let asset_id = generate_id(connection)?;
            let operation_id = generate_id(connection)?;
            let staged_relative =
                AppPaths::staged_asset_relative_path(&asset_draft_id, &asset_id, "png");
            let final_relative = AppPaths::image_relative_path(&asset_id, "png");
            let payload_json = serde_json::to_string(&json!({
                "frontDoc": {"type":"doc","content":[{"type":"paragraph"}]},
                "backDoc": {"type":"doc","content":[{"type":"paragraph"}]},
                "frontText": "",
                "backText": "",
                "tags": []
            }))
            .map_err(CommandError::serialization)?;
            fs::write(source_paths.resolve_relative(&staged_relative), png_bytes())
                .map_err(CommandError::io)?;
            connection
                .execute(
                    "INSERT INTO asset_drafts (
                         id, purpose, owner_id, created_at, updated_at, expires_at,
                         retention_mode, editor_draft_id
                     ) VALUES (?1, 'card', NULL, ?2, ?2, ?3, 'persistent_editor', ?4)",
                    params![
                        asset_draft_id,
                        now,
                        now.saturating_add(7 * 24 * 60 * 60 * 1000),
                        draft_id,
                    ],
                )
                .map_err(CommandError::database)?;
            connection
                .execute(
                    "INSERT INTO editor_drafts (
                         id, open_operation_id, draft_kind, target_entity_id,
                         context_entity_id, base_revision, payload_schema_version,
                         payload_json, payload_sha256, revision, state, asset_draft_id,
                         created_at, updated_at, last_opened_at
                     ) VALUES (
                         ?1, ?2, 'card', NULL,
                         NULL, NULL, 1,
                         ?3, ?4, 1, 'active', ?5,
                         ?6, ?6, ?6
                     )",
                    params![
                        draft_id,
                        operation_id,
                        payload_json,
                        "0".repeat(64),
                        asset_draft_id,
                        now,
                    ],
                )
                .map_err(CommandError::database)?;
            connection
                .execute(
                    "INSERT INTO assets (
                         id, original_filename, mime_type, byte_size, relative_path,
                         created_at, lifecycle_state, draft_id, staged_relative_path,
                         updated_at
                     ) VALUES (
                         ?1, 'draft-diagram.png', 'image/png', ?2, ?3,
                         ?4, 'staged', ?5, ?6, ?4
                     )",
                    params![
                        asset_id,
                        png_bytes().len() as i64,
                        final_relative,
                        now,
                        asset_draft_id,
                        staged_relative,
                    ],
                )
                .map_err(CommandError::database)?;
            connection
                .execute(
                    "INSERT INTO editor_draft_assets (draft_id, asset_id) VALUES (?1, ?2)",
                    params![draft_id, asset_id],
                )
                .map_err(CommandError::database)?;
            Ok((
                draft_id,
                asset_draft_id,
                asset_id,
                staged_relative,
                payload_json,
            ))
        })
        .expect("persistent draft fixture");

    let backup_path = source_paths
        .app_data_dir()
        .join("draft-current.ostudybackup");
    let exported = export_backup(&source, &source_paths, &backup_path).expect("current backup");
    assert_eq!(
        exported.asset_count, 0,
        "staged images are not canonical library assets"
    );
    let package = Connection::open_with_flags(&backup_path, OpenFlags::SQLITE_OPEN_READ_ONLY)
        .expect("backup package");
    assert_eq!(backup_version(&package).expect("version"), BACKUP_VERSION);
    let packaged: (i64, String) = package
        .query_row(
            "SELECT COUNT(*), MIN(relative_path) FROM asset_files",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .expect("packaged staged asset");
    assert_eq!(packaged, (1, staged_relative.clone()));
    drop(package);
    drop(source);

    let target_paths = temp_paths("draft-target");
    let pending = pending_restore_path(&target_paths);
    copy_and_sync(&backup_path, &pending).expect("pending backup");
    let guard = apply_pending_restore(&target_paths)
        .expect("restore should prepare")
        .expect("restore should exist");
    let restored = Database::open(target_paths.database_path()).expect("restored database");
    crate::editor_drafts::reconcile_startup(&restored).expect("draft reconciliation");
    crate::asset_lifecycle::reconcile_startup(&restored, &target_paths)
        .expect("asset reconciliation");
    restored
        .with_connection(|connection| {
            let restored_row: (String, String, String, String, String, i64) = connection
                .query_row(
                    "SELECT ed.state, ed.payload_json, ad.retention_mode,
                            a.lifecycle_state, a.staged_relative_path,
                            (SELECT COUNT(*) FROM editor_draft_assets
                             WHERE draft_id = ed.id AND asset_id = a.id)
                     FROM editor_drafts ed
                     JOIN asset_drafts ad ON ad.id = ed.asset_draft_id
                     JOIN assets a ON a.draft_id = ad.id
                     WHERE ed.id = ?1 AND ad.id = ?2 AND a.id = ?3",
                    params![draft_id, asset_draft_id, asset_id],
                    |row| {
                        Ok((
                            row.get(0)?,
                            row.get(1)?,
                            row.get(2)?,
                            row.get(3)?,
                            row.get(4)?,
                            row.get(5)?,
                        ))
                    },
                )
                .map_err(CommandError::database)?;
            assert_eq!(
                restored_row,
                (
                    "active".to_string(),
                    payload_json,
                    "persistent_editor".to_string(),
                    "staged".to_string(),
                    staged_relative.clone(),
                    1,
                )
            );
            Ok(())
        })
        .expect("restored draft ledger");
    assert_eq!(
        fs::read(target_paths.resolve_relative(&staged_relative)).expect("restored draft image"),
        png_bytes()
    );
    drop(restored);
    guard.commit();
    let _ = fs::remove_dir_all(source_paths.app_data_dir());
    let _ = fs::remove_dir_all(target_paths.app_data_dir());
}

#[test]
fn backup_versions_one_and_two_remain_restorable() {
    let source_paths = temp_paths("legacy-source");
    let source = Database::open(source_paths.database_path()).expect("source database");
    source
        .with_connection(|connection| {
            create_deck_in_connection(
                connection,
                CreateDeckInput {
                    name: "Legacy-compatible deck".to_string(),
                    description: None,
                },
            )?;
            Ok(())
        })
        .expect("legacy fixture");
    let current_path = source_paths.app_data_dir().join("current.ostudybackup");
    export_backup(&source, &source_paths, &current_path).expect("current backup");
    drop(source);

    for version in [1_i64, 2_i64] {
        let legacy_path = source_paths
            .app_data_dir()
            .join(format!("legacy-v{version}.ostudybackup"));
        copy_and_sync(&current_path, &legacy_path).expect("legacy package copy");
        {
            let package = Connection::open(&legacy_path).expect("writable legacy package");
            package
                .execute(
                    "UPDATE backup_metadata SET value = ?1 WHERE key = 'formatVersion'",
                    [version.to_string()],
                )
                .expect("legacy version metadata");
        }
        let summary =
            validate_backup_package(&legacy_path, None).expect("legacy package validation");
        assert_eq!(summary.deck_count, 1);

        let target_paths = temp_paths(&format!("legacy-target-v{version}"));
        copy_and_sync(&legacy_path, &pending_restore_path(&target_paths))
            .expect("pending legacy backup");
        let guard = apply_pending_restore(&target_paths)
            .expect("legacy restore should prepare")
            .expect("legacy restore should exist");
        let restored =
            Database::open(target_paths.database_path()).expect("legacy restored database");
        let deck_count = restored
            .with_connection(|connection| {
                connection
                    .query_row(
                        "SELECT COUNT(*) FROM decks WHERE deleted_at IS NULL",
                        [],
                        |row| row.get::<_, i64>(0),
                    )
                    .map_err(CommandError::database)
            })
            .expect("legacy deck count");
        assert_eq!(deck_count, 1);
        drop(restored);
        guard.commit();
        let _ = fs::remove_dir_all(target_paths.app_data_dir());
    }
    let _ = fs::remove_dir_all(source_paths.app_data_dir());
}
