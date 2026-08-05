//! Exact soft-deletion operations with short-lived, conflict-safe undo.
//!
//! Deletion preserves relationship rows, schedules, and review history while an
//! undo token is valid. Undo restores only rows whose tombstone exactly matches
//! the recorded operation, so it never overwrites a later independent change.

use std::collections::BTreeSet;

use rusqlite::{params, Connection, OptionalExtension, Transaction, TransactionBehavior};
use serde::{Deserialize, Serialize};
use tauri::State;

use crate::{
    database::Database,
    error::CommandError,
    historical_attribution::purge_deck_history,
    util::{generate_id, unix_time_millis, validate_entity_id},
};

pub const DELETION_UNDO_WINDOW_MS: i64 = 10_000;

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeletionResultDto {
    pub affected_count: i64,
    pub undo_token: String,
    pub undo_expires_at: i64,
    pub subject_label: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UndoDeletionInput {
    pub undo_token: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UndoDeletionResultDto {
    pub restored_count: i64,
    pub entity_kind: String,
    pub subject_label: String,
}

struct DeletionOperation {
    entity_kind: String,
    subject_label: String,
    deck_id: Option<String>,
    note_ids: Vec<String>,
    card_ids: Vec<String>,
    deleted_at: i64,
    expires_at: i64,
    restored_at: Option<i64>,
    finalized_at: Option<i64>,
}

#[tauri::command]
pub fn undo_deletion(
    database: State<'_, Database>,
    input: UndoDeletionInput,
) -> Result<UndoDeletionResultDto, CommandError> {
    validate_entity_id(&input.undo_token, "deletion undo")?;
    database.with_connection(|connection| undo_in_connection(connection, &input.undo_token))
}

pub(crate) fn delete_cards(
    connection: &mut Connection,
    note_ids: Vec<String>,
    subject_label: String,
) -> Result<DeletionResultDto, CommandError> {
    let note_ids = normalize_ids(note_ids, "note")?;
    if note_ids.is_empty() {
        return Err(CommandError::new(
            "no_cards_selected",
            "Select at least one card first.",
        ));
    }
    let now = unix_time_millis()?;
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(CommandError::database)?;
    let card_ids = load_active_card_ids(&transaction, &note_ids)?;
    for note_id in &note_ids {
        let changed = transaction
            .execute(
                "UPDATE notes
                 SET deleted_at = ?1, updated_at = ?1, revision = revision + 1
                 WHERE id = ?2 AND deleted_at IS NULL
                   AND EXISTS (
                       SELECT 1 FROM decks d
                       WHERE d.id = notes.deck_id
                         AND d.deleted_at IS NULL AND d.archived_at IS NULL
                   )",
                params![now, note_id],
            )
            .map_err(CommandError::database)?;
        if changed != 1 {
            return Err(CommandError::new(
                "card_not_found",
                "A selected flashcard no longer exists.",
            ));
        }
        transaction
            .execute(
                "UPDATE cards
                 SET deleted_at = ?1, updated_at = ?1
                 WHERE note_id = ?2 AND deleted_at IS NULL",
                params![now, note_id],
            )
            .map_err(CommandError::database)?;
    }
    let result = insert_operation(
        &transaction,
        "cards",
        &subject_label,
        None,
        &note_ids,
        &card_ids,
        now,
    )?;
    transaction.commit().map_err(CommandError::database)?;
    Ok(result)
}

pub(crate) fn delete_deck(
    connection: &mut Connection,
    deck_id: &str,
) -> Result<DeletionResultDto, CommandError> {
    validate_entity_id(deck_id, "deck")?;
    let now = unix_time_millis()?;
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(CommandError::database)?;
    let deck_name: Option<String> = transaction
        .query_row(
            "SELECT name FROM decks
             WHERE id = ?1 AND deleted_at IS NULL",
            [deck_id],
            |row| row.get(0),
        )
        .optional()
        .map_err(CommandError::database)?;
    let Some(deck_name) = deck_name else {
        return Err(CommandError::new(
            "deck_not_found",
            "That deck no longer exists.",
        ));
    };
    let note_ids = load_active_note_ids_for_deck(&transaction, deck_id)?;
    let card_ids = load_active_card_ids(&transaction, &note_ids)?;

    transaction
        .execute(
            "UPDATE decks
             SET deleted_at = ?1, updated_at = ?1, revision = revision + 1
             WHERE id = ?2 AND deleted_at IS NULL",
            params![now, deck_id],
        )
        .map_err(CommandError::database)?;
    transaction
        .execute(
            "UPDATE notes
             SET deleted_at = ?1, updated_at = ?1, revision = revision + 1
             WHERE deck_id = ?2 AND deleted_at IS NULL",
            params![now, deck_id],
        )
        .map_err(CommandError::database)?;
    transaction
        .execute(
            "UPDATE cards
             SET deleted_at = ?1, updated_at = ?1
             WHERE deck_id = ?2 AND deleted_at IS NULL",
            params![now, deck_id],
        )
        .map_err(CommandError::database)?;

    let result = insert_operation(
        &transaction,
        "deck",
        &format!("Deck “{deck_name}”"),
        Some(deck_id),
        &note_ids,
        &card_ids,
        now,
    )?;
    transaction.commit().map_err(CommandError::database)?;
    Ok(result)
}

pub(crate) fn finalize_expired_deletions(database: &Database) -> Result<(), CommandError> {
    database.with_connection(|connection| {
        finalize_expired_in_connection(connection, unix_time_millis()?)
    })
}

fn undo_in_connection(
    connection: &mut Connection,
    undo_token: &str,
) -> Result<UndoDeletionResultDto, CommandError> {
    let now = unix_time_millis()?;
    finalize_expired_in_connection(connection, now)?;
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(CommandError::database)?;
    let operation = load_operation(&transaction, undo_token)?;
    if operation.restored_at.is_some() {
        return Err(CommandError::new(
            "deletion_already_undone",
            "This deletion was already undone.",
        ));
    }
    if operation.finalized_at.is_some() || operation.expires_at < now {
        return Err(CommandError::new(
            "deletion_undo_expired",
            "The time to undo this deletion has expired.",
        ));
    }

    match operation.entity_kind.as_str() {
        "cards" => restore_cards(&transaction, &operation, now)?,
        "deck" => restore_deck(&transaction, &operation, now)?,
        _ => {
            return Err(CommandError::new(
                "invalid_deletion_operation",
                "The deletion operation is invalid.",
            ));
        }
    }
    transaction
        .execute(
            "UPDATE deletion_operations
             SET restored_at = ?1
             WHERE id = ?2 AND restored_at IS NULL AND finalized_at IS NULL",
            params![now, undo_token],
        )
        .map_err(CommandError::database)?;
    transaction.commit().map_err(CommandError::database)?;

    Ok(UndoDeletionResultDto {
        restored_count: i64::try_from(operation.note_ids.len()).unwrap_or(i64::MAX),
        entity_kind: operation.entity_kind,
        subject_label: operation.subject_label,
    })
}

fn restore_cards(
    transaction: &Transaction<'_>,
    operation: &DeletionOperation,
    now: i64,
) -> Result<(), CommandError> {
    for note_id in &operation.note_ids {
        let deck_active: i64 = transaction
            .query_row(
                "SELECT EXISTS(
                     SELECT 1 FROM notes n
                     JOIN decks d ON d.id = n.deck_id
                     WHERE n.id = ?1 AND d.deleted_at IS NULL AND d.archived_at IS NULL
                 )",
                [note_id],
                |row| row.get(0),
            )
            .map_err(CommandError::database)?;
        if deck_active == 0 {
            return Err(CommandError::new(
                "deletion_undo_conflict",
                "The card cannot be restored because its deck is no longer active.",
            ));
        }
        let changed = transaction
            .execute(
                "UPDATE notes
                 SET deleted_at = NULL, updated_at = ?1, revision = revision + 1
                 WHERE id = ?2 AND deleted_at = ?3",
                params![now, note_id, operation.deleted_at],
            )
            .map_err(CommandError::database)?;
        if changed != 1 {
            return Err(undo_conflict());
        }
    }
    restore_exact_cards(transaction, operation, now)
}

fn restore_deck(
    transaction: &Transaction<'_>,
    operation: &DeletionOperation,
    now: i64,
) -> Result<(), CommandError> {
    let deck_id = operation.deck_id.as_deref().ok_or_else(|| {
        CommandError::new(
            "invalid_deletion_operation",
            "The deck deletion has no deck identifier.",
        )
    })?;
    let changed = transaction
        .execute(
            "UPDATE decks
             SET deleted_at = NULL, updated_at = ?1, revision = revision + 1
             WHERE id = ?2 AND deleted_at = ?3",
            params![now, deck_id, operation.deleted_at],
        )
        .map_err(CommandError::database)?;
    if changed != 1 {
        return Err(undo_conflict());
    }
    for note_id in &operation.note_ids {
        let changed = transaction
            .execute(
                "UPDATE notes
                 SET deleted_at = NULL, updated_at = ?1, revision = revision + 1
                 WHERE id = ?2 AND deck_id = ?3 AND deleted_at = ?4",
                params![now, note_id, deck_id, operation.deleted_at],
            )
            .map_err(CommandError::database)?;
        if changed != 1 {
            return Err(undo_conflict());
        }
    }
    restore_exact_cards(transaction, operation, now)
}

fn restore_exact_cards(
    transaction: &Transaction<'_>,
    operation: &DeletionOperation,
    now: i64,
) -> Result<(), CommandError> {
    for card_id in &operation.card_ids {
        let changed = transaction
            .execute(
                "UPDATE cards
                 SET deleted_at = NULL, updated_at = ?1
                 WHERE id = ?2 AND deleted_at = ?3",
                params![now, card_id, operation.deleted_at],
            )
            .map_err(CommandError::database)?;
        if changed != 1 {
            return Err(undo_conflict());
        }
    }
    Ok(())
}

pub(crate) fn finalize_expired_in_connection(
    connection: &mut Connection,
    now: i64,
) -> Result<(), CommandError> {
    let operations = {
        let mut statement = connection
            .prepare(
                "SELECT id, entity_kind, subject_label, deck_id, note_ids_json,
                        card_ids_json, deleted_at, expires_at, restored_at, finalized_at
                 FROM deletion_operations
                 WHERE restored_at IS NULL AND finalized_at IS NULL AND expires_at < ?1
                 ORDER BY expires_at, id",
            )
            .map_err(CommandError::database)?;
        let rows = statement
            .query_map([now], operation_from_row)
            .map_err(CommandError::database)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(CommandError::database)?;
        rows
    };
    if operations.is_empty() {
        return Ok(());
    }
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(CommandError::database)?;
    for (operation_id, operation) in operations {
        if operation.entity_kind == "deck" {
            if let Some(deck_id) = operation.deck_id.as_deref() {
                purge_deck_history(&transaction, deck_id)?;
            }
        }
        for note_id in &operation.note_ids {
            // Relationship cleanup occurs only after undo expires. Review events,
            // schedules, notes, and cards remain soft-deleted for durable history.
            transaction
                .execute("DELETE FROM note_assets WHERE note_id = ?1", [note_id])
                .map_err(CommandError::database)?;
            transaction
                .execute("DELETE FROM note_tags WHERE note_id = ?1", [note_id])
                .map_err(CommandError::database)?;
        }
        transaction
            .execute(
                "UPDATE deletion_operations SET finalized_at = ?1 WHERE id = ?2",
                params![now, operation_id],
            )
            .map_err(CommandError::database)?;
    }
    transaction
        .execute(
            "DELETE FROM tags
             WHERE NOT EXISTS (SELECT 1 FROM note_tags nt WHERE nt.tag_id = tags.id)",
            [],
        )
        .map_err(CommandError::database)?;
    transaction.commit().map_err(CommandError::database)
}

fn insert_operation(
    transaction: &Transaction<'_>,
    entity_kind: &str,
    subject_label: &str,
    deck_id: Option<&str>,
    note_ids: &[String],
    card_ids: &[String],
    deleted_at: i64,
) -> Result<DeletionResultDto, CommandError> {
    let id = generate_id(transaction)?;
    let expires_at = deleted_at.saturating_add(DELETION_UNDO_WINDOW_MS);
    let note_ids_json = serde_json::to_string(note_ids).map_err(CommandError::serialization)?;
    let card_ids_json = serde_json::to_string(card_ids).map_err(CommandError::serialization)?;
    transaction
        .execute(
            "INSERT INTO deletion_operations (
                 id, entity_kind, subject_label, deck_id, note_ids_json, card_ids_json,
                 deleted_at, expires_at, created_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?7)",
            params![
                id,
                entity_kind,
                subject_label,
                deck_id,
                note_ids_json,
                card_ids_json,
                deleted_at,
                expires_at,
            ],
        )
        .map_err(CommandError::database)?;
    Ok(DeletionResultDto {
        affected_count: i64::try_from(note_ids.len()).unwrap_or(i64::MAX),
        undo_token: id,
        undo_expires_at: expires_at,
        subject_label: subject_label.to_string(),
    })
}

