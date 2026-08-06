//! Physical cleanup after a soft-deletion undo window has expired.
//!
//! Review history is owned by the generated card. Deck cleanup therefore finds
//! every card still owned by that deck at purge time; cards moved beforehand are
//! not selected and retain their schedules, sessions, and complete history.

use std::collections::BTreeSet;

use rusqlite::{
    params, params_from_iter, types::Value as SqlValue, Connection, TransactionBehavior,
};

use crate::{
    database::Database,
    error::CommandError,
    projections::scope::{chunks, push_value},
};

struct FinalizedDeletion {
    id: String,
    entity_kind: String,
    deck_id: Option<String>,
    note_ids: Vec<String>,
    card_ids: Vec<String>,
    deleted_at: i64,
}

pub(crate) fn purge_finalized_deletions(database: &Database) -> Result<(), CommandError> {
    database.with_connection(purge_finalized_in_connection)
}

pub(crate) fn purge_finalized_in_connection(
    connection: &mut Connection,
) -> Result<(), CommandError> {
    let operations = {
        let mut statement = connection
            .prepare(
                "SELECT id, entity_kind, deck_id, note_ids_json, card_ids_json, deleted_at
                 FROM deletion_operations
                 WHERE finalized_at IS NOT NULL
                 ORDER BY finalized_at, id",
            )
            .map_err(CommandError::database)?;
        let rows = statement
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, Option<String>>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, i64>(5)?,
                ))
            })
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
    for stored in operations {
        let operation = FinalizedDeletion {
            id: stored.0,
            entity_kind: stored.1,
            deck_id: stored.2,
            note_ids: serde_json::from_str(&stored.3).map_err(CommandError::serialization)?,
            card_ids: serde_json::from_str(&stored.4).map_err(CommandError::serialization)?,
            deleted_at: stored.5,
        };
        purge_operation(&transaction, &operation)?;
    }
    transaction
        .execute(
            "DELETE FROM tags
             WHERE NOT EXISTS (SELECT 1 FROM note_tags nt WHERE nt.tag_id = tags.id)
               AND NOT EXISTS (
                   SELECT 1 FROM review_event_tag_attributions rta
                   WHERE rta.tag_id = tags.id
               )",
            [],
        )
        .map_err(CommandError::database)?;
    transaction.commit().map_err(CommandError::database)
}

fn purge_operation(
    connection: &Connection,
    operation: &FinalizedDeletion,
) -> Result<(), CommandError> {
    let (note_ids, card_ids) = match operation.entity_kind.as_str() {
        "deck" => {
            let deck_id = operation.deck_id.as_deref().ok_or_else(|| {
                CommandError::new(
                    "invalid_deletion_operation",
                    "A finalized deck deletion has no deck identifier.",
                )
            })?;
            load_owned_ids_for_deck(connection, deck_id)?
        }
        "cards" => load_exact_tombstoned_ids(connection, operation)?,
        _ => {
            return Err(CommandError::new(
                "invalid_deletion_operation",
                "A finalized deletion has an unsupported entity type.",
            ));
        }
    };

    let affected_sessions = load_session_ids(connection, &card_ids)?;
    delete_by_card_ids(connection, "review_events", "card_id", &card_ids)?;
    delete_by_card_ids(connection, "schedule_action_events", "card_id", &card_ids)?;
    delete_session_snapshots(connection, &card_ids)?;
    delete_by_card_ids(connection, "practice_session_cards", "card_id", &card_ids)?;
    refresh_or_delete_sessions(connection, &affected_sessions)?;
    connection
        .execute(
            "DELETE FROM schedule_action_operations
             WHERE NOT EXISTS (
                 SELECT 1 FROM schedule_action_events e
                 WHERE e.operation_id = schedule_action_operations.id
             )",
            [],
        )
        .map_err(CommandError::database)?;

    match operation.entity_kind.as_str() {
        "deck" => {
            let deck_id = operation.deck_id.as_deref().unwrap_or_default();
            connection
                .execute("DELETE FROM notes WHERE deck_id = ?1", [deck_id])
                .map_err(CommandError::database)?;
            connection
                .execute(
                    "DELETE FROM decks WHERE id = ?1 AND deleted_at IS NOT NULL",
                    [deck_id],
                )
                .map_err(CommandError::database)?;
        }
        "cards" => delete_exact_notes(connection, &note_ids, operation.deleted_at)?,
        _ => unreachable!(),
    }

    connection
        .execute(
            "DELETE FROM deletion_operations WHERE id = ?1",
            [&operation.id],
        )
        .map_err(CommandError::database)?;
    Ok(())
}

