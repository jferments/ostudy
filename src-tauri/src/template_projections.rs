//! Bounded card-template summary projection.
//!
//! Full fields, rules, documents, and assets are loaded only after the user
//! opens one template through `get_card_template`.

use rusqlite::Connection;
use serde::Serialize;
use tauri::State;

use crate::{database::Database, error::CommandError};

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CardTemplateSummaryDto {
    pub id: String,
    pub name: String,
    pub description: String,
    pub field_count: i64,
    pub rule_count: i64,
    pub asset_count: i64,
    pub created_at: i64,
    pub updated_at: i64,
    pub revision: i64,
}

#[tauri::command]
pub fn list_card_template_summaries(
    database: State<'_, Database>,
) -> Result<Vec<CardTemplateSummaryDto>, CommandError> {
    database.with_connection(list_card_template_summaries_in_connection)
}

fn list_card_template_summaries_in_connection(
    connection: &mut Connection,
) -> Result<Vec<CardTemplateSummaryDto>, CommandError> {
    let mut statement = connection
        .prepare(
            "SELECT t.id, t.name, t.description,
                    COUNT(DISTINCT f.id), COUNT(DISTINCT r.id),
                    COUNT(DISTINCT a.asset_id),
                    t.created_at, t.updated_at, t.revision
             FROM card_templates t
             LEFT JOIN card_template_fields f ON f.template_id = t.id
             LEFT JOIN card_template_rules r ON r.template_id = t.id
             LEFT JOIN card_template_assets a ON a.template_id = t.id
             WHERE t.deleted_at IS NULL
             GROUP BY t.id, t.name, t.description,
                      t.created_at, t.updated_at, t.revision
             ORDER BY t.name COLLATE NOCASE, t.created_at, t.id",
        )
        .map_err(CommandError::database)?;
    statement
        .query_map([], |row| {
            Ok(CardTemplateSummaryDto {
                id: row.get(0)?,
                name: row.get(1)?,
                description: row.get(2)?,
                field_count: row.get(3)?,
                rule_count: row.get(4)?,
                asset_count: row.get(5)?,
                created_at: row.get(6)?,
                updated_at: row.get(7)?,
                revision: row.get(8)?,
            })
        })
        .map_err(CommandError::database)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(CommandError::database)
}
