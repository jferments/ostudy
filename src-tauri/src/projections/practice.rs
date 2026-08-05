use rusqlite::{params, Connection, OptionalExtension};

use crate::{
    error::CommandError,
    projections::learning_signals::{value_sql, LEARNING_SIGNALS_CTE},
};

pub const MAX_PRACTICE_WINDOW: i64 = 50;
pub const DEFAULT_PRACTICE_WINDOW: i64 = 20;

#[derive(Clone, Debug)]
pub struct SessionWindowRow {
    pub position: i64,
    pub card_id: String,
    pub presentation_variant: String,
    pub deck_id: String,
    pub deck_name: String,
    pub due_at: i64,
    pub state: i64,
    pub needs_more_practice: bool,
}

pub fn session_card_count(
    connection: &Connection,
    session_id: &str,
) -> Result<i64, CommandError> {
    connection
        .query_row(
            "SELECT COUNT(*) FROM practice_session_cards WHERE session_id = ?1",
            [session_id],
            |row| row.get(0),
        )
        .map_err(CommandError::database)
}

pub fn session_window(
    connection: &Connection,
    session_id: &str,
    start_position: i64,
    limit: i64,
) -> Result<Vec<SessionWindowRow>, CommandError> {
    if start_position < 0 || !(1..=MAX_PRACTICE_WINDOW).contains(&limit) {
        return Err(CommandError::new(
            "invalid_practice_window",
            format!("Practice windows contain 1 to {MAX_PRACTICE_WINDOW} cards."),
        ));
    }
    let weak = value_sql("psc.card_id");
    let sql = format!(
        "WITH {LEARNING_SIGNALS_CTE}
         SELECT psc.position, psc.card_id, psc.presentation_variant,
                c.deck_id, d.name, s.due_at, s.state, {weak}
         FROM practice_session_cards psc
         JOIN cards c ON c.id = psc.card_id
         JOIN decks d ON d.id = c.deck_id
         JOIN card_schedules s ON s.card_id = c.id
         WHERE psc.session_id = ?1
           AND psc.position >= ?2
         ORDER BY psc.position
         LIMIT ?3"
    );
    let mut statement = connection.prepare(&sql).map_err(CommandError::database)?;
    statement
        .query_map(params![session_id, start_position, limit], |row| {
            Ok(SessionWindowRow {
                position: row.get(0)?,
                card_id: row.get(1)?,
                presentation_variant: row.get(2)?,
                deck_id: row.get(3)?,
                deck_name: row.get(4)?,
                due_at: row.get(5)?,
                state: row.get(6)?,
                needs_more_practice: row.get::<_, i64>(7)? != 0,
            })
        })
        .map_err(CommandError::database)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(CommandError::database)
}

pub fn session_exists(
    connection: &Connection,
    session_id: &str,
) -> Result<bool, CommandError> {
    connection
        .query_row(
            "SELECT 1 FROM practice_sessions WHERE id = ?1",
            [session_id],
            |_| Ok(true),
        )
        .optional()
        .map(|value| value.unwrap_or(false))
        .map_err(CommandError::database)
}
