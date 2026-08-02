#[cfg(test)]
mod stage3_tests {
    use super::*;
    use crate::decks::{create_deck_in_connection, CreateDeckInput};

    fn create_test_deck(connection: &mut Connection, name: &str) -> Result<String, CommandError> {
        Ok(create_deck_in_connection(
            connection,
            CreateDeckInput {
                name: name.to_string(),
                description: None,
            },
        )?
        .id)
    }

    #[test]
    fn cloze_modes_generate_stable_blank_presentations() {
        let source = "{{c1::ACE inhibitors}} reduce formation of {{c2::angiotensin II}}.";
        let parsed = parse_cloze(source).expect("cloze should parse");
        assert_eq!(parsed.groups, vec![1, 2]);
        assert_eq!(
            render_cloze_question(&parsed, Some(1)),
            "[…] reduce formation of angiotensin II."
        );
        assert_eq!(
            render_cloze_question(&parsed, None),
            "[…] reduce formation of […]."
        );

        let one_at_a_time = prepare_generated_note(
            "cloze",
            false,
            source.to_string(),
            "one_at_a_time".to_string(),
            json!({"type": "doc", "content": []}),
            json!({"type": "doc", "content": []}),
            String::new(),
            String::new(),
        )
        .expect("one-at-a-time note should prepare");
        assert_eq!(
            one_at_a_time.cards,
            vec![
                GeneratedCardSpec {
                    direction: "forward",
                    template_ordinal: 1,
                },
                GeneratedCardSpec {
                    direction: "cloze",
                    template_ordinal: 2,
                },
            ]
        );

        let all_together = prepare_generated_note(
            "cloze",
            false,
            source.to_string(),
            "all_together".to_string(),
            json!({"type": "doc", "content": []}),
            json!({"type": "doc", "content": []}),
            String::new(),
            String::new(),
        )
        .expect("all-together note should prepare");
        assert_eq!(
            all_together.cards,
            vec![GeneratedCardSpec {
                direction: "forward",
                template_ordinal: 0,
            }]
        );
    }

    #[test]
    fn malformed_cloze_markers_are_rejected() {
        for invalid in [
            "No blanks",
            "{{c0::zero}}",
            "{{c1::}}",
            "{{c1::nested {{c2::blank}}}}",
            "{{c1::unfinished",
        ] {
            assert!(parse_cloze(invalid).is_err(), "accepted invalid cloze: {invalid}");
        }
    }

    #[test]
    fn reverse_reactivation_preserves_identity_schedule_and_review_history() {
        let database = Database::open_in_memory().expect("database should open");
        database
            .with_connection(|connection| {
                let deck_id = create_test_deck(connection, "Pharmacology")?;
                let note_id = generate_id(connection)?;
                let now = unix_time_millis()?;
                connection
                    .execute(
                        "INSERT INTO notes (
                             id, deck_id, note_type, content_format,
                             content_schema_version, front_doc, back_doc,
                             front_text, back_text, duplicate_hash,
                             reverse_enabled, created_at, updated_at, revision
                         ) VALUES (
                             ?1, ?2, 'basic', 'tiptap-json', 1,
                             ?3, ?4, 'Question', 'Answer', '', 1, ?5, ?5, 1
                         )",
                        params![
                            note_id,
                            deck_id,
                            serde_json::to_string(&plain_text_document("Question"))
                                .map_err(CommandError::serialization)?,
                            serde_json::to_string(&plain_text_document("Answer"))
                                .map_err(CommandError::serialization)?,
                            now,
                        ],
                    )
                    .map_err(CommandError::database)?;

                let both = [
                    GeneratedCardSpec {
                        direction: "forward",
                        template_ordinal: 0,
                    },
                    GeneratedCardSpec {
                        direction: "reverse",
                        template_ordinal: 0,
                    },
                ];
                sync_generated_cards(connection, &note_id, &deck_id, &both, now)?;
                let reverse_id: String = connection
                    .query_row(
                        "SELECT id FROM cards
                         WHERE note_id = ?1 AND direction = 'reverse'",
                        [note_id.as_str()],
                        |row| row.get(0),
                    )
                    .map_err(CommandError::database)?;
                let preserved_due = now + 7 * 86_400_000;
                connection
                    .execute(
                        "UPDATE card_schedules
                         SET due_at = ?1, reps = 1, updated_at = ?2
                         WHERE card_id = ?3",
                        params![preserved_due, now, reverse_id],
                    )
                    .map_err(CommandError::database)?;

                let review_id = generate_id(connection)?;
                let submission_id = generate_id(connection)?;
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
                             3, 1, 'somewhat_confident', 1000,
                             0, 1, ?4, ?5,
                             0.0, 1.0, 0.0, 5.0,
                             0, 1, 'fsrs', 'test', ?4
                         )",
                        params![review_id, submission_id, reverse_id, now, preserved_due],
                    )
                    .map_err(CommandError::database)?;

                sync_generated_cards(
                    connection,
                    &note_id,
                    &deck_id,
                    &[GeneratedCardSpec {
                        direction: "forward",
                        template_ordinal: 0,
                    }],
                    now + 1,
                )?;
                let disabled_at: Option<i64> = connection
                    .query_row(
                        "SELECT deleted_at FROM cards WHERE id = ?1",
                        [reverse_id.as_str()],
                        |row| row.get(0),
                    )
                    .map_err(CommandError::database)?;
                assert!(disabled_at.is_some());

