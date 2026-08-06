use rusqlite::{Connection, OptionalExtension};

use crate::{error::CommandError, studyability};

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
    now_utc_millis: i64,
) -> Result<Vec<DeckSummaryRow>, CommandError> {
    let sql = summary_sql(None);
    let mut statement = connection.prepare(&sql).map_err(CommandError::database)?;
    let summaries = statement
        .query_map([now_utc_millis], row)
        .map_err(CommandError::database)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(CommandError::database)?;
    Ok(summaries)
}

pub fn get(
    connection: &Connection,
    deck_id: &str,
    now_utc_millis: i64,
) -> Result<Option<DeckSummaryRow>, CommandError> {
    let sql = summary_sql(Some("d.id = ?2"));
    connection
        .query_row(&sql, rusqlite::params![now_utc_millis, deck_id], row)
        .optional()
        .map_err(CommandError::database)
}

fn summary_sql(extra_condition: Option<&str>) -> String {
    let due = studyability::studyable_due_sql("?1");
    let extra = extra_condition
        .map(|condition| format!(" AND {condition}"))
        .unwrap_or_default();
    format!(
        "SELECT
             d.id,
             d.name,
             d.description,
             COUNT(DISTINCT CASE
                 WHEN n.deleted_at IS NULL THEN n.id
             END) AS source_note_count,
             COUNT(DISTINCT CASE
                 WHEN n.deleted_at IS NULL AND c.deleted_at IS NULL THEN c.id
             END) AS generated_card_count,
             COALESCE(SUM(CASE WHEN {due} THEN 1 ELSE 0 END), 0) AS studyable_due_count,
             d.archived_at,
             d.created_at,
             d.updated_at,
             d.revision
         FROM decks d
         LEFT JOIN notes n ON n.deck_id = d.id
         LEFT JOIN cards c ON c.note_id = n.id AND c.deck_id = d.id
         LEFT JOIN card_schedules s ON s.card_id = c.id
         WHERE d.deleted_at IS NULL{extra}
         GROUP BY d.id, d.name, d.description, d.archived_at,
                  d.created_at, d.updated_at, d.revision
         ORDER BY
             CASE WHEN d.archived_at IS NULL THEN 0 ELSE 1 END,
             d.sort_order ASC,
             d.name COLLATE NOCASE ASC,
             d.created_at ASC"
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
