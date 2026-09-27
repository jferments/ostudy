//! One transaction owns destination creation, copied content, fresh schedules and
//! the immutable replay receipt. React never compensates a committed mutation.

use std::collections::BTreeSet;

use rusqlite::{params, Connection, OptionalExtension, Transaction, TransactionBehavior};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tauri::State;

use crate::{
    assets::ensure_assets_exist,
    cards::{questions::ValidatedQuestion, validate_document},
    database::Database,
    decks::{
        create_deck_in_transaction, get_deck_from_connection, validate_description, validate_name,
        CreateDeckInput, DeckSummaryDto,
    },
    error::CommandError,
    scheduler::insert_new_schedule,
    util::{generate_id, unix_time_millis, validate_entity_id},
};

use super::{validate_note_ids_for_action, MAX_TRANSFER_NOTES};

const RECEIPT_MAX_BYTES: usize = 32 * 1024;

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SaveSelectionAsDeckInput {
    pub operation_id: String,
    pub name: String,
    pub description: Option<String>,
    pub note_ids: Vec<String>,
}

/// A committed result, not a live projection. Replay survives later edits/deletion.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SaveSelectionAsDeckResultDto {
    pub operation_id: String,
    pub deck: DeckSummaryDto,
    pub affected_count: i64,
    pub skipped_duplicates: i64,
}

struct Source {
    id: String,
    kind: String,
    format: String,
    version: i64,
    front: Option<String>,
    back: Option<String>,
    definition: Option<String>,
    reverse: bool,
    cloze_source: String,
    cloze_mode: String,
    cards: Vec<(String, i64, Option<String>)>,
}

#[tauri::command]
pub fn save_selection_as_deck(
    database: State<'_, Database>,
    input: SaveSelectionAsDeckInput,
) -> Result<SaveSelectionAsDeckResultDto, CommandError> {
    database.with_connection(|connection| save_selection_as_deck_in_connection(connection, input))
}

