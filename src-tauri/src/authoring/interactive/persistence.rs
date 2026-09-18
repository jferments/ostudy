use std::collections::BTreeMap;

use rusqlite::{Transaction, TransactionBehavior};
use serde_json::json;
use sha2::{Digest, Sha256};

use super::*;
use crate::{
    asset_lifecycle::{ensure_assets_available, finalize_assets_for_owner},
    cards::questions::QuestionBody,
    editor_drafts::{commit_canonical_editor_draft, EditorDraftKind},
    scheduler::insert_new_schedule,
    util::{generate_id, unix_time_millis},
};

pub(super) fn save(
    connection: &mut Connection,
    paths: &AppPaths,
    input: SaveQuestionNoteInput,
) -> Result<QuestionWriteReceipt, CommandError> {
    validate_entity_id(&input.operation_id, "question save")?;
    validate_entity_id(&input.deck_id, "deck")?;
    match (&input.note_id, input.revision) {
        (None, None) => {}
        (Some(id), Some(revision)) if revision > 0 && revision < i64::MAX => {
            validate_entity_id(id, "question note")?;
        }
        _ => return Err(changed()),
    }
    if let Some(id) = &input.asset_draft_id {
        validate_entity_id(id, "asset draft")?;
    }
    let question = ValidatedQuestion::parse(&input.definition_json)?;
    let definition_json = question.to_json()?;
    let text = question.search_text()?;
    let identity = question.duplicate_identity()?;
    let duplicate_hash = question.duplicate_hash()?;
    let tags = validate_tags(input.tags.clone())?;
    // Canonical definition spelling makes whitespace-only transport changes
    // equivalent; authored order and the exact intended revision stay significant.
    let request = json!({
        "noteId": input.note_id, "revision": input.revision, "deckId": input.deck_id,
        "definition": question.definition(),
        "tags": tags.iter().map(|tag| &tag.name).collect::<Vec<_>>(),
        "allowDuplicate": input.allow_duplicate,
        "assetDraftId": input.asset_draft_id, "editorDraft": input.editor_draft
    });
    let request_hash = format!("{:x}", Sha256::digest(request.to_string().as_bytes()));
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(CommandError::database)?;

    // Replays must precede current note/draft/asset checks: the originally saved
    // material may since have been edited or deleted. Never recreate it.
    if let Some(receipt) = receipt(&transaction, &input.operation_id, &request_hash)? {
        return Ok(receipt);
    }
    ensure_active_deck(&transaction, &input.deck_id)?;
    if let Some(note_id) = &input.note_id {
        let existing = transaction
            .query_row(
                "SELECT n.revision, n.note_type, n.question_definition
                 FROM notes n JOIN decks d ON d.id = n.deck_id
                 WHERE n.id = ?1 AND n.deleted_at IS NULL
                   AND d.deleted_at IS NULL AND d.archived_at IS NULL",
                [note_id],
                |row| {
                    Ok((
                        row.get::<_, i64>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, Option<String>>(2)?,
                    ))
                },
            )
            .optional()
            .map_err(CommandError::database)?
            .ok_or_else(not_found)?;
        if Some(existing.0) != input.revision {
            return Err(changed());
        }
        if existing.1 != "interactive"
            || existing
                .2
                .as_deref()
                .map(ValidatedQuestion::parse)
                .transpose()?
                .is_none_or(|prior| prior.kind() != question.kind())
        {
            return Err(CommandError::new(
                "card_type_locked",
                "A saved question cannot change type. Create a separate question instead.",
            ));
        }
    }
    ensure_assets_available(
        &transaction,
        paths,
        question.asset_ids(),
        input.asset_draft_id.as_deref(),
    )?;
    if let QuestionBody::ImageOcclusion { image_asset_id, .. } = &question.definition().question {
        let mime: String = transaction
            .query_row(
                "SELECT mime_type FROM assets WHERE id = ?1",
                [image_asset_id],
                |row| row.get(0),
            )
            .map_err(CommandError::database)?;
        if !matches!(mime.as_str(), "image/png" | "image/jpeg" | "image/webp") {
            return Err(CommandError::new(
                "invalid_occlusion_image",
                "Image occlusion requires a PNG, JPEG or WebP image.",
            ));
        }
    }
    if !input.allow_duplicate
        && has_duplicate(
            &transaction,
            &input.deck_id,
            input.note_id.as_deref(),
            &duplicate_hash,
            &identity,
        )?
    {
        return Err(CommandError::new(
            "duplicate_card",
            "An identical question already exists in this deck.",
        ));
    }

    let now = unix_time_millis()?;
    let note_id = match &input.note_id {
        Some(id) => id.clone(),
        None => generate_id(&transaction)?,
    };
    let note_revision = match input.revision {
        Some(revision) => revision.checked_add(1).ok_or_else(changed)?,
        None => 1,
    };
    if let Some(revision) = input.revision {
        let count = transaction
            .execute(
                "UPDATE notes SET deck_id = ?1, question_definition = ?2,
                     front_text = ?3, back_text = ?4, duplicate_hash = ?5,
                     updated_at = ?6, revision = ?7
                 WHERE id = ?8 AND revision = ?9 AND note_type = 'interactive'
                   AND deleted_at IS NULL",
                params![
                    input.deck_id,
                    definition_json,
                    text.prompt,
                    text.answer,
                    duplicate_hash,
                    now,
                    note_revision,
                    note_id,
                    revision
                ],
            )
            .map_err(CommandError::database)?;
        if count != 1 {
            return Err(changed());
        }
    } else {
        transaction
            .execute(
                "INSERT INTO notes (
                     id, deck_id, note_type, content_format, content_schema_version,
                     question_definition, front_text, back_text, duplicate_hash,
                     created_at, updated_at, revision
                 ) VALUES (?1, ?2, 'interactive', 'question-json', 1,
                     ?3, ?4, ?5, ?6, ?7, ?7, 1)",
                params![
                    note_id,
                    input.deck_id,
                    definition_json,
                    text.prompt,
                    text.answer,
                    duplicate_hash,
                    now
                ],
            )
            .map_err(CommandError::database)?;
    }
    let card_ids = sync_presentations(
        &transaction,
        &note_id,
        &input.deck_id,
        &question.presentation_keys(),
        now,
    )?;
    transaction
        .execute("DELETE FROM note_assets WHERE note_id = ?1", [&note_id])
        .map_err(CommandError::database)?;
    for asset_id in question.asset_ids() {
        transaction
            .execute(
                "INSERT INTO note_assets (note_id, asset_id, side) VALUES (?1, ?2, 'content')",
                params![note_id, asset_id],
            )
            .map_err(CommandError::database)?;
    }
    replace_note_tags(&transaction, &note_id, &tags, now)?;
    let finalization = finalize_assets_for_owner(
        &transaction,
        paths,
        question.asset_ids(),
        input.asset_draft_id.as_deref(),
        Some(&note_id),
    )?;
    commit_canonical_editor_draft(
        &transaction,
        input.editor_draft.as_ref(),
        EditorDraftKind::Card,
        Some(&note_id),
        input.revision,
        input.asset_draft_id.as_deref(),
        now,
    )?;
    let result = QuestionWriteReceipt {
        operation_id: input.operation_id,
        note_id,
        note_revision,
        card_ids,
        committed_at: now,
    };
    let card_ids_json =
        serde_json::to_string(&result.card_ids).map_err(CommandError::serialization)?;
    transaction
        .execute(
            "INSERT INTO question_note_writes (
                 operation_id, request_hash, note_id, note_revision, card_ids_json, committed_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                result.operation_id,
                request_hash,
                result.note_id,
                result.note_revision,
                card_ids_json,
                now
            ],
        )
        .map_err(CommandError::database)?;
    transaction.commit().map_err(CommandError::database)?;
    finalization.cleanup();
    Ok(result)
}

