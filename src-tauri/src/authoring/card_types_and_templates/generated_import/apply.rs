use super::*;

#[cfg(test)]
pub(super) fn import_prepared_flashcards(
    connection: &mut Connection,
    prepared: PreparedImport,
) -> Result<FlashcardImportResultDto, CommandError> {
    import_prepared_flashcards_with_commit(connection, prepared, |_, _, _| Ok(()))
}

pub(super) fn import_prepared_flashcards_with_commit<F>(
    connection: &mut Connection,
    prepared: PreparedImport,
    before_commit: F,
) -> Result<FlashcardImportResultDto, CommandError>
where
    F: FnOnce(&Connection, &FlashcardImportResultDto, i64) -> Result<(), CommandError>,
{
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(CommandError::database)?;
    let now = unix_time_millis()?;
    let append_mode = prepared.mode == GeneratedImportDeckMode::Append;
    let deck_id = resolve_generated_import_deck(
        &transaction,
        prepared.mode,
        &prepared.name,
        &prepared.description,
        now,
    )?;

    let destination_duplicates = load_destination_duplicate_pairs(&transaction, &deck_id)?;
    let mut seen = HashSet::new();
    let mut created_notes = 0_i64;
    let mut created_cards = 0_i64;
    let mut skipped = 0_i64;

    for note in prepared.notes {
        let PreparedImportNote {
            prepared,
            tags,
            normalized,
            duplicate_hash,
        } = note;
        let duplicate_in_batch = !seen.insert(normalized.clone());
        let duplicate_in_deck = destination_duplicates.contains(&normalized);
        if duplicate_in_batch || duplicate_in_deck {
            skipped += 1;
            continue;
        }

        let PreparedGeneratedNote {
            note_type,
            reverse_enabled,
            cloze_source,
            cloze_mode,
            content,
            cards,
        } = prepared;
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
                    note_type,
                    content.front_doc_json,
                    content.back_doc_json,
                    content.front_text,
                    content.back_text,
                    duplicate_hash,
                    crate::util::bool_to_integer(reverse_enabled),
                    cloze_source,
                    cloze_mode,
                    now,
                ],
            )
            .map_err(CommandError::database)?;
        sync_generated_cards(&transaction, &note_id, &deck_id, &cards, now)?;
        replace_note_assets(&transaction, &note_id, &content)?;
        replace_note_tags(&transaction, &note_id, &tags, now)?;
        created_notes += 1;
        created_cards += i64::try_from(cards.len()).unwrap_or(i64::MAX);
    }

    if append_mode && created_notes > 0 {
        transaction
            .execute(
                "UPDATE decks
                 SET updated_at = ?2, revision = revision + 1
                 WHERE id = ?1",
                params![deck_id, now],
            )
            .map_err(CommandError::database)?;
    }

    let result = FlashcardImportResultDto {
        deck_id,
        created_note_count: created_notes,
        created_card_count: created_cards,
        skipped_duplicate_count: skipped,
    };
    before_commit(&transaction, &result, now)?;
    transaction.commit().map_err(CommandError::database)?;
    Ok(result)
}

fn load_destination_duplicate_pairs(
    connection: &Connection,
    deck_id: &str,
) -> Result<HashSet<(String, String)>, CommandError> {
    let mut statement = connection
        .prepare(
            "SELECT front_text, back_text FROM notes WHERE deck_id = ?1 AND deleted_at IS NULL",
        )
        .map_err(CommandError::database)?;
    let rows = statement
        .query_map([deck_id], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })
        .map_err(CommandError::database)?;
    let mut duplicates = HashSet::new();
    for row in rows {
        let (front, back) = row.map_err(CommandError::database)?;
        duplicates.insert(normalized_pair(&front, &back));
    }
    Ok(duplicates)
}

fn resolve_generated_import_deck(
    connection: &Connection,
    mode: GeneratedImportDeckMode,
    name: &str,
    description: &str,
    now: i64,
) -> Result<String, CommandError> {
    match mode {
        GeneratedImportDeckMode::Create => {
            insert_generated_import_deck(connection, name, description, now)
        }
        GeneratedImportDeckMode::Append => {
            let mut statement = connection
                .prepare(
                    "SELECT id
                     FROM decks
                     WHERE name = ?1
                       AND deleted_at IS NULL
                       AND archived_at IS NULL
                     ORDER BY created_at ASC, id ASC",
                )
                .map_err(CommandError::database)?;
            let rows = statement
                .query_map([name], |row| row.get::<_, String>(0))
                .map_err(CommandError::database)?;
            let mut ids = Vec::new();
            for row in rows {
                ids.push(row.map_err(CommandError::database)?);
            }
            match ids.as_slice() {
                [deck_id] => Ok(deck_id.clone()),
                [] => Err(CommandError::new(
                    "generated_import_deck_not_found",
                    format!(
                        "The JSON file requested append mode, but no active deck named “{name}” exists."
                    ),
                )),
                _ => Err(CommandError::new(
                    "generated_import_deck_ambiguous",
                    format!(
                        "The JSON file requested append mode, but more than one active deck is named “{name}”. Rename the destination deck so the match is unique."
                    ),
                )),
            }
        }
    }
}

fn insert_generated_import_deck(
    connection: &Connection,
    name: &str,
    description: &str,
    now: i64,
) -> Result<String, CommandError> {
    let deck_id = generate_id(connection)?;
    let sort_order: i64 = connection
        .query_row(
            "SELECT COALESCE(MAX(sort_order), -1) + 1 FROM decks WHERE deleted_at IS NULL",
            [],
            |row| row.get(0),
        )
        .map_err(CommandError::database)?;
    connection
        .execute(
            "INSERT INTO decks (
                 id, name, description, sort_order, created_at, updated_at, revision
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?5, 1)",
            params![deck_id, name, description, sort_order, now],
        )
        .map_err(CommandError::database)?;
    Ok(deck_id)
}
