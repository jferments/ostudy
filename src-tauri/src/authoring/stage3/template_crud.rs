fn list_card_templates_in_connection(
    connection: &mut Connection,
) -> Result<Vec<CardTemplateDto>, CommandError> {
    let ids = {
        let mut statement = connection
            .prepare(
                "SELECT id FROM card_templates
                 WHERE deleted_at IS NULL
                 ORDER BY name COLLATE NOCASE ASC, created_at ASC",
            )
            .map_err(CommandError::database)?;
        let ids = statement
            .query_map([], |row| row.get::<_, String>(0))
            .map_err(CommandError::database)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(CommandError::database)?;
        ids
    };
    ids.into_iter()
        .map(|id| Ok(load_stored_template(connection, &id, true)?.dto))
        .collect()
}

fn create_card_template_in_connection(
    connection: &mut Connection,
    input: CreateCardTemplateInput,
) -> Result<CardTemplateDto, CommandError> {
    let name = validate_template_name(input.name)?;
    let normalized_name = normalize_template_name(&name);
    let description = validate_bounded_visible_text(
        input.description,
        MAX_TEMPLATE_DESCRIPTION_CHARS,
        "Template descriptions",
        true,
    )?;
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(CommandError::database)?;
    ensure_template_name_available(&transaction, &normalized_name, None)?;
    let template_id = generate_id(&transaction)?;
    let fields = validate_template_fields(&transaction, &template_id, input.fields, false)?;
    let rules = validate_template_rules(&transaction, &template_id, input.rules, &fields, false)?;
    let now = unix_time_millis()?;
    transaction
        .execute(
            "INSERT INTO card_templates (
                 id, name, normalized_name, description, created_at, updated_at, revision
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?5, 1)",
            params![template_id, name, normalized_name, description, now],
        )
        .map_err(CommandError::database)?;
    insert_template_children(&transaction, &template_id, &fields, &rules)?;
    transaction.commit().map_err(CommandError::database)?;
    Ok(CardTemplateDto {
        id: template_id,
        name,
        description,
        fields,
        rules,
        created_at: now,
        updated_at: now,
        revision: 1,
    })
}

