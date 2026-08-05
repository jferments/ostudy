//! Bounded reusable-template library summaries.
use crate::{database::Database, error::CommandError};
use serde::Serialize;
use tauri::State;
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CardTemplateSummaryDto {
    pub id: String,
    pub name: String,
    pub description: String,
    pub field_count: i64,
    pub generated_card_rule_count: i64,
    pub image_count: i64,
    pub updated_at: i64,
    pub revision: i64,
}
#[tauri::command]
pub fn list_card_template_summaries(
    database: State<'_, Database>,
) -> Result<Vec<CardTemplateSummaryDto>, CommandError> {
    database.with_connection(|connection| {
        let mut statement = connection
            .prepare(
                "SELECT ct.id,ct.name,ct.description,
                    (SELECT COUNT(*) FROM card_template_fields f WHERE f.template_id=ct.id),
                    (SELECT COUNT(*) FROM card_template_rules r WHERE r.template_id=ct.id),
                    (SELECT COUNT(*) FROM card_template_assets a WHERE a.template_id=ct.id),
                    ct.updated_at,ct.revision
             FROM card_templates ct WHERE ct.deleted_at IS NULL
             ORDER BY ct.name COLLATE NOCASE,ct.created_at,ct.id",
            )
            .map_err(CommandError::database)?;
        let rows = statement
            .query_map([], |row| {
                Ok(CardTemplateSummaryDto {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    description: row.get(2)?,
                    field_count: row.get(3)?,
                    generated_card_rule_count: row.get(4)?,
                    image_count: row.get(5)?,
                    updated_at: row.get(6)?,
                    revision: row.get(7)?,
                })
            })
            .map_err(CommandError::database)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(CommandError::database)?;
        Ok(rows)
    })
}