pub(crate) fn save_selection_as_deck_in_connection(
    connection: &mut Connection,
    input: SaveSelectionAsDeckInput,
) -> Result<SaveSelectionAsDeckResultDto, CommandError> {
    validate_entity_id(&input.operation_id, "save-selection operation")?;
    if input.note_ids.len() > MAX_TRANSFER_NOTES {
        return Err(CommandError::new(
            "too_many_cards_selected",
            "Narrow the selection before saving it as a deck.",
        ));
    }
    let note_ids = validate_note_ids_for_action(input.note_ids, MAX_TRANSFER_NOTES, "Save")?;
    let name = validate_name(input.name)?;
    let description = validate_description(input.description.unwrap_or_default())?;
    let request = serde_json::to_vec(&("selection-deck-v1", &name, &description, &note_ids))
        .map_err(CommandError::serialization)?;
    let fingerprint = format!("{:x}", Sha256::digest(request));
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(CommandError::database)?;

    // Replay precedes current-source checks: edits/deletion do not resurrect data.
    let stored: Option<(String, String)> = transaction
        .query_row(
            "SELECT request_sha256,result_json FROM selection_deck_operations
             WHERE operation_id=?1",
            [input.operation_id.as_str()],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()
        .map_err(CommandError::database)?;
    if let Some((stored_hash, encoded)) = stored {
        if stored_hash != fingerprint {
            return Err(CommandError::new(
                "selection_deck_operation_conflict",
                "This save operation identity was already used for a different selection or deck.",
            ));
        }
        let result = decode_receipt(&input.operation_id, &encoded)?;
        if result.affected_count + result.skipped_duplicates != note_ids.len() as i64 {
            return Err(invalid_receipt());
        }
        return Ok(result);
    }

    let now = unix_time_millis()?;
    let created = create_deck_in_transaction(
        &transaction,
        CreateDeckInput {
            name,
            description: Some(description),
        },
        now,
    )?;
    let mut identities = BTreeSet::new();
    let mut affected_count = 0_i64;
    let mut skipped_duplicates = 0_i64;
    for note_id in note_ids {
        let source = load_source(&transaction, &note_id)?;
        let identity = source_identity(&transaction, &source)?;
        if !identities.insert(identity) {
            skipped_duplicates += 1;
            continue;
        }
        copy_source(&transaction, &source, &created.id, now)?;
        affected_count += 1;
    }
    if affected_count == 0 {
        return Err(CommandError::new(
            "selection_deck_empty",
            "No selected cards could be copied into a new deck.",
        ));
    }
    let result = SaveSelectionAsDeckResultDto {
        operation_id: input.operation_id,
        deck: get_deck_from_connection(&transaction, &created.id)?,
        affected_count,
        skipped_duplicates,
    };
    let encoded = serde_json::to_string(&result).map_err(CommandError::serialization)?;
    decode_receipt(&result.operation_id, &encoded)?;
    transaction
        .execute(
            "INSERT INTO selection_deck_operations
             (operation_id,request_sha256,result_json,created_at) VALUES(?1,?2,?3,?4)",
            params![result.operation_id, fingerprint, encoded, now],
        )
        .map_err(CommandError::database)?;
    transaction.commit().map_err(CommandError::database)?;
    Ok(result)
}

fn load_source(connection: &Connection, id: &str) -> Result<Source, CommandError> {
    let mut source = connection
        .query_row(
            "SELECT n.id,n.note_type,n.content_format,n.content_schema_version,
                    n.front_doc,n.back_doc,n.question_definition,n.reverse_enabled,
                    n.cloze_source,n.cloze_mode
             FROM notes n JOIN decks d ON d.id=n.deck_id
             WHERE n.id=?1 AND n.deleted_at IS NULL
               AND d.deleted_at IS NULL AND d.archived_at IS NULL",
            [id],
            |row| {
                Ok(Source {
                    id: row.get(0)?,
                    kind: row.get(1)?,
                    format: row.get(2)?,
                    version: row.get(3)?,
                    front: row.get(4)?,
                    back: row.get(5)?,
                    definition: row.get(6)?,
                    reverse: row.get(7)?,
                    cloze_source: row.get(8)?,
                    cloze_mode: row.get(9)?,
                    cards: Vec::new(),
                })
            },
        )
        .optional()
        .map_err(CommandError::database)?
        .ok_or_else(|| {
            CommandError::new(
                "card_not_found",
                "A selected note is no longer in an active deck.",
            )
        })?;
    let mut statement = connection
        .prepare(
            "SELECT c.direction,c.template_ordinal,qp.question_key
             FROM cards c LEFT JOIN card_question_presentations qp
               ON qp.card_id=c.id AND qp.note_id=c.note_id
             WHERE c.note_id=?1 AND c.deleted_at IS NULL
             ORDER BY c.direction,c.template_ordinal,c.id",
        )
        .map_err(CommandError::database)?;
    source.cards = statement
        .query_map([id], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))
        .map_err(CommandError::database)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(CommandError::database)?;
    if source.cards.is_empty() {
        return Err(CommandError::new(
            "card_not_found",
            "A selected note has no active study presentation.",
        ));
    }
    Ok(source)
}

