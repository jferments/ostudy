fn insert_organized_card(
    transaction: &Transaction<'_>,
    deck_id: &str,
    content: ValidatedContent,
    tags: &[ValidatedTag],
    duplicate_hash: &str,
    now: i64,
) -> Result<String, CommandError> {
    ensure_assets_exist(transaction, &content_asset_ids(&content))?;
    let note_id = generate_id(transaction)?;
    let card_id = generate_id(transaction)?;

    transaction
        .execute(
            "INSERT INTO notes (
                 id, deck_id, note_type, content_format, content_schema_version,
                 front_doc, back_doc, front_text, back_text, duplicate_hash,
                 created_at, updated_at, revision
             ) VALUES (
                 ?1, ?2, 'basic', 'tiptap-json', 1,
                 ?3, ?4, ?5, ?6, ?7, ?8, ?8, 1
             )",
            params![
                note_id,
                deck_id,
                content.front_doc_json,
                content.back_doc_json,
                content.front_text,
                content.back_text,
                duplicate_hash,
                now,
            ],
        )
        .map_err(CommandError::database)?;
    transaction
        .execute(
            "INSERT INTO cards (
                 id, note_id, deck_id, direction, template_ordinal, created_at, updated_at
             ) VALUES (?1, ?2, ?3, 'forward', 0, ?4, ?4)",
            params![card_id, note_id, deck_id, now],
        )
        .map_err(CommandError::database)?;
    insert_new_schedule(transaction, &card_id, now)?;
    replace_note_assets(transaction, &note_id, &content)?;
    replace_note_tags(transaction, &note_id, tags, now)?;
    Ok(note_id)
}

fn replace_note_assets(
    transaction: &Transaction<'_>,
    note_id: &str,
    content: &ValidatedContent,
) -> Result<(), CommandError> {
    transaction
        .execute("DELETE FROM note_assets WHERE note_id = ?1", [note_id])
        .map_err(CommandError::database)?;
    for asset_id in &content.front_asset_ids {
        transaction
            .execute(
                "INSERT INTO note_assets (note_id, asset_id, side)
                 VALUES (?1, ?2, 'front')",
                params![note_id, asset_id],
            )
            .map_err(CommandError::database)?;
    }
    for asset_id in &content.back_asset_ids {
        transaction
            .execute(
                "INSERT INTO note_assets (note_id, asset_id, side)
                 VALUES (?1, ?2, 'back')",
                params![note_id, asset_id],
            )
            .map_err(CommandError::database)?;
    }
    Ok(())
}

fn content_asset_ids(content: &ValidatedContent) -> Vec<String> {
    content
        .front_asset_ids
        .iter()
        .chain(&content.back_asset_ids)
        .cloned()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

fn ensure_active_deck(
    connection: &Connection,
    deck_id: &str,
) -> Result<(), CommandError> {
    let exists: i64 = connection
        .query_row(
            "SELECT EXISTS(
                 SELECT 1 FROM decks
                 WHERE id = ?1 AND deleted_at IS NULL AND archived_at IS NULL
             )",
            [deck_id],
            |row| row.get(0),
        )
        .map_err(CommandError::database)?;
    if exists == 0 {
        return Err(CommandError::new(
            "deck_not_found",
            "The destination deck no longer exists.",
        ));
    }
    Ok(())
}

fn exact_duplicates(
    connection: &Connection,
    deck_id: &str,
    hash: &str,
    normalized_front: &str,
    normalized_back: &str,
    exclude_note_id: Option<&str>,
    limit: usize,
) -> Result<Vec<DuplicateCardDto>, CommandError> {
    let mut statement = connection
        .prepare(
            "SELECT id, deck_id, front_text, back_text
             FROM notes
             WHERE deck_id = ?1
               AND (duplicate_hash = ?2 OR duplicate_hash = '')
               AND deleted_at IS NULL
               AND (?3 IS NULL OR id != ?3)
             ORDER BY updated_at DESC",
        )
        .map_err(CommandError::database)?;
    let rows = statement
        .query_map(
            params![deck_id, hash, exclude_note_id],
            |row| {
                Ok(DuplicateCardDto {
                    note_id: row.get(0)?,
                    deck_id: row.get(1)?,
                    front_text: row.get(2)?,
                    back_text: row.get(3)?,
                })
            },
        )
        .map_err(CommandError::database)?;

    let mut matches = Vec::new();
    for row in rows {
        let candidate = row.map_err(CommandError::database)?;
        let normalized = normalized_pair(&candidate.front_text, &candidate.back_text);
        if normalized.0 == normalized_front && normalized.1 == normalized_back {
            matches.push(candidate);
            if matches.len() >= limit {
                break;
            }
        }
    }
    Ok(matches)
}

fn has_exact_duplicate(
    connection: &Connection,
    deck_id: &str,
    hash: &str,
    normalized_front: &str,
    normalized_back: &str,
    exclude_note_id: Option<&str>,
) -> Result<bool, CommandError> {
    Ok(!exact_duplicates(
        connection,
        deck_id,
        hash,
        normalized_front,
        normalized_back,
        exclude_note_id,
        20,
    )?
    .is_empty())
}