fn load_operation(
    connection: &Connection,
    operation_id: &str,
) -> Result<DeletionOperation, CommandError> {
    connection
        .query_row(
            "SELECT entity_kind, subject_label, deck_id, note_ids_json, card_ids_json,
                    deleted_at, expires_at, restored_at, finalized_at
             FROM deletion_operations WHERE id = ?1",
            [operation_id],
            |row| {
                let note_ids_json: String = row.get(3)?;
                let card_ids_json: String = row.get(4)?;
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, Option<String>>(2)?,
                    note_ids_json,
                    card_ids_json,
                    row.get::<_, i64>(5)?,
                    row.get::<_, i64>(6)?,
                    row.get::<_, Option<i64>>(7)?,
                    row.get::<_, Option<i64>>(8)?,
                ))
            },
        )
        .optional()
        .map_err(CommandError::database)?
        .ok_or_else(|| {
            CommandError::new(
                "deletion_undo_not_found",
                "This deletion can no longer be undone.",
            )
        })
        .and_then(operation_from_values)
}

fn operation_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<(String, DeletionOperation)> {
    let id: String = row.get(0)?;
    let values = (
        row.get::<_, String>(1)?,
        row.get::<_, String>(2)?,
        row.get::<_, Option<String>>(3)?,
        row.get::<_, String>(4)?,
        row.get::<_, String>(5)?,
        row.get::<_, i64>(6)?,
        row.get::<_, i64>(7)?,
        row.get::<_, Option<i64>>(8)?,
        row.get::<_, Option<i64>>(9)?,
    );
    let operation = operation_from_values(values).map_err(|_| rusqlite::Error::InvalidQuery)?;
    Ok((id, operation))
}