fn load_owned_ids_for_deck(
    connection: &Connection,
    deck_id: &str,
) -> Result<(Vec<String>, Vec<String>), CommandError> {
    let mut note_statement = connection
        .prepare("SELECT id FROM notes WHERE deck_id = ?1 ORDER BY id")
        .map_err(CommandError::database)?;
    let note_ids = note_statement
        .query_map([deck_id], |row| row.get(0))
        .map_err(CommandError::database)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(CommandError::database)?;
    let mut card_statement = connection
        .prepare("SELECT id FROM cards WHERE deck_id = ?1 ORDER BY id")
        .map_err(CommandError::database)?;
    let card_ids = card_statement
        .query_map([deck_id], |row| row.get(0))
        .map_err(CommandError::database)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(CommandError::database)?;
    Ok((note_ids, card_ids))
}

fn load_exact_tombstoned_ids(
    connection: &Connection,
    operation: &FinalizedDeletion,
) -> Result<(Vec<String>, Vec<String>), CommandError> {
    let mut note_ids = Vec::new();
    for chunk in chunks(&operation.note_ids) {
        let mut values = vec![SqlValue::Integer(operation.deleted_at)];
        let placeholders = chunk
            .iter()
            .map(|id| format!("?{}", push_value(&mut values, id.clone())))
            .collect::<Vec<_>>()
            .join(", ");
        let sql = format!(
            "SELECT id FROM notes
             WHERE deleted_at = ?1 AND id IN ({placeholders})
             ORDER BY id"
        );
        let mut statement = connection.prepare(&sql).map_err(CommandError::database)?;
        note_ids.extend(
            statement
                .query_map(params_from_iter(values.iter()), |row| row.get(0))
                .map_err(CommandError::database)?
                .collect::<Result<Vec<String>, _>>()
                .map_err(CommandError::database)?,
        );
    }
    let note_set = note_ids.iter().cloned().collect::<BTreeSet<_>>();
    let mut card_ids = Vec::new();
    for chunk in chunks(&operation.card_ids) {
        let mut values = vec![SqlValue::Integer(operation.deleted_at)];
        let placeholders = chunk
            .iter()
            .map(|id| format!("?{}", push_value(&mut values, id.clone())))
            .collect::<Vec<_>>()
            .join(", ");
        let sql = format!(
            "SELECT id, note_id FROM cards
             WHERE deleted_at = ?1 AND id IN ({placeholders})
             ORDER BY id"
        );
        let mut statement = connection.prepare(&sql).map_err(CommandError::database)?;
        let rows = statement
            .query_map(params_from_iter(values.iter()), |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })
            .map_err(CommandError::database)?;
        for row in rows {
            let (card_id, note_id) = row.map_err(CommandError::database)?;
            if note_set.contains(&note_id) {
                card_ids.push(card_id);
            }
        }
    }
    Ok((note_ids, card_ids))
}

fn load_session_ids(
    connection: &Connection,
    card_ids: &[String],
) -> Result<Vec<String>, CommandError> {
    let mut result = BTreeSet::new();
    for chunk in chunks(card_ids) {
        let mut values = Vec::<SqlValue>::new();
        let placeholders = chunk
            .iter()
            .map(|id| format!("?{}", push_value(&mut values, id.clone())))
            .collect::<Vec<_>>()
            .join(", ");
        let sql = format!(
            "SELECT DISTINCT session_id FROM practice_session_cards
             WHERE card_id IN ({placeholders})"
        );
        let mut statement = connection.prepare(&sql).map_err(CommandError::database)?;
        let rows = statement
            .query_map(params_from_iter(values.iter()), |row| {
                row.get::<_, String>(0)
            })
            .map_err(CommandError::database)?;
        for row in rows {
            result.insert(row.map_err(CommandError::database)?);
        }
    }
    Ok(result.into_iter().collect())
}