fn source_identity(connection: &Connection, source: &Source) -> Result<String, CommandError> {
    let assets = {
        let mut statement = connection
            .prepare("SELECT asset_id FROM note_assets WHERE note_id=?1 ORDER BY asset_id")
            .map_err(CommandError::database)?;
        let rows = statement
            .query_map([source.id.as_str()], |row| row.get::<_, String>(0))
            .map_err(CommandError::database)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(CommandError::database)?;
        rows
    };
    ensure_assets_exist(connection, &assets)?;
    let content = match (source.kind.as_str(), source.format.as_str(), source.version) {
        ("basic" | "cloze", "tiptap-json", 1) if source.definition.is_none() => {
            let front: serde_json::Value =
                serde_json::from_str(source.front.as_deref().ok_or_else(invalid_source)?)
                    .map_err(CommandError::serialization)?;
            let back: serde_json::Value =
                serde_json::from_str(source.back.as_deref().ok_or_else(invalid_source)?)
                    .map_err(CommandError::serialization)?;
            validate_document(&front)?;
            validate_document(&back)?;
            if source.cards.iter().any(|(_, _, key)| key.is_some()) {
                return Err(invalid_source());
            }
            // Conservative exact identity preserves formatting, media, reverse
            // semantics and cloze behavior; equal display text is not enough.
            serde_json::to_string(&(
                &front,
                &back,
                source.reverse,
                &source.cloze_source,
                &source.cloze_mode,
                &source.cards,
            ))
            .map_err(CommandError::serialization)?
        }
        ("interactive", "question-json", 1) if source.front.is_none() && source.back.is_none() => {
            let question =
                ValidatedQuestion::parse(source.definition.as_deref().ok_or_else(invalid_source)?)?;
            let expected = question
                .presentation_keys()
                .into_iter()
                .collect::<BTreeSet<_>>();
            let actual = source
                .cards
                .iter()
                .filter_map(|(_, _, key)| key.clone())
                .collect::<BTreeSet<_>>();
            if expected != actual || actual.len() != source.cards.len() {
                return Err(invalid_source());
            }
            if question.asset_ids().iter().any(|id| !assets.contains(id)) {
                return Err(invalid_source());
            }
            question.duplicate_identity()?
        }
        _ => return Err(invalid_source()),
    };
    Ok(format!("{}|{content}", source.kind))
}

fn copy_source(
    transaction: &Transaction<'_>,
    source: &Source,
    deck_id: &str,
    now: i64,
) -> Result<(), CommandError> {
    let note_id = generate_id(transaction)?;
    transaction
        .execute(
            "INSERT INTO notes(
                 id,deck_id,note_type,content_format,content_schema_version,
                 front_doc,back_doc,front_text,back_text,duplicate_hash,
                 reverse_enabled,cloze_source,cloze_mode,question_definition,
                 created_at,updated_at,revision
             ) SELECT ?1,?2,note_type,content_format,content_schema_version,
                 front_doc,back_doc,front_text,back_text,duplicate_hash,
                 reverse_enabled,cloze_source,cloze_mode,question_definition,?3,?3,1
               FROM notes WHERE id=?4",
            params![note_id, deck_id, now, source.id],
        )
        .map_err(CommandError::database)?;
    for (direction, ordinal, key) in &source.cards {
        let card_id = generate_id(transaction)?;
        transaction
            .execute(
                "INSERT INTO cards(id,note_id,deck_id,direction,template_ordinal,created_at,updated_at)
                 VALUES(?1,?2,?3,?4,?5,?6,?6)",
                params![card_id, note_id, deck_id, direction, ordinal, now],
            )
            .map_err(CommandError::database)?;
        if let Some(key) = key {
            transaction
                .execute(
                    "INSERT INTO card_question_presentations(card_id,note_id,question_key)
                     VALUES(?1,?2,?3)",
                    params![card_id, note_id, key],
                )
                .map_err(CommandError::database)?;
        }
        insert_new_schedule(transaction, &card_id, now)?;
    }
    transaction
        .execute(
            "INSERT INTO note_assets(note_id,asset_id,side)
             SELECT ?1,asset_id,side FROM note_assets WHERE note_id=?2",
            params![note_id, source.id],
        )
        .map_err(CommandError::database)?;
    transaction
        .execute(
            "INSERT INTO note_tags(note_id,tag_id)
             SELECT ?1,tag_id FROM note_tags WHERE note_id=?2",
            params![note_id, source.id],
        )
        .map_err(CommandError::database)?;
    transaction
        .execute(
            "INSERT INTO note_source_citations
             (note_id,citation_id,source_locator_json,created_at,updated_at)
             SELECT ?1,citation_id,source_locator_json,?3,?3
             FROM note_source_citations WHERE note_id=?2",
            params![note_id, source.id, now],
        )
        .map_err(CommandError::database)?;
    transaction
        .execute(
            "INSERT OR IGNORE INTO deck_source_citations(deck_id,citation_id,created_at)
             SELECT ?1,citation_id,?3 FROM note_source_citations WHERE note_id=?2",
            params![deck_id, note_id, now],
        )
        .map_err(CommandError::database)?;
    // Copy matching current provenance only; never relabel evidence for an edit.
    transaction
        .execute(
            "INSERT INTO question_generation_evidence
             (note_id,note_revision,document_id,question_hash,opportunity_id,policy_hash,
              record_json,created_at)
             SELECT ?1,1,e.document_id,e.question_hash,e.opportunity_id,e.policy_hash,
                    e.record_json,?3 FROM question_generation_evidence e
             JOIN notes n ON n.id=e.note_id
             WHERE e.note_id=?2 AND e.note_revision=n.revision",
            params![note_id, source.id, now],
        )
        .map_err(CommandError::database)?;
    Ok(())
}