type StoredOperationValues = (
    String,
    String,
    Option<String>,
    String,
    String,
    i64,
    i64,
    Option<i64>,
    Option<i64>,
);

fn operation_from_values(values: StoredOperationValues) -> Result<DeletionOperation, CommandError> {
    let note_ids =
        serde_json::from_str::<Vec<String>>(&values.3).map_err(CommandError::serialization)?;
    let card_ids =
        serde_json::from_str::<Vec<String>>(&values.4).map_err(CommandError::serialization)?;
    Ok(DeletionOperation {
        entity_kind: values.0,
        subject_label: values.1,
        deck_id: values.2,
        note_ids,
        card_ids,
        deleted_at: values.5,
        expires_at: values.6,
        restored_at: values.7,
        finalized_at: values.8,
    })
}

fn load_active_note_ids_for_deck(
    connection: &Connection,
    deck_id: &str,
) -> Result<Vec<String>, CommandError> {
    let mut statement = connection
        .prepare(
            "SELECT id FROM notes
             WHERE deck_id = ?1 AND deleted_at IS NULL
             ORDER BY created_at, id",
        )
        .map_err(CommandError::database)?;
    let note_ids = statement
        .query_map([deck_id], |row| row.get::<_, String>(0))
        .map_err(CommandError::database)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(CommandError::database)?;
    Ok(note_ids)
}

