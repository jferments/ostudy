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
    database.with_connection(|c|{let mut st=c.prepare("SELECT ct.id,ct.name,ct.description,(SELECT COUNT(*) FROM card_template_fields f WHERE f.template_id=ct.id),(SELECT COUNT(*) FROM card_template_rules r WHERE r.template_id=ct.id),(SELECT COUNT(*) FROM card_template_assets a WHERE a.template_id=ct.id),ct.updated_at,ct.revision FROM card_templates ct WHERE ct.deleted_at IS NULL ORDER BY ct.name COLLATE NOCASE,ct.created_at,ct.id").map_err(CommandError::database)?;st.query_map([],|r|Ok(CardTemplateSummaryDto{id:r.get(0)?,name:r.get(1)?,description:r.get(2)?,field_count:r.get(3)?,generated_card_rule_count:r.get(4)?,image_count:r.get(5)?,updated_at:r.get(6)?,revision:r.get(7)?})).map_err(CommandError::database)?.collect::<Result<Vec<_>,_>>().map_err(CommandError::database)})
}
