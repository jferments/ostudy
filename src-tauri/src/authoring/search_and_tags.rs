struct SearchSql {
    joins: String,
    where_sql: String,
    values: Vec<SqlValue>,
    order_sql: &'static str,
}

fn effective_deck_ids(deck_ids: &[String]) -> Result<Vec<String>, CommandError> {
    let mut unique = BTreeSet::new();
    for deck_id in deck_ids {
        validate_entity_id(deck_id, "deck")?;
        unique.insert(deck_id.clone());
    }
    if unique.len() > MAX_SEARCH_DECKS {
        return Err(CommandError::new(
            "too_many_deck_filters",
            format!("Filter by at most {MAX_SEARCH_DECKS} decks."),
        ));
    }
    Ok(unique.into_iter().collect())
}

fn list_tags_in_connection(
    connection: &Connection,
    deck_ids: &[String],
) -> Result<Vec<TagDto>, CommandError> {
    let context = crate::study_calendar::StudyTimeContext::capture()?;
    let mut values = vec![SqlValue::Integer(context.now_utc_millis)];
    let mut conditions = vec![
        "n.deleted_at IS NULL".to_string(),
        "d.deleted_at IS NULL".to_string(),
        "d.archived_at IS NULL".to_string(),
        "c.deleted_at IS NULL".to_string(),
    ];
    if !deck_ids.is_empty() {
        let placeholders = deck_ids
            .iter()
            .map(|deck_id| {
                values.push(SqlValue::Text(deck_id.clone()));
                format!("?{}", values.len())
            })
            .collect::<Vec<_>>()
            .join(", ");
        conditions.push(format!("n.deck_id IN ({placeholders})"));
    }
    let sql = format!(
        "SELECT t.id, t.name,
                COUNT(DISTINCT n.id),
                COUNT(DISTINCT c.id),
                COUNT(DISTINCT CASE
                    WHEN c.suspended_at IS NULL
                     AND (c.buried_until IS NULL OR c.buried_until <= ?1)
                     AND s.due_at <= ?1 THEN c.id END)
         FROM tags t
         JOIN note_tags nt ON nt.tag_id = t.id
         JOIN notes n ON n.id = nt.note_id
         JOIN decks d ON d.id = n.deck_id
         JOIN cards c ON c.note_id = n.id
         JOIN card_schedules s ON s.card_id = c.id
         WHERE {}
         GROUP BY t.id, t.name
         HAVING COUNT(DISTINCT n.id) > 0
         ORDER BY t.name COLLATE NOCASE, t.id",
        conditions.join(" AND ")
    );
    let mut statement = connection.prepare(&sql).map_err(CommandError::database)?;
    statement
        .query_map(params_from_iter(values.iter()), |row| {
            Ok(TagDto {
                id: row.get(0)?,
                name: row.get(1)?,
                source_note_count: row.get(2)?,
                generated_card_count: row.get(3)?,
                studyable_due_count: row.get(4)?,
            })
        })
        .map_err(CommandError::database)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(CommandError::database)
}

fn validate_search_input(input: &SearchCardsInput) -> Result<(), CommandError> {
    effective_deck_ids(&input.deck_ids)?;
    if input.query.chars().count() > MAX_SEARCH_QUERY_CHARS {
        return Err(CommandError::new(
            "search_too_long",
            format!("Search text cannot exceed {MAX_SEARCH_QUERY_CHARS} characters."),
        ));
    }
    if input.tag_ids.len() > MAX_SEARCH_TAGS {
        return Err(CommandError::new(
            "too_many_tag_filters",
            format!("Filter by at most {MAX_SEARCH_TAGS} tags."),
        ));
    }
    for tag_id in &input.tag_ids {
        validate_entity_id(tag_id, "tag")?;
    }
    if !matches!(
        input.availability_filter.as_str(),
        "all" | "ready_now" | "scheduled_later"
    ) {
        return Err(CommandError::new(
            "invalid_availability_filter",
            "Choose all cards, cards ready now, or cards scheduled later.",
        ));
    }
    if !matches!(
        input.status_filter.as_str(),
        "all" | "active" | "suspended" | "buried" | "needs_more_practice"
    ) {
        return Err(CommandError::new(
            "invalid_status_filter",
            "Choose active, suspended, buried, or Needs more practice cards.",
        ));
    }
    if input.availability_filter != "all"
        && matches!(input.status_filter.as_str(), "suspended" | "buried")
    {
        return Err(CommandError::new(
            "incompatible_card_filters",
            "Suspended or buried cards cannot also be ready now or scheduled later.",
        ));
    }
    if !matches!(
        input.reviewed_filter.as_str(),
        "all"
            | "never"
            | "today"
            | "past_7_dates"
            | "past_30_dates"
            | "before_30_dates"
    ) {
        return Err(CommandError::new(
            "invalid_reviewed_filter",
            "Choose one of the local-calendar study-recency filters.",
        ));
    }
    if !matches!(
        input.sort.as_str(),
        "relevance" | "updated_desc" | "question_asc" | "due_asc" | "last_reviewed_desc"
    ) {
        return Err(CommandError::new(
            "invalid_card_sort",
            "Choose one of the available card sort orders.",
        ));
    }
    if !(1..=MAX_SEARCH_LIMIT).contains(&input.limit) || input.offset < 0 {
        return Err(CommandError::new(
            "invalid_search_page",
            "The requested card-results page is invalid.",
        ));
    }
    Ok(())
}