fn update_card_template_in_connection(
    connection: &mut Connection,
    input: UpdateCardTemplateInput,
) -> Result<CardTemplateDto, CommandError> {
    validate_entity_id(&input.template_id, "card template")?;
    if input.revision < 1 {
        return Err(CommandError::new(
            "invalid_revision",
            "The template revision is invalid.",
        ));
    }
    let name = validate_template_name(input.name)?;
    let normalized_name = normalize_template_name(&name);
    let description = validate_bounded_visible_text(
        input.description,
        MAX_TEMPLATE_DESCRIPTION_CHARS,
        "Template descriptions",
        true,
    )?;
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(CommandError::database)?;
    let current: Option<(i64, i64)> = transaction
        .query_row(
            "SELECT created_at, revision FROM card_templates
             WHERE id = ?1 AND deleted_at IS NULL",
            [input.template_id.as_str()],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()
        .map_err(CommandError::database)?;
    let Some((created_at, current_revision)) = current else {
        return Err(CommandError::new(
            "template_not_found",
            "That card template no longer exists.",
        ));
    };
    if current_revision != input.revision {
        return Err(CommandError::new(
            "template_changed",
            "This template changed in another window. Reload it and try again.",
        ));
    }
    ensure_template_name_available(
        &transaction,
        &normalized_name,
        Some(&input.template_id),
    )?;
    let fields =
        validate_template_fields(&transaction, &input.template_id, input.fields, true)?;
    let rules = validate_template_rules(
        &transaction,
        &input.template_id,
        input.rules,
        &fields,
        true,
    )?;
    let now = unix_time_millis()?;
    transaction
        .execute(
            "UPDATE card_templates
             SET name = ?1, normalized_name = ?2, description = ?3,
                 updated_at = ?4, revision = revision + 1
             WHERE id = ?5 AND revision = ?6 AND deleted_at IS NULL",
            params![
                name,
                normalized_name,
                description,
                now,
                input.template_id,
                input.revision,
            ],
        )
        .map_err(CommandError::database)?;
    transaction
        .execute(
            "DELETE FROM card_template_rules WHERE template_id = ?1",
            [input.template_id.as_str()],
        )
        .map_err(CommandError::database)?;
    transaction
        .execute(
            "DELETE FROM card_template_fields WHERE template_id = ?1",
            [input.template_id.as_str()],
        )
        .map_err(CommandError::database)?;
    insert_template_children(&transaction, &input.template_id, &fields, &rules)?;
    transaction.commit().map_err(CommandError::database)?;
    Ok(CardTemplateDto {
        id: input.template_id,
        name,
        description,
        fields,
        rules,
        created_at,
        updated_at: now,
        revision: input.revision + 1,
    })
}

fn delete_card_template_in_connection(
    connection: &mut Connection,
    template_id: &str,
) -> Result<(), CommandError> {
    let now = unix_time_millis()?;
    let changed = connection
        .execute(
            "UPDATE card_templates
             SET deleted_at = ?1, updated_at = ?1, revision = revision + 1
             WHERE id = ?2 AND deleted_at IS NULL",
            params![now, template_id],
        )
        .map_err(CommandError::database)?;
    if changed == 0 {
        return Err(CommandError::new(
            "template_not_found",
            "That card template no longer exists.",
        ));
    }
    Ok(())
}

fn load_stored_template(
    connection: &Connection,
    template_id: &str,
    require_active: bool,
) -> Result<StoredTemplate, CommandError> {
    let active_clause = if require_active {
        "AND deleted_at IS NULL"
    } else {
        ""
    };
    let sql = format!(
        "SELECT id, name, description, created_at, updated_at, revision
         FROM card_templates WHERE id = ?1 {active_clause}"
    );
    let base = connection
        .query_row(&sql, [template_id], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, i64>(3)?,
                row.get::<_, i64>(4)?,
                row.get::<_, i64>(5)?,
            ))
        })
        .optional()
        .map_err(CommandError::database)?
        .ok_or_else(|| {
            CommandError::new(
                "template_not_found",
                "That card template no longer exists.",
            )
        })?;
    let fields = {
        let mut statement = connection
            .prepare(
                "SELECT id, name, description, position
                 FROM card_template_fields
                 WHERE template_id = ?1 ORDER BY position ASC",
            )
            .map_err(CommandError::database)?;
        let fields = statement
            .query_map([template_id], |row| {
                Ok(CardTemplateFieldDto {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    description: row.get(2)?,
                    position: row.get(3)?,
                })
            })
            .map_err(CommandError::database)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(CommandError::database)?;
        fields
    };
    let rules = {
        let mut statement = connection
            .prepare(
                "SELECT id, name, front_pattern, back_pattern, position
                 FROM card_template_rules
                 WHERE template_id = ?1 ORDER BY position ASC",
            )
            .map_err(CommandError::database)?;
        let rules = statement
            .query_map([template_id], |row| {
                Ok(CardTemplateRuleDto {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    front_pattern: row.get(2)?,
                    back_pattern: row.get(3)?,
                    position: row.get(4)?,
                })
            })
            .map_err(CommandError::database)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(CommandError::database)?;
        rules
    };
    validate_stored_template(&fields, &rules)?;
    let normalized_fields = fields
        .iter()
        .map(|field| (normalize_template_name(&field.name), field.id.clone()))
        .collect();
    Ok(StoredTemplate {
        dto: CardTemplateDto {
            id: base.0,
            name: base.1,
            description: base.2,
            fields,
            rules,
            created_at: base.3,
            updated_at: base.4,
            revision: base.5,
        },
        normalized_fields,
    })
}

fn validate_stored_template(
    fields: &[CardTemplateFieldDto],
    rules: &[CardTemplateRuleDto],
) -> Result<(), CommandError> {
    if fields.is_empty()
        || fields.len() > MAX_TEMPLATE_FIELDS
        || rules.is_empty()
        || rules.len() > MAX_TEMPLATE_RULES
    {
        return Err(CommandError::new(
            "invalid_stored_template",
            "A saved template contains an invalid number of fields or card rules.",
        ));
    }
    let mut field_ids = BTreeSet::new();
    let mut field_names = BTreeSet::new();
    for (position, field) in fields.iter().enumerate() {
        validate_entity_id(&field.id, "template field")?;
        if field.position != i64::try_from(position).unwrap_or(i64::MAX)
            || !field_ids.insert(field.id.clone())
            || !field_names.insert(normalize_template_name(&field.name))
        {
            return Err(CommandError::new(
                "invalid_stored_template",
                "A saved template contains duplicate or out-of-order fields.",
            ));
        }
        validate_template_field_name(field.name.clone())?;
    }
    let mut rule_ids = BTreeSet::new();
    let mut rule_names = BTreeSet::new();
    for (position, rule) in rules.iter().enumerate() {
        validate_entity_id(&rule.id, "template rule")?;
        if rule.position != i64::try_from(position).unwrap_or(i64::MAX)
            || !rule_ids.insert(rule.id.clone())
            || !rule_names.insert(normalize_template_name(&rule.name))
        {
            return Err(CommandError::new(
                "invalid_stored_template",
                "A saved template contains duplicate or out-of-order card rules.",
            ));
        }
        validate_pattern(&rule.front_pattern, fields, "question")?;
        validate_pattern(&rule.back_pattern, fields, "answer")?;
    }
    Ok(())
}