fn delete_by_card_ids(
    connection: &Connection,
    table: &str,
    column: &str,
    card_ids: &[String],
) -> Result<(), CommandError> {
    let allowed = [
        ("review_events", "card_id"),
        ("schedule_action_events", "card_id"),
        ("practice_session_cards", "card_id"),
    ];
    if !allowed.contains(&(table, column)) {
        return Err(CommandError::new(
            "invalid_purge_target",
            "The deletion cleanup requested an unsupported relationship.",
        ));
    }
    for chunk in chunks(card_ids) {
        let mut values = Vec::<SqlValue>::new();
        let placeholders = chunk
            .iter()
            .map(|id| format!("?{}", push_value(&mut values, id.clone())))
            .collect::<Vec<_>>()
            .join(", ");
        connection
            .execute(
                &format!("DELETE FROM {table} WHERE {column} IN ({placeholders})"),
                params_from_iter(values.iter()),
            )
            .map_err(CommandError::database)?;
    }
    Ok(())
}

fn delete_session_snapshots(
    connection: &Connection,
    card_ids: &[String],
) -> Result<(), CommandError> {
    for chunk in chunks(card_ids) {
        let mut values = Vec::<SqlValue>::new();
        let placeholders = chunk
            .iter()
            .map(|id| format!("?{}", push_value(&mut values, id.clone())))
            .collect::<Vec<_>>()
            .join(", ");
        connection
            .execute(
                &format!(
                    "DELETE FROM practice_session_card_snapshots
                     WHERE card_id IN ({placeholders})"
                ),
                params_from_iter(values.iter()),
            )
            .map_err(CommandError::database)?;
    }
    Ok(())
}

fn refresh_or_delete_sessions(
    connection: &Connection,
    session_ids: &[String],
) -> Result<(), CommandError> {
    for session_id in session_ids {
        connection
            .execute(
                "UPDATE practice_sessions
                 SET card_count = (
                         SELECT COUNT(*) FROM practice_session_cards
                         WHERE session_id = practice_sessions.id
                     ),
                     viewed_count = (
                         SELECT COUNT(*) FROM practice_session_cards
                         WHERE session_id = practice_sessions.id AND viewed_at IS NOT NULL
                     ),
                     revealed_count = (
                         SELECT COUNT(*) FROM practice_session_cards
                         WHERE session_id = practice_sessions.id AND revealed_at IS NOT NULL
                     ),
                     rated_count = (
                         SELECT COUNT(*) FROM practice_session_cards
                         WHERE session_id = practice_sessions.id AND rated_at IS NOT NULL
                     ),
                     correct_count = (
                         SELECT COUNT(*) FROM practice_session_cards
                         WHERE session_id = practice_sessions.id AND rating BETWEEN 2 AND 4
                     )
                 WHERE id = ?1",
                [session_id],
            )
            .map_err(CommandError::database)?;
        connection
            .execute(
                "DELETE FROM practice_sessions
                 WHERE id = ?1 AND NOT EXISTS (
                     SELECT 1 FROM practice_session_cards psc
                     WHERE psc.session_id = practice_sessions.id
                 )",
                [session_id],
            )
            .map_err(CommandError::database)?;
    }
    Ok(())
}

fn delete_exact_notes(
    connection: &Connection,
    note_ids: &[String],
    deleted_at: i64,
) -> Result<(), CommandError> {
    for chunk in chunks(note_ids) {
        let mut values = vec![SqlValue::Integer(deleted_at)];
        let placeholders = chunk
            .iter()
            .map(|id| format!("?{}", push_value(&mut values, id.clone())))
            .collect::<Vec<_>>()
            .join(", ");
        connection
            .execute(
                &format!(
                    "DELETE FROM notes
                     WHERE deleted_at = ?1 AND id IN ({placeholders})"
                ),
                params_from_iter(values.iter()),
            )
            .map_err(CommandError::database)?;
    }
    Ok(())
}