                sync_generated_cards(connection, &note_id, &deck_id, &both, now + 2)?;
                let restored: (String, i64, i64, Option<i64>) = connection
                    .query_row(
                        "SELECT c.id, s.due_at, s.reps, c.deleted_at
                         FROM cards c
                         JOIN card_schedules s ON s.card_id = c.id
                         WHERE c.note_id = ?1 AND c.direction = 'reverse'",
                        [note_id.as_str()],
                        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
                    )
                    .map_err(CommandError::database)?;
                assert_eq!(restored.0, reverse_id);
                assert_eq!(restored.1, preserved_due);
                assert_eq!(restored.2, 1);
                assert_eq!(restored.3, None);
                let review_count: i64 = connection
                    .query_row(
                        "SELECT COUNT(*) FROM review_events WHERE card_id = ?1",
                        [reverse_id.as_str()],
                        |row| row.get(0),
                    )
                    .map_err(CommandError::database)?;
                assert_eq!(review_count, 1);
                Ok(())
            })
            .expect("reverse lifecycle should preserve history");
    }

    #[test]
    fn template_substitution_is_literal_and_rejects_unknown_fields() {
        let fields = vec![CardTemplateFieldDto {
            id: "a".repeat(32),
            name: "Name".to_string(),
            description: String::new(),
            position: 0,
        }];
        assert!(validate_pattern("What is {{Unknown}}?", &fields, "question").is_err());
        assert!(parse_pattern_placeholders("{{Name}").is_err());

        let normalized_fields = HashMap::from([("name".to_string(), fields[0].id.clone())]);
        let values = HashMap::from([(
            fields[0].id.clone(),
            "<script>alert('not executed')</script>".to_string(),
        )]);
        assert_eq!(
            render_pattern("What is {{Name}}?", &normalized_fields, &values)
                .expect("literal substitution should render"),
            "What is <script>alert('not executed')</script>?"
        );
    }

    #[test]
    fn deleting_a_template_does_not_delete_generated_cards() {
        let database = Database::open_in_memory().expect("database should open");
        database
            .with_connection(|connection| {
                let deck_id = create_test_deck(connection, "Pharmacology")?;
                let template = create_card_template_in_connection(
                    connection,
                    CreateCardTemplateInput {
                        name: "Medication".to_string(),
                        description: "Generate focused medication cards".to_string(),
                        fields: vec![
                            CardTemplateFieldInput {
                                id: None,
                                name: "Name".to_string(),
                                description: String::new(),
                            },
                            CardTemplateFieldInput {
                                id: None,
                                name: "Drug class".to_string(),
                                description: String::new(),
                            },
                        ],
                        rules: vec![CardTemplateRuleInput {
                            id: None,
                            name: "Class".to_string(),
                            front_pattern: "What is the drug class of {{Name}}?".to_string(),
                            back_pattern: "{{Drug class}}".to_string(),
                        }],
                    },
                )?;
                let generated = generate_template_cards_in_connection(
                    connection,
                    GenerateTemplateCardsInput {
                        deck_id: deck_id.clone(),
                        template_id: template.id.clone(),
                        values: vec![
                            TemplateValueInput {
                                field_id: template.fields[0].id.clone(),
                                value: "Lisinopril".to_string(),
                            },
                            TemplateValueInput {
                                field_id: template.fields[1].id.clone(),
                                value: "ACE inhibitor".to_string(),
                            },
                        ],
                        tags: vec!["Medication".to_string()],
                    },
                )?;
                assert_eq!(generated.created_count, 1);

                delete_card_template_in_connection(connection, &template.id)?;
                let active_templates: i64 = connection
                    .query_row(
                        "SELECT COUNT(*) FROM card_templates WHERE deleted_at IS NULL",
                        [],
                        |row| row.get(0),
                    )
                    .map_err(CommandError::database)?;
                let generated_cards: i64 = connection
                    .query_row(
                        "SELECT COUNT(*)
                         FROM cards c
                         JOIN notes n ON n.id = c.note_id
                         WHERE c.deck_id = ?1
                           AND c.deleted_at IS NULL
                           AND n.deleted_at IS NULL
                           AND n.note_type = 'basic'",
                        [deck_id.as_str()],
                        |row| row.get(0),
                    )
                    .map_err(CommandError::database)?;
                assert_eq!(active_templates, 0);
                assert_eq!(generated_cards, 1);
                Ok(())
            })
            .expect("generated cards should outlive their authoring template");
    }

    #[test]
    fn revised_stage3_inputs_reject_unknown_fields() {
        let error = serde_json::from_value::<CreateGeneratedCardInput>(json!({
            "deckId": "a",
            "cardType": "basic",
            "reverseEnabled": false,
            "clozeSource": "",
            "clozeMode": "one_at_a_time",
            "frontDoc": {"type": "doc", "content": []},
            "backDoc": {"type": "doc", "content": []},
            "frontText": "Question",
            "backText": "Answer",
            "tags": [],
            "allowDuplicate": false,
            "executableTemplate": "javascript"
        }))
        .expect_err("unknown Stage 3 fields must fail visibly");
        assert!(error.to_string().contains("executableTemplate"));
    }
}
