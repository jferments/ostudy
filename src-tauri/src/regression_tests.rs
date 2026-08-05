use rusqlite::{params, Connection};
use serde_json::{json, Value};

use crate::{
    cards::{validate_content, validate_document},
    database::Database,
    decks::{create_deck_in_connection, CreateDeckInput},
    error::CommandError,
    scheduler::insert_new_schedule,
    settings::load_settings,
    study_calendar::StudyTimeContext,
    studyability::learning_signal_cte,
    util::{generate_id, unix_time_millis},
    workload::{planned_scheduled_cards_scoped, workload_overview_scoped},
};

fn paragraph(text: &str) -> Value {
    json!({
        "type": "doc",
        "content": [{
            "type": "paragraph",
            "content": [{"type": "text", "text": text}]
        }]
    })
}

fn create_deck(connection: &mut Connection, name: &str) -> Result<String, CommandError> {
    Ok(create_deck_in_connection(
        connection,
        CreateDeckInput {
            name: name.to_string(),
            description: None,
        },
    )?
    .id)
}

fn create_basic_card(
    connection: &mut Connection,
    deck_id: &str,
    question: &str,
    answer: &str,
    now: i64,
) -> Result<(String, String), CommandError> {
    let content = validate_content(
        paragraph(question),
        paragraph(answer),
        "untrusted question snapshot".to_string(),
        "untrusted answer snapshot".to_string(),
    )?;
    let note_id = generate_id(connection)?;
    let card_id = generate_id(connection)?;
    connection
        .execute(
            "INSERT INTO notes (
                 id, deck_id, note_type, content_format, content_schema_version,
                 front_doc, back_doc, front_text, back_text, duplicate_hash,
                 created_at, updated_at, revision
             ) VALUES (
                 ?1, ?2, 'basic', 'tiptap-json', 1,
                 ?3, ?4, ?5, ?6, '', ?7, ?7, 1
             )",
            params![
                note_id,
                deck_id,
                content.front_doc_json,
                content.back_doc_json,
                content.front_text,
                content.back_text,
                now,
            ],
        )
        .map_err(CommandError::database)?;
    connection
        .execute(
            "INSERT INTO cards (
                 id, note_id, deck_id, direction, template_ordinal,
                 created_at, updated_at
             ) VALUES (?1, ?2, ?3, 'forward', 0, ?4, ?4)",
            params![card_id, note_id, deck_id, now],
        )
        .map_err(CommandError::database)?;
    insert_new_schedule(connection, &card_id, now)?;
    Ok((note_id, card_id))
}

fn insert_review(
    connection: &Connection,
    card_id: &str,
    reviewed_at: i64,
    remembered: bool,
) -> Result<(), CommandError> {
    let event_id = generate_id(connection)?;
    let submission_id = generate_id(connection)?;
    let (rating, confidence) = if remembered {
        (3_i64, "somewhat_confident")
    } else {
        (1_i64, "wrong")
    };
    connection
        .execute(
            "INSERT INTO review_events (
                 id, submission_id, session_id, card_id, reviewed_at,
                 rating, answer_correct, confidence, response_time_ms,
                 state_before, state_after, due_before, due_after,
                 stability_before, stability_after,
                 difficulty_before, difficulty_after,
                 scheduled_days_before, scheduled_days_after,
                 scheduler_name, scheduler_version, created_at
             ) VALUES (
                 ?1, ?2, NULL, ?3, ?4,
                 ?5, ?6, ?7, 1000,
                 2, 2, ?4, ?4,
                 1.0, 1.0, 5.0, 5.0,
                 1, 1, 'fsrs', 'test', ?4
             )",
            params![
                event_id,
                submission_id,
                card_id,
                reviewed_at,
                rating,
                i64::from(remembered),
                confidence,
            ],
        )
        .map_err(CommandError::database)?;
    Ok(())
}

