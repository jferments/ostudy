use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use rusqlite::{params, hooks::Action, Connection, OpenFlags};

use crate::{
    analytics::RecentReviewAttributionRow,
    database::{Database, LATEST_SCHEMA_VERSION},
    deletion, decks,
    error::CommandError,
    practice_selection::{
        self, GetPracticeSessionCardsInput, StartDeckPracticeInput, StartSelectedPracticeInput,
    },
    scheduler,
    studyability::{self, CardAvailability},
    tags, util,
};

#[test]
fn moved_card_history_follows_current_owner_and_keeps_review_presentation() {
    let database = Database::open_in_memory().expect("database should open");
    database
        .with_connection(|connection| {
            let deck_a = decks::create_deck_in_connection(
                connection,
                decks::CreateDeckInput {
                    name: "Old owner".to_string(),
                    description: None,
                },
            )?;
            let deck_b = decks::create_deck_in_connection(
                connection,
                decks::CreateDeckInput {
                    name: "Current owner".to_string(),
                    description: None,
                },
            )?;
            let note_id = util::generate_id(connection)?;
            let card_id = util::generate_id(connection)?;
            connection
                .execute(
                    "INSERT INTO notes (
                         id, deck_id, note_type, content_format, content_schema_version,
                         front_doc, back_doc, front_text, back_text,
                         created_at, updated_at, revision
                     ) VALUES (?1, ?2, 'basic', 'tiptap-json', 1, ?3, ?4, ?5, ?6, 1, 1, 1)",
                    params![
                        note_id,
                        deck_a.id,
                        r#"{"type":"doc","content":[{"type":"paragraph","content":[{"type":"text","text":"What is preload?"}]}]}"#,
                        r#"{"type":"doc","content":[{"type":"paragraph","content":[{"type":"text","text":"End-diastolic filling."}]}]}"#,
                        "What is preload?",
                        "End-diastolic filling."
                    ],
                )
                .map_err(CommandError::database)?;
            connection
                .execute(
                    "INSERT INTO cards (
                         id, note_id, deck_id, direction, template_ordinal,
                         created_at, updated_at
                     ) VALUES (?1, ?2, ?3, 'forward', 0, 1, 1)",
                    params![card_id, note_id, deck_a.id],
                )
                .map_err(CommandError::database)?;
            scheduler::insert_new_schedule(connection, &card_id, 1)?;

            let session = practice_selection::start_deck_practice(
                connection,
                StartDeckPracticeInput {
                    operation_id: util::generate_id(connection)?,
                    deck_id: deck_a.id.clone(),
                    shuffle: false,
                    requested_limit: 10,
                    exclude_card_ids: vec![],
                    continuation_token: None,
                },
            )?;
            let started = session
                .cards
                .first()
                .expect("practice card should exist")
                .clone();
            let reviewed_at = 1_800_000_000_000_i64;
            let preview = scheduler::preview_review(
                connection,
                scheduler::PreviewReviewInput {
                    card_id: card_id.clone(),
                    review_time: reviewed_at,
                    desired_retention: 0.9,
                },
            )?;
            scheduler::apply_review(
                connection,
                scheduler::ReviewCardInput {
                    submission_id: util::generate_id(connection)?,
                    session_id: Some(session.session_id.clone()),
                    card_id: card_id.clone(),
                    reviewed_at,
                    grade: 3,
                    answer_correct: Some(true),
                    confidence: "somewhat_confident".to_string(),
                    response_time_ms: 1200,
                    expected_schedule_revision: preview.schedule_revision,
                    desired_retention: 0.9,
                },
            )?;

            connection
                .execute(
                    "UPDATE notes SET deck_id = ?1, updated_at = 2, revision = revision + 1
                     WHERE id = ?2",
                    params![deck_b.id, note_id],
                )
                .map_err(CommandError::database)?;
            connection
                .execute(
                    "UPDATE cards SET deck_id = ?1, updated_at = 2 WHERE id = ?2",
                    params![deck_b.id, card_id],
                )
                .map_err(CommandError::database)?;

            let scoped = crate::analytics::recent_review_attributions_for_scope(
                connection,
                crate::analytics::AnalyticsScope {
                    deck_id: Some(deck_b.id.clone()),
                    tag_id: None,
                    start_timestamp: 0,
                    end_timestamp: i64::MAX,
                    limit: 10,
                },
            )?;
            assert_eq!(scoped.len(), 1);
            let attribution = &scoped[0];
            assert_eq!(attribution.owner_deck_id_at_review, deck_a.id);
            assert_eq!(attribution.captured_deck_name, "Old owner");
            assert_eq!(attribution.question_excerpt, "What is preload?");
            assert_eq!(attribution.card_label, "Basic");
            assert_eq!(attribution.presentation_variant, "");
            assert_eq!(attribution.current_deck_id.as_deref(), Some(deck_b.id.as_str()));
            assert_eq!(attribution.current_deck_name.as_deref(), Some("Current owner"));
            assert!(started.question.contains("What is preload?"));
            Ok(())
        })
        .expect("moved-card history should remain queryable");
}

