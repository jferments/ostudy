struct GeneratedNoteDraft {
    card_type: String,
    reverse_enabled: bool,
    cloze_source: String,
    cloze_mode: String,
    front_doc: Value,
    back_doc: Value,
    front_text: String,
    back_text: String,
}

fn create_generated_card_in_database(
    database: State<'_, Database>,
    paths: State<'_, AppPaths>,
    input: CreateGeneratedCardInput,
) -> Result<OrganizedCardDto, CommandError> {
    let CreateGeneratedCardInput {
        deck_id,
        card_type,
        reverse_enabled,
        cloze_source,
        cloze_mode,
        front_doc,
        back_doc,
        front_text,
        back_text,
        tags,
        allow_duplicate,
    } = input;
    validate_entity_id(&deck_id, "deck")?;
    let prepared = prepare_generated_note(GeneratedNoteDraft {
        card_type,
        reverse_enabled,
        cloze_source,
        cloze_mode,
        front_doc,
        back_doc,
        front_text,
        back_text,
    })?;
    let tags = validate_tags(tags)?;
    let normalized = normalized_pair(&prepared.content.front_text, &prepared.content.back_text);
    let hash = duplicate_hash(&normalized.0, &normalized.1);

    let note_id = database.with_connection(|connection| {
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(CommandError::database)?;
        ensure_active_deck(&transaction, &deck_id)?;
        ensure_assets_exist(&transaction, &content_asset_ids(&prepared.content))?;
        if !allow_duplicate
            && duplicate_checkable(&prepared.content.front_text, &prepared.content.back_text)
            && has_exact_duplicate(
                &transaction,
                &deck_id,
                &hash,
                &normalized.0,
                &normalized.1,
                None,
            )?
        {
            return Err(duplicate_error());
        }

        let now = unix_time_millis()?;
        let note_id = generate_id(&transaction)?;
        transaction
            .execute(
                "INSERT INTO notes (
                     id, deck_id, note_type, content_format, content_schema_version,
                     front_doc, back_doc, front_text, back_text, duplicate_hash,
                     reverse_enabled, cloze_source, cloze_mode,
                     created_at, updated_at, revision
                 ) VALUES (
                     ?1, ?2, ?3, 'tiptap-json', 1,
                     ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?12, 1
                 )",
                params![
                    note_id,
                    deck_id,
                    prepared.note_type,
                    prepared.content.front_doc_json,
                    prepared.content.back_doc_json,
                    prepared.content.front_text,
                    prepared.content.back_text,
                    hash,
                    crate::util::bool_to_integer(prepared.reverse_enabled),
                    prepared.cloze_source,
                    prepared.cloze_mode,
                    now,
                ],
            )
            .map_err(CommandError::database)?;
        sync_generated_cards(
            &transaction,
            &note_id,
            &deck_id,
            &prepared.cards,
            now,
        )?;
        replace_note_assets(&transaction, &note_id, &prepared.content)?;
        replace_note_tags(&transaction, &note_id, &tags, now)?;
        transaction.commit().map_err(CommandError::database)?;
        Ok(note_id)
    })?;

    database.with_connection(|connection| hydrate_organized_card(connection, &paths, &note_id))
}

