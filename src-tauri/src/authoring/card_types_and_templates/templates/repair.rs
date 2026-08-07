use super::*;

pub(crate) fn apply_stage3_data_repairs(
    connection: &Connection,
) -> Result<i64, CommandError> {
    let before = connection.total_changes();
    normalize_abandoned_image_occlusion(connection)?;
    normalize_legacy_template_documents(connection)?;
    normalize_explicit_template_placeholders(connection)?;
    normalize_cloze_card_rows(connection)?;
    let changed = connection.total_changes().saturating_sub(before);
    Ok(i64::try_from(changed).unwrap_or(i64::MAX))
}

fn normalize_abandoned_image_occlusion(
    connection: &Connection,
) -> Result<(), CommandError> {
    let note_ids = {
        let mut statement = connection
            .prepare("SELECT id FROM notes WHERE note_type = 'image_occlusion'")
            .map_err(CommandError::database)?;
        let rows = statement
            .query_map([], |row| row.get::<_, String>(0))
            .map_err(CommandError::database)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(CommandError::database)?;
        rows
    };
    let now = unix_time_millis()?;
    for note_id in note_ids {
        connection
            .execute(
                "UPDATE notes
                 SET note_type = 'basic', reverse_enabled = 0,
                     cloze_source = '', cloze_mode = 'one_at_a_time',
                     updated_at = MAX(updated_at, ?1), revision = revision + 1
                 WHERE id = ?2 AND note_type = 'image_occlusion'",
                params![now, note_id],
            )
            .map_err(CommandError::database)?;
        let cards = {
            let mut statement = connection
                .prepare(
                    "SELECT rowid, id, deleted_at
                     FROM cards WHERE note_id = ?1
                     ORDER BY CASE WHEN deleted_at IS NULL THEN 0 ELSE 1 END,
                              created_at, id",
                )
                .map_err(CommandError::database)?;
            let rows = statement
                .query_map([note_id.as_str()], |row| {
                    Ok((
                        row.get::<_, i64>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, Option<i64>>(2)?,
                    ))
                })
                .map_err(CommandError::database)?
                .collect::<Result<Vec<_>, _>>()
                .map_err(CommandError::database)?;
            rows
        };
        if let Some(canonical) = cards.first() {
            connection
                .execute(
                    "UPDATE cards
                     SET direction = 'forward', template_ordinal = 0,
                         deleted_at = NULL, updated_at = MAX(updated_at, ?1)
                     WHERE id = ?2",
                    params![now, canonical.1],
                )
                .map_err(CommandError::database)?;
            for (rowid, card_id, _) in cards.iter().skip(1) {
                connection
                    .execute(
                        "UPDATE cards
                         SET direction = 'forward', template_ordinal = ?1,
                             deleted_at = COALESCE(deleted_at, ?2),
                             updated_at = MAX(updated_at, ?2)
                         WHERE id = ?3",
                        params![1_500_000_i64.saturating_add(*rowid), now, card_id],
                    )
                    .map_err(CommandError::database)?;
            }
        }
    }
    Ok(())
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

fn normalize_explicit_template_placeholders(
    connection: &Connection,
) -> Result<(), CommandError> {
    let rules = {
        let mut statement = connection
            .prepare(
                "SELECT id, template_id, front_doc, back_doc
                 FROM card_template_rules
                 WHERE front_doc IS NOT NULL AND back_doc IS NOT NULL",
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

    let mut lookups = HashMap::<String, AutomaticTemplateFieldLookup>::new();
    for (rule_id, template_id, front_json, back_json) in rules {
        if !lookups.contains_key(&template_id) {
            let fields = {
                let mut statement = connection
                    .prepare(
                        "SELECT id, name, description, position
                         FROM card_template_fields
                         WHERE template_id = ?1 ORDER BY position",
                    )
                    .map_err(CommandError::database)?;
                let rows = statement
                    .query_map([template_id.as_str()], |row| {
                        Ok(CardTemplateFieldDto {
                            id: row.get(0)?,
                            name: row.get(1)?,
                            description: row.get(2)?,
                            position: row.get(3)?,
                        })
                    })
                    .map_err(CommandError::database)?;
                rows.collect::<Result<Vec<_>, _>>()
                    .map_err(CommandError::database)?
            };
            lookups.insert(
                template_id.clone(),
                automatic_template_field_lookup_from_dtos(&fields)?,
            );
        }
        let lookup = lookups.get(&template_id).ok_or_else(|| {
            CommandError::new(
                "invalid_stored_template",
                "A saved template is missing its field lookup.",
            )
        })?;
        let front_doc: Value =
            serde_json::from_str(&front_json).map_err(CommandError::serialization)?;
        let back_doc: Value =
            serde_json::from_str(&back_json).map_err(CommandError::serialization)?;
        let normalized_front = normalize_typed_template_layout(&front_doc, lookup, "question")?;
        let normalized_back = normalize_typed_template_layout(&back_doc, lookup, "answer")?;
        let normalized_front_json =
            serde_json::to_string(&normalized_front).map_err(CommandError::serialization)?;
        let normalized_back_json =
            serde_json::to_string(&normalized_back).map_err(CommandError::serialization)?;
        if normalized_front_json != front_json || normalized_back_json != back_json {
            connection
                .execute(
                    "UPDATE card_template_rules
                     SET front_doc = ?1, back_doc = ?2
                     WHERE id = ?3",
                    params![normalized_front_json, normalized_back_json, rule_id],
                )
                .map_err(CommandError::database)?;
        }
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
                     SET direction = 'forward', template_ordinal = ?1,
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