#[test]
fn recent_review_tags_preserve_identity_and_match_active_note_currentness() {
    let database = Database::open_in_memory().expect("database should open");
    database
        .with_connection(|connection| {
            let deck = decks::create_deck_in_connection(
                connection,
                decks::CreateDeckInput {
                    name: "Tag history".to_string(),
                    description: None,
                },
            )?;
            let note_id = util::generate_id(connection)?;
            let card_id = util::generate_id(connection)?;
            connection
                .execute(
                    "INSERT INTO notes (
                         id, deck_id, note_type, content_format, content_schema_version,
                         front_doc, back_doc, front_text, back_text,
                         created_at, updated_at, revision
                     ) VALUES (?1, ?2, 'basic', 'tiptap-json', 1, ?3, ?4, 'Question', 'Answer', 1, 1, 1)",
                    params![
                        note_id,
                        deck.id,
                        r#"{"type":"doc","content":[{"type":"paragraph","content":[{"type":"text","text":"Question"}]}]}"#,
                        r#"{"type":"doc","content":[{"type":"paragraph","content":[{"type":"text","text":"Answer"}]}]}"#,
                    ],
                )
                .map_err(CommandError::database)?;
            connection
                .execute(
                    "INSERT INTO cards (
                         id, note_id, deck_id, direction, template_ordinal,
                         created_at, updated_at
                     ) VALUES (?1, ?2, ?3, 'forward', 0, 1, 1)",
                    params![card_id, note_id, deck.id],
                )
                .map_err(CommandError::database)?;
            scheduler::insert_new_schedule(connection, &card_id, 1)?;
            let tag_a = tags::create_tag(
                connection,
                tags::CreateTagInput {
                    name: "Cardiology".to_string(),
                },
            )?;
            tags::set_note_tags(
                connection,
                tags::SetNoteTagsInput {
                    note_id: note_id.clone(),
                    tag_ids: vec![tag_a.id.clone()],
                    expected_revision: 1,
                },
            )?;

            let session = practice_selection::start_deck_practice(
                connection,
                StartDeckPracticeInput {
                    operation_id: util::generate_id(connection)?,
                    deck_id: deck.id.clone(),
                    shuffle: false,
                    requested_limit: 10,
                    exclude_card_ids: vec![],
                    continuation_token: None,
                },
            )?;
            let reviewed_at = 1_800_000_000_000_i64;
            let preview = scheduler::preview_review(
                connection,
                scheduler::PreviewReviewInput {
                    card_id: card_id.clone(),
                    review_time: reviewed_at,
                    desired_retention: 0.9,
                },
            )?;
            scheduler::apply_review(
                connection,
                scheduler::ReviewCardInput {
                    submission_id: util::generate_id(connection)?,
                    session_id: Some(session.session_id.clone()),
                    card_id: card_id.clone(),
                    reviewed_at,
                    grade: 3,
                    answer_correct: Some(true),
                    confidence: "somewhat_confident".to_string(),
                    response_time_ms: 900,
                    expected_schedule_revision: preview.schedule_revision,
                    desired_retention: 0.9,
                },
            )?;

            let tag_b = tags::create_tag(
                connection,
                tags::CreateTagInput {
                    name: "Hemodynamics".to_string(),
                },
            )?;
            let current_revision: i64 = connection
                .query_row(
                    "SELECT revision FROM notes WHERE id = ?1",
                    [note_id.as_str()],
                    |row| row.get(0),
                )
                .map_err(CommandError::database)?;
            tags::set_note_tags(
                connection,
                tags::SetNoteTagsInput {
                    note_id: note_id.clone(),
                    tag_ids: vec![tag_b.id.clone()],
                    expected_revision: current_revision,
                },
            )?;

            let recent = crate::analytics::recent_review_attributions_for_scope(
                connection,
                crate::analytics::AnalyticsScope {
                    deck_id: Some(deck.id.clone()),
                    tag_id: None,
                    start_timestamp: 0,
                    end_timestamp: i64::MAX,
                    limit: 10,
                },
            )?;
            assert_eq!(recent.len(), 1);
            assert_eq!(recent[0].tags.len(), 1);
            assert_eq!(recent[0].tags[0].tag_id, tag_a.id);
            assert_eq!(recent[0].tags[0].tag_name, "Cardiology");
            assert!(!recent[0].tags[0].is_current);

            let current_scoped = crate::analytics::recent_review_attributions_for_scope(
                connection,
                crate::analytics::AnalyticsScope {
                    deck_id: None,
                    tag_id: Some(tag_b.id.clone()),
                    start_timestamp: 0,
                    end_timestamp: i64::MAX,
                    limit: 10,
                },
            )?;
            assert_eq!(current_scoped.len(), 1);
            assert_eq!(current_scoped[0].tags[0].tag_id, tag_a.id);
            assert!(!current_scoped[0].tags[0].is_current);
            Ok(())
        })
        .expect("tag attribution history should remain queryable");
}