fn decode_receipt(
    operation: &str,
    encoded: &str,
) -> Result<SaveSelectionAsDeckResultDto, CommandError> {
    if encoded.len() > RECEIPT_MAX_BYTES {
        return Err(invalid_receipt());
    }
    let result: SaveSelectionAsDeckResultDto =
        serde_json::from_str(encoded).map_err(|_| invalid_receipt())?;
    validate_entity_id(&result.deck.id, "saved deck").map_err(|_| invalid_receipt())?;
    if result.operation_id != operation
        || result.affected_count <= 0
        || result.affected_count > MAX_TRANSFER_NOTES as i64
        || result.skipped_duplicates < 0
        || result.skipped_duplicates > MAX_TRANSFER_NOTES as i64 - result.affected_count
        || result.deck.source_note_count != result.affected_count
        || result.deck.generated_card_count < result.affected_count
        || result.deck.studyable_due_count != result.deck.generated_card_count
        || result.deck.archived_at.is_some()
        || result.deck.revision != 1
        || result.deck.created_at < 0
        || result.deck.updated_at != result.deck.created_at
        || validate_name(result.deck.name.clone()).as_ref().ok() != Some(&result.deck.name)
        || validate_description(result.deck.description.clone())
            .as_ref()
            .ok()
            != Some(&result.deck.description)
    {
        return Err(invalid_receipt());
    }
    Ok(result)
}

/// Read-only admission of operation history retained in backups and on reopen.
pub(crate) fn validate_stored_receipts(connection: &Connection) -> Result<(), String> {
    let mut statement = connection
        .prepare("SELECT operation_id,request_sha256,result_json,created_at FROM selection_deck_operations")
        .map_err(|error| error.to_string())?;
    let mut rows = statement.query([]).map_err(|error| error.to_string())?;
    while let Some(row) = rows.next().map_err(|error| error.to_string())? {
        let operation: String = row.get(0).map_err(|error| error.to_string())?;
        let fingerprint: String = row.get(1).map_err(|error| error.to_string())?;
        let encoded: String = row.get(2).map_err(|error| error.to_string())?;
        let created_at: i64 = row.get(3).map_err(|error| error.to_string())?;
        validate_entity_id(&operation, "selection receipt").map_err(|error| error.message)?;
        if created_at < 0
            || fingerprint.len() != 64
            || !fingerprint.bytes().all(|value| value.is_ascii_hexdigit())
        {
            return Err("A selection-deck operation receipt is invalid.".to_string());
        }
        let result = decode_receipt(&operation, &encoded).map_err(|error| error.message)?;
        if result.deck.created_at != created_at {
            return Err("A selection-deck receipt timestamp is inconsistent.".to_string());
        }
    }
    Ok(())
}

fn invalid_source() -> CommandError {
    CommandError::new(
        "unsupported_card_type",
        "A selected note has invalid or unsupported content, assets, or study presentations.",
    )
}

fn invalid_receipt() -> CommandError {
    CommandError::new(
        "selection_deck_receipt_invalid",
        "The saved deck operation result is invalid. Preserve the library and inspect its diagnostics.",
    )
}

#[cfg(test)]
#[path = "selection_deck_tests.rs"]
mod tests;
