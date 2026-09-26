//! Atomic save-selection-as-deck mutation.
//!
//! One native transaction owns destination-deck creation, selected-note copying,
//! fresh schedule creation and the replay receipt. A lost response can therefore
//! replay the committed result without creating another deck, while a failed
//! copy leaves no partial destination for React to compensate.

use std::collections::BTreeSet;

use rusqlite::{params, Connection, OptionalExtension, Transaction, TransactionBehavior};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tauri::State;

use crate::{
    cards::questions::ValidatedQuestion,
    database::Database,
    decks::{
        create_deck_in_transaction, get_deck_from_connection, validate_description, validate_name,
        CreateDeckInput, DeckSummaryDto,
    },
    error::CommandError,
    scheduler::insert_new_schedule,
    util::{generate_id, unix_time_millis, validate_entity_id},
};

use super::{
    card_types_and_templates::duplicate_identity, duplicate_checkable, duplicate_hash,
    normalized_pair, transfer::copy_generated_note_to_deck, validate_note_ids_for_action,
    SourceNote, MAX_TRANSFER_NOTES,
};

const RECEIPT_MAX_BYTES: usize = 8 * 1024;

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SaveSelectionAsDeckInput {
    pub operation_id: String,
    pub name: String,
    pub description: Option<String>,
    pub note_ids: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveSelectionAsDeckResultDto {
    pub operation_id: String,
    pub deck: DeckSummaryDto,
    pub affected_count: i64,
    pub skipped_duplicates: i64,
}

#[derive(Clone)]
struct StoredSelectionNote {
    id: String,
    deck_id: String,
    note_type: String,
    content_format: String,
    content_schema_version: i64,
    front_doc: String,
    back_doc: String,
    front_text: String,
    back_text: String,
    duplicate_hash: String,
    question_definition: Option<String>,
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
    let note_ids =
        validate_note_ids_for_action(input.note_ids, MAX_TRANSFER_NOTES, "Save")?;
    let name = validate_name(input.name)?;
    let description = validate_description(input.description.unwrap_or_default())?;
    let request_sha256 = request_fingerprint(&name, &description, &note_ids)?;

    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(CommandError::database)?;

    if let Some((stored_hash, result_json)) = transaction
        .query_row(
            "SELECT request_sha256, result_json
             FROM selection_deck_operations
             WHERE operation_id = ?1",
            [input.operation_id.as_str()],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
        )
        .optional()
        .map_err(CommandError::database)?
    {
        if stored_hash != request_sha256 {
            return Err(operation_conflict());
        }
        let result: SaveSelectionAsDeckResultDto =
            serde_json::from_str(&result_json).map_err(|_| {
                CommandError::new(
                    "selection_deck_receipt_invalid",
                    "The saved result for this deck operation is invalid. Restart ostudy before retrying.",
                )
            })?;
        transaction.commit().map_err(CommandError::database)?;
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

    let mut affected_count = 0_i64;
    let mut skipped_duplicates = 0_i64;
    for note_id in &note_ids {
        let source = load_selection_note(&transaction, note_id)?;
        if is_duplicate_in_deck(&transaction, &created.id, &source)? {
            skipped_duplicates += 1;
            continue;
        }
        copy_note(&transaction, &source, &created.id, now)?;
        affected_count += 1;
    }

    if affected_count == 0 {
        return Err(CommandError::new(
            "selection_deck_empty",
            "No selected cards could be copied into a new deck.",
        ));
    }

    let deck = get_deck_from_connection(&transaction, &created.id)?;
    let result = SaveSelectionAsDeckResultDto {
        operation_id: input.operation_id.clone(),
        deck,
        affected_count,
        skipped_duplicates,
    };
    let result_json = serde_json::to_string(&result).map_err(CommandError::serialization)?;
    if result_json.len() > RECEIPT_MAX_BYTES {
        return Err(CommandError::new(
            "selection_deck_receipt_too_large",
            "The committed deck result exceeds the supported receipt size.",
        ));
    }
    transaction
        .execute(
            "INSERT INTO selection_deck_operations(
                 operation_id, request_sha256, result_json, created_at
             ) VALUES (?1, ?2, ?3, ?4)",
            params![input.operation_id, request_sha256, result_json, now],
        )
        .map_err(CommandError::database)?;
    transaction.commit().map_err(CommandError::database)?;
    Ok(result)
}

fn request_fingerprint(
    name: &str,
    description: &str,
    note_ids: &[String],
) -> Result<String, CommandError> {
    let encoded = serde_json::to_vec(&("selection-deck-v1", name, description, note_ids))
        .map_err(CommandError::serialization)?;
    Ok(format!("{:x}", Sha256::digest(encoded)))
}

fn load_selection_note(
    connection: &Connection,
    note_id: &str,
) -> Result<StoredSelectionNote, CommandError> {
    connection
        .query_row(
            "SELECT n.id,n.deck_id,n.note_type,n.content_format,n.content_schema_version,
                    COALESCE(n.front_doc,''),COALESCE(n.back_doc,''),
                    n.front_text,n.back_text,n.duplicate_hash,n.question_definition
             FROM notes n
             JOIN decks d ON d.id=n.deck_id
             WHERE n.id=?1 AND n.deleted_at IS NULL
               AND d.deleted_at IS NULL AND d.archived_at IS NULL",
            [note_id],
            |row| {
                Ok(StoredSelectionNote {
                    id: row.get(0)?,
                    deck_id: row.get(1)?,
                    note_type: row.get(2)?,
                    content_format: row.get(3)?,
                    content_schema_version: row.get(4)?,
                    front_doc: row.get(5)?,
                    back_doc: row.get(6)?,
                    front_text: row.get(7)?,
                    back_text: row.get(8)?,
                    duplicate_hash: row.get(9)?,
                    question_definition: row.get(10)?,
                })
            },
        )
        .optional()
        .map_err(CommandError::database)?
        .ok_or_else(|| {
            CommandError::new(
                "card_not_found",
                "A selected flashcard no longer exists in an active deck.",
            )
        })
}

fn is_duplicate_in_deck(
    transaction: &Transaction<'_>,
    deck_id: &str,
    source: &StoredSelectionNote,
) -> Result<bool, CommandError> {
    if source.note_type == "interactive" {
        let definition = source.question_definition.as_deref().ok_or_else(|| {
            CommandError::new(
                "invalid_question",
                "A selected interactive question is missing its definition.",
            )
        })?;
        let expected = ValidatedQuestion::parse(definition)?.duplicate_identity()?;
        let mut statement = transaction
            .prepare(
                "SELECT question_definition FROM notes
                 WHERE deck_id=?1 AND note_type='interactive'
                   AND deleted_at IS NULL",
            )
            .map_err(CommandError::database)?;
        let rows = statement
            .query_map([deck_id], |row| row.get::<_, String>(0))
            .map_err(CommandError::database)?;
        for row in rows {
            let definition = row.map_err(CommandError::database)?;
            if ValidatedQuestion::parse(&definition)?.duplicate_identity()? == expected {
                return Ok(true);
            }
        }
        return Ok(false);
    }

    if !matches!(source.note_type.as_str(), "basic" | "cloze") {
        return Err(unsupported_type());
    }
    if !duplicate_checkable(&source.front_text, &source.back_text) {
        return Ok(false);
    }
    let normalized = normalized_pair(&source.front_text, &source.back_text);
    let source_hash = if source.duplicate_hash.is_empty() {
        duplicate_hash(&normalized.0, &normalized.1)
    } else {
        source.duplicate_hash.clone()
    };
    let identity = duplicate_identity::from_serialized_documents(
        &source.front_doc,
        &source.back_doc,
        &source.front_text,
        &source.back_text,
    )?;
    duplicate_identity::has_exact_duplicate(
        transaction,
        deck_id,
        &source_hash,
        &identity,
        None,
    )
}

fn copy_note(
    transaction: &Transaction<'_>,
    source: &StoredSelectionNote,
    deck_id: &str,
    now: i64,
) -> Result<(), CommandError> {
    match source.note_type.as_str() {
        "basic" | "cloze" => {
            let normalized = normalized_pair(&source.front_text, &source.back_text);
            let source_hash = if source.duplicate_hash.is_empty() {
                duplicate_hash(&normalized.0, &normalized.1)
            } else {
                source.duplicate_hash.clone()
            };
            let recall = SourceNote {
                id: source.id.clone(),
                deck_id: source.deck_id.clone(),
                front_doc: source.front_doc.clone(),
                back_doc: source.back_doc.clone(),
                front_text: source.front_text.clone(),
                back_text: source.back_text.clone(),
                duplicate_hash: source.duplicate_hash.clone(),
            };
            copy_generated_note_to_deck(transaction, &recall, deck_id, &source_hash, now)
        }
        "interactive" => copy_interactive_note(transaction, source, deck_id, now),
        _ => Err(unsupported_type()),
    }
}

fn copy_interactive_note(
    transaction: &Transaction<'_>,
    source: &StoredSelectionNote,
    deck_id: &str,
    now: i64,
) -> Result<(), CommandError> {
    if source.content_format != "question-json" || source.content_schema_version != 1 {
        return Err(unsupported_type());
    }
    let definition = source.question_definition.as_deref().ok_or_else(|| {
        CommandError::new(
            "invalid_question",
            "A selected interactive question is missing its definition.",
        )
    })?;
    let question = ValidatedQuestion::parse(definition)?;
    let expected_keys = question
        .presentation_keys()
        .into_iter()
        .collect::<BTreeSet<_>>();

    let source_cards = {
        let mut statement = transaction
            .prepare(
                "SELECT c.direction,c.template_ordinal,qp.question_key
                 FROM cards c
                 LEFT JOIN card_question_presentations qp ON qp.card_id=c.id
                 WHERE c.note_id=?1 AND c.deleted_at IS NULL
                 ORDER BY c.template_ordinal,c.direction,c.id",
            )
            .map_err(CommandError::database)?;
        statement
            .query_map([source.id.as_str()], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, i64>(1)?,
                    row.get::<_, Option<String>>(2)?,
                ))
            })
            .map_err(CommandError::database)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(CommandError::database)?
    };
    if source_cards.is_empty() {
        return Err(CommandError::new(
            "card_not_found",
            "A selected interactive question has no active study presentation.",
        ));
    }
    let actual_keys = source_cards
        .iter()
        .filter_map(|(_, _, key)| key.clone())
        .collect::<BTreeSet<_>>();
    if source_cards.iter().any(|(_, _, key)| key.is_none()) || actual_keys != expected_keys {
        return Err(CommandError::new(
            "invalid_question",
            "A selected interactive question does not match its study presentations.",
        ));
    }

    let note_id = generate_id(transaction)?;
    transaction
        .execute(
            "INSERT INTO notes(
                 id,deck_id,note_type,content_format,content_schema_version,
                 question_definition,front_text,back_text,duplicate_hash,
                 created_at,updated_at,revision
             ) VALUES(?1,?2,'interactive','question-json',1,?3,?4,?5,?6,?7,?7,1)",
            params![
                note_id,
                deck_id,
                definition,
                source.front_text,
                source.back_text,
                source.duplicate_hash,
                now
            ],
        )
        .map_err(CommandError::database)?;

    for (direction, ordinal, question_key) in source_cards {
        let card_id = generate_id(transaction)?;
        let question_key = question_key.ok_or_else(|| {
            CommandError::new(
                "invalid_question",
                "A selected interactive question is missing a presentation key.",
            )
        })?;
        transaction
            .execute(
                "INSERT INTO cards(
                     id,note_id,deck_id,direction,template_ordinal,created_at,updated_at
                 ) VALUES(?1,?2,?3,?4,?5,?6,?6)",
                params![card_id, note_id, deck_id, direction, ordinal, now],
            )
            .map_err(CommandError::database)?;
        transaction
            .execute(
                "INSERT INTO card_question_presentations(card_id,note_id,question_key)
                 VALUES(?1,?2,?3)",
                params![card_id, note_id, question_key],
            )
            .map_err(CommandError::database)?;
        insert_new_schedule(transaction, &card_id, now)?;
    }
    copy_note_relationships(transaction, &source.id, &note_id)
}