fn load_active_card_ids(
    connection: &Connection,
    note_ids: &[String],
) -> Result<Vec<String>, CommandError> {
    let mut card_ids = BTreeSet::new();
    for note_id in note_ids {
        let note_exists: i64 = connection
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM notes WHERE id = ?1 AND deleted_at IS NULL)",
                [note_id],
                |row| row.get(0),
            )
            .map_err(CommandError::database)?;
        if note_exists == 0 {
            return Err(CommandError::new(
                "card_not_found",
                "A selected flashcard no longer exists.",
            ));
        }
        let mut statement = connection
            .prepare(
                "SELECT id FROM cards
                 WHERE note_id = ?1 AND deleted_at IS NULL
                 ORDER BY created_at, id",
            )
            .map_err(CommandError::database)?;
        let rows = statement
            .query_map([note_id], |row| row.get::<_, String>(0))
            .map_err(CommandError::database)?;
        for row in rows {
            card_ids.insert(row.map_err(CommandError::database)?);
        }
    }
    Ok(card_ids.into_iter().collect())
}

fn normalize_ids(values: Vec<String>, label: &str) -> Result<Vec<String>, CommandError> {
    let mut unique = BTreeSet::new();
    for value in values {
        validate_entity_id(&value, label)?;
        unique.insert(value);
    }
    Ok(unique.into_iter().collect())
}