fn build_search_sql(
    input: &SearchCardsInput,
    context: &crate::study_calendar::StudyTimeContext,
) -> Result<SearchSql, CommandError> {
    let fts_query = build_fts_query(&input.query);
    let mut joins = String::from(" LEFT JOIN learning_signals ls ON ls.card_id = c.id");
    if fts_query.is_some() {
        joins.push_str(" JOIN card_search_fts ON card_search_fts.rowid = n.rowid");
    }
    let mut conditions = vec![
        "c.deleted_at IS NULL".to_string(),
        "n.deleted_at IS NULL".to_string(),
        "d.deleted_at IS NULL".to_string(),
        "d.archived_at IS NULL".to_string(),
    ];
    let mut values = Vec::<SqlValue>::new();
    let deck_ids = effective_deck_ids(&input.deck_ids)?;
    if !deck_ids.is_empty() {
        let placeholders = deck_ids
            .into_iter()
            .map(|deck_id| format!("?{}", push_value(&mut values, deck_id)))
            .collect::<Vec<_>>()
            .join(", ");
        conditions.push(format!("c.deck_id IN ({placeholders})"));
    }
    if let Some(query) = fts_query {
        conditions.push(format!(
            "card_search_fts MATCH ?{}",
            push_value(&mut values, query)
        ));
    }
    for tag_id in &input.tag_ids {
        let index = push_value(&mut values, tag_id.clone());
        conditions.push(format!(
            "EXISTS (
                 SELECT 1 FROM note_tags filter_nt
                 WHERE filter_nt.note_id = n.id AND filter_nt.tag_id = ?{index}
             )"
        ));
    }
    let now_index = push_value(&mut values, context.now_utc_millis);
    match input.status_filter.as_str() {
        "active" => {
            conditions.push("c.suspended_at IS NULL".to_string());
            conditions.push(format!(
                "(c.buried_until IS NULL OR c.buried_until <= ?{now_index})"
            ));
        }
        "suspended" => conditions.push("c.suspended_at IS NOT NULL".to_string()),
        "buried" => {
            conditions.push("c.suspended_at IS NULL".to_string());
            conditions.push(format!("c.buried_until > ?{now_index}"));
        }
        "needs_more_practice" => {
            conditions.push("c.suspended_at IS NULL".to_string());
            conditions.push(format!(
                "(c.buried_until IS NULL OR c.buried_until <= ?{now_index})"
            ));
            conditions.push("COALESCE(ls.needs_more_practice, 0) = 1".to_string());
        }
        _ => {}
    }
    match input.availability_filter.as_str() {
        "ready_now" => {
            conditions.push("c.suspended_at IS NULL".to_string());
            conditions.push(format!(
                "(c.buried_until IS NULL OR c.buried_until <= ?{now_index})"
            ));
            conditions.push(format!("s.due_at <= ?{now_index}"));
        }
        "scheduled_later" => {
            conditions.push("c.suspended_at IS NULL".to_string());
            conditions.push(format!(
                "(c.buried_until IS NULL OR c.buried_until <= ?{now_index})"
            ));
            conditions.push(format!("s.due_at > ?{now_index}"));
        }
        _ => {}
    }
    let today = context.today_start_utc_millis;
    let seven = context.resolve_date_start(context.date_after_days(-6)?)?;
    let thirty = context.resolve_date_start(context.date_after_days(-29)?)?;
    match input.reviewed_filter.as_str() {
        "never" => conditions.push("s.reps = 0".to_string()),
        "today" => {
            conditions.push("s.reps > 0".to_string());
            conditions.push(format!(
                "s.last_review_at >= ?{}",
                push_value(&mut values, today)
            ));
        }
        "past_7_dates" => {
            conditions.push("s.reps > 0".to_string());
            conditions.push(format!(
                "s.last_review_at >= ?{}",
                push_value(&mut values, seven)
            ));
        }
        "past_30_dates" => {
            conditions.push("s.reps > 0".to_string());
            conditions.push(format!(
                "s.last_review_at >= ?{}",
                push_value(&mut values, thirty)
            ));
        }
        "before_30_dates" => {
            conditions.push("s.reps > 0".to_string());
            conditions.push(format!(
                "s.last_review_at < ?{}",
                push_value(&mut values, thirty)
            ));
        }
        _ => {}
    }
    let order_sql = match input.sort.as_str() {
        "relevance" if fts_query_is_present(&input.query) => {
            "bm25(card_search_fts) ASC, n.updated_at DESC, c.template_ordinal ASC"
        }
        "question_asc" => "n.front_text COLLATE NOCASE, n.updated_at DESC, c.template_ordinal ASC",
        "due_asc" => "s.due_at, n.updated_at DESC, c.template_ordinal ASC",
        "last_reviewed_desc" => {
            "CASE WHEN s.reps > 0 THEN 0 ELSE 1 END, s.last_review_at DESC, n.updated_at DESC, c.template_ordinal ASC"
        }
        _ => "n.updated_at DESC, n.created_at DESC, c.template_ordinal, c.direction",
    };
    Ok(SearchSql {
        joins,
        where_sql: conditions.join(" AND "),
        values,
        order_sql,
    })
}

