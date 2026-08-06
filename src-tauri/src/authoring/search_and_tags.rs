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
    let time = StudyTimeContext::capture()?;
    let mut values = vec![SqlValue::Integer(time.now_utc_millis)];
    let studyable_due = studyability::studyable_due_sql("?1");
    let mut sql = format!(
        "SELECT
             t.id,
             t.name,
             COUNT(DISTINCT n.id),
             COUNT(DISTINCT CASE WHEN c.deleted_at IS NULL THEN c.id END),
             COALESCE(SUM(CASE WHEN {studyable_due} THEN 1 ELSE 0 END), 0)
         FROM tags t
         JOIN note_tags nt ON nt.tag_id = t.id
         JOIN notes n ON n.id = nt.note_id
         JOIN decks d ON d.id = n.deck_id
         LEFT JOIN cards c ON c.note_id = n.id
         LEFT JOIN card_schedules s ON s.card_id = c.id
         WHERE n.deleted_at IS NULL
           AND d.deleted_at IS NULL
           AND d.archived_at IS NULL"
    );
    if !deck_ids.is_empty() {
        let placeholders = deck_ids
            .iter()
            .map(|deck_id| format!("?{}", push_value(&mut values, deck_id.clone())))
            .collect::<Vec<_>>()
            .join(", ");
        sql.push_str(&format!(" AND n.deck_id IN ({placeholders})"));
    }
    sql.push_str(
        " GROUP BY t.id, t.name
          HAVING COUNT(DISTINCT n.id) > 0
          ORDER BY t.name COLLATE NOCASE ASC, t.id ASC",
    );
    let mut statement = connection.prepare(&sql).map_err(CommandError::database)?;
    let tags = statement
        .query_map(params_from_iter(values.iter()), tag_from_row)
        .map_err(CommandError::database)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(CommandError::database)?;
    Ok(tags)
}

fn tag_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<TagDto> {
    Ok(TagDto {
        id: row.get(0)?,
        name: row.get(1)?,
        source_note_count: row.get(2)?,
        generated_card_count: row.get(3)?,
        studyable_due_count: row.get(4)?,
    })
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
            "incompatible_search_filters",
            "Ready-now and scheduled-later filters apply only to active studyable cards.",
        ));
    }
    if !matches!(
        input.reviewed_filter.as_str(),
        "all"
            | "never"
            | "today"
            | "past_7_local_dates"
            | "past_30_local_dates"
            | "before_past_30_local_dates"
    ) {
        return Err(CommandError::new(
            "invalid_reviewed_filter",
            "Choose one of the available local-date study filters.",
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
    time: &StudyTimeContext,
) -> Result<SearchSql, CommandError> {
    let fts_query = build_fts_query(&input.query);
    let mut joins = String::new();
    if fts_query.is_some() {
        joins.push_str(" JOIN card_search_fts ON card_search_fts.rowid = n.rowid");
    }
    joins.push(' ');
    joins.push_str(signal_projection::join_sql());

    let mut conditions = vec![studyability::current_card_sql("c", "n", "d").to_string()];
    let mut values: Vec<SqlValue> = Vec::new();

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

    match input.status_filter.as_str() {
        "active" => {
            let now = push_value(&mut values, time.now_utc_millis);
            conditions.push(studyability::studyable_card_sql(&format!("?{now}")));
        }
        "suspended" => conditions.push("c.suspended_at IS NOT NULL".to_string()),
        "buried" => {
            let now = push_value(&mut values, time.now_utc_millis);
            conditions.push("c.suspended_at IS NULL".to_string());
            conditions.push("c.archive_suspended_at IS NULL".to_string());
            conditions.push(format!("c.buried_until > ?{now}"));
        }
        "needs_more_practice" => {
            let now = push_value(&mut values, time.now_utc_millis);
            conditions.push(studyability::studyable_card_sql(&format!("?{now}")));
            conditions.push(format!("{} = 1", signal_projection::value_sql()));
        }
        _ => {}
    }

    match input.availability_filter.as_str() {
        "ready_now" => {
            let now = push_value(&mut values, time.now_utc_millis);
            conditions.push(studyability::studyable_due_sql(&format!("?{now}")));
        }
        "scheduled_later" => {
            let now = push_value(&mut values, time.now_utc_millis);
            conditions.push(studyability::studyable_card_sql(&format!("?{now}")));
            conditions.push(format!("s.due_at > ?{now}"));
        }
        _ => {}
    }

    match input.reviewed_filter.as_str() {
        "never" => conditions.push(
            "NOT EXISTS (SELECT 1 FROM review_events reviewed WHERE reviewed.card_id = c.id)"
                .to_string(),
        ),
        "today" => {
            let date = push_value(&mut values, time.local_date.to_string());
            conditions.push(review_local_date_exists_sql(&format!("= ?{date}")));
        }
        "past_7_local_dates" => {
            let start = time.trailing_local_dates(7)?.start_local_date.to_string();
            let date = push_value(&mut values, start);
            conditions.push(review_local_date_exists_sql(&format!(">= ?{date}")));
        }
        "past_30_local_dates" => {
            let start = time.trailing_local_dates(30)?.start_local_date.to_string();
            let date = push_value(&mut values, start);
            conditions.push(review_local_date_exists_sql(&format!(">= ?{date}")));
        }
        "before_past_30_local_dates" => {
            let start = time.trailing_local_dates(30)?.start_local_date.to_string();
            let date = push_value(&mut values, start);
            conditions.push(format!(
                "(
                    SELECT MAX(review_attr.review_local_date)
                    FROM review_events reviewed
                    JOIN review_event_attributions review_attr
                      ON review_attr.review_event_id = reviewed.id
                    WHERE reviewed.card_id = c.id
                 ) < ?{date}"
            ));
        }
        _ => {}
    }

    let order_sql = match input.sort.as_str() {
        "relevance" if fts_query_is_present(&input.query) => {
            "bm25(card_search_fts) ASC, n.updated_at DESC, c.template_ordinal ASC"
        }
        "question_asc" => {
            "CASE WHEN n.note_type = 'basic' AND c.direction = 'reverse' THEN n.back_text ELSE n.front_text END COLLATE NOCASE ASC, n.updated_at DESC, c.template_ordinal ASC"
        }
        "due_asc" => "s.due_at ASC, n.updated_at DESC, c.template_ordinal ASC",
        "last_reviewed_desc" => {
            "CASE WHEN s.reps > 0 THEN 0 ELSE 1 END ASC, s.last_review_at DESC, n.updated_at DESC, c.template_ordinal ASC"
        }
        _ => "n.updated_at DESC, n.created_at DESC, c.template_ordinal ASC, c.direction ASC",
    };
    Ok(SearchSql {
        joins,
        where_sql: conditions.join(" AND "),
        values,
        order_sql,
    })
}

fn review_local_date_exists_sql(predicate: &str) -> String {
    format!(
        "EXISTS (
             SELECT 1
             FROM review_events reviewed
             JOIN review_event_attributions review_attr
               ON review_attr.review_event_id = reviewed.id
             WHERE reviewed.card_id = c.id
               AND review_attr.review_local_date {predicate}
         )"
    )
}