fn receipt(
    connection: &Connection,
    operation_id: &str,
    request_hash: &str,
) -> Result<Option<QuestionWriteReceipt>, CommandError> {
    let stored = connection
        .query_row(
            "SELECT request_hash, note_id, note_revision, card_ids_json, committed_at
             FROM question_note_writes WHERE operation_id = ?1",
            [operation_id],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, i64>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, i64>(4)?,
                ))
            },
        )
        .optional()
        .map_err(CommandError::database)?;
    let Some(stored) = stored else {
        return Ok(None);
    };
    if stored.0 != request_hash {
        return Err(operation_conflict());
    }
    Ok(Some(QuestionWriteReceipt {
        operation_id: operation_id.to_string(),
        note_id: stored.1,
        note_revision: stored.2,
        card_ids: serde_json::from_str(&stored.3).map_err(CommandError::serialization)?,
        committed_at: stored.4,
    }))
}

fn has_duplicate(
    connection: &Connection,
    deck_id: &str,
    exclude_note_id: Option<&str>,
    hash: &str,
    identity: &str,
) -> Result<bool, CommandError> {
    let mut statement = connection
        .prepare(
            "SELECT question_definition FROM notes
             WHERE deck_id = ?1 AND duplicate_hash = ?2 AND note_type = 'interactive'
               AND deleted_at IS NULL AND (?3 IS NULL OR id <> ?3)",
        )
        .map_err(CommandError::database)?;
    let mut rows = statement
        .query(params![deck_id, hash, exclude_note_id])
        .map_err(CommandError::database)?;
    while let Some(row) = rows.next().map_err(CommandError::database)? {
        let source: String = row.get(0).map_err(CommandError::database)?;
        if ValidatedQuestion::parse(&source)?.duplicate_identity()? == identity {
            return Ok(true);
        }
    }
    Ok(false)
}