#[test]
fn recent_review_limit_precedes_tag_fan_out_and_query_count_is_fixed() {
    let database = Database::open_in_memory().expect("database should open");
    database
        .with_connection(|connection| {
            let deck = decks::create_deck_in_connection(
                connection,
                decks::CreateDeckInput {
                    name: "Bounded analytics".to_string(),
                    description: None,
                },
            )?;
            for index in 0..3 {
                let note_id = format!("{:032x}", index + 1);
                let card_id = format!("{:032x}", index + 101);
                connection
                    .execute(
                        "INSERT INTO notes (
                             id, deck_id, note_type, content_format, content_schema_version,
                             front_doc, back_doc, front_text, back_text,
                             created_at, updated_at, revision
                         ) VALUES (?1, ?2, 'basic', 'tiptap-json', 1, ?3, ?4, ?5, ?6, 1, 1, 1)",
                        params![
                            note_id,
                            deck.id,
                            r#"{"type":"doc","content":[{"type":"paragraph","content":[{"type":"text","text":"Question"}]}]}"#,
                            r#"{"type":"doc","content":[{"type":"paragraph","content":[{"type":"text","text":"Answer"}]}]}"#,
                            format!("Question {index}"),
                            format!("Answer {index}")
                        ],
                    )
                    .map_err(CommandError::database)?;
                connection
                    .execute(
                        "INSERT INTO cards (
                             id, note_id, deck_id, direction, template_ordinal,
                             created_at, updated_at
                         ) VALUES (?1, ?2, ?3, 'forward', 0, 1, 1)",
                        params![card_id, note_id, deck.id],
                    )
                    .map_err(CommandError::database)?;
                scheduler::insert_new_schedule(connection, &card_id, 1)?;
                let tag = tags::create_tag(
                    connection,
                    tags::CreateTagInput {
                        name: format!("Tag {index}"),
                    },
                )?;
                tags::set_note_tags(
                    connection,
                    tags::SetNoteTagsInput {
                        note_id: note_id.clone(),
                        tag_ids: vec![tag.id],
                        expected_revision: 1,
                    },
                )?;
                let session = practice_selection::start_selected_practice(
                    connection,
                    StartSelectedPracticeInput {
                        operation_id: util::generate_id(connection)?,
                        card_ids: vec![card_id.clone()],
                    },
                )?;
                let reviewed_at = 1_800_000_000_000_i64 + i64::from(index);
                let preview = scheduler::preview_review(
                    connection,
                    scheduler::PreviewReviewInput {
                        card_id: card_id.clone(),
                        review_time: reviewed_at,
                        desired_retention: 0.9,
                    },
                )?;
                scheduler::apply_review(
                    connection,
                    scheduler::ReviewCardInput {
                        submission_id: util::generate_id(connection)?,
                        session_id: Some(session.session_id.clone()),
                        card_id,
                        reviewed_at,
                        grade: 3,
                        answer_correct: Some(true),
                        confidence: "somewhat_confident".to_string(),
                        response_time_ms: 500,
                        expected_schedule_revision: preview.schedule_revision,
                        desired_retention: 0.9,
                    },
                )?;
            }

            let query_count = Arc::new(AtomicUsize::new(0));
            let count = Arc::clone(&query_count);
            connection.update_hook(Some(move |action, _database, table, _rowid| {
                if matches!(action, Action::SQLITE_SELECT) && table == "review_events" {
                    count.fetch_add(1, Ordering::SeqCst);
                }
            }));
            let recent = crate::analytics::recent_review_attributions_for_scope(
                connection,
                crate::analytics::AnalyticsScope {
                    deck_id: Some(deck.id.clone()),
                    tag_id: None,
                    start_timestamp: 0,
                    end_timestamp: i64::MAX,
                    limit: 2,
                },
            )?;
            connection.update_hook::<fn(Action, &str, &str, i64)>(None);
            assert_eq!(recent.len(), 2);
            assert_eq!(query_count.load(Ordering::SeqCst), 0);
            Ok(())
        })
        .expect("bounded attribution query should complete");
}

