use rusqlite::{params, Connection};
use serde_json::json;

use super::*;
use crate::{
    cards::validate_content,
    database::Database,
    decks::{create_deck_in_connection, CreateDeckInput},
    scheduler::{insert_new_schedule, load_schedule},
    util::generate_id,
};

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
    question: &str,
    position: i64,
) -> Result<String, CommandError> {
    let note_id = generate_id(connection)?;
    let card_id = generate_id(connection)?;
    let content = validate_content(
        paragraph(question),
        paragraph("Answer"),
        question.to_string(),
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
                 ?3, ?4, ?5, ?6, ?7, 10, 10, 1
             )",
            params![
                note_id,
                deck_id,
                content.front_doc_json,
                content.back_doc_json,
                content.front_text,
                content.back_text,
                format!("operation-test-{position}"),
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
    Ok(card_id)
}

fn create_session(
    connection: &mut Connection,
    card_ids: &[String],
) -> Result<String, CommandError> {
    let session_id = generate_id(connection)?;
    connection
        .execute(
            "INSERT INTO practice_sessions (
                 id, selected_deck_ids_json, shuffle, card_count, mode, started_at
             ) VALUES (?1, '[]', 0, ?2, 'custom', 10)",
            params![
                session_id,
                i64::try_from(card_ids.len()).unwrap_or(i64::MAX)
            ],
        )
        .map_err(CommandError::database)?;
    for (position, card_id) in card_ids.iter().enumerate() {
        connection
            .execute(
                "INSERT INTO practice_session_cards (
                     session_id, card_id, position, presentation_variant
                 ) VALUES (?1, ?2, ?3, '')",
                params![
                    session_id,
                    card_id,
                    i64::try_from(position).unwrap_or(i64::MAX),
                ],
            )
            .map_err(CommandError::database)?;
    }
    Ok(session_id)
}

fn review_input(
    session_id: &str,
    card_id: &str,
    submission: char,
    outcome: &str,
) -> SubmitReviewInput {
    SubmitReviewInput {
        session_id: session_id.to_string(),
        card_id: card_id.to_string(),
        submission_id: submission.to_string().repeat(32),
        outcome: outcome.to_string(),
        response_time_ms: 1_250,
        schedule_revision: 1,
    }
}

#[test]
fn equivalent_review_retry_returns_the_original_committed_result() {
    let database = Database::open_in_memory().expect("database should open");
    database
        .with_connection(|connection| {
            let deck_id = create_deck_in_connection(
                connection,
                CreateDeckInput {
                    name: "Retry safety".to_string(),
                    description: None,
                },
            )?
            .id;
            let first_card = create_card(connection, &deck_id, "First", 0)?;
            let second_card = create_card(connection, &deck_id, "Second", 1)?;
            let session_id =
                create_session(connection, &[first_card.clone(), second_card.clone()])?;

            record_practice_event(connection, &session_id, &first_card, true)?;
            let first_input = review_input(&session_id, &first_card, 'a', "somewhat_confident");
            let first = submit_review_in_connection(connection, first_input.clone())?;
            assert_eq!(first.rated_count, 1);
            assert_eq!(first.correct_count, 1);
            assert!(!first.completed);

            record_practice_event(connection, &session_id, &second_card, true)?;
            let second = submit_review_in_connection(
                connection,
                review_input(&session_id, &second_card, 'b', "very_confident"),
            )?;
            assert_eq!(second.rated_count, 2);
            assert!(second.completed);

            let replay = submit_review_in_connection(connection, first_input)?;
            assert_eq!(replay.card_id, first.card_id);
            assert_eq!(replay.rating, first.rating);
            assert_eq!(replay.outcome, first.outcome);
            assert_eq!(replay.due_at, first.due_at);
            assert_eq!(replay.interval_ms, first.interval_ms);
            assert_eq!(replay.rated_count, 1);
            assert_eq!(replay.correct_count, 1);
            assert!(!replay.completed);

            let event_count: i64 = connection
                .query_row(
                    "SELECT COUNT(*) FROM review_events WHERE submission_id = ?1",
                    ["a".repeat(32)],
                    |row| row.get(0),
                )
                .map_err(CommandError::database)?;
            assert_eq!(event_count, 1);
            assert_eq!(load_schedule(connection, &first_card)?.reps, 1);

            let stored: (Option<String>, Option<i64>, Option<i64>, Option<i64>) = connection
                .query_row(
                    "SELECT request_fingerprint, result_completed,
                            result_rated_count, result_correct_count
                     FROM review_events WHERE submission_id = ?1",
                    ["a".repeat(32)],
                    |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
                )
                .map_err(CommandError::database)?;
            assert!(stored
                .0
                .as_deref()
                .is_some_and(|value| value.starts_with("review:v1:")));
            assert_eq!(stored.1, Some(0));
            assert_eq!(stored.2, Some(1));
            assert_eq!(stored.3, Some(1));
            Ok(())
        })
        .expect("an equivalent retry should be side-effect free");
}

#[test]
fn reused_submission_id_with_a_different_payload_is_rejected() {
    let database = Database::open_in_memory().expect("database should open");
    database
        .with_connection(|connection| {
            let deck_id = create_deck_in_connection(
                connection,
                CreateDeckInput {
                    name: "Conflict safety".to_string(),
                    description: None,
                },
            )?
            .id;
            let card_id = create_card(connection, &deck_id, "Question", 0)?;
            let session_id = create_session(connection, std::slice::from_ref(&card_id))?;
            record_practice_event(connection, &session_id, &card_id, true)?;

            let accepted = review_input(&session_id, &card_id, 'c', "somewhat_confident");
            submit_review_in_connection(connection, accepted.clone())?;

            let mut conflicting = accepted;
            conflicting.outcome = "wrong".to_string();
            let error = submit_review_in_connection(connection, conflicting)
                .expect_err("a reused identifier must describe the same request");
            assert_eq!(error.code, "idempotency_conflict");

            let event_count: i64 = connection
                .query_row(
                    "SELECT COUNT(*) FROM review_events WHERE submission_id = ?1",
                    ["c".repeat(32)],
                    |row| row.get(0),
                )
                .map_err(CommandError::database)?;
            assert_eq!(event_count, 1);
            assert_eq!(load_schedule(connection, &card_id)?.reps, 1);
            Ok(())
        })
        .expect("conflicting retries should not mutate state");
}
