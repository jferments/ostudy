#[tauri::command]
pub fn transfer_cards(
    database: State<'_, Database>,
    input: TransferCardsInput,
) -> Result<TransferCardsResultDto, CommandError> {
    validate_entity_id(&input.target_deck_id, "destination deck")?;
    let note_ids = validate_note_ids_for_action(
        input.note_ids,
        MAX_TRANSFER_NOTES,
        "Move or copy",
    )?;
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
                TransferMode::Copy => copy_generated_note_to_deck(
                    &transaction,
                    &source,
                    &input.target_deck_id,
                    &source_hash,
                    now,
                )?,
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

fn copy_generated_note_to_deck(
    transaction: &Transaction<'_>,
    source: &SourceNote,
    target_deck_id: &str,
    duplicate_hash: &str,
    now: i64,
) -> Result<(), CommandError> {
    let metadata = transaction
        .query_row(
            "SELECT note_type, content_format, content_schema_version,
                    reverse_enabled, cloze_source, cloze_mode
             FROM notes WHERE id = ?1 AND deleted_at IS NULL",
            [source.id.as_str()],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, i64>(2)?,
                    row.get::<_, i64>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, String>(5)?,
                ))
            },
        )
        .optional()
        .map_err(CommandError::database)?
        .ok_or_else(|| CommandError::new("card_not_found", "A selected card no longer exists."))?;
    let (
        note_type,
        content_format,
        content_schema_version,
        reverse_enabled,
        cloze_source,
        cloze_mode,
    ) = metadata;
    if !matches!(note_type.as_str(), "basic" | "cloze")
        || content_format != CONTENT_FORMAT
        || content_schema_version != CONTENT_SCHEMA_VERSION
    {
        return Err(CommandError::new(
            "unsupported_card_type",
            "A selected card uses a format this version cannot copy.",
        ));
    }

    let new_note_id = generate_id(transaction)?;
    transaction
        .execute(
            "INSERT INTO notes (
                 id, deck_id, note_type, content_format, content_schema_version,
                 front_doc, back_doc, front_text, back_text, duplicate_hash,
                 reverse_enabled, cloze_source, cloze_mode,
                 created_at, updated_at, revision
             ) VALUES (
                 ?1, ?2, ?3, ?4, ?5,
                 ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?14, 1
             )",
            params![
                new_note_id,
                target_deck_id,
                note_type,
                content_format,
                content_schema_version,
                source.front_doc,
                source.back_doc,
                source.front_text,
                source.back_text,
                duplicate_hash,
                reverse_enabled,
                cloze_source,
                cloze_mode,
                now,
            ],
        )
        .map_err(CommandError::database)?;

    let source_cards = {
        let mut statement = transaction
            .prepare(
                "SELECT direction, template_ordinal
                 FROM cards
                 WHERE note_id = ?1 AND deleted_at IS NULL
                 ORDER BY CASE direction WHEN 'forward' THEN 0 ELSE 1 END,
                          template_ordinal, direction",
            )
            .map_err(CommandError::database)?;
        statement
            .query_map([source.id.as_str()], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
            })
            .map_err(CommandError::database)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(CommandError::database)?
    };
    if source_cards.is_empty() {
        return Err(CommandError::new(
            "card_not_found",
            "A selected card has no active study presentation.",
        ));
    }
    for (direction, template_ordinal) in source_cards {
        let new_card_id = generate_id(transaction)?;
        transaction
            .execute(
                "INSERT INTO cards (
                     id, note_id, deck_id, direction, template_ordinal, created_at, updated_at
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?6)",
                params![
                    new_card_id,
                    new_note_id,
                    target_deck_id,
                    direction,
                    template_ordinal,
                    now,
                ],
            )
            .map_err(CommandError::database)?;
        insert_new_schedule(transaction, &new_card_id, now)?;
    }

    transaction
        .execute(
            "INSERT INTO note_assets (note_id, asset_id, side)
             SELECT ?1, asset_id, side FROM note_assets WHERE note_id = ?2",
            params![new_note_id, source.id],
        )
        .map_err(CommandError::database)?;
    transaction
        .execute(
            "INSERT INTO note_tags (note_id, tag_id)
             SELECT ?1, tag_id FROM note_tags WHERE note_id = ?2",
            params![new_note_id, source.id],
        )
        .map_err(CommandError::database)?;
    Ok(())
}