#[test]
fn deleting_old_owner_preserves_moved_history_and_deleting_new_owner_purges_it() {
    let database = Database::open_in_memory().expect("database should open");
    database
        .with_connection(|connection| {
            let old_deck = decks::create_deck_in_connection(
                connection,
                decks::CreateDeckInput {
                    name: "Old deck".to_string(),
                    description: None,
                },
            )?;
            let current_deck = decks::create_deck_in_connection(
                connection,
                decks::CreateDeckInput {
                    name: "Current deck".to_string(),
                    description: None,
                },
            )?;
            let note_id = util::generate_id(connection)?;
            let card_id = util::generate_id(connection)?;
            connection
                .execute(
                    "INSERT INTO notes (
                         id, deck_id, note_type, content_format, content_schema_version,
                         front_doc, back_doc, front_text, back_text,
                         created_at, updated_at, revision
                     ) VALUES (?1, ?2, 'basic', 'tiptap-json', 1, ?3, ?4, 'Question', 'Answer', 1, 1, 1)",
                    params![
                        note_id,
                        old_deck.id,
                        r#"{"type":"doc","content":[{"type":"paragraph","content":[{"type":"text","text":"Question"}]}]}"#,
                        r#"{"type":"doc","content":[{"type":"paragraph","content":[{"type":"text","text":"Answer"}]}]}"#,
                    ],
                )
                .map_err(CommandError::database)?;
            connection
                .execute(
                    "INSERT INTO cards (
                         id, note_id, deck_id, direction, template_ordinal,
                         created_at, updated_at
                     ) VALUES (?1, ?2, ?3, 'forward', 0, 1, 1)",
                    params![card_id, note_id, old_deck.id],
                )
                .map_err(CommandError::database)?;
            scheduler::insert_new_schedule(connection, &card_id, 1)?;
            let session = practice_selection::start_deck_practice(
                connection,
                StartDeckPracticeInput {
                    operation_id: util::generate_id(connection)?,
                    deck_id: old_deck.id.clone(),
                    shuffle: false,
                    requested_limit: 10,
                    exclude_card_ids: vec![],
                    continuation_token: None,
                },
            )?;
            let reviewed_at = 1_800_000_000_000_i64;
            let preview = scheduler::preview_review(
                connection,
                scheduler::PreviewReviewInput {
                    card_id: card_id.clone(),
                    review_time: reviewed_at,
                    desired_retention: 0.9,
                },
            )?;
            scheduler::apply_review(
                connection,
                scheduler::ReviewCardInput {
                    submission_id: util::generate_id(connection)?,
                    session_id: Some(session.session_id),
                    card_id: card_id.clone(),
                    reviewed_at,
                    grade: 3,
                    answer_correct: Some(true),
                    confidence: "somewhat_confident".to_string(),
                    response_time_ms: 500,
                    expected_schedule_revision: preview.schedule_revision,
                    desired_retention: 0.9,
                },
            )?;
            connection
                .execute(
                    "UPDATE notes SET deck_id = ?1, revision = revision + 1 WHERE id = ?2",
                    params![current_deck.id, note_id],
                )
                .map_err(CommandError::database)?;
            connection
                .execute(
                    "UPDATE cards SET deck_id = ?1 WHERE id = ?2",
                    params![current_deck.id, card_id],
                )
                .map_err(CommandError::database)?;

            let deletion = deletion::delete_deck(connection, &old_deck.id)?;
            deletion::finalize_deletion(connection, &deletion.undo_token)?;
            let history_count: i64 = connection
                .query_row("SELECT COUNT(*) FROM review_events", [], |row| row.get(0))
                .map_err(CommandError::database)?;
            assert_eq!(history_count, 1);

            let current_deletion = deletion::delete_deck(connection, &current_deck.id)?;
            deletion::finalize_deletion(connection, &current_deletion.undo_token)?;
            let history_count: i64 = connection
                .query_row("SELECT COUNT(*) FROM review_events", [], |row| row.get(0))
                .map_err(CommandError::database)?;
            assert_eq!(history_count, 0);
            Ok(())
        })
        .expect("deletion history ownership should remain correct");
}