fn duplicate_error() -> CommandError {
    CommandError::new(
        "duplicate_card",
        "An identical question and answer already exist in this deck.",
    )
}

fn prepare_bulk_row(
    index: usize,
    row: BulkCardRowInput,
) -> Result<PreparedBulkRow, CommandError> {
    if row.question.trim().is_empty() || row.answer.trim().is_empty() {
        return Err(CommandError::new(
            "invalid_bulk_card",
            format!("Bulk row {} needs both a question and an answer.", index + 1),
        ));
    }
    let front_doc = plain_text_document(&row.question);
    let back_doc = plain_text_document(&row.answer);
    let content = validate_content(
        front_doc,
        back_doc,
        row.question,
        row.answer,
    )?;
    let tags = validate_tags(row.tags)?;
    let normalized = normalized_pair(&content.front_text, &content.back_text);
    let hash = duplicate_hash(&normalized.0, &normalized.1);
    Ok((content, tags, normalized, hash))
}

fn plain_text_document(text: &str) -> Value {
    let paragraphs = text
        .lines()
        .map(|line| {
            if line.is_empty() {
                json!({"type": "paragraph"})
            } else {
                json!({
                    "type": "paragraph",
                    "content": [{"type": "text", "text": line}]
                })
            }
        })
        .collect::<Vec<_>>();
    json!({
        "type": "doc",
        "content": if paragraphs.is_empty() {
            vec![json!({"type": "paragraph"})]
        } else {
            paragraphs
        }
    })
}

fn validate_note_ids(values: Vec<String>) -> Result<Vec<String>, CommandError> {
    let mut unique = BTreeSet::new();
    for value in values {
        validate_entity_id(&value, "note")?;
        unique.insert(value);
    }
    if unique.is_empty() {
        return Err(CommandError::new(
            "no_cards_selected",
            "Select at least one card to move or copy.",
        ));
    }
    if unique.len() > MAX_TRANSFER_NOTES {
        return Err(CommandError::new(
            "too_many_cards_selected",
            format!("Move or copy at most {MAX_TRANSFER_NOTES} cards at one time."),
        ));
    }
    Ok(unique.into_iter().collect())
}

fn load_source_note(
    connection: &Connection,
    note_id: &str,
) -> Result<SourceNote, CommandError> {
    connection
        .query_row(
            "SELECT id, deck_id, front_doc, back_doc, front_text, back_text,
                    duplicate_hash
             FROM notes
             WHERE id = ?1 AND deleted_at IS NULL",
            [note_id],
            |row| {
                Ok(SourceNote {
                    id: row.get(0)?,
                    deck_id: row.get(1)?,
                    front_doc: row.get(2)?,
                    back_doc: row.get(3)?,
                    front_text: row.get(4)?,
                    back_text: row.get(5)?,
                    duplicate_hash: row.get(6)?,
                })
            },
        )
        .optional()
        .map_err(CommandError::database)?
        .ok_or_else(|| CommandError::new("card_not_found", "A selected card no longer exists."))
}

fn copy_note_to_deck(
    transaction: &Transaction<'_>,
    source: &SourceNote,
    target_deck_id: &str,
    duplicate_hash: &str,
    now: i64,
) -> Result<(), CommandError> {
    let new_note_id = generate_id(transaction)?;
    transaction
        .execute(
            "INSERT INTO notes (
                 id, deck_id, note_type, content_format, content_schema_version,
                 front_doc, back_doc, front_text, back_text, duplicate_hash,
                 created_at, updated_at, revision
             ) VALUES (
                 ?1, ?2, 'basic', 'tiptap-json', 1,
                 ?3, ?4, ?5, ?6, ?7, ?8, ?8, 1
             )",
            params![
                new_note_id,
                target_deck_id,
                source.front_doc,
                source.back_doc,
                source.front_text,
                source.back_text,
                duplicate_hash,
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
                 ORDER BY template_ordinal, direction",
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
             SELECT ?1, asset_id, side
             FROM note_assets
             WHERE note_id = ?2",
            params![new_note_id, source.id],
        )
        .map_err(CommandError::database)?;
    transaction
        .execute(
            "INSERT INTO note_tags (note_id, tag_id)
             SELECT ?1, tag_id
             FROM note_tags
             WHERE note_id = ?2",
            params![new_note_id, source.id],
        )
        .map_err(CommandError::database)?;
    Ok(())
}

#[derive(Clone, Copy)]
enum TransferMode {
    Move,
    Copy,
}

impl TransferMode {
    fn parse(value: &str) -> Result<Self, CommandError> {
        match value {
            "move" => Ok(Self::Move),
            "copy" => Ok(Self::Copy),
            _ => Err(CommandError::new(
                "invalid_transfer_mode",
                "Choose whether to move or copy the selected cards.",
            )),
        }
    }
}