fn sync_presentations(
    transaction: &Transaction<'_>,
    note_id: &str,
    deck_id: &str,
    keys: &[String],
    now: i64,
) -> Result<Vec<String>, CommandError> {
    let mut statement = transaction
        .prepare(
            "SELECT qp.question_key, c.id FROM card_question_presentations qp
             JOIN cards c ON c.id = qp.card_id WHERE qp.note_id = ?1",
        )
        .map_err(CommandError::database)?;
    let existing = statement
        .query_map([note_id], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })
        .map_err(CommandError::database)?
        .collect::<Result<BTreeMap<_, _>, _>>()
        .map_err(CommandError::database)?;
    drop(statement);
    let mut ordinal: i64 = transaction
        .query_row(
            "SELECT COALESCE(MAX(template_ordinal), -1) FROM cards WHERE note_id = ?1",
            [note_id],
            |row| row.get(0),
        )
        .map_err(CommandError::database)?;
    let mut cards = Vec::new();
    for key in keys {
        let card_id = if let Some(id) = existing.get(key) {
            transaction
                .execute(
                    "UPDATE cards SET deck_id = ?1, deleted_at = NULL, updated_at = ?2
                     WHERE id = ?3",
                    params![deck_id, now, id],
                )
                .map_err(CommandError::database)?;
            id.clone()
        } else {
            ordinal = ordinal.checked_add(1).ok_or_else(changed)?;
            let card_id = generate_id(transaction)?;
            transaction
                .execute(
                    "INSERT INTO cards (
                         id, note_id, deck_id, direction, template_ordinal, created_at, updated_at
                     ) VALUES (?1, ?2, ?3, 'forward', ?4, ?5, ?5)",
                    params![card_id, note_id, deck_id, ordinal, now],
                )
                .map_err(CommandError::database)?;
            transaction
                .execute(
                    "INSERT INTO card_question_presentations (card_id, note_id, question_key)
                     VALUES (?1, ?2, ?3)",
                    params![card_id, note_id, key],
                )
                .map_err(CommandError::database)?;
            insert_new_schedule(transaction, &card_id, now)?;
            card_id
        };
        cards.push(card_id);
    }
    for (key, card_id) in existing {
        if !keys.contains(&key) {
            transaction
                .execute(
                    "UPDATE cards SET deleted_at = COALESCE(deleted_at, ?1), updated_at = ?1
                     WHERE id = ?2",
                    params![now, card_id],
                )
                .map_err(CommandError::database)?;
        }
    }
    Ok(cards)
}