fn insert_reset_event(
    connection: &Connection,
    card_id: &str,
    occurred_at: i64,
) -> Result<(), CommandError> {
    let operation_id = generate_id(connection)?;
    connection
        .execute(
            "INSERT INTO schedule_action_operations (
                 id, request_fingerprint, session_id, action, due_at,
                 requested_count, changed_count, unchanged_count, rejected_count,
                 occurred_at, created_at
             ) VALUES (?1, ?2, NULL, 'reset', NULL, 1, 1, 0, 0, ?3, ?3)",
            params![
                operation_id,
                format!("regression-reset:v1:{card_id}:{occurred_at}"),
                occurred_at,
            ],
        )
        .map_err(CommandError::database)?;
    connection
        .execute(
            "INSERT INTO schedule_action_events (
                 id, operation_id, session_id, card_id, action,
                 due_before, due_after, state_before, state_after,
                 suspended_before, suspended_after,
                 buried_until_before, buried_until_after,
                 schedule_revision_before, schedule_revision_after,
                 occurred_at, created_at
             ) VALUES (
                 ?1, ?2, NULL, ?3, 'reset',
                 ?4, ?4, 2, 0,
                 NULL, NULL, NULL, NULL,
                 1, 2, ?4, ?4
             )",
            params![generate_id(connection)?, operation_id, card_id, occurred_at],
        )
        .map_err(CommandError::database)?;
    Ok(())
}

fn projected_needs_more_practice(
    connection: &Connection,
    card_id: &str,
) -> Result<bool, CommandError> {
    let sql = format!(
        "WITH {signals}
         SELECT COALESCE((
             SELECT needs_more_practice FROM learning_signals WHERE card_id = ?1
         ), 0)",
        signals = learning_signal_cte(),
    );
    connection
        .query_row(&sql, [card_id], |row| row.get::<_, i64>(0))
        .map(|value| value != 0)
        .map_err(CommandError::database)
}

fn fts_count(connection: &Connection, query: &str) -> Result<i64, CommandError> {
    connection
        .query_row(
            "SELECT COUNT(*) FROM card_search_fts
             WHERE card_search_fts MATCH ?1",
            [query],
            |row| row.get(0),
        )
        .map_err(CommandError::database)
}

#[test]
fn rich_content_derives_authoritative_text_and_rejects_unsafe_structures() {
    let content = validate_content(
        paragraph("Rendered question"),
        paragraph("Rendered answer"),
        "Different snapshot".to_string(),
        "Another snapshot".to_string(),
    )
    .expect("valid documents should be accepted");
    assert_eq!(content.front_text, "Rendered question");
    assert_eq!(content.back_text, "Rendered answer");

    let remote_image = json!({
        "type": "doc",
        "content": [{
            "type": "image",
            "attrs": {
                "assetId": "a".repeat(32),
                "src": "https://example.org/tracker.png"
            }
        }]
    });
    assert_eq!(
        validate_document(&remote_image)
            .expect_err("remote image URLs must fail")
            .code,
        "invalid_image_reference"
    );

    let unknown_child = json!({
        "type": "doc",
        "content": [{"type": "futureWidget"}]
    });
    assert_eq!(
        validate_document(&unknown_child)
            .expect_err("unknown nodes must fail visibly")
            .code,
        "unsupported_card_content"
    );

    let misplaced_known_child = json!({
        "type": "doc",
        "content": [{
            "type": "paragraph",
            "content": [{"type": "horizontalRule"}]
        }]
    });
    assert_eq!(
        validate_document(&misplaced_known_child)
            .expect_err("known nodes in invalid positions must be malformed")
            .code,
        "invalid_card_document"
    );

    let oversized_snapshot = "x".repeat(200_001);
    let oversized_error = match validate_content(
        paragraph("Question"),
        paragraph("Answer"),
        oversized_snapshot,
        String::new(),
    ) {
        Ok(_) => panic!("oversized snapshots must be bounded"),
        Err(error) => error,
    };
    assert_eq!(oversized_error.code, "card_text_too_large");
}