#[test]
fn due_boundary_never_overrides_archive_or_manual_suspension() {
    let now = 1_800_000_000_000_i64;
    assert!(CardAvailability {
        suspended_at: None,
        archive_suspended_at: None,
        buried_until: None,
        due_at: now
    }
    .is_studyable_due(now));
    assert!(!CardAvailability {
        suspended_at: Some(now - 1),
        archive_suspended_at: None,
        buried_until: None,
        due_at: now
    }
    .is_studyable_due(now));
    assert!(!CardAvailability {
        suspended_at: None,
        archive_suspended_at: Some(now - 1),
        buried_until: None,
        due_at: now
    }
    .is_studyable_due(now));
    assert!(!CardAvailability {
        suspended_at: None,
        archive_suspended_at: None,
        buried_until: Some(now + 1),
        due_at: now
    }
    .is_studyable_due(now));
}

#[test]
fn public_baseline_preserves_stage36_38_attribution_schema() {
    let database = Database::open_in_memory().expect("database should open");
    database
        .with_connection(|connection| {
            let schema_version: i64 = connection
                .query_row("PRAGMA user_version", [], |row| row.get(0))
                .expect("schema version should load");
            let schema_lineage: String = connection
                .query_row(
                    "SELECT value FROM schema_metadata WHERE key = 'schema_lineage'",
                    [],
                    |row| row.get(0),
                )
                .expect("public schema lineage should load");
            let public_baseline_present: i64 = connection
                .query_row(
                    "SELECT EXISTS(
                         SELECT 1 FROM schema_migrations
                         WHERE version = 1 AND name = 'public_baseline'
                     )",
                    [],
                    |row| row.get(0),
                )
                .expect("public baseline ledger entry should load");
            let canonical_migration_present: i64 = connection
                .query_row(
                    "SELECT EXISTS(
                         SELECT 1 FROM schema_migrations
                         WHERE version = 2 AND name = 'canonical_documents'
                     )",
                    [],
                    |row| row.get(0),
                )
                .expect("canonical migration ledger entry should load");
            let migration_count: i64 = connection
                .query_row("SELECT COUNT(*) FROM schema_migrations", [], |row| {
                    row.get(0)
                })
                .expect("migration count should load");
            let repair_state: (i64, i64) = connection
                .query_row(
                    "SELECT COUNT(*), COALESCE(SUM(changed_rows), 0) FROM data_repairs",
                    [],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )
                .expect("repair state should load");
            let attribution_tables: i64 = connection
                .query_row(
                    "SELECT COUNT(*) FROM sqlite_master
                     WHERE type = 'table'
                       AND name IN (
                           'practice_session_card_snapshots',
                           'practice_session_card_tag_snapshots',
                           'review_event_attributions',
                           'review_event_tag_attributions'
                       )",
                    [],
                    |row| row.get(0),
                )
                .expect("attribution tables should load");
            assert_eq!(schema_version, LATEST_SCHEMA_VERSION);
            assert_eq!(schema_lineage, "ostudy-public-v1");
            assert_eq!(public_baseline_present, 1);
            assert_eq!(canonical_migration_present, 1);
            assert_eq!(migration_count, LATEST_SCHEMA_VERSION);
            assert_eq!(repair_state, (3, 0));
            assert_eq!(attribution_tables, 4);
            Ok(())
        })
        .expect("public schema should retain Stage 3.6-3.8 attribution state");
}