//! Bounded deck-card summary projection and lazy organized-card detail entry point.

use rusqlite::{params_from_iter, types::Value as SqlValue};
use serde::{Deserialize, Serialize};
use tauri::State;

use crate::{
    authoring::{render_projected_presentation, ProjectedPresentationInput},
    database::Database,
    error::CommandError,
    study_calendar::StudyTimeContext,
    util::validate_entity_id,
};

const MAX_PAGE: i64 = 100;
const MAX_SEARCH_CHARS: usize = 500;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DeckCardPageInput {
    pub deck_id: String,
    #[serde(default)]
    pub query: String,
    #[serde(default = "default_sort")]
    pub sort: String,
    #[serde(default = "default_limit")]
    pub limit: i64,
    #[serde(default)]
    pub offset: i64,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeckCardSummaryDto {
    pub note_id: String,
    pub representative_card_id: String,
    pub card_type: String,
    pub card_label: String,
    pub question_text: String,
    pub answer_text: String,
    pub generated_card_count: i64,
    pub studyable_due_count: i64,
    pub last_reviewed_at: Option<i64>,
    pub updated_at: i64,
    pub revision: i64,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeckCardPageDto {
    pub cards: Vec<DeckCardSummaryDto>,
    pub total_count: i64,
    pub limit: i64,
    pub offset: i64,
}

#[tauri::command]
pub fn list_deck_cards(
    database: State<'_, Database>,
    input: DeckCardPageInput,
) -> Result<DeckCardPageDto, CommandError> {
    validate_entity_id(&input.deck_id, "deck")?;
    if input.query.chars().count() > MAX_SEARCH_CHARS {
        return Err(CommandError::new(
            "search_too_long",
            format!("Deck-card search cannot exceed {MAX_SEARCH_CHARS} characters."),
        ));
    }
    if !(1..=MAX_PAGE).contains(&input.limit) || input.offset < 0 {
        return Err(CommandError::new(
            "invalid_deck_card_page",
            format!("Deck-card pages must contain 1 to {MAX_PAGE} rows."),
        ));
    }
    if !matches!(
        input.sort.as_str(),
        "updated_desc" | "question_asc" | "due_asc"
    ) {
        return Err(CommandError::new(
            "invalid_deck_card_sort",
            "Choose recently updated, question, or due-date sorting.",
        ));
    }
    database.with_connection(|connection| {
        let context = StudyTimeContext::capture()?;
        let mut values = vec![
            SqlValue::Text(input.deck_id.clone()),
            SqlValue::Integer(context.now_utc_millis),
        ];
        let mut conditions = vec![
            "n.deck_id = ?1".to_string(),
            "n.deleted_at IS NULL".to_string(),
            "d.deleted_at IS NULL".to_string(),
            "d.archived_at IS NULL".to_string(),
            "c.deleted_at IS NULL".to_string(),
            "c.direction = 'forward'".to_string(),
            "c.template_ordinal = 0".to_string(),
        ];
        if !input.query.trim().is_empty() {
            values.push(SqlValue::Text(format!("%{}%", input.query.trim())));
            conditions.push(format!(
                "(n.front_text LIKE ?{} ESCAPE '\\'
                   OR n.back_text LIKE ?{} ESCAPE '\\')",
                values.len(),
                values.len()
            ));
        }
        let where_sql = conditions.join(" AND ");
        let total_count: i64 = connection
            .query_row(
                &format!(
                    "SELECT COUNT(DISTINCT n.id)
                     FROM notes n
                     JOIN decks d ON d.id = n.deck_id
                     JOIN cards c ON c.note_id = n.id
                     WHERE {where_sql}"
                ),
                params_from_iter(values.iter()),
                |row| row.get(0),
            )
            .map_err(CommandError::database)?;
        values.push(SqlValue::Integer(input.limit));
        let limit_index = values.len();
        values.push(SqlValue::Integer(input.offset));
        let offset_index = values.len();
        let order_sql = match input.sort.as_str() {
            "question_asc" => "n.front_text COLLATE NOCASE, n.updated_at DESC, n.id",
            "due_asc" => "MIN(s.due_at), n.updated_at DESC, n.id",
            _ => "n.updated_at DESC, n.created_at DESC, n.id",
        };
        let sql = format!(
            "SELECT n.id,
                    MIN(CASE WHEN c.direction = 'forward' THEN c.id END),
                    n.note_type, n.front_doc, n.back_doc,
                    n.front_text, n.back_text, n.cloze_source, n.cloze_mode,
                    COUNT(c.id),
                    COALESCE(SUM(CASE
                        WHEN c.suspended_at IS NULL
                         AND (c.buried_until IS NULL OR c.buried_until <= ?2)
                         AND s.due_at <= ?2 THEN 1 ELSE 0 END), 0),
                    MAX(CASE WHEN s.reps > 0 THEN s.last_review_at END),
                    n.updated_at, n.revision,
                    MIN(s.reps)
             FROM notes n
             JOIN decks d ON d.id = n.deck_id
             JOIN cards c ON c.note_id = n.id
             JOIN card_schedules s ON s.card_id = c.id
             WHERE {where_sql}
             GROUP BY n.id, n.note_type, n.front_doc, n.back_doc,
                      n.front_text, n.back_text, n.cloze_source, n.cloze_mode,
                      n.updated_at, n.revision
             ORDER BY {order_sql}
             LIMIT ?{limit_index} OFFSET ?{offset_index}"
        );
        let mut statement = connection.prepare(&sql).map_err(CommandError::database)?;
        let rows = statement
            .query_map(params_from_iter(values.iter()), |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, String>(5)?,
                    row.get::<_, String>(6)?,
                    row.get::<_, String>(7)?,
                    row.get::<_, String>(8)?,
                    row.get::<_, i64>(9)?,
                    row.get::<_, i64>(10)?,
                    row.get::<_, Option<i64>>(11)?,
                    row.get::<_, i64>(12)?,
                    row.get::<_, i64>(13)?,
                    row.get::<_, i64>(14)?,
                ))
            })
            .map_err(CommandError::database)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(CommandError::database)?;
        let cards = rows
            .into_iter()
            .map(
                |(
                    note_id,
                    representative_card_id,
                    note_type,
                    front_doc_json,
                    back_doc_json,
                    front_text,
                    back_text,
                    cloze_source,
                    cloze_mode,
                    generated_card_count,
                    studyable_due_count,
                    last_reviewed_at,
                    updated_at,
                    revision,
                    reps,
                )| {
                    let rendered = render_projected_presentation(ProjectedPresentationInput {
                        direction: "forward".to_string(),
                        template_ordinal: 0,
                        note_type,
                        front_doc_json,
                        back_doc_json,
                        front_text,
                        back_text,
                        cloze_source,
                        cloze_mode,
                        reps,
                        presentation_variant: None,
                    })?;
                    Ok(DeckCardSummaryDto {
                        note_id,
                        representative_card_id,
                        card_type: rendered.card_type,
                        card_label: rendered.card_label,
                        question_text: rendered.front_text,
                        answer_text: rendered.back_text,
                        generated_card_count,
                        studyable_due_count,
                        last_reviewed_at,
                        updated_at,
                        revision,
                    })
                },
            )
            .collect::<Result<Vec<_>, CommandError>>()?;
        Ok(DeckCardPageDto {
            cards,
            total_count,
            limit: input.limit,
            offset: input.offset,
        })
    })
}

fn default_sort() -> String {
    "updated_desc".to_string()
}

const fn default_limit() -> i64 {
    50
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pages_are_bounded() {
        assert_eq!(MAX_PAGE, 100);
        assert_eq!(default_limit(), 50);
    }
}
