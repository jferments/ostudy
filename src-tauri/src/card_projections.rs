//! Bounded deck-card list projections.
use crate::{database::Database, error::CommandError, util::validate_entity_id};
use rusqlite::{params_from_iter, types::Value as SqlValue};
use serde::{Deserialize, Serialize};
use tauri::State;
const MAX_PAGE: i64 = 100;
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DeckCardPageInput {
    pub deck_id: String,
    #[serde(default)]
    pub query: String,
    #[serde(default = "default_limit")]
    pub limit: i64,
    #[serde(default)]
    pub offset: i64,
}
fn default_limit() -> i64 {
    50
}
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeckCardSummaryDto {
    pub note_id: String,
    pub primary_card_id: String,
    pub question_excerpt: String,
    pub answer_excerpt: String,
    pub generated_card_count: i64,
    pub image_count: i64,
    pub updated_at: i64,
}
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeckCardPageDto {
    pub cards: Vec<DeckCardSummaryDto>,
    pub total_count: i64,
}
#[tauri::command]
pub fn list_deck_cards(
    database: State<'_, Database>,
    input: DeckCardPageInput,
) -> Result<DeckCardPageDto, CommandError> {
    validate_entity_id(&input.deck_id, "deck")?;
    if !(1..=MAX_PAGE).contains(&input.limit)
        || input.offset < 0
        || input.query.chars().count() > 500
    {
        return Err(CommandError::new(
            "invalid_card_page",
            "The requested deck-card page is invalid.",
        ));
    }
    database.with_connection(|c|{let mut values=vec![SqlValue::Text(input.deck_id.clone())];let mut filter="".to_string();if !input.query.trim().is_empty(){values.push(SqlValue::Text(format!("%{}%",input.query.trim().replace('%',"\\%").replace('_',"\\_"))));filter=format!(" AND (n.front_text LIKE ?{} ESCAPE '\\' OR n.back_text LIKE ?{} ESCAPE '\\')",values.len(),values.len());}let count_sql=format!("SELECT COUNT(*) FROM notes n JOIN decks d ON d.id=n.deck_id WHERE n.deck_id=?1 AND n.deleted_at IS NULL AND d.deleted_at IS NULL AND d.archived_at IS NULL{filter}");let total=c.query_row(&count_sql,params_from_iter(values.iter()),|r|r.get(0)).map_err(CommandError::database)?;values.push(SqlValue::Integer(input.limit));let li=values.len();values.push(SqlValue::Integer(input.offset));let oi=values.len();let sql=format!("SELECT n.id,(SELECT id FROM cards pc WHERE pc.note_id=n.id AND pc.deleted_at IS NULL ORDER BY CASE pc.direction WHEN 'forward' THEN 0 ELSE 1 END,pc.template_ordinal,pc.id LIMIT 1),substr(n.front_text,1,300),substr(n.back_text,1,300),(SELECT COUNT(*) FROM cards gc WHERE gc.note_id=n.id AND gc.deleted_at IS NULL),(SELECT COUNT(DISTINCT na.asset_id) FROM note_assets na WHERE na.note_id=n.id),n.updated_at FROM notes n JOIN decks d ON d.id=n.deck_id WHERE n.deck_id=?1 AND n.deleted_at IS NULL AND d.deleted_at IS NULL AND d.archived_at IS NULL{filter} ORDER BY n.updated_at DESC,n.created_at DESC,n.id LIMIT ?{li} OFFSET ?{oi}");let mut st=c.prepare(&sql).map_err(CommandError::database)?;let cards=st.query_map(params_from_iter(values.iter()),|r|Ok(DeckCardSummaryDto{note_id:r.get(0)?,primary_card_id:r.get(1)?,question_excerpt:r.get(2)?,answer_excerpt:r.get(3)?,generated_card_count:r.get(4)?,image_count:r.get(5)?,updated_at:r.get(6)?})).map_err(CommandError::database)?.collect::<Result<Vec<_>,_>>().map_err(CommandError::database)?;Ok(DeckCardPageDto{cards,total_count:total})})
}