fn search_cards_in_connection(
    connection: &Connection,
    input: SearchCardsInput,
) -> Result<CardSearchPageDto, CommandError> {
    let context = crate::study_calendar::StudyTimeContext::capture()?;
    let mut search = build_search_sql(&input, &context)?;
    let count_sql = format!(
        "WITH {signals}
         SELECT COUNT(*)
         FROM notes n
         JOIN cards c ON c.note_id = n.id
         JOIN decks d ON d.id = c.deck_id
         JOIN card_schedules s ON s.card_id = c.id
         {joins}
         WHERE {where_sql}",
        signals = crate::projections::learning_signals::LEARNING_SIGNAL_CTE,
        joins = search.joins,
        where_sql = search.where_sql,
    );
    let total_count = connection
        .query_row(
            &count_sql,
            params_from_iter(search.values.iter()),
            |row| row.get(0),
        )
        .map_err(CommandError::database)?;
    let limit_index = push_value(&mut search.values, input.limit);
    let offset_index = push_value(&mut search.values, input.offset);
    let query_sql = format!(
        "WITH {signals}
         SELECT c.id, n.id, c.deck_id, d.name,
                COALESCE((
                    SELECT group_concat(name, char(31))
                    FROM (
                        SELECT t.name AS name
                        FROM note_tags nt JOIN tags t ON t.id = nt.tag_id
                        WHERE nt.note_id = n.id
                        ORDER BY t.name COLLATE NOCASE
                    )
                ), ''),
                s.due_at,
                CASE WHEN s.reps > 0 THEN s.last_review_at ELSE NULL END,
                s.reps, c.suspended_at, c.buried_until,
                n.created_at, n.updated_at, n.note_type, c.direction,
                c.template_ordinal, n.front_doc, n.back_doc,
                n.front_text, n.back_text, n.cloze_source, n.cloze_mode,
                COALESCE(ls.needs_more_practice, 0)
         FROM notes n
         JOIN cards c ON c.note_id = n.id
         JOIN decks d ON d.id = c.deck_id
         JOIN card_schedules s ON s.card_id = c.id
         {joins}
         WHERE {where_sql}
         ORDER BY {order_sql}
         LIMIT ?{limit_index} OFFSET ?{offset_index}",
        signals = crate::projections::learning_signals::LEARNING_SIGNAL_CTE,
        joins = search.joins,
        where_sql = search.where_sql,
        order_sql = search.order_sql,
    );
    let mut statement = connection
        .prepare(&query_sql)
        .map_err(CommandError::database)?;
    let raw = statement
        .query_map(params_from_iter(search.values.iter()), |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, String>(4)?,
                row.get::<_, i64>(5)?,
                row.get::<_, Option<i64>>(6)?,
                row.get::<_, i64>(7)?,
                row.get::<_, Option<i64>>(8)?,
                row.get::<_, Option<i64>>(9)?,
                row.get::<_, i64>(10)?,
                row.get::<_, i64>(11)?,
                row.get::<_, String>(12)?,
                row.get::<_, String>(13)?,
                row.get::<_, i64>(14)?,
                row.get::<_, String>(15)?,
                row.get::<_, String>(16)?,
                row.get::<_, String>(17)?,
                row.get::<_, String>(18)?,
                row.get::<_, String>(19)?,
                row.get::<_, String>(20)?,
                row.get::<_, i64>(21)?,
            ))
        })
        .map_err(CommandError::database)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(CommandError::database)?;
    let cards = raw
        .into_iter()
        .map(
            |(
                card_id,
                note_id,
                deck_id,
                deck_name,
                tags,
                due_at,
                last_reviewed_at,
                review_count,
                suspended_at,
                buried_until,
                created_at,
                updated_at,
                note_type,
                direction,
                template_ordinal,
                front_doc_json,
                back_doc_json,
                front_text,
                back_text,
                cloze_source,
                cloze_mode,
                weak,
            )| {
                let rendered = render_projected_presentation(ProjectedPresentationInput {
                    direction,
                    template_ordinal,
                    note_type,
                    front_doc_json,
                    back_doc_json,
                    front_text,
                    back_text,
                    cloze_source,
                    cloze_mode,
                    reps: review_count,
                    presentation_variant: None,
                })?;
                Ok(CardBrowserRowDto {
                    card_id,
                    note_id,
                    deck_id,
                    deck_name,
                    card_type: rendered.card_type,
                    card_label: rendered.card_label,
                    front_text: rendered.front_text,
                    back_text: rendered.back_text,
                    tags: split_tags(&tags),
                    due_at,
                    last_reviewed_at,
                    review_count,
                    suspended_at,
                    buried_until,
                    needs_more_practice: weak != 0,
                    created_at,
                    updated_at,
                })
            },
        )
        .collect::<Result<Vec<_>, CommandError>>()?;
    Ok(CardSearchPageDto { cards, total_count })
}