fn copy_note_relationships(
    transaction: &Transaction<'_>,
    source_note_id: &str,
    target_note_id: &str,
) -> Result<(), CommandError> {
    transaction
        .execute(
            "INSERT INTO note_assets(note_id,asset_id,side)
             SELECT ?1,asset_id,side FROM note_assets WHERE note_id=?2",
            params![target_note_id, source_note_id],
        )
        .map_err(CommandError::database)?;
    transaction
        .execute(
            "INSERT INTO note_tags(note_id,tag_id)
             SELECT ?1,tag_id FROM note_tags WHERE note_id=?2",
            params![target_note_id, source_note_id],
        )
        .map_err(CommandError::database)?;
    Ok(())
}

fn unsupported_type() -> CommandError {
    CommandError::new(
        "unsupported_card_type",
        "A selected card uses a format this version cannot copy.",
    )
}

fn operation_conflict() -> CommandError {
    CommandError::new(
        "selection_deck_operation_conflict",
        "This save operation identity was already used for a different card selection or deck.",
    )
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::{
        cards::validate_content,
        decks::{create_deck_in_connection, CreateDeckInput},
    };

    fn id(character: char) -> String {
        std::iter::repeat_n(character, 32).collect()
    }

    fn rich(text: &str) -> serde_json::Value {
        json!({"type":"doc","content":[{"type":"paragraph","content":[{"type":"text","text":text}]}]})
    }

    fn seed_basic(database: &Database) -> String {
        database
            .with_connection(|connection| {
                let deck = create_deck_in_connection(
                    connection,
                    CreateDeckInput {
                        name: "Source".to_string(),
                        description: None,
                    },
                )?;
                let content = validate_content(
                    super::super::plain_text_document("Question"),
                    super::super::plain_text_document("Answer"),
                    "Question".to_string(),
                    "Answer".to_string(),
                )?;
                let normalized =
                    super::super::normalized_pair(&content.front_text, &content.back_text);
                let transaction = connection
                    .transaction_with_behavior(TransactionBehavior::Immediate)
                    .map_err(CommandError::database)?;
                let note = super::super::insert_organized_card(
                    &transaction,
                    &deck.id,
                    content,
                    &[],
                    &super::super::duplicate_hash(&normalized.0, &normalized.1),
                    100,
                )?;
                transaction.commit().map_err(CommandError::database)?;
                Ok(note)
            })
            .unwrap()
    }

    fn input(operation_id: String, note_ids: Vec<String>, name: &str) -> SaveSelectionAsDeckInput {
        SaveSelectionAsDeckInput {
            operation_id,
            name: name.to_string(),
            description: Some("Saved from a selection.".to_string()),
            note_ids,
        }
    }

    #[test]
    fn committed_operation_replays_without_creating_another_deck() {
        let database = Database::open_in_memory().unwrap();
        let note = seed_basic(&database);
        let operation = id('a');
        let first = database
            .with_connection(|connection| {
                save_selection_as_deck_in_connection(
                    connection,
                    input(operation.clone(), vec![note.clone()], "Weak topics"),
                )
            })
            .unwrap();
        let replay = database
            .with_connection(|connection| {
                save_selection_as_deck_in_connection(
                    connection,
                    input(operation.clone(), vec![note.clone()], "Weak topics"),
                )
            })
            .unwrap();
        assert_eq!(first.operation_id, replay.operation_id);
        assert_eq!(first.deck.id, replay.deck.id);
        assert_eq!(first.affected_count, replay.affected_count);
        assert_eq!(first.skipped_duplicates, replay.skipped_duplicates);

        database
            .with_connection(|connection| {
                let decks: i64 = connection
                    .query_row(
                        "SELECT COUNT(*) FROM decks WHERE deleted_at IS NULL",
                        [],
                        |row| row.get(0),
                    )
                    .map_err(CommandError::database)?;
                let receipts: i64 = connection
                    .query_row("SELECT COUNT(*) FROM selection_deck_operations", [], |row| {
                        row.get(0)
                    })
                    .map_err(CommandError::database)?;
                assert_eq!(decks, 2);
                assert_eq!(receipts, 1);
                Ok(())
            })
            .unwrap();

        let conflict = database
            .with_connection(|connection| {
                save_selection_as_deck_in_connection(
                    connection,
                    input(operation, vec![note], "Different name"),
                )
            })
            .unwrap_err();
        assert_eq!(conflict.code, "selection_deck_operation_conflict");
    }

    #[test]
    fn copy_failure_rolls_back_destination_and_receipt() {
        let database = Database::open_in_memory().unwrap();
        let note = seed_basic(&database);
        let error = database
            .with_connection(|connection| {
                save_selection_as_deck_in_connection(
                    connection,
                    input(id('b'), vec![note, id('f')], "Incomplete"),
                )
            })
            .unwrap_err();
        assert_eq!(error.code, "card_not_found");

        database
            .with_connection(|connection| {
                let decks: i64 = connection
                    .query_row(
                        "SELECT COUNT(*) FROM decks WHERE deleted_at IS NULL",
                        [],
                        |row| row.get(0),
                    )
                    .map_err(CommandError::database)?;
                let receipts: i64 = connection
                    .query_row("SELECT COUNT(*) FROM selection_deck_operations", [], |row| {
                        row.get(0)
                    })
                    .map_err(CommandError::database)?;
                assert_eq!(decks, 1);
                assert_eq!(receipts, 0);
                Ok(())
            })
            .unwrap();
    }

    #[test]
    fn copied_cards_receive_fresh_schedules_without_changing_source_history() {
        let database = Database::open_in_memory().unwrap();
        let note = seed_basic(&database);
        database
            .with_connection(|connection| {
                connection
                    .execute(
                        "UPDATE card_schedules
                         SET reps=7,lapses=2,revision=revision+1
                         WHERE card_id IN(SELECT id FROM cards WHERE note_id=?1)",
                        [note.as_str()],
                    )
                    .map_err(CommandError::database)?;
                Ok(())
            })
            .unwrap();

        let result = database
            .with_connection(|connection| {
                save_selection_as_deck_in_connection(
                    connection,
                    input(id('c'), vec![note.clone()], "Fresh copy"),
                )
            })
            .unwrap();

        database
            .with_connection(|connection| {
                let source: (i64, i64) = connection
                    .query_row(
                        "SELECT reps,lapses FROM card_schedules
                         WHERE card_id IN(SELECT id FROM cards WHERE note_id=?1)",
                        [note.as_str()],
                        |row| Ok((row.get(0)?, row.get(1)?)),
                    )
                    .map_err(CommandError::database)?;
                let copied: (i64, i64) = connection
                    .query_row(
                        "SELECT s.reps,s.lapses FROM card_schedules s
                         JOIN cards c ON c.id=s.card_id
                         WHERE c.deck_id=?1",
                        [result.deck.id.as_str()],
                        |row| Ok((row.get(0)?, row.get(1)?)),
                    )
                    .map_err(CommandError::database)?;
                assert_eq!(source, (7, 2));
                assert_eq!(copied, (0, 0));
                Ok(())
            })
            .unwrap();
    }

    #[test]
    fn interactive_question_copy_preserves_definition_and_keys_with_fresh_schedule() {
        let database = Database::open_in_memory().unwrap();
        let (source_note, definition) = database
            .with_connection(|connection| {
                let deck = create_deck_in_connection(
                    connection,
                    CreateDeckInput {
                        name: "Questions".to_string(),
                        description: None,
                    },
                )?;
                let definition = json!({
                    "schemaVersion": 1,
                    "question": {
                        "kind": "true_false",
                        "statement": rich("A triangle has three sides."),
                        "correct": true,
                        "explanation": null,
                        "correctedStatement": null
                    }
                })
                .to_string();
                let question = ValidatedQuestion::parse(&definition)?;
                let transaction = connection
                    .transaction_with_behavior(TransactionBehavior::Immediate)
                    .map_err(CommandError::database)?;
                let note_id = generate_id(&transaction)?;
                super::super::interactive::insert_generated(
                    &transaction,
                    &note_id,
                    &deck.id,
                    &question,
                    100,
                )?;
                transaction.commit().map_err(CommandError::database)?;
                Ok((note_id, definition))
            })
            .unwrap();

        let result = database
            .with_connection(|connection| {
                save_selection_as_deck_in_connection(
                    connection,
                    input(id('d'), vec![source_note], "Question copy"),
                )
            })
            .unwrap();

        database
            .with_connection(|connection| {
                let copied_definition: String = connection
                    .query_row(
                        "SELECT question_definition FROM notes
                         WHERE deck_id=?1 AND note_type='interactive'",
                        [result.deck.id.as_str()],
                        |row| row.get(0),
                    )
                    .map_err(CommandError::database)?;
                assert_eq!(
                    ValidatedQuestion::parse(&copied_definition)?.duplicate_identity()?,
                    ValidatedQuestion::parse(&definition)?.duplicate_identity()?
                );
                let presentations: i64 = connection
                    .query_row(
                        "SELECT COUNT(*) FROM card_question_presentations qp
                         JOIN cards c ON c.id=qp.card_id
                         WHERE c.deck_id=?1",
                        [result.deck.id.as_str()],
                        |row| row.get(0),
                    )
                    .map_err(CommandError::database)?;
                let fresh: i64 = connection
                    .query_row(
                        "SELECT COUNT(*) FROM card_schedules s
                         JOIN cards c ON c.id=s.card_id
                         WHERE c.deck_id=?1 AND s.reps=0 AND s.lapses=0",
                        [result.deck.id.as_str()],
                        |row| row.get(0),
                    )
                    .map_err(CommandError::database)?;
                assert_eq!(presentations, 1);
                assert_eq!(fresh, 1);
                Ok(())
            })
            .unwrap();
    }
}
