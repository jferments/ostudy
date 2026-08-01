fn list_tags_in_connection(
    connection: &Connection,
    deck_id: Option<&str>,
) -> Result<Vec<TagDto>, CommandError> {
    let (sql, parameter): (&str, Option<&str>) = if deck_id.is_some() {
        (
            "SELECT t.id, t.name, COUNT(DISTINCT n.id)
             FROM tags t
             JOIN note_tags nt ON nt.tag_id = t.id
             JOIN notes n ON n.id = nt.note_id
             JOIN decks d ON d.id = n.deck_id
             WHERE n.deleted_at IS NULL
               AND d.deleted_at IS NULL
               AND d.archived_at IS NULL
               AND n.deck_id = ?1
             GROUP BY t.id, t.name
             HAVING COUNT(DISTINCT n.id) > 0
             ORDER BY t.name COLLATE NOCASE ASC",
            deck_id,
        )
    } else {
        (
            "SELECT t.id, t.name, COUNT(DISTINCT n.id)
             FROM tags t
             JOIN note_tags nt ON nt.tag_id = t.id
             JOIN notes n ON n.id = nt.note_id
             JOIN decks d ON d.id = n.deck_id
             WHERE n.deleted_at IS NULL
               AND d.deleted_at IS NULL
               AND d.archived_at IS NULL
             GROUP BY t.id, t.name
             HAVING COUNT(DISTINCT n.id) > 0
             ORDER BY t.name COLLATE NOCASE ASC",
            None,
        )
    };
    let mut statement = connection.prepare(sql).map_err(CommandError::database)?;
    let rows = match parameter {
        Some(deck_id) => statement
            .query_map([deck_id], tag_from_row)
            .map_err(CommandError::database)?,
        None => statement
            .query_map([], tag_from_row)
            .map_err(CommandError::database)?,
    };
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
    if let Some(deck_id) = &input.deck_id {
        validate_entity_id(deck_id, "deck")?;
    }
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
        input.sort.as_str(),
        "relevance" | "updated_desc" | "question_asc" | "due_asc"
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

fn search_cards_in_connection(
    connection: &Connection,
    input: SearchCardsInput,
) -> Result<CardSearchPageDto, CommandError> {
    let now = unix_time_millis()?;
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
        "c.direction = 'forward'".to_string(),
    ];
    let mut values: Vec<SqlValue> = Vec::new();

    if let Some(deck_id) = input.deck_id {
        conditions.push(format!("c.deck_id = ?{}", push_value(&mut values, deck_id)));
    }
    if let Some(query) = fts_query {
        conditions.push(format!(
            "card_search_fts MATCH ?{}",
            push_value(&mut values, query)
        ));
    }
    for tag_id in input.tag_ids {
        let index = push_value(&mut values, tag_id);
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

    let where_sql = conditions.join(" AND ");
    let count_sql = format!(
        "SELECT COUNT(*)
         FROM notes n
         JOIN cards c ON c.note_id = n.id
         JOIN decks d ON d.id = c.deck_id
         JOIN card_schedules s ON s.card_id = c.id
         {joins}
         WHERE {where_sql}"
    );
    let total_count: i64 = connection
        .query_row(
            &count_sql,
            params_from_iter(values.iter()),
            |row| row.get(0),
        )
        .map_err(CommandError::database)?;

    let order_sql = match input.sort.as_str() {
        "relevance" if fts_query_is_present(&input.query) => {
            "bm25(card_search_fts) ASC, n.updated_at DESC"
        }
        "question_asc" => "n.front_text COLLATE NOCASE ASC, n.updated_at DESC",
        "due_asc" => "s.due_at ASC, n.updated_at DESC",
        _ => "n.updated_at DESC, n.created_at DESC",
    };
    let limit_index = push_value(&mut values, input.limit);
    let offset_index = push_value(&mut values, input.offset);

    let query_sql = format!(
        "SELECT
             c.id,
             n.id,
             c.deck_id,
             d.name,
             n.front_text,
             n.back_text,
             COALESCE((
                 SELECT group_concat(t.name, char(31))
                 FROM note_tags nt
                 JOIN tags t ON t.id = nt.tag_id
                 WHERE nt.note_id = n.id
                 ORDER BY t.name COLLATE NOCASE
             ), ''),
             s.due_at,
             n.created_at,
             n.updated_at
         FROM notes n
         JOIN cards c ON c.note_id = n.id
         JOIN decks d ON d.id = c.deck_id
         JOIN card_schedules s ON s.card_id = c.id
         {joins}
         WHERE {where_sql}
         ORDER BY {order_sql}
         LIMIT ?{limit_index} OFFSET ?{offset_index}"
    );
    let mut statement = connection
        .prepare(&query_sql)
        .map_err(CommandError::database)?;
    let cards = statement
        .query_map(params_from_iter(values.iter()), |row| {
            let tags: String = row.get(6)?;
            Ok(CardBrowserRowDto {
                card_id: row.get(0)?,
                note_id: row.get(1)?,
                deck_id: row.get(2)?,
                deck_name: row.get(3)?,
                front_text: row.get(4)?,
                back_text: row.get(5)?,
                tags: split_tags(&tags),
                due_at: row.get(7)?,
                created_at: row.get(8)?,
                updated_at: row.get(9)?,
            })
        })
        .map_err(CommandError::database)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(CommandError::database)?;

    Ok(CardSearchPageDto { cards, total_count })
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
    statement
        .query_map([note_id], |row| row.get(0))
        .map_err(CommandError::database)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(CommandError::database)
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

    transaction
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
