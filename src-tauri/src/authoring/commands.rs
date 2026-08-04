#[tauri::command]
pub fn list_tags(
    database: State<'_, Database>,
    input: ListTagsInput,
) -> Result<Vec<TagDto>, CommandError> {
    let deck_ids = effective_deck_ids(&input.deck_ids)?;
    database.with_connection(|connection| list_tags_in_connection(connection, &deck_ids))
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
pub fn list_matching_cards(
    database: State<'_, Database>,
    input: SearchCardsInput,
) -> Result<Vec<SelectionCardDto>, CommandError> {
    validate_search_input(&input)?;
    database.with_connection(|connection| list_matching_cards_in_connection(connection, input))
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
    let CreateOrganizedCardInput {
        deck_id,
        front_doc,
        back_doc,
        front_text,
        back_text,
        tags,
        allow_duplicate,
        asset_draft_id,
    } = input;
    validate_entity_id(&deck_id, "deck")?;
    if let Some(draft_id) = asset_draft_id.as_deref() {
        validate_entity_id(draft_id, "asset draft")?;
    }
    let content = validate_content(front_doc, back_doc, front_text, back_text)?;
    let asset_ids = content_asset_ids(&content);
    let tags = validate_tags(tags)?;
    let normalized = normalized_pair(&content.front_text, &content.back_text);
    let duplicate_hash = duplicate_hash(&normalized.0, &normalized.1);

    let note_id = database.with_connection(|connection| {
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(CommandError::database)?;
        ensure_active_deck(&transaction, &deck_id)?;
        ensure_assets_available(
            &transaction,
            &paths,
            &asset_ids,
            asset_draft_id.as_deref(),
        )?;
        if !allow_duplicate
            && duplicate_checkable(&content.front_text, &content.back_text)
            && has_exact_duplicate(
                &transaction,
                &deck_id,
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
            &deck_id,
            content,
            &tags,
            &duplicate_hash,
            now,
        )?;
        let finalization = finalize_assets_for_owner(
            &transaction,
            &paths,
            &asset_ids,
            asset_draft_id.as_deref(),
            Some(&note_id),
        )?;
        transaction.commit().map_err(CommandError::database)?;
        finalization.cleanup();
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
    let UpdateOrganizedCardInput {
        note_id,
        deck_id,
        revision,
        front_doc,
        back_doc,
        front_text,
        back_text,
        tags,
        allow_duplicate,
        asset_draft_id,
    } = input;
    validate_entity_id(&note_id, "note")?;
    validate_entity_id(&deck_id, "deck")?;
    if let Some(draft_id) = asset_draft_id.as_deref() {
        validate_entity_id(draft_id, "asset draft")?;
    }
    if revision < 1 {
        return Err(CommandError::new(
            "invalid_revision",
            "The card revision is invalid.",
        ));
    }
    let content = validate_content(front_doc, back_doc, front_text, back_text)?;
    let asset_ids = content_asset_ids(&content);
    let tags = validate_tags(tags)?;
    let normalized = normalized_pair(&content.front_text, &content.back_text);
    let duplicate_hash = duplicate_hash(&normalized.0, &normalized.1);

    database.with_connection(|connection| {
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(CommandError::database)?;
        ensure_assets_available(
            &transaction,
            &paths,
            &asset_ids,
            asset_draft_id.as_deref(),
        )?;
        ensure_active_deck(&transaction, &deck_id)?;

        let note_exists: i64 = transaction
            .query_row(
                "SELECT EXISTS(
                     SELECT 1 FROM notes WHERE id = ?1 AND deleted_at IS NULL
                 )",
                [note_id.as_str()],
                |row| row.get(0),
            )
            .map_err(CommandError::database)?;
        if note_exists == 0 {
            return Err(CommandError::new(
                "card_not_found",
                "That flashcard no longer exists.",
            ));
        }

        if !allow_duplicate
            && duplicate_checkable(&content.front_text, &content.back_text)
            && has_exact_duplicate(
                &transaction,
                &deck_id,
                &duplicate_hash,
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
                 SET deck_id = ?1, front_doc = ?2, back_doc = ?3,
                     front_text = ?4, back_text = ?5, duplicate_hash = ?6,
                     updated_at = ?7, revision = revision + 1
                 WHERE id = ?8 AND revision = ?9 AND deleted_at IS NULL",
                params![
                    deck_id,
                    content.front_doc_json,
                    content.back_doc_json,
                    content.front_text,
                    content.back_text,
                    duplicate_hash,
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
        transaction
            .execute(
                "UPDATE cards
                 SET deck_id = ?1, updated_at = ?2
                 WHERE note_id = ?3 AND deleted_at IS NULL",
                params![deck_id, now, note_id],
            )
            .map_err(CommandError::database)?;
        replace_note_assets(&transaction, &note_id, &content)?;
        replace_note_tags(&transaction, &note_id, &tags, now)?;
        let finalization = finalize_assets_for_owner(
            &transaction,
            &paths,
            &asset_ids,
            asset_draft_id.as_deref(),
            Some(&note_id),
        )?;
        transaction.commit().map_err(CommandError::database)?;
        finalization.cleanup();
        Ok(())
    })?;

    database.with_connection(|connection| hydrate_organized_card(connection, &paths, &note_id))
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

fn validate_bulk_rows(deck_id: &str, rows: &[BulkCardRowInput]) -> Result<(), CommandError> {
    validate_entity_id(deck_id, "deck")?;
    if rows.is_empty() {
        return Err(CommandError::new(
            "empty_bulk_entry",
            "Add at least one question and answer.",
        ));
    }
    if rows.len() > MAX_BULK_ROWS {
        return Err(CommandError::new(
            "too_many_bulk_cards",
            format!("Add at most {MAX_BULK_ROWS} cards at one time."),
        ));
    }
    Ok(())
}

#[tauri::command]
pub fn analyze_bulk_cards(
    database: State<'_, Database>,
    input: AnalyzeBulkCardsInput,
) -> Result<BulkAnalyzeResultDto, CommandError> {
    validate_bulk_rows(&input.deck_id, &input.rows)?;
    let prepared = input
        .rows
        .into_iter()
        .enumerate()
        .map(|(index, row)| prepare_bulk_row(index, row))
        .collect::<Result<Vec<_>, CommandError>>()?;
    let row_count = i64::try_from(prepared.len()).unwrap_or(i64::MAX);

    database.with_connection(|connection| {
        ensure_active_deck(connection, &input.deck_id)?;
        let mut seen = HashSet::new();
        let mut duplicate_row_indices = Vec::new();
        for (index, (content, _, normalized, hash)) in prepared.into_iter().enumerate() {
            let duplicate_in_batch = !seen.insert(normalized.clone());
            let duplicate_in_deck = has_exact_duplicate(
                connection,
                &input.deck_id,
                &hash,
                &normalized.0,
                &normalized.1,
                None,
            )?;
            if duplicate_checkable(&content.front_text, &content.back_text)
                && (duplicate_in_batch || duplicate_in_deck)
            {
                duplicate_row_indices.push(i64::try_from(index).unwrap_or(i64::MAX));
            }
        }
        let duplicate_count = i64::try_from(duplicate_row_indices.len()).unwrap_or(i64::MAX);
        Ok(BulkAnalyzeResultDto {
            unique_count: row_count.saturating_sub(duplicate_count),
            duplicate_count,
            duplicate_row_indices,
        })
    })
}

#[tauri::command]
pub fn bulk_create_cards(
    database: State<'_, Database>,
    input: BulkCreateCardsInput,
) -> Result<BulkCreateResultDto, CommandError> {
    validate_bulk_rows(&input.deck_id, &input.rows)?;
    let BulkCreateCardsInput { deck_id, rows } = input;
    let prepared = rows
        .into_iter()
        .enumerate()
        .map(|(index, row)| prepare_bulk_row(index, row))
        .collect::<Result<Vec<_>, CommandError>>()?;

    database.with_connection(|connection| {
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(CommandError::database)?;
        ensure_active_deck(&transaction, &deck_id)?;
        let now = unix_time_millis()?;
        let mut seen = HashSet::new();
        let mut created_count = 0_i64;
        let mut skipped_duplicates = 0_i64;

        for (content, tags, normalized, hash) in prepared {
            let duplicate_in_batch = !seen.insert(normalized.clone());
            let duplicate_in_deck = has_exact_duplicate(
                &transaction,
                &deck_id,
                &hash,
                &normalized.0,
                &normalized.1,
                None,
            )?;
            if duplicate_in_batch || duplicate_in_deck {
                skipped_duplicates += 1;
                continue;
            }

            insert_organized_card(
                &transaction,
                &deck_id,
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
pub fn bulk_tag_cards(
    database: State<'_, Database>,
    input: BulkTagCardsInput,
) -> Result<BulkActionResultDto, CommandError> {
    let note_ids = validate_note_ids_for_action(
        input.note_ids,
        MAX_BULK_ACTION_NOTES,
        "Change tags on",
    )?;
    let tags = validate_tags(input.tags)?;
    if tags.is_empty() {
        return Err(CommandError::new(
            "no_tags_selected",
            "Add at least one tag to apply or remove.",
        ));
    }
    let mode = TagChangeMode::parse(&input.mode)?;

    database.with_connection(|connection| {
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(CommandError::database)?;
        let now = unix_time_millis()?;
        let tag_ids = match mode {
            TagChangeMode::Add => resolve_tag_ids(&transaction, &tags, now)?,
            TagChangeMode::Remove => find_existing_tag_ids(&transaction, &tags)?,
        };

        for note_id in &note_ids {
            ensure_active_note(&transaction, note_id)?;
            match mode {
                TagChangeMode::Add => {
                    for tag_id in &tag_ids {
                        transaction
                            .execute(
                                "INSERT OR IGNORE INTO note_tags (note_id, tag_id)
                                 VALUES (?1, ?2)",
                                params![note_id, tag_id],
                            )
                            .map_err(CommandError::database)?;
                    }
                }
                TagChangeMode::Remove => {
                    for tag_id in &tag_ids {
                        transaction
                            .execute(
                                "DELETE FROM note_tags WHERE note_id = ?1 AND tag_id = ?2",
                                params![note_id, tag_id],
                            )
                            .map_err(CommandError::database)?;
                    }
                }
            }
        }
        cleanup_unused_tags(&transaction)?;
        transaction.commit().map_err(CommandError::database)?;
        Ok(BulkActionResultDto {
            affected_count: i64::try_from(note_ids.len()).unwrap_or(i64::MAX),
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
        let rows = statement
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                ))
            })
            .map_err(|error| format!("Could not read duplicate backfill rows: {error}"))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| format!("Could not collect duplicate backfill rows: {error}"))?;
        rows
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
