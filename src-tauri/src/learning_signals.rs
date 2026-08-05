//! Deterministic learning signals shared by workload, search, and analytics.

use rusqlite::{params_from_iter, types::Value as SqlValue, Connection};

use crate::{
    error::CommandError,
    projections::{learning_signals as projection, scope::append_in_filter},
    studyability,
};

/// A reset creates a new current-learning epoch without deleting historical
/// review evidence. One SQL projection supplies the signal for the requested
/// card; list paths join the same projection instead of calling this per row.
pub fn needs_more_practice(connection: &Connection, card_id: &str) -> Result<bool, CommandError> {
    let sql = format!(
        "WITH {ctes}
         SELECT {value}
         FROM cards c
         {join}
         WHERE c.id = ?1",
        ctes = projection::LEARNING_SIGNAL_CTES,
        value = projection::value_sql(),
        join = projection::join_sql(),
    );
    connection
        .query_row(&sql, [card_id], |row| row.get::<_, i64>(0))
        .map(|value| value != 0)
        .map_err(CommandError::database)
}

pub fn count_needing_more_practice(
    connection: &Connection,
    deck_ids: &[String],
) -> Result<i64, CommandError> {
    let mut conditions = vec![studyability::current_card_sql("c", "n", "d").to_string()];
    let mut values = Vec::<SqlValue>::new();
    append_in_filter(&mut conditions, &mut values, "c.deck_id", deck_ids);
    let sql = format!(
        "WITH {ctes}
         SELECT COALESCE(SUM(CASE WHEN {value} = 1 THEN 1 ELSE 0 END), 0)
         FROM cards c
         JOIN notes n ON n.id = c.note_id
         JOIN decks d ON d.id = c.deck_id
         {join}
         WHERE {conditions}",
        ctes = projection::LEARNING_SIGNAL_CTES,
        value = projection::value_sql(),
        join = projection::join_sql(),
        conditions = conditions.join(" AND "),
    );
    connection
        .query_row(&sql, params_from_iter(values.iter()), |row| row.get(0))
        .map_err(CommandError::database)
}

#[cfg(test)]
mod tests {
    use crate::projections::learning_signals::needs_more_practice_from_recent;

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
