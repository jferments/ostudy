#[tauri::command]
pub fn list_tags(
    database: State<'_, Database>,
    input: ListTagsInput,
) -> Result<Vec<TagDto>, CommandError> {
    if let Some(deck_id) = &input.deck_id {
        validate_entity_id(deck_id, "deck")?;
    }
    database.with_connection(|connection| list_tags_in_connection(connection, input.deck_id.as_deref()))
}

#[tauri::command]
pub fn search_cards(
    database: State<'_, Database>,
    input: SearchCardsInput,
) -> Result<CardSearchPageDto, CommandError> {
    validate_search_input(&input)?;
    database.with_connection(|connection| search_cards_in_connection(connection, input))
}

#[tauri::command]
pub fn get_organized_card(
    database: State<'_, Database>,
    paths: State<'_, AppPaths>,
    note_id: String,
) -> Result<OrganizedCardDto, CommandError> {
    validate_entity_id(&note_id, "note")?;
    database.with_connection(|connection| hydrate_organized_card(connection, &paths, &note_id))
}

#[tauri::command]
pub fn create_organized_card(
    database: State<'_, Database>,
    paths: State<'_, AppPaths>,
    input: CreateOrganizedCardInput,
) -> Result<OrganizedCardDto, CommandError> {
    validate_entity_id(&input.deck_id, "deck")?;
    let content = validate_content(
        input.front_doc,
        input.back_doc,
        input.front_text,
        input.back_text,
    )?;
    let tags = validate_tags(input.tags)?;
    let normalized = normalized_pair(&content.front_text, &content.back_text);
    let duplicate_hash = duplicate_hash(&normalized.0, &normalized.1);

    let note_id = database.with_connection(|connection| {
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(CommandError::database)?;
        ensure_active_deck(&transaction, &input.deck_id)?;
        if !input.allow_duplicate
            && duplicate_checkable(&content.front_text, &content.back_text)
            && has_exact_duplicate(
                &transaction,
                &input.deck_id,
                &duplicate_hash,
                &normalized.0,
                &normalized.1,
                None,
            )?
        {
            return Err(duplicate_error());
        }
        let now = unix_time_millis()?;
        let note_id = insert_organized_card(
            &transaction,
            &input.deck_id,
            content,
            &tags,
            &duplicate_hash,
            now,
        )?;
        transaction.commit().map_err(CommandError::database)?;
        Ok(note_id)
    })?;

    database.with_connection(|connection| hydrate_organized_card(connection, &paths, &note_id))
}

#[tauri::command]
pub fn update_organized_card(
    database: State<'_, Database>,
    paths: State<'_, AppPaths>,
    input: UpdateOrganizedCardInput,
) -> Result<OrganizedCardDto, CommandError> {
    validate_entity_id(&input.note_id, "note")?;
    if input.revision < 1 {
        return Err(CommandError::new(
            "invalid_revision",
            "The card revision is invalid.",
        ));
    }
    let content = validate_content(
        input.front_doc,
        input.back_doc,
        input.front_text,
        input.back_text,
    )?;
    let tags = validate_tags(input.tags)?;
    let normalized = normalized_pair(&content.front_text, &content.back_text);
    let duplicate_hash = duplicate_hash(&normalized.0, &normalized.1);

    database.with_connection(|connection| {
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(CommandError::database)?;
        ensure_assets_exist(&transaction, &content_asset_ids(&content))?;

        let deck_id: String = transaction
            .query_row(
                "SELECT deck_id FROM notes WHERE id = ?1 AND deleted_at IS NULL",
                [input.note_id.as_str()],
                |row| row.get(0),
            )
            .optional()
            .map_err(CommandError::database)?
            .ok_or_else(|| {
                CommandError::new("card_not_found", "That flashcard no longer exists.")
            })?;

        if !input.allow_duplicate
            && duplicate_checkable(&content.front_text, &content.back_text)
            && has_exact_duplicate(
                &transaction,
                &deck_id,
                &duplicate_hash,
                &normalized.0,
                &normalized.1,
                Some(&input.note_id),
            )?
        {
            return Err(duplicate_error());
        }

        let now = unix_time_millis()?;
        let changed = transaction
            .execute(
                "UPDATE notes
                 SET front_doc = ?1, back_doc = ?2, front_text = ?3, back_text = ?4,
                     duplicate_hash = ?5, updated_at = ?6, revision = revision + 1
                 WHERE id = ?7 AND revision = ?8 AND deleted_at IS NULL",
                params![
                    content.front_doc_json,
                    content.back_doc_json,
                    content.front_text,
                    content.back_text,
                    duplicate_hash,
                    now,
                    input.note_id,
                    input.revision,
                ],
            )
            .map_err(CommandError::database)?;
        if changed != 1 {
            return Err(CommandError::new(
                "card_changed",
                "This card changed in another window. Reload it and try again.",
            ));
        }
        transaction
            .execute(
                "UPDATE cards SET updated_at = ?1
                 WHERE note_id = ?2 AND deleted_at IS NULL",
                params![now, input.note_id],
            )
            .map_err(CommandError::database)?;
        replace_note_assets(&transaction, &input.note_id, &content)?;
        replace_note_tags(&transaction, &input.note_id, &tags, now)?;
        transaction.commit().map_err(CommandError::database)
    })?;

    database.with_connection(|connection| {
        hydrate_organized_card(connection, &paths, &input.note_id)
    })
}

