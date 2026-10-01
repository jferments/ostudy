//! Atomic whole-deck movement with durable replay.
//!
//! Merge keeps note/card identities, schedules, review evidence, assets, tags,
//! source locators and generated-question evidence intact. The empty source deck
//! is archived rather than deleted so historical deck identity remains available.

use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tauri::State;

use crate::{
    database::Database,
    error::CommandError,
    util::{unix_time_millis, validate_entity_id},
};

const RECEIPT_KIND: &str = "deck_merge_v1";
const RECEIPT_MAX_BYTES: usize = 32 * 1024;

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MergeDecksInput {
    pub operation_id: String,
    pub source_deck_id: String,
    pub destination_deck_id: String,
    pub source_revision: i64,
    pub destination_revision: i64,
    pub expected_source_note_count: i64,
    pub expected_source_card_count: i64,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MergeDecksResultDto {
    pub operation_id: String,
    pub source_deck_id: String,
    pub destination_deck_id: String,
    pub moved_note_count: i64,
    pub moved_card_count: i64,
    pub retargeted_document_count: i64,
    pub retargeted_study_target_count: i64,
    pub source_revision: i64,
    pub destination_revision: i64,
    pub source_archived_at: i64,
    pub merged_at: i64,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct MergeReceipt {
    kind: String,
    request: MergeDecksInput,
    result: MergeDecksResultDto,
}

#[tauri::command]
pub fn merge_decks(
    database: State<'_, Database>,
    input: MergeDecksInput,
) -> Result<MergeDecksResultDto, CommandError> {
    database.with_connection(|connection| merge_in_connection(connection, input))
}

pub(crate) fn merge_in_connection(
    connection: &mut Connection,
    input: MergeDecksInput,
) -> Result<MergeDecksResultDto, CommandError> {
    validate_request(&input)?;
    let fingerprint = request_fingerprint(&input)?;
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(CommandError::database)?;

    if let Some(result) = replay_receipt(&transaction, &input, &fingerprint)? {
        return Ok(result);
    }

    check_active_deck(&transaction, &input.source_deck_id, input.source_revision)?;
    check_active_deck(
        &transaction,
        &input.destination_deck_id,
        input.destination_revision,
    )?;
    let now = unix_time_millis()?;
    ensure_no_conflicting_work(&transaction, &input, now)?;

    transaction
        .execute_batch(
            "DROP TABLE IF EXISTS temp.ostudy_deck_merge_notes;
             CREATE TEMP TABLE ostudy_deck_merge_notes(
                 id TEXT PRIMARY KEY
             ) WITHOUT ROWID;",
        )
        .map_err(CommandError::database)?;
    transaction
        .execute(
            "INSERT INTO temp.ostudy_deck_merge_notes(id)
             SELECT id FROM notes
             WHERE deck_id=?1 AND deleted_at IS NULL",
            [&input.source_deck_id],
        )
        .map_err(CommandError::database)?;

    let moved_note_count: i64 = transaction
        .query_row(
            "SELECT COUNT(*) FROM temp.ostudy_deck_merge_notes",
            [],
            |row| row.get(0),
        )
        .map_err(CommandError::database)?;
    let moved_card_count: i64 = transaction
        .query_row(
            "SELECT COUNT(*) FROM cards
             WHERE note_id IN (SELECT id FROM temp.ostudy_deck_merge_notes)
               AND deleted_at IS NULL",
            [],
            |row| row.get(0),
        )
        .map_err(CommandError::database)?;
    if moved_note_count != input.expected_source_note_count
        || moved_card_count != input.expected_source_card_count
    {
        return Err(CommandError::new(
            "deck_merge_source_changed",
            "The source deck changed after Merge was opened. Refresh the deck library and review the counts before trying again.",
        ));
    }
    if moved_note_count == 0 {
        return Err(CommandError::new(
            "deck_merge_source_empty",
            "The source deck has no active flashcards to merge.",
        ));
    }

    let inconsistent_cards: i64 = transaction
        .query_row(
            "SELECT COUNT(*) FROM cards c
             WHERE c.note_id IN (SELECT id FROM temp.ostudy_deck_merge_notes)
               AND c.deleted_at IS NULL AND c.deck_id<>?1",
            [&input.source_deck_id],
            |row| row.get(0),
        )
        .map_err(CommandError::database)?;
    if inconsistent_cards != 0 {
        return Err(CommandError::new(
            "deck_merge_invalid_source",
            "The source deck has inconsistent card ownership. No material was moved.",
        ));
    }

    transaction
        .execute(
            "INSERT OR IGNORE INTO deck_source_citations(deck_id,citation_id,created_at)
             SELECT ?2,citation_id,?3
             FROM deck_source_citations WHERE deck_id=?1
             UNION
             SELECT ?2,citation.citation_id,?3
             FROM note_source_citations citation
             WHERE citation.note_id IN (SELECT id FROM temp.ostudy_deck_merge_notes)",
            params![input.source_deck_id, input.destination_deck_id, now],
        )
        .map_err(CommandError::database)?;

    let changed_cards = transaction
        .execute(
            "UPDATE cards
             SET deck_id=?1,updated_at=MAX(updated_at,?2)
             WHERE note_id IN (SELECT id FROM temp.ostudy_deck_merge_notes)
               AND deleted_at IS NULL AND deck_id=?3",
            params![input.destination_deck_id, now, input.source_deck_id],
        )
        .map_err(CommandError::database)?;
    if i64::try_from(changed_cards).ok() != Some(moved_card_count) {
        return Err(deck_changed());
    }

    transaction
        .execute(
            "DELETE FROM card_search_fts
             WHERE note_id IN (SELECT id FROM temp.ostudy_deck_merge_notes)",
            [],
        )
        .map_err(CommandError::database)?;

    let changed_notes = transaction
        .execute(
            "UPDATE notes
             SET deck_id=?1,updated_at=MAX(updated_at,?2)
             WHERE id IN (SELECT id FROM temp.ostudy_deck_merge_notes)
               AND deleted_at IS NULL AND deck_id=?3",
            params![input.destination_deck_id, now, input.source_deck_id],
        )
        .map_err(CommandError::database)?;
    if i64::try_from(changed_notes).ok() != Some(moved_note_count) {
        return Err(deck_changed());
    }

    transaction
        .execute(
            "INSERT OR REPLACE INTO card_search_fts(
                 rowid,note_id,deck_id,question,answer,tags
             )
             SELECT n.rowid,n.id,n.deck_id,n.front_text,n.back_text,
                    COALESCE((
                        SELECT group_concat(t.name,' ')
                        FROM note_tags nt JOIN tags t ON t.id=nt.tag_id
                        WHERE nt.note_id=n.id
                    ),'')
             FROM notes n
             WHERE n.id IN (SELECT id FROM temp.ostudy_deck_merge_notes)
               AND n.deleted_at IS NULL",
            [],
        )
        .map_err(CommandError::database)?;

    let retargeted_documents = transaction
        .execute(
            "UPDATE documents
             SET generated_deck_id=?1,updated_at=MAX(updated_at,?2),revision=revision+1
             WHERE generated_deck_id=?3 AND lifecycle_state<>'processing'",
            params![input.destination_deck_id, now, input.source_deck_id],
        )
        .map_err(CommandError::database)?;
    let retargeted_targets = transaction
        .execute(
            "UPDATE study_targets
             SET deck_id=?1,updated_at=MAX(updated_at,?2),revision=revision+1
             WHERE deck_id=?3",
            params![input.destination_deck_id, now, input.source_deck_id],
        )
        .map_err(CommandError::database)?;

    let changed_source = transaction
        .execute(
            "UPDATE decks
             SET archived_at=?1,updated_at=MAX(updated_at,?1),revision=revision+1
             WHERE id=?2 AND revision=?3
               AND deleted_at IS NULL AND archived_at IS NULL",
            params![now, input.source_deck_id, input.source_revision],
        )
        .map_err(CommandError::database)?;
    let changed_destination = transaction
        .execute(
            "UPDATE decks
             SET updated_at=MAX(updated_at,?1),revision=revision+1
             WHERE id=?2 AND revision=?3
               AND deleted_at IS NULL AND archived_at IS NULL",
            params![now, input.destination_deck_id, input.destination_revision],
        )
        .map_err(CommandError::database)?;
    if changed_source != 1 || changed_destination != 1 {
        return Err(deck_changed());
    }

    transaction
        .execute_batch("DROP TABLE temp.ostudy_deck_merge_notes;")
        .map_err(CommandError::database)?;

    let result = MergeDecksResultDto {
        operation_id: input.operation_id.clone(),
        source_deck_id: input.source_deck_id.clone(),
        destination_deck_id: input.destination_deck_id.clone(),
        moved_note_count,
        moved_card_count,
        retargeted_document_count: i64::try_from(retargeted_documents).unwrap_or(i64::MAX),
        retargeted_study_target_count: i64::try_from(retargeted_targets).unwrap_or(i64::MAX),
        source_revision: input.source_revision + 1,
        destination_revision: input.destination_revision + 1,
        source_archived_at: now,
        merged_at: now,
    };
    store_receipt(&transaction, &input, &fingerprint, &result, now)?;
    transaction.commit().map_err(CommandError::database)?;
    Ok(result)
}

fn validate_request(input: &MergeDecksInput) -> Result<(), CommandError> {
    for (value, label) in [
        (&input.operation_id, "deck merge operation"),
        (&input.source_deck_id, "source deck"),
        (&input.destination_deck_id, "destination deck"),
    ] {
        validate_entity_id(value, label)?;
    }
    if input.source_deck_id == input.destination_deck_id {
        return Err(CommandError::new(
            "same_destination_deck",
            "Choose a different destination deck.",
        ));
    }
    if !(1..i64::MAX).contains(&input.source_revision)
        || !(1..i64::MAX).contains(&input.destination_revision)
        || input.expected_source_note_count <= 0
        || input.expected_source_card_count < 0
    {
        return Err(CommandError::new(
            "invalid_deck_merge",
            "Refresh the deck library and choose a nonempty active source deck.",
        ));
    }
    Ok(())
}

fn request_fingerprint(input: &MergeDecksInput) -> Result<String, CommandError> {
    let bytes = serde_json::to_vec(&(RECEIPT_KIND, input)).map_err(CommandError::serialization)?;
    Ok(format!("{:x}", Sha256::digest(bytes)))
}

fn replay_receipt(
    connection: &Connection,
    input: &MergeDecksInput,
    fingerprint: &str,
) -> Result<Option<MergeDecksResultDto>, CommandError> {
    let stored: Option<(String, String, i64)> = connection
        .query_row(
            "SELECT request_sha256,result_json,created_at
             FROM deck_merge_operations WHERE operation_id=?1",
            [&input.operation_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .optional()
        .map_err(CommandError::database)?;
    let Some((stored_fingerprint, encoded, created_at)) = stored else {
        return Ok(None);
    };
    if stored_fingerprint != fingerprint {
        return Err(CommandError::new(
            "deck_merge_operation_conflict",
            "This merge identity was already used for different decks or counts. Refresh before starting another merge.",
        ));
    }
    let receipt = decode_receipt(
        &input.operation_id,
        &stored_fingerprint,
        &encoded,
        created_at,
    )?;
    if receipt.request != *input {
        return Err(invalid_receipt());
    }
    Ok(Some(receipt.result))
}

fn store_receipt(
    connection: &Connection,
    input: &MergeDecksInput,
    fingerprint: &str,
    result: &MergeDecksResultDto,
    now: i64,
) -> Result<(), CommandError> {
    let receipt = MergeReceipt {
        kind: RECEIPT_KIND.to_string(),
        request: input.clone(),
        result: result.clone(),
    };
    let encoded = serde_json::to_string(&receipt).map_err(CommandError::serialization)?;
    decode_receipt(&input.operation_id, fingerprint, &encoded, now)?;
    connection
        .execute(
            "INSERT INTO deck_merge_operations(
                 operation_id,request_sha256,result_json,created_at
             ) VALUES(?1,?2,?3,?4)",
            params![input.operation_id, fingerprint, encoded, now],
        )
        .map_err(CommandError::database)?;
    Ok(())
}

fn decode_receipt(
    operation_id: &str,
    fingerprint: &str,
    encoded: &str,
    created_at: i64,
) -> Result<MergeReceipt, CommandError> {
    if encoded.len() > RECEIPT_MAX_BYTES {
        return Err(invalid_receipt());
    }
    let receipt: MergeReceipt = serde_json::from_str(encoded).map_err(|_| invalid_receipt())?;
    if receipt.kind != RECEIPT_KIND
        || receipt.request.operation_id != operation_id
        || receipt.result.operation_id != operation_id
        || request_fingerprint(&receipt.request)? != fingerprint
        || receipt.result.source_deck_id != receipt.request.source_deck_id
        || receipt.result.destination_deck_id != receipt.request.destination_deck_id
        || receipt.result.moved_note_count != receipt.request.expected_source_note_count
        || receipt.result.moved_card_count != receipt.request.expected_source_card_count
        || receipt.result.source_revision != receipt.request.source_revision + 1
        || receipt.result.destination_revision != receipt.request.destination_revision + 1
        || receipt.result.source_archived_at != created_at
        || receipt.result.merged_at != created_at
        || receipt.result.retargeted_document_count < 0
        || receipt.result.retargeted_study_target_count < 0
        || created_at < 0
    {
        return Err(invalid_receipt());
    }
    Ok(receipt)
}

pub(crate) fn validate_stored_receipts(connection: &Connection) -> Result<(), String> {
    let mut statement = connection
        .prepare(
            "SELECT operation_id,request_sha256,result_json,created_at
             FROM deck_merge_operations",
        )
        .map_err(|error| error.to_string())?;
    let mut rows = statement.query([]).map_err(|error| error.to_string())?;
    while let Some(row) = rows.next().map_err(|error| error.to_string())? {
        let operation_id: String = row.get(0).map_err(|error| error.to_string())?;
        let fingerprint: String = row.get(1).map_err(|error| error.to_string())?;
        let encoded: String = row.get(2).map_err(|error| error.to_string())?;
        let created_at: i64 = row.get(3).map_err(|error| error.to_string())?;
        validate_entity_id(&operation_id, "deck merge receipt").map_err(|error| error.message)?;
        if fingerprint.len() != 64
            || !fingerprint.bytes().all(|value| value.is_ascii_hexdigit())
        {
            return Err("A deck-merge receipt fingerprint is invalid.".to_string());
        }
        decode_receipt(&operation_id, &fingerprint, &encoded, created_at)
            .map_err(|error| error.message)?;
    }
    Ok(())
}

fn check_active_deck(
    connection: &Connection,
    deck_id: &str,
    expected_revision: i64,
) -> Result<(), CommandError> {
    let current: Option<(i64, Option<i64>, Option<i64>)> = connection
        .query_row(
            "SELECT revision,archived_at,deleted_at FROM decks WHERE id=?1",
            [deck_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .optional()
        .map_err(CommandError::database)?;
    match current {
        Some((revision, None, None)) if revision == expected_revision => Ok(()),
        _ => Err(deck_changed()),
    }
}

fn ensure_no_conflicting_work(
    connection: &Connection,
    input: &MergeDecksInput,
    now: i64,
) -> Result<(), CommandError> {
    let active_generation: bool = connection
        .query_row(
            "SELECT
                 EXISTS(
                     SELECT 1 FROM documents
                     WHERE generated_deck_id IN (?1,?2)
                       AND lifecycle_state='processing'
                 )
                 OR EXISTS(
                     SELECT 1 FROM study_target_batches batch
                     JOIN study_targets target ON target.id=batch.target_id
                     WHERE target.deck_id IN (?1,?2)
                       AND batch.state IN ('queued','preparing','composing','committing')
                 )",
            params![input.source_deck_id, input.destination_deck_id],
            |row| row.get(0),
        )
        .map_err(CommandError::database)?;
    if active_generation {
        return Err(CommandError::new(
            "deck_merge_generation_active",
            "Wait for study-material generation on these decks to finish before merging them.",
        ));
    }

    let pending_deletion: bool = connection
        .query_row(
            "SELECT EXISTS(
                 SELECT 1 FROM deletion_operations operation
                 WHERE operation.restored_at IS NULL
                   AND operation.finalized_at IS NULL
                   AND operation.expires_at>=?3
                   AND (
                       operation.deck_id IN (?1,?2)
                       OR EXISTS(
                           SELECT 1 FROM json_each(operation.note_ids_json) item
                           JOIN notes note ON note.id=CAST(item.value AS TEXT)
                           WHERE note.deck_id IN (?1,?2)
                       )
                   )
             )",
            params![input.source_deck_id, input.destination_deck_id, now],
            |row| row.get(0),
        )
        .map_err(CommandError::database)?;
    if pending_deletion {
        return Err(CommandError::new(
            "deck_merge_deletion_pending",
            "Wait for the current Undo period on these decks to finish, or undo the deletion, before merging.",
        ));
    }
    Ok(())
}

fn deck_changed() -> CommandError {
    CommandError::new(
        "deck_merge_changed",
        "A deck changed, was archived, or was deleted. Refresh both decks before merging.",
    )
}

fn invalid_receipt() -> CommandError {
    CommandError::new(
        "deck_merge_receipt_invalid",
        "The saved merge result is invalid. Preserve the library and inspect diagnostics.",
    )
}

#[cfg(test)]
#[path = "merge_tests.rs"]
mod tests;