#[test]
fn fts_projection_tracks_content_tags_renames_and_soft_deletion() {
    let database = Database::open_in_memory().expect("database should open");
    database
        .with_connection(|connection| {
            let deck_id = create_deck(connection, "Pharmacology")?;
            let (note_id, _) = create_basic_card(
                connection,
                &deck_id,
                "What reverses warfarin?",
                "Vitamin K",
                10,
            )?;
            assert_eq!(fts_count(connection, "warfarin")?, 1);

            let tag_id = generate_id(connection)?;
            connection
                .execute(
                    "INSERT INTO tags (
                         id, name, normalized_name, created_at, updated_at
                     ) VALUES (?1, 'Anticoagulants', 'anticoagulants', 10, 10)",
                    [tag_id.as_str()],
                )
                .map_err(CommandError::database)?;
            connection
                .execute(
                    "INSERT INTO note_tags (note_id, tag_id) VALUES (?1, ?2)",
                    params![note_id, tag_id],
                )
                .map_err(CommandError::database)?;
            assert_eq!(fts_count(connection, "anticoagulants")?, 1);

            connection
                .execute(
                    "UPDATE tags
                     SET name = 'Blood thinners', normalized_name = 'blood thinners',
                         updated_at = 11
                     WHERE id = ?1",
                    [tag_id.as_str()],
                )
                .map_err(CommandError::database)?;
            assert_eq!(fts_count(connection, "anticoagulants")?, 0);
            assert_eq!(fts_count(connection, "thinners")?, 1);

            connection
                .execute(
                    "UPDATE notes
                     SET front_text = 'Which lab monitors warfarin?', updated_at = 12
                     WHERE id = ?1",
                    [note_id.as_str()],
                )
                .map_err(CommandError::database)?;
            assert_eq!(fts_count(connection, "monitors")?, 1);

            connection
                .execute(
                    "UPDATE notes SET deleted_at = 13, updated_at = 13 WHERE id = ?1",
                    [note_id.as_str()],
                )
                .map_err(CommandError::database)?;
            assert_eq!(fts_count(connection, "warfarin")?, 0);
            assert_eq!(fts_count(connection, "thinners")?, 0);
            Ok(())
        })
        .expect("FTS triggers should remain synchronized");
}

#[test]
fn generated_card_identity_is_unique_even_while_a_presentation_is_disabled() {
    let database = Database::open_in_memory().expect("database should open");
    database
        .with_connection(|connection| {
            let deck_id = create_deck(connection, "Identity")?;
            let (note_id, forward_id) =
                create_basic_card(connection, &deck_id, "Question", "Answer", 20)?;
            connection
                .execute(
                    "INSERT INTO cards (
                         id, note_id, deck_id, direction, template_ordinal,
                         created_at, updated_at
                     ) VALUES (?1, ?2, ?3, 'reverse', 0, 20, 20)",
                    params![generate_id(connection)?, note_id, deck_id],
                )
                .map_err(CommandError::database)?;

            connection
                .execute(
                    "UPDATE cards SET deleted_at = 21 WHERE id = ?1",
                    [forward_id.as_str()],
                )
                .map_err(CommandError::database)?;
            let duplicate = connection.execute(
                "INSERT INTO cards (
                     id, note_id, deck_id, direction, template_ordinal,
                     created_at, updated_at
                 ) VALUES (?1, ?2, ?3, 'forward', 0, 22, 22)",
                params![generate_id(connection)?, note_id, deck_id],
            );
            assert!(
                duplicate.is_err(),
                "disabled identities must be reactivated, not replaced"
            );

            let count: i64 = connection
                .query_row(
                    "SELECT COUNT(*) FROM cards WHERE note_id = ?1",
                    [note_id.as_str()],
                    |row| row.get(0),
                )
                .map_err(CommandError::database)?;
            assert_eq!(count, 2);
            Ok(())
        })
        .expect("generated-card identity constraints should hold");
}

