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
    let mut values: Vec<SqlValue> = Vec::new();
    let mut sql = String::from(
        "SELECT t.id, t.name, COUNT(DISTINCT c.id)
         FROM tags t
         JOIN note_tags nt ON nt.tag_id = t.id
         JOIN notes n ON n.id = nt.note_id
         JOIN cards c ON c.note_id = n.id AND c.deleted_at IS NULL
         JOIN decks d ON d.id = n.deck_id
         WHERE n.deleted_at IS NULL
           AND d.deleted_at IS NULL
           AND d.archived_at IS NULL",
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
          HAVING COUNT(DISTINCT c.id) > 0
          ORDER BY t.name COLLATE NOCASE ASC",
    );

    let mut statement = connection.prepare(&sql).map_err(CommandError::database)?;
    let rows = statement
        .query_map(params_from_iter(values.iter()), tag_from_row)
        .map_err(CommandError::database)?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(CommandError::database)
}

fn tag_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<TagDto> {
    Ok(TagDto {
        id: row.get(0)?,
        name: row.get(1)?,
        card_count: row.get(2)?,
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
    if !matches!(input.due_filter.as_str(), "all" | "due" | "not_due") {
        return Err(CommandError::new(
            "invalid_due_filter",
            "Choose all cards, due cards, or cards that are not due.",
        ));
    }
    if !matches!(
        input.reviewed_filter.as_str(),
        "all"
            | "never"
            | "last_1_day"
            | "last_7_days"
            | "last_30_days"
            | "older_30_days"
    ) {
        return Err(CommandError::new(
            "invalid_reviewed_filter",
            "Choose one of the available studied-recency filters.",
        ));
    }
    if !matches!(
        input.sort.as_str(),
        "relevance"
            | "updated_desc"
            | "question_asc"
            | "due_asc"
            | "last_reviewed_desc"
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

fn build_search_sql(input: &SearchCardsInput, now: i64) -> Result<SearchSql, CommandError> {
    let fts_query = build_fts_query(&input.query);
    let mut joins = String::new();
    if fts_query.is_some() {
        joins.push_str(" JOIN card_search_fts ON card_search_fts.rowid = n.rowid");
    }

    let mut conditions = vec![
        "c.deleted_at IS NULL".to_string(),
        "c.suspended_at IS NULL".to_string(),
        "n.deleted_at IS NULL".to_string(),
        "d.deleted_at IS NULL".to_string(),
        "d.archived_at IS NULL".to_string(),
    ];
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
    match input.due_filter.as_str() {
        "due" => conditions.push(format!(
            "s.due_at <= ?{}",
            push_value(&mut values, now)
        )),
        "not_due" => conditions.push(format!(
            "s.due_at > ?{}",
            push_value(&mut values, now)
        )),
        _ => {}
    }

    const DAY_MS: i64 = 86_400_000;
    match input.reviewed_filter.as_str() {
        "never" => conditions.push("s.reps = 0".to_string()),
        "last_1_day" => {
            conditions.push("s.reps > 0".to_string());
            conditions.push(format!(
                "s.last_review_at >= ?{}",
                push_value(&mut values, now.saturating_sub(DAY_MS))
            ));
        }
        "last_7_days" => {
            conditions.push("s.reps > 0".to_string());
            conditions.push(format!(
                "s.last_review_at >= ?{}",
                push_value(&mut values, now.saturating_sub(7 * DAY_MS))
            ));
        }
        "last_30_days" => {
            conditions.push("s.reps > 0".to_string());
            conditions.push(format!(
                "s.last_review_at >= ?{}",
                push_value(&mut values, now.saturating_sub(30 * DAY_MS))
            ));
        }
        "older_30_days" => {
            conditions.push("s.reps > 0".to_string());
            conditions.push(format!(
                "s.last_review_at < ?{}",
                push_value(&mut values, now.saturating_sub(30 * DAY_MS))
            ));
        }
        _ => {}
    }

    let order_sql = match input.sort.as_str() {
        "relevance" if fts_query_is_present(&input.query) => {
            "bm25(card_search_fts) ASC, n.updated_at DESC"
        }
        "question_asc" => "n.front_text COLLATE NOCASE ASC, n.updated_at DESC",
        "due_asc" => "s.due_at ASC, n.updated_at DESC",
        "last_reviewed_desc" => {
            "CASE WHEN s.reps > 0 THEN 0 ELSE 1 END ASC, s.last_review_at DESC, n.updated_at DESC"
        }
        _ => "n.updated_at DESC, n.created_at DESC",
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
    let now = unix_time_millis()?;
    let mut search = build_search_sql(&input, now)?;
    let count_sql = format!(
        "SELECT COUNT(*)
         FROM notes n
         JOIN cards c ON c.note_id = n.id
         JOIN decks d ON d.id = c.deck_id
         JOIN card_schedules s ON s.card_id = c.id
         {joins}
         WHERE {where_sql}",
        joins = search.joins,
        where_sql = search.where_sql,
    );
    let total_count: i64 = connection
        .query_row(
            &count_sql,
            params_from_iter(search.values.iter()),
            |row| row.get(0),
        )
        .map_err(CommandError::database)?;

    let limit_index = push_value(&mut search.values, input.limit);
    let offset_index = push_value(&mut search.values, input.offset);
    let query_sql = format!(
        "SELECT
             c.id,
             n.id,
             c.deck_id,
             d.name,
             COALESCE((
                 SELECT group_concat(t.name, char(31))
                 FROM note_tags nt
                 JOIN tags t ON t.id = nt.tag_id
                 WHERE nt.note_id = n.id
                 ORDER BY t.name COLLATE NOCASE
             ), ''),
             s.due_at,
             CASE WHEN s.reps > 0 THEN s.last_review_at ELSE NULL END,
             s.reps,
             n.created_at,
             n.updated_at
         FROM notes n
         JOIN cards c ON c.note_id = n.id
         JOIN decks d ON d.id = c.deck_id
         JOIN card_schedules s ON s.card_id = c.id
         {joins}
         WHERE {where_sql}
         ORDER BY {order_sql}
         LIMIT ?{limit_index} OFFSET ?{offset_index}",
        joins = search.joins,
        where_sql = search.where_sql,
        order_sql = search.order_sql,
    );
    let raw_rows = {
        let mut statement = connection
            .prepare(&query_sql)
            .map_err(CommandError::database)?;
        statement
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
                    row.get::<_, i64>(8)?,
                    row.get::<_, i64>(9)?,
                ))
            })
            .map_err(CommandError::database)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(CommandError::database)?
    };

    let mut cards = Vec::with_capacity(raw_rows.len());
    for (
        card_id,
        note_id,
        deck_id,
        deck_name,
        tags,
        due_at,
        last_reviewed_at,
        review_count,
        created_at,
        updated_at,
    ) in raw_rows
    {
        let rendered = get_card_text_from_connection(connection, &card_id)?;
        cards.push(CardBrowserRowDto {
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
            created_at,
            updated_at,
        });
    }

    Ok(CardSearchPageDto { cards, total_count })
}

fn list_matching_cards_in_connection(
    connection: &Connection,
    input: SearchCardsInput,
) -> Result<Vec<SelectionCardDto>, CommandError> {
    let now = unix_time_millis()?;
    let mut search = build_search_sql(&input, now)?;
    let maximum = i64::try_from(MAX_SEARCH_SELECTION + 1).unwrap_or(i64::MAX);
    let limit_index = push_value(&mut search.values, maximum);
    let sql = format!(
        "SELECT c.id, n.id, c.deck_id
         FROM notes n
         JOIN cards c ON c.note_id = n.id
         JOIN decks d ON d.id = c.deck_id
         JOIN card_schedules s ON s.card_id = c.id
         {joins}
         WHERE {where_sql}
         ORDER BY {order_sql}
         LIMIT ?{limit_index}",
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
                "More than {MAX_SEARCH_SELECTION} cards match. Narrow the filters before selecting or practicing all results."
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

fn hydrate_organized_card(
    connection: &Connection,
    paths: &AppPaths,
    note_id: &str,
) -> Result<OrganizedCardDto, CommandError> {
    hydrate_typed_note(connection, paths, note_id)
}

fn load_tag_names(
    connection: &Connection,
    note_id: &str,
) -> Result<Vec<String>, CommandError> {
    let mut statement = connection
        .prepare(
            "SELECT t.name
             FROM tags t
             JOIN note_tags nt ON nt.tag_id = t.id
             WHERE nt.note_id = ?1
             ORDER BY t.name COLLATE NOCASE ASC",
        )
        .map_err(CommandError::database)?;
    let names = statement
        .query_map([note_id], |row| row.get(0))
        .map_err(CommandError::database)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(CommandError::database)?;
    Ok(names)
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
                        "INSERT INTO tags (
                             id, name, normalized_name, created_at, updated_at
                         ) VALUES (?1, ?2, ?3, ?4, ?4)",
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

    cleanup_unused_tags(transaction)?;
    Ok(())
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