fn list_matching_cards_in_connection(
    connection: &Connection,
    input: SearchCardsInput,
) -> Result<Vec<SelectionCardDto>, CommandError> {
    let context = crate::study_calendar::StudyTimeContext::capture()?;
    let mut search = build_search_sql(&input, &context)?;
    let maximum = i64::try_from(MAX_SEARCH_SELECTION + 1).unwrap_or(i64::MAX);
    let limit_index = push_value(&mut search.values, maximum);
    let sql = format!(
        "WITH {signals}
         SELECT c.id, n.id, c.deck_id
         FROM notes n
         JOIN cards c ON c.note_id = n.id
         JOIN decks d ON d.id = c.deck_id
         JOIN card_schedules s ON s.card_id = c.id
         {joins}
         WHERE {where_sql}
         ORDER BY {order_sql}
         LIMIT ?{limit_index}",
        signals = crate::projections::learning_signals::LEARNING_SIGNAL_CTE,
        joins = search.joins,
        where_sql = search.where_sql,
        order_sql = search.order_sql,
    );
    let mut statement = connection.prepare(&sql).map_err(CommandError::database)?;
    let cards = statement
        .query_map(params_from_iter(search.values.iter()), |row| {
            Ok(SelectionCardDto {
                card_id: row.get(0)?,
                note_id: row.get(1)?,
                deck_id: row.get(2)?,
            })
        })
        .map_err(CommandError::database)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(CommandError::database)?;
    if cards.len() > MAX_SEARCH_SELECTION {
        return Err(CommandError::new(
            "too_many_matching_cards",
            format!(
                "More than {MAX_SEARCH_SELECTION} cards match. Narrow the filters before selecting all results."
            ),
        ));
    }
    Ok(cards)
}

fn fts_query_is_present(query: &str) -> bool {
    build_fts_query(query).is_some()
}

fn push_value(values: &mut Vec<SqlValue>, value: impl Into<SqlValue>) -> usize {
    values.push(value.into());
    values.len()
}

fn build_fts_query(value: &str) -> Option<String> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    for character in value.chars() {
        if character.is_alphanumeric() || matches!(character, '_' | '-') {
            current.push(character);
            if current.chars().count() >= MAX_TAG_CHARS {
                tokens.push(std::mem::take(&mut current));
            }
        } else if !current.is_empty() {
            tokens.push(std::mem::take(&mut current));
        }
        if tokens.len() >= 20 {
            break;
        }
    }
    if !current.is_empty() && tokens.len() < 20 {
        tokens.push(current);
    }
    let quoted = tokens
        .into_iter()
        .filter(|token| !token.is_empty())
        .map(|token| format!("\"{}\"*", token.replace('"', "\"\"")))
        .collect::<Vec<_>>();
    (!quoted.is_empty()).then(|| quoted.join(" AND "))
}
