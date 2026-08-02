pub(crate) fn normalize_stage3_schema(
    connection: &mut Connection,
) -> Result<(), CommandError> {
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(CommandError::database)?;
    normalize_legacy_template_documents(&transaction)?;
    normalize_cloze_card_rows(&transaction)?;
    transaction.commit().map_err(CommandError::database)
}

fn normalize_legacy_template_documents(connection: &Connection) -> Result<(), CommandError> {
    let legacy_rules = {
        let mut statement = connection
            .prepare(
                "SELECT id, template_id, front_pattern, back_pattern
                 FROM card_template_rules
                 WHERE front_doc IS NULL OR back_doc IS NULL",
            )
            .map_err(CommandError::database)?;
        let rows = statement
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                ))
            })
            .map_err(CommandError::database)?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(CommandError::database)?
    };
    for (rule_id, template_id, front_pattern, back_pattern) in legacy_rules {
        let fields = {
            let mut statement = connection
                .prepare(
                    "SELECT id, name FROM card_template_fields
                     WHERE template_id = ?1 ORDER BY position",
                )
                .map_err(CommandError::database)?;
            let rows = statement
                .query_map([template_id.as_str()], |row| {
                    Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
                })
                .map_err(CommandError::database)?;
            rows.collect::<Result<Vec<_>, _>>()
                .map_err(CommandError::database)?
        };
        let field_map = fields
            .into_iter()
            .map(|(id, name)| (normalize_template_name(&name), id))
            .collect::<HashMap<_, _>>();
        let front_doc = legacy_pattern_document(&front_pattern, &field_map)?;
        let back_doc = legacy_pattern_document(&back_pattern, &field_map)?;
        connection
            .execute(
                "UPDATE card_template_rules SET front_doc = ?1, back_doc = ?2 WHERE id = ?3",
                params![
                    serde_json::to_string(&front_doc).map_err(CommandError::serialization)?,
                    serde_json::to_string(&back_doc).map_err(CommandError::serialization)?,
                    rule_id,
                ],
            )
            .map_err(CommandError::database)?;
    }
    Ok(())
}

fn legacy_pattern_document(
    pattern: &str,
    fields: &HashMap<String, String>,
) -> Result<Value, CommandError> {
    let mut blocks = Vec::new();
    for line in pattern.split('\n') {
        let trimmed = line.trim();
        if let Some(field_name) = trimmed
            .strip_prefix("{{")
            .and_then(|value| value.strip_suffix("}}"))
            .filter(|_| trimmed.matches("{{").count() == 1)
        {
            let field_id = fields
                .get(&normalize_template_name(field_name))
                .ok_or_else(|| {
                    CommandError::new(
                        "unknown_template_field",
                        "A legacy template rule refers to a field that no longer exists.",
                    )
                })?;
            blocks.push(json!({
                "type": TEMPLATE_BLOCK_NODE,
                "attrs": {"fieldId": field_id, "label": collapse_whitespace(field_name)}
            }));
            continue;
        }
        let mut inline = Vec::new();
        let mut cursor = 0;
        while let Some(relative_open) = line[cursor..].find("{{") {
            let open = cursor + relative_open;
            if open > cursor {
                inline.push(json!({"type": "text", "text": &line[cursor..open]}));
            }
            let after_open = open + 2;
            let relative_close = line[after_open..].find("}}").ok_or_else(|| {
                CommandError::new(
                    "invalid_template_placeholder",
                    "A legacy template contains an incomplete field placeholder.",
                )
            })?;
            let close = after_open + relative_close;
            let raw_name = &line[after_open..close];
            let field_name = normalize_template_name(raw_name);
            let field_id = fields.get(&field_name).ok_or_else(|| {
                CommandError::new(
                    "unknown_template_field",
                    "A legacy template rule refers to a field that no longer exists.",
                )
            })?;
            inline.push(json!({
                "type": TEMPLATE_INLINE_NODE,
                "attrs": {"fieldId": field_id, "label": collapse_whitespace(raw_name)}
            }));
            cursor = close + 2;
        }
        if cursor < line.len() {
            inline.push(json!({"type": "text", "text": &line[cursor..]}));
        }
        blocks.push(if inline.is_empty() {
            json!({"type": "paragraph"})
        } else {
            json!({"type": "paragraph", "content": inline})
        });
    }
    Ok(json!({"type": "doc", "content": blocks}))
}

fn normalize_cloze_card_rows(connection: &Connection) -> Result<(), CommandError> {
    let note_ids = {
        let mut statement = connection
            .prepare("SELECT id FROM notes WHERE note_type = 'cloze' AND deleted_at IS NULL")
            .map_err(CommandError::database)?;
        let rows = statement
            .query_map([], |row| row.get::<_, String>(0))
            .map_err(CommandError::database)?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(CommandError::database)?
    };
    let now = unix_time_millis()?;
    for note_id in note_ids {
        let cards = {
            let mut statement = connection
                .prepare(
                    "SELECT rowid, id, direction, template_ordinal, deleted_at
                     FROM cards WHERE note_id = ?1
                     ORDER BY CASE WHEN deleted_at IS NULL THEN 0 ELSE 1 END,
                              CASE WHEN direction = 'forward' THEN 0 ELSE 1 END,
                              created_at, id",
                )
                .map_err(CommandError::database)?;
            let rows = statement
                .query_map([note_id.as_str()], |row| {
                    Ok((
                        row.get::<_, i64>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, i64>(3)?,
                        row.get::<_, Option<i64>>(4)?,
                    ))
                })
                .map_err(CommandError::database)?;
            rows.collect::<Result<Vec<_>, _>>()
                .map_err(CommandError::database)?
        };
        let Some(canonical) = cards.first() else {
            continue;
        };
        for (rowid, card_id, _, _, _) in cards.iter().skip(1) {
            connection
                .execute(
                    "UPDATE cards
                     SET direction = 'cloze', template_ordinal = ?1,
                         deleted_at = COALESCE(deleted_at, ?2), updated_at = ?2
                     WHERE id = ?3",
                    params![1_000_000_i64.saturating_add(*rowid), now, card_id],
                )
                .map_err(CommandError::database)?;
        }
        connection
            .execute(
                "UPDATE cards
                 SET direction = 'forward', template_ordinal = 0,
                     deleted_at = NULL, updated_at = ?1
                 WHERE id = ?2",
                params![now, canonical.1],
            )
            .map_err(CommandError::database)?;
    }
    Ok(())
}