#[test]
fn workload_excludes_suspended_and_buried_cards_and_protects_review_backlog() {
    let database = Database::open_in_memory().expect("database should open");
    database
        .with_connection(|connection| {
            let deck_id = create_deck(connection, "Workload")?;
            let now = unix_time_millis()?;
            let (_, review_id) = create_basic_card(connection, &deck_id, "Review", "A", now)?;
            let (_, new_id) = create_basic_card(connection, &deck_id, "New", "A", now)?;
            let (_, suspended_id) = create_basic_card(connection, &deck_id, "Suspended", "A", now)?;
            let (_, buried_id) = create_basic_card(connection, &deck_id, "Buried", "A", now)?;

            connection
                .execute(
                    "UPDATE card_schedules SET state = 2, due_at = ?1 WHERE card_id = ?2",
                    params![now - 1_000, review_id],
                )
                .map_err(CommandError::database)?;
            connection
                .execute(
                    "UPDATE card_schedules SET due_at = ?1 WHERE card_id = ?2",
                    params![now - 1_000, new_id],
                )
                .map_err(CommandError::database)?;
            connection
                .execute(
                    "UPDATE card_schedules SET state = 2, due_at = ?1 WHERE card_id = ?2",
                    params![now - 1_000, suspended_id],
                )
                .map_err(CommandError::database)?;
            connection
                .execute(
                    "UPDATE cards SET suspended_at = ?1 WHERE id = ?2",
                    params![now, suspended_id],
                )
                .map_err(CommandError::database)?;
            connection
                .execute(
                    "UPDATE card_schedules SET state = 2, due_at = ?1 WHERE card_id = ?2",
                    params![now - 1_000, buried_id],
                )
                .map_err(CommandError::database)?;
            connection
                .execute(
                    "UPDATE cards SET buried_until = ?1 WHERE id = ?2",
                    params![now + 86_400_000, buried_id],
                )
                .map_err(CommandError::database)?;

            let mut settings = load_settings(connection)?;
            settings.daily_new_limit = 1;
            settings.daily_review_limit = 1;
            let context = StudyTimeContext::capture()?;
            let overview = workload_overview_scoped(
                connection,
                std::slice::from_ref(&deck_id),
                &[],
                &settings,
                &context,
            )?;
            assert_eq!(overview.due_reviews, 1);
            assert_eq!(overview.due_new, 1);
            assert_eq!(overview.scheduled_reviews, 1);
            assert_eq!(overview.scheduled_new, 1);
            assert_eq!(overview.extra_available, 2);

            let (planned, _) = planned_scheduled_cards_scoped(
                connection,
                std::slice::from_ref(&deck_id),
                &[],
                &settings,
                &context,
            )?;
            assert_eq!(planned.len(), 2);
            assert_eq!(planned[0].card_id, review_id);
            assert_eq!(planned[1].card_id, new_id);

            settings.daily_review_limit = 0;
            let protected =
                workload_overview_scoped(connection, &[deck_id], &[], &settings, &context)?;
            assert_eq!(protected.scheduled_reviews, 0);
            assert_eq!(protected.scheduled_new, 0);
            assert_eq!(protected.omitted_due_reviews, 1);
            assert_eq!(protected.omitted_due_new, 1);
            Ok(())
        })
        .expect("workload policy should remain authoritative");
}

#[test]
fn needs_more_practice_uses_the_current_post_reset_learning_epoch() {
    let database = Database::open_in_memory().expect("database should open");
    database
        .with_connection(|connection| {
            let deck_id = create_deck(connection, "Signals")?;
            let (_, card_id) = create_basic_card(connection, &deck_id, "Signal", "A", 1)?;

            for (time, remembered) in [
                (100, false),
                (200, true),
                (300, false),
                (400, true),
                (500, false),
            ] {
                insert_review(connection, &card_id, time, remembered)?;
            }
            assert!(projected_needs_more_practice(connection, &card_id)?);

            insert_review(connection, &card_id, 600, true)?;
            insert_review(connection, &card_id, 700, true)?;
            assert!(!projected_needs_more_practice(connection, &card_id)?);

            insert_reset_event(connection, &card_id, 800)?;
            assert!(!projected_needs_more_practice(connection, &card_id)?);
            insert_review(connection, &card_id, 900, false)?;
            insert_review(connection, &card_id, 1_000, false)?;
            assert!(projected_needs_more_practice(connection, &card_id)?);
            Ok(())
        })
        .expect("learning signals should be reproducible from history");
}

#[test]
fn immutable_review_references_prevent_physical_card_removal() {
    let database = Database::open_in_memory().expect("database should open");
    database
        .with_connection(|connection| {
            let deck_id = create_deck(connection, "History")?;
            let (_, card_id) = create_basic_card(connection, &deck_id, "Question", "Answer", 1)?;
            insert_review(connection, &card_id, 2, true)?;

            let deletion =
                connection.execute("DELETE FROM cards WHERE id = ?1", [card_id.as_str()]);
            assert!(
                deletion.is_err(),
                "review history must retain its card identity"
            );
            let review_count: i64 = connection
                .query_row(
                    "SELECT COUNT(*) FROM review_events WHERE card_id = ?1",
                    [card_id.as_str()],
                    |row| row.get(0),
                )
                .map_err(CommandError::database)?;
            assert_eq!(review_count, 1);
            Ok(())
        })
        .expect("review foreign keys should remain restrictive");
}
