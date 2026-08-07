use super::*;
use crate::decks::{create_deck_in_connection, CreateDeckInput};

#[test]
fn removing_last_current_tag_assignment_keeps_historical_tag_identity() {
    let database = Database::open_in_memory().expect("database should open");
    database
        .with_connection(|connection| {
            let deck = create_deck_in_connection(
                connection,
                CreateDeckInput {
                    name: "Historical tags".to_string(),
                    description: None,
                },
            )?;
            let content = validate_content(
                plain_text_document("Question"),
                plain_text_document("Answer"),
                "Question".to_string(),
                "Answer".to_string(),
            )?;
            let normalized = normalized_pair(&content.front_text, &content.back_text);
            let transaction = connection.transaction().map_err(CommandError::database)?;
            let note_id = insert_organized_card(
                &transaction,
                &deck.id,
                content,
                &validate_tags(vec!["Renamed tag".to_string()])?,
                &duplicate_hash(&normalized.0, &normalized.1),
                100,
            )?;
            let card_id: String = transaction
                .query_row(
                    "SELECT id FROM cards WHERE note_id = ?1 AND direction = 'forward'",
                    [note_id.as_str()],
                    |row| row.get(0),
                )
                .map_err(CommandError::database)?;
            let tag_id: String = transaction
                .query_row(
                    "SELECT t.id
                     FROM tags t
                     JOIN note_tags nt ON nt.tag_id = t.id
                     WHERE nt.note_id = ?1",
                    [note_id.as_str()],
                    |row| row.get(0),
                )
                .map_err(CommandError::database)?;
            let review_id = generate_id(&transaction)?;
            transaction
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
                         ?1, ?2, NULL, ?3, 200,
                         3, 1, 'somewhat_confident', 500,
                         0, 1, 100, 300,
                         0.0, 1.0,
                         0.0, 5.0,
                         0, 1,
                         'fsrs', 'test', 200
                     )",
                    params![review_id, generate_id(&transaction)?, card_id],
                )
                .map_err(CommandError::database)?;
            transaction
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
                         ?1, 200, '2026-08-06',
                         'Etc/UTC', 0,
                         ?2, ?3, 1,
                         ?4, 'Historical tags',
                         'forward', 'basic', 'Basic', 'forward',
                         'Question', ?5, 0, 'captured'
                     )",
                    params![review_id, card_id, note_id, deck.id, "a".repeat(64)],
                )
                .map_err(CommandError::database)?;
            transaction
                .execute(
                    "INSERT INTO review_event_tag_attributions (
                         review_event_id, tag_id, tag_name
                     ) VALUES (?1, ?2, 'Original captured tag')",
                    params![review_id, tag_id],
                )
                .map_err(CommandError::database)?;

            replace_note_tags(&transaction, &note_id, &[], 300)?;
            let current_assignments: i64 = transaction
                .query_row(
                    "SELECT COUNT(*) FROM note_tags WHERE tag_id = ?1",
                    [tag_id.as_str()],
                    |row| row.get(0),
                )
                .map_err(CommandError::database)?;
            let retained_name: String = transaction
                .query_row(
                    "SELECT name FROM tags WHERE id = ?1",
                    [tag_id.as_str()],
                    |row| row.get(0),
                )
                .map_err(CommandError::database)?;
            assert_eq!(current_assignments, 0);
            assert_eq!(retained_name, "Renamed tag");
            transaction.commit().map_err(CommandError::database)?;
            Ok(())
        })
        .expect("historical tag identity should survive retagging");
}