#[tauri::command]
pub fn find_duplicate_cards(
    database: State<'_, Database>,
    input: DuplicateCheckInput,
) -> Result<Vec<DuplicateCardDto>, CommandError> {
    validate_entity_id(&input.deck_id, "deck")?;
    if let Some(note_id) = &input.exclude_note_id {
        validate_entity_id(note_id, "note")?;
    }
    if !duplicate_checkable(&input.front_text, &input.back_text) {
        return Ok(Vec::new());
    }
    let normalized = normalized_pair(&input.front_text, &input.back_text);
    let hash = duplicate_hash(&normalized.0, &normalized.1);
    database.with_connection(|connection| {
        exact_duplicates(
            connection,
            &input.deck_id,
            &hash,
            &normalized.0,
            &normalized.1,
            input.exclude_note_id.as_deref(),
            20,
        )
    })
}

#[tauri::command]
pub fn bulk_create_cards(
    database: State<'_, Database>,
    input: BulkCreateCardsInput,
) -> Result<BulkCreateResultDto, CommandError> {
    validate_entity_id(&input.deck_id, "deck")?;
    if input.rows.is_empty() {
        return Err(CommandError::new(
            "empty_bulk_entry",
            "Add at least one question and answer.",
        ));
    }
    if input.rows.len() > MAX_BULK_ROWS {
        return Err(CommandError::new(
            "too_many_bulk_cards",
            format!("Add at most {MAX_BULK_ROWS} cards at one time."),
        ));
    }

    let prepared = input
        .rows
        .into_iter()
        .enumerate()
        .map(|(index, row)| prepare_bulk_row(index, row))
        .collect::<Result<Vec<_>, CommandError>>()?;

    database.with_connection(|connection| {
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(CommandError::database)?;
        ensure_active_deck(&transaction, &input.deck_id)?;
        let now = unix_time_millis()?;
        let mut seen = HashSet::new();
        let mut created_count = 0_i64;
        let mut skipped_duplicates = 0_i64;

        for (content, tags, normalized, hash) in prepared {
            let duplicate_in_batch = !seen.insert(normalized.clone());
            let duplicate_in_deck = has_exact_duplicate(
                &transaction,
                &input.deck_id,
                &hash,
                &normalized.0,
                &normalized.1,
                None,
            )?;
            if input.skip_duplicates && (duplicate_in_batch || duplicate_in_deck) {
                skipped_duplicates += 1;
                continue;
            }

            insert_organized_card(
                &transaction,
                &input.deck_id,
                content,
                &tags,
                &hash,
                now,
            )?;
            created_count += 1;
        }

        transaction.commit().map_err(CommandError::database)?;
        Ok(BulkCreateResultDto {
            created_count,
            skipped_duplicates,
        })
    })
}

