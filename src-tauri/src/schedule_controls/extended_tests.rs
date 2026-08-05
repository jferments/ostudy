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

    fn test_id(value: u8) -> String {
        format!("{value:032x}")
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

    fn create_deck(connection: &mut Connection) -> Result<String, CommandError> {
        Ok(create_deck_in_connection(
            connection,
            CreateDeckInput {
                name: "Transactional scheduling".to_string(),
                description: None,
            },
        )?
        .id)
    }

    fn create_card(
        connection: &mut Connection,
        deck_id: &str,
        position: usize,
    ) -> Result<String, CommandError> {
        let note_id = generate_id(connection)?;
        let card_id = generate_id(connection)?;
        let question = format!("Question {position}");
        let content = validate_content(
            paragraph(&question),
            paragraph("Answer"),
            question,
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
                    format!("schedule-batch-{position}"),
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

    fn create_cards(
        connection: &mut Connection,
        count: usize,
    ) -> Result<(String, Vec<String>), CommandError> {
        let deck_id = create_deck(connection)?;
        let mut card_ids = Vec::with_capacity(count);
        for position in 0..count {
            card_ids.push(create_card(connection, &deck_id, position)?);
        }
        Ok((deck_id, card_ids))
    }

    fn create_session(
        connection: &Connection,
        deck_id: &str,
        card_ids: &[String],
        id_value: u8,
    ) -> Result<String, CommandError> {
        let session_id = test_id(id_value);
        connection
            .execute(
                "INSERT INTO practice_sessions (
                     id, selected_deck_ids_json, shuffle, card_count, mode, started_at
                 ) VALUES (?1, ?2, 0, ?3, 'custom', 10)",
                params![
                    session_id,
                    serde_json::to_string(&vec![deck_id])
                        .map_err(CommandError::serialization)?,
                    i64::try_from(card_ids.len()).unwrap_or(i64::MAX),
                ],
            )
            .map_err(CommandError::database)?;
        for (position, card_id) in card_ids.iter().enumerate() {
            connection
                .execute(
                    "INSERT INTO practice_session_cards (
                         session_id, card_id, position
                     ) VALUES (?1, ?2, ?3)",
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

    fn action_input(
        card_ids: &[String],
        mutation: u8,
        action: &str,
    ) -> ScheduleActionInput {
        ScheduleActionInput {
            mutation_id: test_id(mutation),
            card_ids: card_ids.to_vec(),
            action: action.to_string(),
            reschedule: None,
            session_id: None,
        }
    }

    fn operation_counts(
        connection: &Connection,
        mutation: u8,
    ) -> Result<(i64, i64), CommandError> {
        let mutation_id = test_id(mutation);
        let parent_count = connection
            .query_row(
                "SELECT COUNT(*) FROM schedule_action_operations WHERE id = ?1",
                [mutation_id.as_str()],
                |row| row.get(0),
            )
            .map_err(CommandError::database)?;
        let child_count = connection
            .query_row(
                "SELECT COUNT(*) FROM schedule_action_events WHERE operation_id = ?1",
                [mutation_id.as_str()],
                |row| row.get(0),
            )
            .map_err(CommandError::database)?;
        Ok((parent_count, child_count))
    }

    fn suspended_at(
        connection: &Connection,
        card_id: &str,
    ) -> Result<Option<i64>, CommandError> {
        connection
            .query_row(
                "SELECT suspended_at FROM cards WHERE id = ?1",
                [card_id],
                |row| row.get(0),
            )
            .map_err(CommandError::database)
    }

    #[test]
    fn multi_card_suspend_commits_one_parent_and_every_child_event() {
        let database = Database::open_in_memory().expect("database should open");
        database
            .with_connection(|connection| {
                let (_, card_ids) = create_cards(connection, 3)?;
                let result = apply_in_connection(
                    connection,
                    action_input(&card_ids, 1, "suspend"),
                )?;

                assert_eq!(
                    result,
                    ScheduleActionResultDto {
                        requested_count: 3,
                        changed_count: 3,
                        unchanged_count: 0,
                        rejected_count: 0,
                        action: "suspend".to_string(),
                    }
                );
                assert_eq!(operation_counts(connection, 1)?, (1, 3));
                for card_id in &card_ids {
                    assert!(suspended_at(connection, card_id)?.is_some());
                    assert_eq!(load_schedule(connection, card_id)?.revision, 2);
                }
                Ok(())
            })
            .expect("the complete batch should commit");
    }

    #[test]
    fn one_invalid_member_rolls_back_the_complete_selection() {
        let database = Database::open_in_memory().expect("database should open");
        database
            .with_connection(|connection| {
                let (_, mut card_ids) = create_cards(connection, 1)?;
                let valid_card = card_ids[0].clone();
                card_ids.push(test_id(250));

                let error = apply_in_connection(
                    connection,
                    action_input(&card_ids, 2, "suspend"),
                )
                .expect_err("a missing member must reject the whole operation");
                assert_eq!(error.code, "schedule_card_not_found");
                assert_eq!(suspended_at(connection, &valid_card)?, None);
                assert_eq!(load_schedule(connection, &valid_card)?.revision, 1);
                assert_eq!(operation_counts(connection, 2)?, (0, 0));
                Ok(())
            })
            .expect("rejected batches should leave no durable writes");
    }

    #[test]
    fn canonical_retry_is_side_effect_free_and_conflicting_reuse_fails() {
        let database = Database::open_in_memory().expect("database should open");
        database
            .with_connection(|connection| {
                let (_, card_ids) = create_cards(connection, 2)?;
                let first = apply_in_connection(
                    connection,
                    action_input(&card_ids, 3, "suspend"),
                )?;

                let mut reordered = card_ids.clone();
                reordered.reverse();
                reordered.push(card_ids[0].clone());
                let replay = apply_in_connection(
                    connection,
                    action_input(&reordered, 3, "suspend"),
                )?;
                assert_eq!(replay, first);
                assert_eq!(operation_counts(connection, 3)?, (1, 2));
                for card_id in &card_ids {
                    assert_eq!(load_schedule(connection, card_id)?.revision, 2);
                }

                let conflict = apply_in_connection(
                    connection,
                    action_input(&card_ids, 3, "resume"),
                )
                .expect_err("one operation ID cannot describe another request");
                assert_eq!(conflict.code, "idempotency_conflict");
                for card_id in &card_ids {
                    assert!(suspended_at(connection, card_id)?.is_some());
                }
                Ok(())
            })
            .expect("equivalent retries must not repeat side effects");
    }

    #[test]
    fn changed_and_already_satisfied_cards_are_reported_without_revision_churn() {
        let database = Database::open_in_memory().expect("database should open");
        database
            .with_connection(|connection| {
                let (_, card_ids) = create_cards(connection, 2)?;
                apply_in_connection(
                    connection,
                    action_input(&card_ids[..1], 4, "suspend"),
                )?;
                let first_revision = load_schedule(connection, &card_ids[0])?.revision;

                let mixed = apply_in_connection(
                    connection,
                    action_input(&card_ids, 5, "suspend"),
                )?;
                assert_eq!((mixed.requested_count, mixed.changed_count), (2, 1));
                assert_eq!(mixed.unchanged_count, 1);
                assert_eq!(operation_counts(connection, 5)?, (1, 1));
                assert_eq!(load_schedule(connection, &card_ids[0])?.revision, first_revision);
                assert_eq!(load_schedule(connection, &card_ids[1])?.revision, 2);

                let no_op = apply_in_connection(
                    connection,
                    action_input(&card_ids, 6, "suspend"),
                )?;
                assert_eq!((no_op.changed_count, no_op.unchanged_count), (0, 2));
                assert_eq!(operation_counts(connection, 6)?, (1, 0));
                assert_eq!(load_schedule(connection, &card_ids[0])?.revision, 2);
                assert_eq!(load_schedule(connection, &card_ids[1])?.revision, 2);
                Ok(())
            })
            .expect("already-satisfied controls should be explicit no-ops");
    }

    #[test]
    fn reset_preserves_review_history_and_exact_reschedule_can_be_a_no_op() {
        let database = Database::open_in_memory().expect("database should open");
        database
            .with_connection(|connection| {
                let (_, card_ids) = create_cards(connection, 1)?;
                let card_id = &card_ids[0];

                let fresh_reset = apply_in_connection(
                    connection,
                    action_input(&card_ids, 7, "reset"),
                )?;
                assert_eq!((fresh_reset.changed_count, fresh_reset.unchanged_count), (0, 1));
                assert_eq!(load_schedule(connection, card_id)?.revision, 1);

                let tomorrow = StudyTimeContext::capture()?.tomorrow_start_utc_millis;
                connection.execute(
                    "UPDATE card_schedules SET due_at = ?1 WHERE card_id = ?2",
                    params![tomorrow, card_id],
                ).map_err(CommandError::database)?;
                let mut same_due = action_input(&card_ids, 8, "reschedule");
                same_due.reschedule = Some(RescheduleIntentInput::Tomorrow);
                let reschedule = apply_in_connection(connection, same_due)?;
                assert_eq!((reschedule.changed_count, reschedule.unchanged_count), (0, 1));
                assert_eq!(load_schedule(connection, card_id)?.revision, 1);

                connection
                    .execute(
                        "UPDATE card_schedules
                         SET state = 2, due_at = 30, stability = 2.0,
                             difficulty = 5.0, reps = 4, lapses = 1
                         WHERE card_id = ?1",
                        [card_id],
                    )
                    .map_err(CommandError::database)?;
                insert_review(connection, card_id)?;
                let reset = apply_in_connection(
                    connection,
                    action_input(&card_ids, 9, "reset"),
                )?;
                assert_eq!((reset.changed_count, reset.unchanged_count), (1, 0));
                let schedule = load_schedule(connection, card_id)?;
                assert_eq!(schedule.state, 0);
                assert_eq!(schedule.reps, 0);
                assert_eq!(schedule.lapses, 0);
                assert_eq!(schedule.revision, 2);
                let review_count: i64 = connection
                    .query_row(
                        "SELECT COUNT(*) FROM review_events WHERE card_id = ?1",
                        [card_id],
                        |row| row.get(0),
                    )
                    .map_err(CommandError::database)?;
                assert_eq!(review_count, 1);
                Ok(())
            })
            .expect("manual reset must preserve immutable review evidence");
    }

    #[test]
    fn session_batch_dismissal_and_completion_share_the_schedule_transaction() {
        let database = Database::open_in_memory().expect("database should open");
        database
            .with_connection(|connection| {
                let (deck_id, card_ids) = create_cards(connection, 2)?;
                let session_id = create_session(connection, &deck_id, &card_ids, 10)?;
                let mut input = action_input(&card_ids, 11, "bury");
                input.session_id = Some(session_id.clone());

                let result = apply_in_connection(connection, input)?;
                assert_eq!((result.requested_count, result.changed_count), (2, 2));
                for card_id in &card_ids {
                    let state: (Option<i64>, Option<String>) = connection
                        .query_row(
                            "SELECT dismissed_at, disposition
                             FROM practice_session_cards
                             WHERE session_id = ?1 AND card_id = ?2",
                            params![session_id, card_id],
                            |row| Ok((row.get(0)?, row.get(1)?)),
                        )
                        .map_err(CommandError::database)?;
                    assert!(state.0.is_some());
                    assert_eq!(state.1.as_deref(), Some("buried"));
                }
                let completed_at: Option<i64> = connection
                    .query_row(
                        "SELECT completed_at FROM practice_sessions WHERE id = ?1",
                        [session_id.as_str()],
                        |row| row.get(0),
                    )
                    .map_err(CommandError::database)?;
                assert!(completed_at.is_some());
                assert_eq!(operation_counts(connection, 11)?, (1, 2));
                Ok(())
            })
            .expect("session state and schedule state should commit together");
    }

    #[test]
    fn session_only_dismissal_counts_as_a_change_without_schedule_revision_churn() {
        let database = Database::open_in_memory().expect("database should open");
        database
            .with_connection(|connection| {
                let (deck_id, card_ids) = create_cards(connection, 1)?;
                apply_in_connection(
                    connection,
                    action_input(&card_ids, 12, "suspend"),
                )?;
                let revision = load_schedule(connection, &card_ids[0])?.revision;
                let session_id = create_session(connection, &deck_id, &card_ids, 13)?;
                let mut input = action_input(&card_ids, 14, "suspend");
                input.session_id = Some(session_id.clone());

                let result = apply_in_connection(connection, input)?;
                assert_eq!((result.changed_count, result.unchanged_count), (1, 0));
                assert_eq!(load_schedule(connection, &card_ids[0])?.revision, revision);
                let revisions: (i64, i64) = connection
                    .query_row(
                        "SELECT schedule_revision_before, schedule_revision_after
                         FROM schedule_action_events WHERE operation_id = ?1",
                        [test_id(14)],
                        |row| Ok((row.get(0)?, row.get(1)?)),
                    )
                    .map_err(CommandError::database)?;
                assert_eq!(revisions, (revision, revision));
                Ok(())
            })
            .expect("session-only durable work should be represented accurately");
    }

    #[test]
    fn session_membership_mismatch_rejects_every_selected_card() {
        let database = Database::open_in_memory().expect("database should open");
        database
            .with_connection(|connection| {
                let (deck_id, card_ids) = create_cards(connection, 2)?;
                let session_id = create_session(connection, &deck_id, &card_ids[..1], 15)?;
                let mut input = action_input(&card_ids, 16, "suspend");
                input.session_id = Some(session_id);

                let error = apply_in_connection(connection, input)
                    .expect_err("every selected card must belong to the session");
                assert_eq!(error.code, "schedule_session_card_mismatch");
                for card_id in &card_ids {
                    assert_eq!(suspended_at(connection, card_id)?, None);
                    assert_eq!(load_schedule(connection, card_id)?.revision, 1);
                }
                assert_eq!(operation_counts(connection, 16)?, (0, 0));
                Ok(())
            })
            .expect("session validation should precede every mutation");
    }

    #[test]
    fn strict_inputs_reject_unknown_fields_empty_selections_and_invalid_dates() {
        let unknown = serde_json::from_value::<ScheduleActionInput>(json!({
            "mutationId": test_id(17),
            "cardIds": [test_id(18)],
            "action": "suspend",
            "legacyFlag": true
        }))
        .expect_err("unknown fields must not be ignored");
        assert!(unknown.to_string().contains("legacyFlag"));

        assert_eq!(
            validate_input(action_input(&[], 19, "suspend"))
                .expect_err("an empty selection must fail")
                .code,
            "invalid_schedule_selection"
        );

        let card_ids = vec![test_id(20)];
        let mut invalid = action_input(&card_ids, 21, "reschedule");
        invalid.reschedule = Some(RescheduleIntentInput::CalendarDays { days: -1 });
        assert_eq!(
            validate_input(invalid)
                .expect_err("negative dates must fail")
                .code,
            "invalid_reschedule_date"
        );

        let mut unexpected = action_input(&card_ids, 22, "suspend");
        unexpected.reschedule = Some(RescheduleIntentInput::DueNow);
        assert_eq!(
            validate_input(unexpected)
                .expect_err("non-reschedule actions cannot carry dates")
                .code,
            "unexpected_reschedule_date"
        );
    }
}
