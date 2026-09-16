use super::*;
use crate::cards::questions::ValidatedQuestion;

pub(super) fn search_cards_in_connection(
    connection: &Connection,
    input: SearchCardsInput,
) -> Result<CardSearchPageDto, CommandError> {
    let time = StudyTimeContext::capture()?;
    let mut search = build_search_sql(&input, &time)?;
    let count_sql = format!(
        "WITH {signals}
         SELECT COUNT(*)
         FROM notes n
         JOIN cards c ON c.note_id = n.id
         JOIN decks d ON d.id = c.deck_id
         JOIN card_schedules s ON s.card_id = c.id
         {joins}
         WHERE {where_sql}",
        signals = signal_projection::LEARNING_SIGNAL_CTES,
        joins = search.joins,
        where_sql = search.where_sql,
    );
    let total_count: i64 = connection
        .query_row(&count_sql, params_from_iter(search.values.iter()), |row| {
            row.get(0)
        })
        .map_err(CommandError::database)?;
    let limit_index = push_value(&mut search.values, input.limit);
    let offset_index = push_value(&mut search.values, input.offset);
    let query_sql = format!(
        "WITH {signals}
         SELECT
             c.id, n.id, c.deck_id, d.name,
             COALESCE((
                 SELECT group_concat(t.name, char(31))
                 FROM note_tags nt
                 JOIN tags t ON t.id = nt.tag_id
                 WHERE nt.note_id = n.id
                 ORDER BY t.name COLLATE NOCASE, t.id
             ), ''),
             s.due_at,
             CASE WHEN s.reps > 0 THEN s.last_review_at ELSE NULL END,
             s.reps, c.suspended_at, c.buried_until, n.created_at, n.updated_at,
             c.direction, c.template_ordinal, n.note_type,
             n.front_doc, n.back_doc, n.front_text, n.back_text,
             n.cloze_source, COALESCE(n.cloze_mode, 'one_at_a_time'),
             {signal_value}, n.question_definition, qp.question_key
         FROM notes n
         JOIN cards c ON c.note_id = n.id
         JOIN decks d ON d.id = c.deck_id
         JOIN card_schedules s ON s.card_id = c.id
         LEFT JOIN card_question_presentations qp ON qp.card_id = c.id
         {joins}
         WHERE {where_sql}
         ORDER BY {order_sql}
         LIMIT ?{limit_index} OFFSET ?{offset_index}",
        signals = signal_projection::LEARNING_SIGNAL_CTES,
        signal_value = signal_projection::value_sql(),
        joins = search.joins,
        where_sql = search.where_sql,
        order_sql = search.order_sql,
    );
    let mut statement = connection
        .prepare(&query_sql)
        .map_err(CommandError::database)?;
    let cards = statement
        .query_map(params_from_iter(search.values.iter()), |row| {
            let direction: String = row.get(12)?;
            let template_ordinal: i64 = row.get(13)?;
            let note_type: String = row.get(14)?;
            let source_front_text: String = row.get(17)?;
            let source_back_text: String = row.get(18)?;
            let reps: i64 = row.get(7)?;
            let (card_type, card_label, front_text, back_text) = if note_type == "interactive" {
                let definition_json: Option<String> = row.get(22)?;
                let presentation_key: Option<String> = row.get(23)?;
                let definition_json = definition_json.ok_or_else(|| {
                    projection_error(
                        22,
                        "An interactive question is missing its canonical definition.",
                    )
                })?;
                let presentation_key = presentation_key.ok_or_else(|| {
                    projection_error(
                        23,
                        "An interactive question is missing its presentation identity.",
                    )
                })?;
                let question = ValidatedQuestion::parse(&definition_json)
                    .map_err(|error| projection_error(22, &error.message))?;
                let prompt = question
                    .prompt_text(&presentation_key)
                    .map_err(|error| projection_error(23, &error.message))?;
                (
                    question.kind().as_str().to_string(),
                    question.kind().label().to_string(),
                    prompt,
                    source_back_text.clone(),
                )
            } else {
                let front_doc_json: Option<String> = row.get(15)?;
                let back_doc_json: Option<String> = row.get(16)?;
                let cloze_source: String = row.get(19)?;
                let cloze_mode: String = row.get(20)?;
                let rendered = render_projected_presentation(ProjectedPresentationInput {
                    direction: &direction,
                    template_ordinal,
                    note_type: &note_type,
                    front_doc_json: front_doc_json.as_deref().ok_or_else(|| {
                        projection_error(15, "A recall card is missing its question document.")
                    })?,
                    back_doc_json: back_doc_json.as_deref().ok_or_else(|| {
                        projection_error(16, "A recall card is missing its answer document.")
                    })?,
                    front_text: &source_front_text,
                    back_text: &source_back_text,
                    cloze_source: &cloze_source,
                    cloze_mode: &cloze_mode,
                    reps,
                    presentation_variant: None,
                })
                .map_err(|error| projection_error(14, &error.message))?;
                (
                    rendered.card_type,
                    rendered.card_label,
                    rendered.front_text,
                    rendered.back_text,
                )
            };
            let grouped_tags: String = row.get(4)?;
            Ok(CardBrowserRowDto {
                card_id: row.get(0)?,
                note_id: row.get(1)?,
                deck_id: row.get(2)?,
                deck_name: row.get(3)?,
                card_type,
                card_label,
                front_text,
                back_text,
                tags: split_tags(&grouped_tags),
                due_at: row.get(5)?,
                last_reviewed_at: row.get(6)?,
                review_count: reps,
                suspended_at: row.get(8)?,
                buried_until: row.get(9)?,
                needs_more_practice: row.get::<_, i64>(21)? != 0,
                created_at: row.get(10)?,
                updated_at: row.get(11)?,
            })
        })
        .map_err(CommandError::database)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(CommandError::database)?;
    Ok(CardSearchPageDto { cards, total_count })
}

