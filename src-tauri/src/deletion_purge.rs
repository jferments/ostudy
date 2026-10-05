//! Physical cleanup after a soft-deletion undo window has expired.
//!
//! Review history is owned by the generated card. Deck cleanup therefore finds
//! every card still owned by that deck at purge time; cards moved beforehand are
//! not selected and retain their schedules, sessions, and complete history.
//!
//! A collection copy that release schema 6 retired into its original counts as
//! part of the original's history. It is purged with the original, wherever it
//! lives, and a deck purge first returns retired copies whose original now
//! lives elsewhere to that original's deck.

use std::collections::BTreeSet;

use rusqlite::{params_from_iter, types::Value as SqlValue, Connection, TransactionBehavior};

use crate::{
    database::Database,
    error::CommandError,
    learning_metrics,
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
            rehome_retired_copies(connection, deck_id)?;
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
    let (retired_note_ids, card_ids) = with_retired_copies(connection, card_ids)?;

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
    // Retired copies go first: card_merges must not outlive a surviving card.
    delete_notes(connection, &retired_note_ids)?;

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

/// Moves the retired copies in a deck being purged whose original card now
/// lives in another deck to that deck, with all their cards, so purging the
/// deck cannot delete review history that counts under a kept card.
fn rehome_retired_copies(connection: &Connection, deck_id: &str) -> Result<(), CommandError> {
    connection
        .execute(
            "UPDATE notes
             SET deck_id = (
                 SELECT survivor.deck_id FROM card_merges merged
                 JOIN cards survivor ON survivor.id = merged.surviving_card_id
                 WHERE merged.retired_note_id = notes.id
                 ORDER BY merged.retired_card_id LIMIT 1)
             WHERE deck_id = ?1 AND EXISTS (
                 SELECT 1 FROM card_merges merged
                 JOIN cards survivor ON survivor.id = merged.surviving_card_id
                 WHERE merged.retired_note_id = notes.id AND survivor.deck_id <> ?1)",
            [deck_id],
        )
        .map_err(CommandError::database)?;
    connection
        .execute(
            "UPDATE cards
             SET deck_id = (SELECT n.deck_id FROM notes n WHERE n.id = cards.note_id)
             WHERE deck_id = ?1
               AND note_id IN (SELECT retired_note_id FROM card_merges)
               AND (SELECT n.deck_id FROM notes n WHERE n.id = cards.note_id) <> ?1",
            [deck_id],
        )
        .map_err(CommandError::database)?;
    Ok(())
}

/// Adds the copies retired into any of `card_ids`, with all of their cards.
/// Returns the retired notes, which the caller deletes, and the full card set.
fn with_retired_copies(
    connection: &Connection,
    card_ids: Vec<String>,
) -> Result<(Vec<String>, Vec<String>), CommandError> {
    let mut retired_note_ids = BTreeSet::new();
    for chunk in chunks(&card_ids) {
        let mut values = Vec::<SqlValue>::new();
        let placeholders = chunk
            .iter()
            .map(|id| format!("?{}", push_value(&mut values, id.clone())))
            .collect::<Vec<_>>()
            .join(", ");
        let sql = format!(
            "SELECT DISTINCT retired_note_id FROM card_merges
             WHERE surviving_card_id IN ({placeholders})"
        );
        let mut statement = connection.prepare(&sql).map_err(CommandError::database)?;
        let rows = statement
            .query_map(params_from_iter(values.iter()), |row| {
                row.get::<_, String>(0)
            })
            .map_err(CommandError::database)?;
        for row in rows {
            retired_note_ids.insert(row.map_err(CommandError::database)?);
        }
    }
    if retired_note_ids.is_empty() {
        return Ok((Vec::new(), card_ids));
    }
    let retired_note_ids = retired_note_ids.into_iter().collect::<Vec<_>>();
    let mut all_card_ids = card_ids.into_iter().collect::<BTreeSet<_>>();
    for chunk in chunks(&retired_note_ids) {
        let mut values = Vec::<SqlValue>::new();
        let placeholders = chunk
            .iter()
            .map(|id| format!("?{}", push_value(&mut values, id.clone())))
            .collect::<Vec<_>>()
            .join(", ");
        let sql = format!("SELECT id FROM cards WHERE note_id IN ({placeholders})");
        let mut statement = connection.prepare(&sql).map_err(CommandError::database)?;
        let rows = statement
            .query_map(params_from_iter(values.iter()), |row| {
                row.get::<_, String>(0)
            })
            .map_err(CommandError::database)?;
        for row in rows {
            all_card_ids.insert(row.map_err(CommandError::database)?);
        }
    }
    Ok((retired_note_ids, all_card_ids.into_iter().collect()))
}

fn delete_notes(connection: &Connection, note_ids: &[String]) -> Result<(), CommandError> {
    for chunk in chunks(note_ids) {
        let mut values = Vec::<SqlValue>::new();
        let placeholders = chunk
            .iter()
            .map(|id| format!("?{}", push_value(&mut values, id.clone())))
            .collect::<Vec<_>>()
            .join(", ");
        connection
            .execute(
                &format!("DELETE FROM notes WHERE id IN ({placeholders})"),
                params_from_iter(values.iter()),
            )
            .map_err(CommandError::database)?;
    }
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
        learning_metrics::refresh_session_counts(connection, session_id)?;
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

#[cfg(test)]
mod tests {
    use rusqlite::params;

    use super::*;
    use crate::database::subjects_fixture::{
        advance_release_library, release_library, seed, Note, SubjectsLibrary, DAY, T0,
    };

    const DELETED_AT: i64 = T0 + 30 * DAY;

    fn upgraded() -> (Connection, SubjectsLibrary) {
        let connection = release_library(5);
        let library = seed(&connection);
        advance_release_library(&connection, 5, 6);
        (connection, library)
    }

    fn finalize(connection: &Connection, kind: &str, deck_id: Option<&str>, notes: &[&Note]) {
        let note_ids = notes.iter().map(|note| note.id.clone()).collect::<Vec<_>>();
        let card_ids = notes
            .iter()
            .flat_map(|note| note.cards.iter().cloned())
            .collect::<Vec<_>>();
        connection
            .execute(
                "INSERT INTO deletion_operations (id, entity_kind, subject_label, deck_id,
                     note_ids_json, card_ids_json, deleted_at, expires_at, finalized_at,
                     created_at)
                 VALUES (?1, ?2, 'Deleted', ?3, ?4, ?5, ?6, ?6, ?6, ?6)",
                params![
                    format!("de{}", "1".repeat(30)),
                    kind,
                    deck_id,
                    serde_json::to_string(&note_ids).unwrap(),
                    serde_json::to_string(&card_ids).unwrap(),
                    DELETED_AT
                ],
            )
            .unwrap();
    }

    fn exists(connection: &Connection, table: &str, column: &str, id: &str) -> bool {
        connection
            .query_row(
                &format!("SELECT EXISTS(SELECT 1 FROM {table} WHERE {column} = ?1)"),
                [id],
                |row| row.get(0),
            )
            .unwrap()
    }

    fn merge_count(connection: &Connection) -> i64 {
        connection
            .query_row("SELECT COUNT(*) FROM card_merges", [], |row| row.get(0))
            .unwrap()
    }

    #[test]
    fn purging_an_original_purges_the_copies_retired_into_it() {
        let (mut connection, library) = upgraded();
        let original = &library.o_mech[0];
        connection
            .execute(
                "UPDATE notes SET deleted_at = ?2, updated_at = ?2 WHERE id = ?1",
                params![original.id, DELETED_AT],
            )
            .unwrap();
        connection
            .execute(
                "UPDATE cards SET deleted_at = ?2, updated_at = ?2 WHERE note_id = ?1",
                params![original.id, DELETED_AT],
            )
            .unwrap();
        finalize(&connection, "cards", None, &[original]);
        let merges = merge_count(&connection);

        purge_finalized_in_connection(&mut connection).unwrap();

        // The copies in both bundles went with the original, reviews included.
        for note in [original, &library.cm_bio[0], &library.cm_phys[0]] {
            assert!(!exists(&connection, "notes", "id", &note.id));
            assert!(!exists(
                &connection,
                "review_events",
                "card_id",
                note.card()
            ));
        }
        assert_eq!(merge_count(&connection), merges - 2);
        assert!(exists(&connection, "notes", "id", &library.cm_phys[2].id));
    }

    #[test]
    fn purging_a_deck_keeps_a_retired_copy_whose_original_moved_away() {
        let (mut connection, library) = upgraded();
        // After the upgrade the learner moved the first Biology card to Misc and
        // deleted Cell biology. The copy retired into it stayed behind.
        connection
            .execute(
                "UPDATE notes SET deck_id = ?2 WHERE id = ?1",
                params![library.o_bio[0].id, library.k_misc],
            )
            .unwrap();
        connection
            .execute(
                "UPDATE cards SET deck_id = ?2 WHERE note_id = ?1",
                params![library.o_bio[0].id, library.k_misc],
            )
            .unwrap();
        for sql in [
            "UPDATE decks SET deleted_at = ?2 WHERE id = ?1",
            "UPDATE notes SET deleted_at = ?2 WHERE deck_id = ?1 AND deleted_at IS NULL",
            "UPDATE cards SET deleted_at = ?2 WHERE deck_id = ?1 AND deleted_at IS NULL",
        ] {
            connection
                .execute(sql, params![library.k_bio, DELETED_AT])
                .unwrap();
        }
        finalize(&connection, "deck", Some(&library.k_bio), &[]);

        purge_finalized_in_connection(&mut connection).unwrap();

        assert!(!exists(&connection, "decks", "id", &library.k_bio));
        // The copy and its review now live with the original, in Misc.
        let copy = &library.cb[0];
        let deck: String = connection
            .query_row(
                "SELECT deck_id FROM cards WHERE id = ?1",
                [copy.card()],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(deck, library.k_misc);
        assert!(exists(&connection, "review_events", "card_id", copy.card()));
        assert!(exists(
            &connection,
            "card_merges",
            "retired_card_id",
            copy.card()
        ));
        // Copies retired into originals in the purged deck went with them.
        for note in [&library.cb[1], &library.cq] {
            assert!(!exists(&connection, "notes", "id", &note.id));
            assert!(!exists(
                &connection,
                "card_merges",
                "retired_card_id",
                note.card()
            ));
        }
    }
}