fn search_cards_in_connection(
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
             {signal_value}
         FROM notes n
         JOIN cards c ON c.note_id = n.id
         JOIN decks d ON d.id = c.deck_id
         JOIN card_schedules s ON s.card_id = c.id
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
            let front_doc_json: String = row.get(15)?;
            let back_doc_json: String = row.get(16)?;
            let source_front_text: String = row.get(17)?;
            let source_back_text: String = row.get(18)?;
            let cloze_source: String = row.get(19)?;
            let cloze_mode: String = row.get(20)?;
            let reps: i64 = row.get(7)?;
            let rendered = render_projected_presentation(ProjectedPresentationInput {
                direction: &direction,
                template_ordinal,
                note_type: &note_type,
                front_doc_json: &front_doc_json,
                back_doc_json: &back_doc_json,
                front_text: &source_front_text,
                back_text: &source_back_text,
                cloze_source: &cloze_source,
                cloze_mode: &cloze_mode,
                reps,
                presentation_variant: None,
            })
            .map_err(|error| {
                rusqlite::Error::FromSqlConversionFailure(
                    14,
                    rusqlite::types::Type::Text,
                    Box::new(std::io::Error::other(error.message)),
                )
            })?;
            let grouped_tags: String = row.get(4)?;
            Ok(CardBrowserRowDto {
                card_id: row.get(0)?,
                note_id: row.get(1)?,
                deck_id: row.get(2)?,
                deck_name: row.get(3)?,
                card_type: rendered.card_type,
                card_label: rendered.card_label,
                front_text: rendered.front_text,
                back_text: rendered.back_text,
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

fn list_matching_cards_in_connection(
    connection: &Connection,
    input: SearchCardsInput,
) -> Result<Vec<SelectionCardDto>, CommandError> {
    let time = StudyTimeContext::capture()?;
    let mut search = build_search_sql(&input, &time)?;
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
        signals = signal_projection::LEARNING_SIGNAL_CTES,
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
            format!("More than {MAX_SEARCH_SELECTION} cards match. Narrow the filters before selecting or practicing all results."),
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

fn hydrate_organized_card(
    connection: &Connection,
    paths: &AppPaths,
    note_id: &str,
) -> Result<OrganizedCardDto, CommandError> {
    let card = get_card_from_connection(connection, paths, note_id)?;
    let tags = load_tag_names(connection, note_id)?;
    let due_at = connection
        .query_row(
            "SELECT s.due_at
             FROM card_schedules s
             JOIN cards c ON c.id = s.card_id
             WHERE c.note_id = ?1
               AND c.deleted_at IS NULL
               AND c.direction = 'forward'",
            [note_id],
            |row| row.get(0),
        )
        .map_err(CommandError::database)?;
    Ok(organized_from_card(card, tags, due_at))
}

fn organized_from_card(card: CardDto, tags: Vec<String>, due_at: i64) -> OrganizedCardDto {
    OrganizedCardDto {
        id: card.id,
        note_id: card.note_id,
        deck_id: card.deck_id,
        direction: card.direction,
        front_doc: card.front_doc,
        back_doc: card.back_doc,
        front_text: card.front_text,
        back_text: card.back_text,
        assets: card.assets,
        tags,
        due_at,
        created_at: card.created_at,
        updated_at: card.updated_at,
        revision: card.revision,
    }
}

fn load_tag_names(connection: &Connection, note_id: &str) -> Result<Vec<String>, CommandError> {
    let mut statement = connection
        .prepare(
            "SELECT t.name
             FROM tags t
             JOIN note_tags nt ON nt.tag_id = t.id
             WHERE nt.note_id = ?1
             ORDER BY t.name COLLATE NOCASE ASC",
        )
        .map_err(CommandError::database)?;
    let tags = statement
        .query_map([note_id], |row| row.get(0))
        .map_err(CommandError::database)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(CommandError::database)?;
    Ok(tags)
}

fn validate_tags(values: Vec<String>) -> Result<Vec<ValidatedTag>, CommandError> {
    if values.len() > MAX_TAGS_PER_NOTE {
        return Err(CommandError::new(
            "too_many_tags",
            format!("A card can have at most {MAX_TAGS_PER_NOTE} tags."),
        ));
    }
    let mut seen = BTreeSet::new();
    let mut tags = Vec::new();
    for value in values {
        let name = collapse_whitespace(&value);
        if name.is_empty()
            || name.chars().count() > MAX_TAG_CHARS
            || name.chars().any(char::is_control)
        {
            return Err(CommandError::new(
                "invalid_tag",
                format!("Each tag must contain 1 to {MAX_TAG_CHARS} visible characters."),
            ));
        }
        let normalized = name.to_lowercase();
        if seen.insert(normalized.clone()) {
            tags.push(ValidatedTag { name, normalized });
        }
    }
    Ok(tags)
}

fn replace_note_tags(
    transaction: &Transaction<'_>,
    note_id: &str,
    tags: &[ValidatedTag],
    now: i64,
) -> Result<(), CommandError> {
    transaction
        .execute("DELETE FROM note_tags WHERE note_id = ?1", [note_id])
        .map_err(CommandError::database)?;
    for tag in tags {
        let existing: Option<String> = transaction
            .query_row(
                "SELECT id FROM tags WHERE normalized_name = ?1",
                [tag.normalized.as_str()],
                |row| row.get(0),
            )
            .optional()
            .map_err(CommandError::database)?;
        let tag_id = match existing {
            Some(id) => id,
            None => {
                let id = generate_id(transaction)?;
                transaction
                    .execute(
                        "INSERT INTO tags (id, name, normalized_name, created_at, updated_at)
                         VALUES (?1, ?2, ?3, ?4, ?4)",
                        params![id, tag.name, tag.normalized, now],
                    )
                    .map_err(CommandError::database)?;
                id
            }
        };
        transaction
            .execute(
                "INSERT INTO note_tags (note_id, tag_id) VALUES (?1, ?2)",
                params![note_id, tag_id],
            )
            .map_err(CommandError::database)?;
    }
    cleanup_unused_tags(transaction)
}

fn cleanup_unused_tags(connection: &Connection) -> Result<(), CommandError> {
    connection
        .execute(
            "DELETE FROM tags
             WHERE NOT EXISTS (
                 SELECT 1 FROM note_tags nt WHERE nt.tag_id = tags.id
             )",
            [],
        )
        .map_err(CommandError::database)?;
    Ok(())
}