fn projection_error(column: usize, message: &str) -> rusqlite::Error {
    rusqlite::Error::FromSqlConversionFailure(
        column,
        rusqlite::types::Type::Text,
        Box::new(std::io::Error::other(message.to_string())),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{database::Database, scheduler::insert_new_schedule};
    use serde_json::json;

    fn id(value: u32) -> String {
        format!("{value:032x}")
    }

    fn rich(text: &str) -> Value {
        json!({"type":"doc","content":[{"type":"paragraph","content":[{"type":"text","text":text}]}]})
    }

    #[test]
    fn interactive_rows_use_typed_metadata_without_fake_front_back_documents() {
        let database = Database::open_in_memory().unwrap();
        database
            .with_connection(|connection| {
                connection
                    .execute(
                        "INSERT INTO decks (id, name, created_at, updated_at) VALUES (?1, 'Questions', 1, 1)",
                        [id(1)],
                    )
                    .map_err(CommandError::database)?;
                let definition = json!({
                    "schemaVersion":1,
                    "question":{
                        "kind":"single_choice",
                        "prompt":rich("Which ion is extracellular?"),
                        "options":[
                            {"id":id(10),"content":rich("Potassium"),"rationale":null},
                            {"id":id(11),"content":rich("Sodium"),"rationale":null}
                        ],
                        "correctOptionId":id(11),
                        "explanation":null
                    }
                });
                connection
                    .execute(
                        "INSERT INTO notes (
                        id, deck_id, note_type, content_format, content_schema_version,
                        question_definition, front_text, back_text, duplicate_hash,
                        created_at, updated_at, revision
                     ) VALUES (?1, ?2, 'interactive', 'question-json', 1, ?3, ?4, ?5, '', 1, 1, 1)",
                        params![
                            id(2),
                            id(1),
                            definition.to_string(),
                            "Which ion is extracellular?",
                            "Sodium"
                        ],
                    )
                    .map_err(CommandError::database)?;
                connection
                    .execute(
                        "INSERT INTO cards (id, note_id, deck_id, direction, template_ordinal, created_at, updated_at)
                     VALUES (?1, ?2, ?3, 'forward', 0, 1, 1)",
                        params![id(3), id(2), id(1)],
                    )
                    .map_err(CommandError::database)?;
                insert_new_schedule(connection, &id(3), 1)?;
                connection
                    .execute(
                        "INSERT INTO card_question_presentations (card_id, note_id, question_key)
                     VALUES (?1, ?2, 'main')",
                        params![id(3), id(2)],
                    )
                    .map_err(CommandError::database)?;
                let page = search_cards_in_connection(
                    connection,
                    SearchCardsInput {
                        deck_ids: vec![id(1)],
                        query: String::new(),
                        tag_ids: vec![],
                        availability_filter: "all".to_string(),
                        status_filter: "active".to_string(),
                        reviewed_filter: "all".to_string(),
                        sort: "updated_desc".to_string(),
                        limit: 20,
                        offset: 0,
                    },
                )?;
                assert_eq!(page.total_count, 1);
                assert_eq!(page.cards[0].card_type, "single_choice");
                assert_eq!(page.cards[0].card_label, "Single-best-answer");
                assert_eq!(page.cards[0].front_text, "Which ion is extracellular?");
                assert_eq!(page.cards[0].back_text, "Sodium");
                Ok(())
            })
            .unwrap();
    }
}