fn undo_conflict() -> CommandError {
    CommandError::new(
        "deletion_undo_conflict",
        "This item changed after it was deleted and cannot be safely restored.",
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        cards::validate_content,
        decks::{create_deck_in_connection, CreateDeckInput},
        scheduler::insert_new_schedule,
        util::generate_id,
    };
    use serde_json::json;

    fn create_note(
        connection: &mut Connection,
        deck_id: &str,
        label: &str,
    ) -> Result<(String, String), CommandError> {
        let content = validate_content(
            json!({"type":"doc","content":[{"type":"paragraph","content":[{"type":"text","text":label}]}]}),
            json!({"type":"doc","content":[{"type":"paragraph","content":[{"type":"text","text":"Answer"}]}]}),
            label.to_string(),
            "Answer".to_string(),
        )?;
        let note_id = generate_id(connection)?;
        let card_id = generate_id(connection)?;
        connection
            .execute(
                "INSERT INTO notes (
                 id, deck_id, note_type, content_format, content_schema_version,
                 front_doc, back_doc, front_text, back_text, duplicate_hash,
                 created_at, updated_at, revision
             ) VALUES (?1, ?2, 'basic', 'tiptap-json', 1, ?3, ?4, ?5, ?6, '', 1, 1, 1)",
                params![
                    note_id,
                    deck_id,
                    content.front_doc_json,
                    content.back_doc_json,
                    content.front_text,
                    content.back_text
                ],
            )
            .map_err(CommandError::database)?;
        connection.execute(
            "INSERT INTO cards (id, note_id, deck_id, direction, template_ordinal, created_at, updated_at)
             VALUES (?1, ?2, ?3, 'forward', 0, 1, 1)",
            params![card_id, note_id, deck_id],
        ).map_err(CommandError::database)?;
        insert_new_schedule(connection, &card_id, 1)?;
        Ok((note_id, card_id))
    }

    #[test]
    fn card_deletion_restores_exact_rows_and_preserves_relationships() {
        let database = Database::open_in_memory().expect("database should open");
        database
            .with_connection(|connection| {
                let deck = create_deck_in_connection(
                    connection,
                    CreateDeckInput {
                        name: "Undo".to_string(),
                        description: None,
                    },
                )?;
                let (note_id, card_id) = create_note(connection, &deck.id, "Question")?;
                let tag_id = generate_id(connection)?;
                connection
                    .execute(
                        "INSERT INTO tags (id, name, normalized_name, created_at, updated_at)
                 VALUES (?1, 'Undo', 'undo', 1, 1)",
                        [tag_id.as_str()],
                    )
                    .map_err(CommandError::database)?;
                connection
                    .execute(
                        "INSERT INTO note_tags (note_id, tag_id) VALUES (?1, ?2)",
                        params![note_id, tag_id],
                    )
                    .map_err(CommandError::database)?;

                let deleted =
                    delete_cards(connection, vec![note_id.clone()], "Flashcard".to_string())?;
                assert_eq!(deleted.affected_count, 1);
                let restored = undo_in_connection(connection, &deleted.undo_token)?;
                assert_eq!(restored.restored_count, 1);
                let state: (Option<i64>, Option<i64>, i64) = connection
                    .query_row(
                        "SELECT n.deleted_at, c.deleted_at,
                        (SELECT COUNT(*) FROM note_tags WHERE note_id = n.id)
                 FROM notes n JOIN cards c ON c.note_id = n.id
                 WHERE n.id = ?1 AND c.id = ?2",
                        params![note_id, card_id],
                        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
                    )
                    .map_err(CommandError::database)?;
                assert_eq!(state, (None, None, 1));
                Ok(())
            })
            .expect("card should be restored exactly");
    }

    #[test]
    fn deck_undo_restores_only_rows_tombstoned_by_that_operation() {
        let database = Database::open_in_memory().expect("database should open");
        database
            .with_connection(|connection| {
                let deck = create_deck_in_connection(
                    connection,
                    CreateDeckInput {
                        name: "Deck undo".to_string(),
                        description: None,
                    },
                )?;
                let (active_note, active_card) = create_note(connection, &deck.id, "Active")?;
                let (old_note, old_card) = create_note(connection, &deck.id, "Old deleted")?;
                connection
                    .execute(
                        "UPDATE notes SET deleted_at = 5 WHERE id = ?1",
                        [old_note.as_str()],
                    )
                    .map_err(CommandError::database)?;
                connection
                    .execute(
                        "UPDATE cards SET deleted_at = 5 WHERE id = ?1",
                        [old_card.as_str()],
                    )
                    .map_err(CommandError::database)?;

                let deleted = delete_deck(connection, &deck.id)?;
                undo_in_connection(connection, &deleted.undo_token)?;
                let restored: (Option<i64>, Option<i64>) = connection.query_row(
                "SELECT n.deleted_at, c.deleted_at FROM notes n JOIN cards c ON c.note_id = n.id
                 WHERE n.id = ?1 AND c.id = ?2",
                params![active_note, active_card],
                |row| Ok((row.get(0)?, row.get(1)?)),
            ).map_err(CommandError::database)?;
                let old: (Option<i64>, Option<i64>) = connection.query_row(
                "SELECT n.deleted_at, c.deleted_at FROM notes n JOIN cards c ON c.note_id = n.id
                 WHERE n.id = ?1 AND c.id = ?2",
                params![old_note, old_card],
                |row| Ok((row.get(0)?, row.get(1)?)),
            ).map_err(CommandError::database)?;
                assert_eq!(restored, (None, None));
                assert_eq!(old, (Some(5), Some(5)));
                Ok(())
            })
            .expect("deck undo should not revive prior tombstones");
    }

    #[test]
    fn expired_and_conflicting_undo_requests_fail_visibly() {
        let database = Database::open_in_memory().expect("database should open");
        database
            .with_connection(|connection| {
                let deck = create_deck_in_connection(
                    connection,
                    CreateDeckInput {
                        name: "Expiry".to_string(),
                        description: None,
                    },
                )?;
                let (note_id, _) = create_note(connection, &deck.id, "Question")?;
                let deleted =
                    delete_cards(connection, vec![note_id.clone()], "Flashcard".to_string())?;
                connection
                    .execute(
                        "UPDATE deletion_operations
                         SET deleted_at = 1, expires_at = 1, created_at = 1
                         WHERE id = ?1",
                        [deleted.undo_token.as_str()],
                    )
                    .map_err(CommandError::database)?;
                let expired = undo_in_connection(connection, &deleted.undo_token)
                    .expect_err("expired undo must fail");
                assert_eq!(expired.code, "deletion_undo_expired");

                let second_note = create_note(connection, &deck.id, "Second")?.0;
                let second = delete_cards(
                    connection,
                    vec![second_note.clone()],
                    "Flashcard".to_string(),
                )?;
                connection
                    .execute(
                        "UPDATE notes SET deleted_at = 999 WHERE id = ?1",
                        [second_note.as_str()],
                    )
                    .map_err(CommandError::database)?;
                let conflict = undo_in_connection(connection, &second.undo_token)
                    .expect_err("changed tombstone must fail");
                assert_eq!(conflict.code, "deletion_undo_conflict");
                Ok(())
            })
            .expect("undo failures should be explicit");
    }
}
