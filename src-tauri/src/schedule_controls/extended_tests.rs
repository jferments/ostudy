mod extended_tests {
    use super::*;
    use rusqlite::params;
    use serde_json::json;

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
    ) -> Result<(String, String, String), CommandError> {
        let deck_id = create_deck_in_connection(
            connection,
            CreateDeckInput {
                name: "Scheduling".to_string(),
                description: None,
            },
        )?
        .id;
        let note_id = generate_id(connection)?;
        let card_id = generate_id(connection)?;
        let content = validate_content(
            paragraph("Question"),
            paragraph("Answer"),
            String::new(),
            String::new(),
        )?;
        connection
            .execute(
                "INSERT INTO notes (
                     id, deck_id, note_type, content_format, content_schema_version,
                     front_doc, back_doc, front_text, back_text, duplicate_hash,
                     created_at, updated_at, revision
                 ) VALUES (
                     ?1, ?2, 'basic', 'tiptap-json', 1,
                     ?3, ?4, ?5, ?6, '', 10, 10, 1
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
        Ok((deck_id, note_id, card_id))
    }

    fn insert_review(connection: &Connection, card_id: &str) -> Result<(), CommandError> {
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
                     ?1, ?2, NULL, ?3, 20,
                     3, 1, 'somewhat_confident', 1000,
                     2, 2, 10, 30,
                     1.0, 2.0, 5.0, 5.0,
                     1, 2, 'fsrs', 'test', 20
                 )",
                params![generate_id(connection)?, generate_id(connection)?, card_id],
            )
            .map_err(CommandError::database)?;
        Ok(())
    }

    fn action_input(card_id: &str, mutation: char, action: &str) -> ScheduleActionInput {
        ScheduleActionInput {
            mutation_id: mutation.to_string().repeat(32),
            card_ids: vec![card_id.to_string()],
            action: action.to_string(),
            due_at: None,
            session_id: None,
        }
    }

    #[test]
    fn reset_preserves_review_history_and_same_mutation_is_idempotent() {
        let database = Database::open_in_memory().expect("database should open");
        database
            .with_connection(|connection| {
                let (_, _, card_id) = create_card(connection)?;
                connection
                    .execute(
                        "UPDATE card_schedules
                         SET state = 2, due_at = 30, stability = 2.0,
                             difficulty = 5.0, reps = 4, lapses = 1
                         WHERE card_id = ?1",
                        [card_id.as_str()],
                    )
                    .map_err(CommandError::database)?;
                insert_review(connection, &card_id)?;

                let first = apply_in_connection(
                    connection,
                    action_input(&card_id, 'a', "reset"),
                )?;
                assert_eq!(first.action, "reset");
                let reset = load_schedule(connection, &card_id)?;
                assert_eq!(reset.state, 0);
                assert_eq!(reset.reps, 0);
                assert_eq!(reset.lapses, 0);
                assert_eq!(reset.revision, 2);

                let repeated = apply_in_connection(
                    connection,
                    action_input(&card_id, 'a', "reset"),
                )?;
                assert_eq!(repeated.action, "reset");
                assert_eq!(load_schedule(connection, &card_id)?.revision, 2);

                let action_count: i64 = connection
                    .query_row(
                        "SELECT COUNT(*) FROM schedule_action_events
                         WHERE mutation_id = ?1",
                        ["a".repeat(32)],
                        |row| row.get(0),
                    )
                    .map_err(CommandError::database)?;
                let review_count: i64 = connection
                    .query_row(
                        "SELECT COUNT(*) FROM review_events WHERE card_id = ?1",
                        [card_id.as_str()],
                        |row| row.get(0),
                    )
                    .map_err(CommandError::database)?;
                assert_eq!(action_count, 1);
                assert_eq!(review_count, 1);
                Ok(())
            })
            .expect("reset should be transactional and retry-safe");
    }

    #[test]
    fn suspend_resume_and_reschedule_increment_schedule_revisions() {
        let database = Database::open_in_memory().expect("database should open");
        database
            .with_connection(|connection| {
                let (_, _, card_id) = create_card(connection)?;

                apply_in_connection(
                    connection,
                    action_input(&card_id, 'b', "suspend"),
                )?;
                let suspended: Option<i64> = connection
                    .query_row(
                        "SELECT suspended_at FROM cards WHERE id = ?1",
                        [card_id.as_str()],
                        |row| row.get(0),
                    )
                    .map_err(CommandError::database)?;
                assert!(suspended.is_some());
                assert_eq!(load_schedule(connection, &card_id)?.revision, 2);

                apply_in_connection(connection, action_input(&card_id, 'c', "resume"))?;
                let resumed: Option<i64> = connection
                    .query_row(
                        "SELECT suspended_at FROM cards WHERE id = ?1",
                        [card_id.as_str()],
                        |row| row.get(0),
                    )
                    .map_err(CommandError::database)?;
                assert_eq!(resumed, None);
                assert_eq!(load_schedule(connection, &card_id)?.revision, 3);

                let mut reschedule = action_input(&card_id, 'd', "reschedule");
                reschedule.due_at = Some(123_456);
                apply_in_connection(connection, reschedule)?;
                let schedule = load_schedule(connection, &card_id)?;
                assert_eq!(schedule.due_at, 123_456);
                assert_eq!(schedule.revision, 4);
                Ok(())
            })
            .expect("manual controls should update exact-card state");
    }

    #[test]
    fn burying_a_session_card_dismisses_it_and_completes_the_session() {
        let database = Database::open_in_memory().expect("database should open");
        database
            .with_connection(|connection| {
                let (deck_id, _, card_id) = create_card(connection)?;
                let session_id = "e".repeat(32);
                connection
                    .execute(
                        "INSERT INTO practice_sessions (
                             id, selected_deck_ids_json, shuffle, card_count,
                             viewed_count, revealed_count, started_at, completed_at,
                             rated_count, correct_count, mode
                         ) VALUES (?1, ?2, 0, 1, 0, 0, 10, NULL, 0, 0, 'scheduled')",
                        params![session_id, serde_json::to_string(&vec![deck_id]).unwrap()],
                    )
                    .map_err(CommandError::database)?;
                connection
                    .execute(
                        "INSERT INTO practice_session_cards (
                             session_id, card_id, position
                         ) VALUES (?1, ?2, 0)",
                        params![session_id, card_id],
                    )
                    .map_err(CommandError::database)?;

                let mut bury = action_input(&card_id, 'f', "bury");
                bury.session_id = Some(session_id.clone());
                apply_in_connection(connection, bury)?;

                let session_card: (Option<i64>, Option<String>) = connection
                    .query_row(
                        "SELECT dismissed_at, disposition
                         FROM practice_session_cards
                         WHERE session_id = ?1 AND card_id = ?2",
                        params![session_id, card_id],
                        |row| Ok((row.get(0)?, row.get(1)?)),
                    )
                    .map_err(CommandError::database)?;
                assert!(session_card.0.is_some());
                assert_eq!(session_card.1.as_deref(), Some("buried"));
                let completed_at: Option<i64> = connection
                    .query_row(
                        "SELECT completed_at FROM practice_sessions WHERE id = ?1",
                        [session_id.as_str()],
                        |row| row.get(0),
                    )
                    .map_err(CommandError::database)?;
                assert!(completed_at.is_some());
                Ok(())
            })
            .expect("bury should update the schedule, session card, and counters atomically");
    }

    #[test]
    fn strict_inputs_reject_unknown_fields_and_invalid_due_dates() {
        let unknown = serde_json::from_value::<ScheduleActionInput>(json!({
            "mutationId": "a".repeat(32),
            "cardIds": ["b".repeat(32)],
            "action": "suspend",
            "legacyFlag": true
        }))
        .expect_err("unknown fields must not be ignored");
        assert!(unknown.to_string().contains("legacyFlag"));

        let mut invalid = action_input(&"b".repeat(32), 'a', "reschedule");
        invalid.due_at = Some(-1);
        assert_eq!(
            validate_input(&invalid)
                .expect_err("negative dates must fail")
                .code,
            "invalid_reschedule_date"
        );

        let mut unexpected = action_input(&"b".repeat(32), 'a', "suspend");
        unexpected.due_at = Some(1);
        assert_eq!(
            validate_input(&unexpected)
                .expect_err("non-reschedule actions cannot carry dates")
                .code,
            "unexpected_reschedule_date"
        );
    }
}
