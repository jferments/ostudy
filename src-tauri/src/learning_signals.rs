//! Deterministic learning signals shared by workload selection and card search.

use rusqlite::{params, Connection};

use crate::error::CommandError;

/// A reset creates a new current-learning epoch without deleting historical
/// review evidence. Only reviews after the latest reset participate here.
pub fn needs_more_practice(connection: &Connection, card_id: &str) -> Result<bool, CommandError> {
    let reset_at: Option<i64> = connection
        .query_row(
            "SELECT MAX(occurred_at) FROM schedule_action_events
             WHERE card_id = ?1 AND action = 'reset'",
            [card_id],
            |row| row.get(0),
        )
        .map_err(CommandError::database)?;

    let mut statement = connection
        .prepare(
            "SELECT answer_correct
             FROM review_events
             WHERE card_id = ?1
               AND reviewed_at > ?2
             ORDER BY reviewed_at DESC, created_at DESC
             LIMIT 5",
        )
        .map_err(CommandError::database)?;
    let outcomes = statement
        .query_map(params![card_id, reset_at.unwrap_or(-1)], |row| {
            row.get::<_, i64>(0)
        })
        .map_err(CommandError::database)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(CommandError::database)?;

    Ok(needs_more_practice_from_recent(&outcomes))
}

pub fn count_needing_more_practice(
    connection: &Connection,
    deck_ids: &[String],
) -> Result<i64, CommandError> {
    let mut sql = String::from(
        "SELECT c.id
         FROM cards c
         JOIN notes n ON n.id = c.note_id
         JOIN decks d ON d.id = c.deck_id
         WHERE c.deleted_at IS NULL
           AND c.suspended_at IS NULL
           AND n.deleted_at IS NULL
           AND d.deleted_at IS NULL
           AND d.archived_at IS NULL",
    );
    let mut values: Vec<String> = Vec::new();
    if !deck_ids.is_empty() {
        let placeholders = deck_ids
            .iter()
            .map(|deck_id| {
                values.push(deck_id.clone());
                format!("?{}", values.len())
            })
            .collect::<Vec<_>>()
            .join(", ");
        sql.push_str(&format!(" AND c.deck_id IN ({placeholders})"));
    }
    let mut statement = connection.prepare(&sql).map_err(CommandError::database)?;
    let cards = statement
        .query_map(rusqlite::params_from_iter(values.iter()), |row| {
            row.get::<_, String>(0)
        })
        .map_err(CommandError::database)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(CommandError::database)?;
    let mut count = 0_i64;
    for card_id in cards {
        if needs_more_practice(connection, &card_id)? {
            count += 1;
        }
    }
    Ok(count)
}

fn needs_more_practice_from_recent(recent_descending: &[i64]) -> bool {
    if recent_descending.len() >= 2 && recent_descending[0] == 1 && recent_descending[1] == 1 {
        return false;
    }
    if recent_descending.len() >= 2 && recent_descending[0] == 0 && recent_descending[1] == 0 {
        return true;
    }
    recent_descending
        .iter()
        .take(5)
        .filter(|value| **value == 0)
        .count()
        >= 3
}

#[cfg(test)]
mod tests {
    use super::needs_more_practice_from_recent;

    #[test]
    fn repeated_wrong_answers_require_more_practice() {
        assert!(needs_more_practice_from_recent(&[0, 0]));
        assert!(needs_more_practice_from_recent(&[0, 1, 0, 1, 0]));
    }

    #[test]
    fn two_remembered_answers_clear_the_signal() {
        assert!(!needs_more_practice_from_recent(&[1, 1, 0, 0]));
        assert!(!needs_more_practice_from_recent(&[1, 0]));
    }
}