fn update_generated_card_in_database(
    database: State<'_, Database>,
    paths: State<'_, AppPaths>,
    input: UpdateGeneratedCardInput,
) -> Result<OrganizedCardDto, CommandError> {
    let UpdateGeneratedCardInput {
        note_id,
        deck_id,
        revision,
        card_type,
        reverse_enabled,
        cloze_source,
        cloze_mode,
        front_doc,
        back_doc,
        front_text,
        back_text,
        tags,
        allow_duplicate,
    } = input;
    validate_entity_id(&note_id, "note")?;
    validate_entity_id(&deck_id, "deck")?;
    if revision < 1 {
        return Err(CommandError::new(
            "invalid_revision",
            "The card revision is invalid.",
        ));
    }
    let prepared = prepare_generated_note(GeneratedNoteDraft {
        card_type,
        reverse_enabled,
        cloze_source,
        cloze_mode,
        front_doc,
        back_doc,
        front_text,
        back_text,
    })?;
    let tags = validate_tags(tags)?;
    let normalized = normalized_pair(&prepared.content.front_text, &prepared.content.back_text);
    let hash = duplicate_hash(&normalized.0, &normalized.1);

    database.with_connection(|connection| {
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(CommandError::database)?;
        ensure_active_deck(&transaction, &deck_id)?;
        ensure_assets_exist(&transaction, &content_asset_ids(&prepared.content))?;
        let existing_type: Option<String> = transaction
            .query_row(
                "SELECT note_type FROM notes WHERE id = ?1 AND deleted_at IS NULL",
                [note_id.as_str()],
                |row| row.get(0),
            )
            .optional()
            .map_err(CommandError::database)?;
        let Some(existing_type) = existing_type else {
            return Err(CommandError::new(
                "card_not_found",
                "That flashcard no longer exists.",
            ));
        };
        if display_card_type(&existing_type) != display_card_type(prepared.note_type) {
            return Err(CommandError::new(
                "card_type_locked",
                "A saved card cannot be converted between Basic and Fill in the blank. Create a new card instead.",
            ));
        }
        if !allow_duplicate
            && duplicate_checkable(&prepared.content.front_text, &prepared.content.back_text)
            && has_exact_duplicate(
                &transaction,
                &deck_id,
                &hash,
                &normalized.0,
                &normalized.1,
                Some(&note_id),
            )?
        {
            return Err(duplicate_error());
        }

        let now = unix_time_millis()?;
        let changed = transaction
            .execute(
                "UPDATE notes
                 SET deck_id = ?1, note_type = ?2,
                     front_doc = ?3, back_doc = ?4,
                     front_text = ?5, back_text = ?6, duplicate_hash = ?7,
                     reverse_enabled = ?8, cloze_source = ?9, cloze_mode = ?10,
                     updated_at = ?11, revision = revision + 1
                 WHERE id = ?12 AND revision = ?13 AND deleted_at IS NULL",
                params![
                    deck_id,
                    prepared.note_type,
                    prepared.content.front_doc_json,
                    prepared.content.back_doc_json,
                    prepared.content.front_text,
                    prepared.content.back_text,
                    hash,
                    crate::util::bool_to_integer(prepared.reverse_enabled),
                    prepared.cloze_source,
                    prepared.cloze_mode,
                    now,
                    note_id,
                    revision,
                ],
            )
            .map_err(CommandError::database)?;
        if changed != 1 {
            return Err(CommandError::new(
                "card_changed",
                "This card changed in another window. Reload it and try again.",
            ));
        }
        sync_generated_cards(
            &transaction,
            &note_id,
            &deck_id,
            &prepared.cards,
            now,
        )?;
        replace_note_assets(&transaction, &note_id, &prepared.content)?;
        replace_note_tags(&transaction, &note_id, &tags, now)?;
        transaction.commit().map_err(CommandError::database)
    })?;

    database.with_connection(|connection| {
        hydrate_organized_card(connection, &paths, &note_id)
    })
}

fn prepare_generated_note(
    input: GeneratedNoteDraft,
) -> Result<PreparedGeneratedNote, CommandError> {
    let GeneratedNoteDraft {
        card_type,
        reverse_enabled,
        cloze_source,
        cloze_mode,
        front_doc,
        back_doc,
        front_text,
        back_text,
    } = input;
    match card_type.as_str() {
        "basic" => {
            let content = validate_content(front_doc, back_doc, front_text, back_text)?;
            let mut cards = vec![GeneratedCardSpec {
                direction: "forward",
                template_ordinal: 0,
            }];
            if reverse_enabled {
                cards.push(GeneratedCardSpec {
                    direction: "reverse",
                    template_ordinal: 0,
                });
            }
            Ok(PreparedGeneratedNote {
                note_type: "basic",
                reverse_enabled,
                cloze_source: String::new(),
                cloze_mode: "one_at_a_time".to_string(),
                content,
                cards,
            })
        }
        "cloze" => {
            validate_cloze_mode(&cloze_mode)?;
            let parsed = parse_cloze(&cloze_source)?;
            let primary_group = if cloze_mode == "all_together" {
                None
            } else {
                parsed.groups.first().copied()
            };
            let primary_question = render_cloze_question(&parsed, primary_group);
            let primary_answer = render_cloze_answer(&parsed, primary_group);
            let content = validate_content(
                plain_text_document(&primary_question),
                plain_text_document(&primary_answer),
                primary_question,
                primary_answer,
            )?;
            let cards = if cloze_mode == "all_together" {
                vec![GeneratedCardSpec {
                    direction: "forward",
                    template_ordinal: 0,
                }]
            } else {
                parsed
                    .groups
                    .iter()
                    .enumerate()
                    .map(|(index, group)| GeneratedCardSpec {
                        direction: if index == 0 { "forward" } else { "cloze" },
                        template_ordinal: i64::from(*group),
                    })
                    .collect()
            };
            Ok(PreparedGeneratedNote {
                note_type: "cloze",
                reverse_enabled: false,
                cloze_source,
                cloze_mode,
                content,
                cards,
            })
        }
        _ => Err(CommandError::new(
            "invalid_card_type",
            "Choose Basic or Fill in the blank as the card type.",
        )),
    }
}
