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
        marker: char,
    ) -> Result<String, CommandError> {
        let session_id = marker.to_string().repeat(32);
        connection
            .execute(
                "INSERT INTO practice_sessions (
                     id, selected_deck_ids_json, shuffle, card_count, mode, started_at
                 ) VALUES (?1, ?2, 0, ?3, 'custom', 10)",
                params![
                    session_id,
                    serde_json::to_string(&vec![deck_id]).map_err(CommandError::serialization)?,
                    i64::try_from(card_ids.len()).unwrap_or(i64::MAX),
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
        mutation: char,
        action: &str,
    ) -> ScheduleActionInput {
        ScheduleActionInput {
            mutation_id: mutation.to_string().repeat(32),
            card_ids: card_ids.to_vec(),
            action: action.to_string(),
            due_at: None,
            session_id: None,
        }
    }

    fn operation_counts(
        connection: &Connection,
        mutation_id: &str,
    ) -> Result<(i64, i64), CommandError> {
        let parents = connection
            .query_row(
                "SELECT COUNT(*) FROM schedule_action_operations WHERE id = ?1",
                [mutation_id],
                |row| row.get(0),
            )
            .map_err(CommandError::database)?;
        let children = connection
            .query_row(
                "SELECT COUNT(*) FROM schedule_action_events WHERE operation_id = ?1",
                [mutation_id],
                |row| row.get(0),
            )
            .map_err(CommandError::database)?;
        Ok((parents, children))
    }

    #[test]
    fn multi_card_suspend_commits_one_parent_and_all_child_events() {
        let database = Database::open_in_memory().expect("database should open");
        database
            .with_connection(|connection| {
                let (_, card_ids) = create_cards(connection, 3)?;
                let result = apply_in_connection(
                    connection,
                    action_input(&card_ids, 'a', "suspend"),
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
                assert_eq!(operation_counts(connection, &"a".repeat(32))?, (1, 3));
                for card_id in &card_ids {
                    let suspended_at: Option<i64> = connection
                        .query_row(
                            "SELECT suspended_at FROM cards WHERE id = ?1",
                            [card_id],
                            |row| row.get(0),
                        )
                        .map_err(CommandError::database)?;
                    assert!(suspended_at.is_some());
                    assert_eq!(load_schedule(connection, card_id)?.revision, 2);
                }
                let stored: (String, i64, i64, i64) = connection
                    .query_row(
                        "SELECT request_fingerprint, requested_count,
                                changed_count, unchanged_count
                         FROM schedule_action_operations WHERE id = ?1",
                        ["a".repeat(32)],
                        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
                    )
                    .map_err(CommandError::database)?;
                assert!(stored.0.starts_with("schedule-action:v1:"));
                assert_eq!((stored.1, stored.2, stored.3), (3, 3, 0));
                Ok(())
            })
            .expect("the whole batch should commit");
    }

    #[test]
    fn invalid_batch_member_rolls_back_every_card_and_audit_row() {
        let database = Database::open_in_memory().expect("database should open");
        database
            .with_connection(|connection| {
                let (_, mut card_ids) = create_cards(connection, 1)?;
                let valid = card_ids[0].clone();
                card_ids.push("f".repeat(32));

                let error = apply_in_connection(
                    connection,
                    action_input(&card_ids, 'b', "suspend"),
                )
                .expect_err("one missing member must reject the whole batch");
                assert_eq!(error.code, "schedule_card_not_found");

                let suspended_at: Option<i64> = connection
                    .query_row(
                        "SELECT suspended_at FROM cards WHERE id = ?1",
                        [valid.as_str()],
                        |row| row.get(0),
                    )
                    .map_err(CommandError::database)?;
                assert_eq!(suspended_at, None);
                assert_eq!(load_schedule(connection, &valid)?.revision, 1);
                assert_eq!(operation_counts(connection, &"b".repeat(32))?, (0, 0));
                Ok(())
            })
            .expect("the rejected batch should leave durable state untouched");
    }

    #[test]
    fn equivalent_retry_is_side_effect_free_and_conflicting_reuse_is_rejected() {
        let database = Database::open_in_memory().expect("database should open");
        database
            .with_connection(|connection| {
                let (_, card_ids) = create_cards(connection, 2)?;
                let first = apply_in_connection(
                    connection,
                    action_input(&card_ids, 'c', "suspend"),
                )?;

                let mut reversed = card_ids.clone();
                reversed.reverse();
                reversed.push(card_ids[0].clone());
                let replay = apply_in_connection(
                    connection,
                    action_input(&reversed, 'c', "suspend"),
                )?;
                assert_eq!(replay, first);
                assert_eq!(operation_counts(connection, &"c".repeat(32))?, (1, 2));
                for card_id in &card_ids {
                    assert_eq!(load_schedule(connection, card_id)?.revision, 2);
                }

                let conflict = apply_in_connection(
                    connection,
                    action_input(&card_ids, 'c', "resume"),
                )
                .expect_err("one operation ID cannot describe another request");
                assert_eq!(conflict.code, "idempotency_conflict");
                for card_id in &card_ids {
                    let suspended_at: Option<i64> = connection
                        .query_row(
                            "SELECT suspended_at FROM cards WHERE id = ?1",
                            [card_id],
                            |row| row.get(0),
                        )
                        .map_err(CommandError::database)?;
                    assert!(suspended_at.is_some());
                }
                Ok(())
            })
            .expect("equivalent retries should not repeat side effects");
    }

    #[test]
    fn authoritative_result_counts_mixed_and_complete_no_ops() {
        let database = Database::open_in_memory().expect("database should open");
        database
            .with_connection(|connection| {
                let (_, card_ids) = create_cards(connection, 2)?;
                apply_in_connection(
                    connection,
                    action_input(&card_ids[..1], 'd', "suspend"),
                )?;
                let first_revision = load_schedule(connection, &card_ids[0])?.revision;

                let mixed = apply_in_connection(
                    connection,
                    action_input(&card_ids, 'e', "suspend"),
                )?;
                assert_eq!(mixed.requested_count, 2);
                assert_eq!(mixed.changed_count, 1);
                assert_eq!(mixed.unchanged_count, 1);
                assert_eq!(operation_counts(connection, &"e".repeat(32))?, (1, 1));
                assert_eq!(load_schedule(connection, &card_ids[0])?.revision, first_revision);
                assert_eq!(load_schedule(connection, &card_ids[1])?.revision, 2);

                let no_op = apply_in_connection(
                    connection,
                    action_input(&card_ids, 'f', "suspend"),
                )?;
                assert_eq!(no_op.changed_count, 0);
                assert_eq!(no_op.unchanged_count, 2);
                assert_eq!(operation_counts(connection, &"f".repeat(32))?, (1, 0));
                assert_eq!(load_schedule(connection, &card_ids[0])?.revision, 2);
                assert_eq!(load_schedule(connection, &card_ids[1])?.revision, 2);
                Ok(())
            })
            .expect("already satisfied controls should be explicit no-ops");
    }

    #[test]
    fn reset_and_exact_reschedule_no_ops_do_not_churn_revisions_or_history() {
        let database = Database::open_in_memory().expect("database should open");
        database
            .with_connection(|connection| {
                let (_, card_ids) = create_cards(connection, 1)?;
                let card_id = &card_ids[0];

                let fresh = apply_in_connection(
                    connection,
                    action_input(&card_ids, 'g', "reset"),
                )?;
                assert_eq!((fresh.changed_count, fresh.unchanged_count), (0, 1));
                assert_eq!(load_schedule(connection, card_id)?.revision, 1);
                assert_eq!(operation_counts(connection, &"g".repeat(32))?, (1, 0));

                let current_due = load_schedule(connection, card_id)?.due_at;
                let mut same_due = action_input(&card_ids, 'h', "reschedule");
                same_due.due_at = Some(current_due);
                let rescheduled = apply_in_connection(connection, same_due)?;
                assert_eq!((rescheduled.changed_count, rescheduled.unchanged_count), (0, 1));
                assert_eq!(load_schedule(connection, card_id)?.revision, 1);
                assert_eq!(operation_counts(connection, &"h".repeat(32))?, (1, 0));

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
                    action_input(&card_ids, 'i', "reset"),
                )?;
                assert_eq!((reset.changed_count, reset.unchanged_count), (1, 0));
                let reset_schedule = load_schedule(connection, card_id)?;
                assert_eq!(reset_schedule.state, 0);
                assert_eq!(reset_schedule.reps, 0);
                assert_eq!(reset_schedule.lapses, 0);
                assert_eq!(reset_schedule.revision, 2);
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
            .expect("manual no-ops should not rewrite schedule history");
    }

    #[test]
    fn repeated_bury_before_the_same_or_later_deadline_is_a_no_op() {
        let database = Database::open_in_memory().expect("database should open");
        database
            .with_connection(|connection| {
                let (_, card_ids) = create_cards(connection, 1)?;
                let first = apply_in_connection(
                    connection,
                    action_input(&card_ids, 'j', "bury"),
                )?;
                assert_eq!(first.changed_count, 1);
                let revision = load_schedule(connection, &card_ids[0])?.revision;

                let repeated = apply_in_connection(
                    connection,
                    action_input(&card_ids, 'k', "bury"),
                )?;
                assert_eq!((repeated.changed_count, repeated.unchanged_count), (0, 1));
                assert_eq!(load_schedule(connection, &card_ids[0])?.revision, revision);
                assert_eq!(operation_counts(connection, &"k".repeat(32))?, (1, 0));
                Ok(())
            })
            .expect("bury should not shorten or repeatedly extend the same deadline");
    }

    #[test]
    fn session_batch_dismissal_and_completion_commit_with_schedule_changes() {
        let database = Database::open_in_memory().expect("database should open");
        database
            .with_connection(|connection| {
                let (deck_id, card_ids) = create_cards(connection, 2)?;
                let session_id = create_session(connection, &deck_id, &card_ids, 'l')?;
                let mut input = action_input(&card_ids, 'm', "bury");
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
                assert_eq!(operation_counts(connection, &"m".repeat(32))?, (1, 2));
                Ok(())
            })
            .expect("session disposition and schedule changes must share one commit");
    }

    #[test]
    fn stale_session_card_is_dismissed_even_when_control_state_is_already_satisfied() {
        let database = Database::open_in_memory().expect("database should open");
        database
            .with_connection(|connection| {
                let (deck_id, card_ids) = create_cards(connection, 1)?;
                apply_in_connection(
                    connection,
                    action_input(&card_ids, 'n', "suspend"),
                )?;
                let revision = load_schedule(connection, &card_ids[0])?.revision;
                let session_id = create_session(connection, &deck_id, &card_ids, 'o')?;
                let mut input = action_input(&card_ids, 'p', "suspend");
                input.session_id = Some(session_id.clone());

                let result = apply_in_connection(connection, input)?;
                assert_eq!((result.changed_count, result.unchanged_count), (1, 0));
                assert_eq!(load_schedule(connection, &card_ids[0])?.revision, revision);
                let event_revisions: (i64, i64) = connection
                    .query_row(
                        "SELECT schedule_revision_before, schedule_revision_after
                         FROM schedule_action_events WHERE operation_id = ?1",
                        ["p".repeat(32)],
                        |row| Ok((row.get(0)?, row.get(1)?)),
                    )
                    .map_err(CommandError::database)?;
                assert_eq!(event_revisions, (revision, revision));
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
            .expect("session-only durable work should still count as a changed card");
    }

    #[test]
    fn session_membership_mismatch_rejects_the_whole_batch() {
        let database = Database::open_in_memory().expect("database should open");
        database
            .with_connection(|connection| {
                let (deck_id, card_ids) = create_cards(connection, 2)?;
                let session_id = create_session(connection, &deck_id, &card_ids[..1], 'q')?;
                let mut input = action_input(&card_ids, 'r', "suspend");
                input.session_id = Some(session_id);

                let error = apply_in_connection(connection, input)
                    .expect_err("every card must belong to the supplied session");
                assert_eq!(error.code, "schedule_session_card_mismatch");
                for card_id in &card_ids {
                    let suspended_at: Option<i64> = connection
                        .query_row(
                            "SELECT suspended_at FROM cards WHERE id = ?1",
                            [card_id],
                            |row| row.get(0),
                        )
                        .map_err(CommandError::database)?;
                    assert_eq!(suspended_at, None);
                    assert_eq!(load_schedule(connection, card_id)?.revision, 1);
                }
                assert_eq!(operation_counts(connection, &"r".repeat(32))?, (0, 0));
                Ok(())
            })
            .expect("session validation must happen before any mutation");
    }

    #[test]
    fn strict_inputs_reject_unknown_fields_empty_selections_and_invalid_due_dates() {
        let unknown = serde_json::from_value::<ScheduleActionInput>(json!({
            "mutationId": "a".repeat(32),
            "cardIds": ["b".repeat(32)],
            "action": "suspend",
            "legacyFlag": true
        }))
        .expect_err("unknown fields must not be ignored");
        assert!(unknown.to_string().contains("legacyFlag"));

        let empty = action_input(&[], 'a', "suspend");
        assert_eq!(
            validate_input(empty)
                .expect_err("an empty batch must fail")
                .code,
            "invalid_schedule_selection"
        );

        let card_ids = vec!["b".repeat(32)];
        let mut invalid = action_input(&card_ids, 'a', "reschedule");
        invalid.due_at = Some(-1);
        assert_eq!(
            validate_input(invalid)
                .expect_err("negative dates must fail")
                .code,
            "invalid_reschedule_date"
        );

        let mut unexpected = action_input(&card_ids, 'a', "suspend");
        unexpected.due_at = Some(1);
        assert_eq!(
            validate_input(unexpected)
                .expect_err("non-reschedule actions cannot carry dates")
                .code,
            "unexpected_reschedule_date"
        );
    }
}
