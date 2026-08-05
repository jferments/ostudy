use rusqlite::{params, Connection, OptionalExtension};

use crate::{
    error::CommandError,
    study_calendar::StudyTimeContext,
    studyability::studyable_due_sql,
};

#[derive(Clone, Debug)]
pub struct DeckSummaryRow {
    pub id: String,
    pub name: String,
    pub description: String,
    pub source_note_count: i64,
    pub generated_card_count: i64,
    pub studyable_due_count: i64,
    pub archived_at: Option<i64>,
    pub created_at: i64,
    pub updated_at: i64,
    pub revision: i64,
}

pub fn list(
    connection: &Connection,
    time: &StudyTimeContext,
    include_archived: bool,
) -> Result<Vec<DeckSummaryRow>, CommandError> {
    let archived_condition = if include_archived {
        ""
    } else {
        "AND d.archived_at IS NULL"
    };
    let sql = query_sql(archived_condition, "");
    let mut statement = connection.prepare(&sql).map_err(CommandError::database)?;
    statement
        .query_map([time.now_utc_millis], row)
        .map_err(CommandError::database)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(CommandError::database)
}

pub fn get(
    connection: &Connection,
    time: &StudyTimeContext,
    deck_id: &str,
    include_archived: bool,
) -> Result<DeckSummaryRow, CommandError> {
    let archived_condition = if include_archived {
        ""
    } else {
        "AND d.archived_at IS NULL"
    };
    let sql = query_sql(archived_condition, "AND d.id = ?2");
    connection
        .query_row(&sql, params![time.now_utc_millis, deck_id], row)
        .optional()
        .map_err(CommandError::database)?
        .ok_or_else(|| CommandError::new("deck_not_found", "That deck no longer exists."))
}

fn query_sql(archived_condition: &str, id_condition: &str) -> String {
    let ready = studyable_due_sql("c", "n", "d", "s", "?1");
    format!(
        "SELECT
             d.id, d.name, d.description,
             COUNT(DISTINCT CASE
                 WHEN n.deleted_at IS NULL AND c.deleted_at IS NULL THEN n.id
             END) AS source_note_count,
             COUNT(CASE
                 WHEN n.deleted_at IS NULL AND c.deleted_at IS NULL THEN c.id
             END) AS generated_card_count,
             COALESCE(SUM(CASE WHEN {ready} THEN 1 ELSE 0 END), 0)
                 AS studyable_due_count,
             d.archived_at, d.created_at, d.updated_at, d.revision
         FROM decks d
         LEFT JOIN cards c ON c.deck_id = d.id
         LEFT JOIN notes n ON n.id = c.note_id
         LEFT JOIN card_schedules s ON s.card_id = c.id
         WHERE d.deleted_at IS NULL {archived_condition} {id_condition}
         GROUP BY d.id, d.name, d.description, d.archived_at,
                  d.created_at, d.updated_at, d.revision, d.sort_order
         ORDER BY d.archived_at IS NOT NULL, d.sort_order ASC,
                  d.name COLLATE NOCASE ASC, d.created_at ASC"
    )
}

fn row(row: &rusqlite::Row<'_>) -> rusqlite::Result<DeckSummaryRow> {
    Ok(DeckSummaryRow {
        id: row.get(0)?,
        name: row.get(1)?,
        description: row.get(2)?,
        source_note_count: row.get(3)?,
        generated_card_count: row.get(4)?,
        studyable_due_count: row.get(5)?,
        archived_at: row.get(6)?,
        created_at: row.get(7)?,
        updated_at: row.get(8)?,
        revision: row.get(9)?,
    })
}