#[tauri::command]
pub fn transfer_cards(
    database: State<'_, Database>,
    input: TransferCardsInput,
) -> Result<TransferCardsResultDto, CommandError> {
    validate_entity_id(&input.target_deck_id, "destination deck")?;
    let note_ids = validate_note_ids(input.note_ids)?;
    let mode = TransferMode::parse(&input.mode)?;

    database.with_connection(|connection| {
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(CommandError::database)?;
        ensure_active_deck(&transaction, &input.target_deck_id)?;
        let now = unix_time_millis()?;
        let mut affected_count = 0_i64;
        let mut skipped_duplicates = 0_i64;

        for note_id in note_ids {
            let source = load_source_note(&transaction, &note_id)?;
            if source.deck_id == input.target_deck_id {
                return Err(CommandError::new(
                    "same_destination_deck",
                    "Choose a different destination deck.",
                ));
            }
            let normalized = normalized_pair(&source.front_text, &source.back_text);
            let source_hash = if source.duplicate_hash.is_empty() {
                duplicate_hash(&normalized.0, &normalized.1)
            } else {
                source.duplicate_hash.clone()
            };
            if input.skip_duplicates
                && duplicate_checkable(&source.front_text, &source.back_text)
                && has_exact_duplicate(
                    &transaction,
                    &input.target_deck_id,
                    &source_hash,
                    &normalized.0,
                    &normalized.1,
                    None,
                )?
            {
                skipped_duplicates += 1;
                continue;
            }

            match mode {
                TransferMode::Move => {
                    transaction
                        .execute(
                            "UPDATE notes
                             SET deck_id = ?1, updated_at = ?2, revision = revision + 1
                             WHERE id = ?3 AND deleted_at IS NULL",
                            params![input.target_deck_id, now, source.id],
                        )
                        .map_err(CommandError::database)?;
                    transaction
                        .execute(
                            "UPDATE cards
                             SET deck_id = ?1, updated_at = ?2
                             WHERE note_id = ?3 AND deleted_at IS NULL",
                            params![input.target_deck_id, now, source.id],
                        )
                        .map_err(CommandError::database)?;
                }
                TransferMode::Copy => {
                    copy_note_to_deck(
                        &transaction,
                        &source,
                        &input.target_deck_id,
                        &source_hash,
                        now,
                    )?;
                }
            }
            affected_count += 1;
        }

        transaction.commit().map_err(CommandError::database)?;
        Ok(TransferCardsResultDto {
            affected_count,
            skipped_duplicates,
        })
    })
}

pub(crate) fn backfill_duplicate_hashes(connection: &mut Connection) -> Result<(), String> {
    let rows = {
        let mut statement = connection
            .prepare(
                "SELECT id, front_text, back_text
                 FROM notes
                 WHERE duplicate_hash = '' AND deleted_at IS NULL",
            )
            .map_err(|error| format!("Could not prepare duplicate backfill: {error}"))?;
        statement
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                ))
            })
            .map_err(|error| format!("Could not read duplicate backfill rows: {error}"))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| format!("Could not collect duplicate backfill rows: {error}"))?
    };

    if rows.is_empty() {
        return Ok(());
    }
    let transaction = connection
        .transaction()
        .map_err(|error| format!("Could not start duplicate backfill: {error}"))?;
    for (note_id, front_text, back_text) in rows {
        let normalized = normalized_pair(&front_text, &back_text);
        let hash = duplicate_hash(&normalized.0, &normalized.1);
        transaction
            .execute(
                "UPDATE notes SET duplicate_hash = ?1 WHERE id = ?2",
                params![hash, note_id],
            )
            .map_err(|error| format!("Could not update duplicate fingerprint: {error}"))?;
    }
    transaction
        .commit()
        .map_err(|error| format!("Could not commit duplicate backfill: {error}"))
}
