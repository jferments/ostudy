fn create_card_template_in_connection(
    connection: &mut Connection,
    paths: &AppPaths,
    input: CreateCardTemplateInput,
) -> Result<CardTemplateDto, CommandError> {
    let name = validate_template_name(input.name)?;
    let description = validate_template_description(input.description)?;
    if let Some(draft_id) = input.asset_draft_id.as_deref() {
        validate_entity_id(draft_id, "asset draft")?;
    }
    let fields = validate_template_fields(input.fields)?;
    let field_ids = fields
        .iter()
        .filter_map(|field| field.id.clone())
        .collect::<BTreeSet<_>>();
    let rules = validate_template_rules(input.rules, &field_ids)?;
    let template_asset_ids = template_asset_ids_from_rules(&rules)?;
    ensure_assets_available(
        connection,
        paths,
        &template_asset_ids,
        input.asset_draft_id.as_deref(),
    )?;
    let now = unix_time_millis()?;
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(CommandError::database)?;
    let template_id = generate_id(&transaction)?;
    transaction
        .execute(
            "INSERT INTO card_templates (
                 id, name, description, created_at, updated_at, revision
             ) VALUES (?1, ?2, ?3, ?4, ?4, 1)",
            params![template_id, name, description, now],
        )
        .map_err(CommandError::database)?;
    insert_template_fields(&transaction, &template_id, &fields, now)?;
    insert_template_rules(&transaction, &template_id, &rules, now)?;
    replace_template_assets(&transaction, &template_id, &template_asset_ids, now)?;
    let finalization = finalize_assets_for_owner(
        &transaction,
        paths,
        &template_asset_ids,
        input.asset_draft_id.as_deref(),
        Some(&template_id),
    )?;
    transaction.commit().map_err(CommandError::database)?;
    finalization.cleanup();
    load_stored_template(connection, paths, &template_id, true).map(|stored| stored.dto)
}

fn update_card_template_in_connection(
    connection: &mut Connection,
    paths: &AppPaths,
    input: UpdateCardTemplateInput,
) -> Result<CardTemplateDto, CommandError> {
    validate_entity_id(&input.template_id, "card template")?;
    if input.revision < 1 {
        return Err(CommandError::new(
            "invalid_revision",
            "The card-template revision is invalid.",
        ));
    }
    let name = validate_template_name(input.name)?;
    let description = validate_template_description(input.description)?;
    if let Some(draft_id) = input.asset_draft_id.as_deref() {
        validate_entity_id(draft_id, "asset draft")?;
    }
    let fields = validate_template_fields(input.fields)?;
    let field_ids = fields
        .iter()
        .filter_map(|field| field.id.clone())
        .collect::<BTreeSet<_>>();
    let rules = validate_template_rules(input.rules, &field_ids)?;
    let template_asset_ids = template_asset_ids_from_rules(&rules)?;
    ensure_assets_available(
        connection,
        paths,
        &template_asset_ids,
        input.asset_draft_id.as_deref(),
    )?;
    let now = unix_time_millis()?;
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(CommandError::database)?;
    let changed = transaction
        .execute(
            "UPDATE card_templates
             SET name = ?1, description = ?2, updated_at = ?3, revision = revision + 1
             WHERE id = ?4 AND revision = ?5",
            params![name, description, now, input.template_id, input.revision],
        )
        .map_err(CommandError::database)?;
    if changed != 1 {
        return Err(CommandError::new(
            "template_changed",
            "This card template changed in another window. Reload it and try again.",
        ));
    }
    transaction
        .execute(
            "DELETE FROM card_template_fields WHERE template_id = ?1",
            [input.template_id.as_str()],
        )
        .map_err(CommandError::database)?;
    transaction
        .execute(
            "DELETE FROM card_template_rules WHERE template_id = ?1",
            [input.template_id.as_str()],
        )
        .map_err(CommandError::database)?;
    insert_template_fields(&transaction, &input.template_id, &fields, now)?;
    insert_template_rules(&transaction, &input.template_id, &rules, now)?;
    replace_template_assets(&transaction, &input.template_id, &template_asset_ids, now)?;
    let finalization = finalize_assets_for_owner(
        &transaction,
        paths,
        &template_asset_ids,
        input.asset_draft_id.as_deref(),
        Some(&input.template_id),
    )?;
    transaction.commit().map_err(CommandError::database)?;
    finalization.cleanup();
    load_stored_template(connection, paths, &input.template_id, true).map(|stored| stored.dto)
}

fn delete_card_template_in_connection(
    connection: &Connection,
    template_id: &str,
) -> Result<(), CommandError> {
    let changed = connection
        .execute("DELETE FROM card_templates WHERE id = ?1", [template_id])
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
    paths: &AppPaths,
    template_id: &str,
    include_assets: bool,
) -> Result<StoredTemplate, CommandError> {
    let base = connection
        .query_row(
            "SELECT id, name, description, created_at, updated_at, revision
             FROM card_templates WHERE id = ?1",
            [template_id],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, i64>(3)?,
                    row.get::<_, i64>(4)?,
                    row.get::<_, i64>(5)?,
                ))
            },
        )
        .optional()
        .map_err(CommandError::database)?
        .ok_or_else(|| {
            CommandError::new(
                "template_not_found",
                "That card template no longer exists.",
            )
        })?;
    let fields = load_template_fields(connection, template_id)?;
    let rules = load_template_rules(connection, template_id)?;
    let assets = if include_assets {
        load_template_assets(connection, paths, template_id)?
    } else {
        Vec::new()
    };
    Ok(StoredTemplate {
        dto: CardTemplateDto {
            id: base.0,
            name: base.1,
            description: base.2,
            fields,
            rules,
            assets,
            created_at: base.3,
            updated_at: base.4,
            revision: base.5,
        },
    })
}

fn load_template_fields(
    connection: &Connection,
    template_id: &str,
) -> Result<Vec<CardTemplateFieldDto>, CommandError> {
    let mut statement = connection
        .prepare(
            "SELECT id, name, description, position
             FROM card_template_fields
             WHERE template_id = ?1
             ORDER BY position, created_at, id",
        )
        .map_err(CommandError::database)?;
    statement
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
        .map_err(CommandError::database)
}

fn load_template_rules(
    connection: &Connection,
    template_id: &str,
) -> Result<Vec<CardTemplateRuleDto>, CommandError> {
    let mut statement = connection
        .prepare(
            "SELECT id, name, front_doc, back_doc, position
             FROM card_template_rules
             WHERE template_id = ?1
             ORDER BY position, created_at, id",
        )
        .map_err(CommandError::database)?;
    statement
        .query_map([template_id], |row| {
            let front_doc: String = row.get(2)?;
            let back_doc: String = row.get(3)?;
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                front_doc,
                back_doc,
                row.get::<_, i64>(4)?,
            ))
        })
        .map_err(CommandError::database)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(CommandError::database)?
        .into_iter()
        .map(|(id, name, front_doc, back_doc, position)| {
            Ok(CardTemplateRuleDto {
                id,
                name,
                front_doc: serde_json::from_str(&front_doc)
                    .map_err(CommandError::serialization)?,
                back_doc: serde_json::from_str(&back_doc)
                    .map_err(CommandError::serialization)?,
                position,
            })
        })
        .collect()
}

fn load_template_assets(
    connection: &Connection,
    paths: &AppPaths,
    template_id: &str,
) -> Result<Vec<AssetDto>, CommandError> {
    let mut statement = connection
        .prepare(
            "SELECT a.id, a.original_filename, a.mime_type, a.byte_size,
                    a.relative_path, a.created_at
             FROM card_template_assets cta
             JOIN assets a ON a.id = cta.asset_id
             WHERE cta.template_id = ?1
               AND a.deleted_at IS NULL
               AND a.lifecycle_state = 'ready'
             ORDER BY cta.position, a.created_at, a.id",
        )
        .map_err(CommandError::database)?;
    statement
        .query_map([template_id], |row| {
            let relative_path: String = row.get(4)?;
            Ok(AssetDto {
                id: row.get(0)?,
                original_filename: row.get(1)?,
                mime_type: row.get(2)?,
                byte_size: row.get(3)?,
                file_path: paths
                    .resolve_relative(&relative_path)
                    .to_string_lossy()
                    .into_owned(),
                created_at: row.get(5)?,
            })
        })
        .map_err(CommandError::database)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(CommandError::database)
}

fn validate_template_name(value: String) -> Result<String, CommandError> {
    let value = collapse_whitespace(&value);
    if value.is_empty() || value.chars().count() > MAX_TEMPLATE_NAME_CHARS {
        return Err(CommandError::new(
            "invalid_template_name",
            format!("Template names must contain 1 to {MAX_TEMPLATE_NAME_CHARS} characters."),
        ));
    }
    Ok(value)
}

fn validate_template_description(value: String) -> Result<String, CommandError> {
    let value = value.trim().to_string();
    if value.chars().count() > MAX_TEMPLATE_DESCRIPTION_CHARS {
        return Err(CommandError::new(
            "invalid_template_description",
            format!("Template descriptions cannot exceed {MAX_TEMPLATE_DESCRIPTION_CHARS} characters."),
        ));
    }
    Ok(value)
}

fn validate_template_fields(
    fields: Vec<CardTemplateFieldInput>,
) -> Result<Vec<CardTemplateFieldInput>, CommandError> {
    if fields.is_empty() || fields.len() > MAX_TEMPLATE_FIELDS {
        return Err(CommandError::new(
            "invalid_template_fields",
            format!("Templates must define 1 to {MAX_TEMPLATE_FIELDS} fields."),
        ));
    }
    let mut seen = BTreeSet::new();
    let mut validated = Vec::with_capacity(fields.len());
    for mut field in fields {
        if let Some(field_id) = field.id.as_deref() {
            validate_entity_id(field_id, "template field")?;
        }
        field.name = collapse_whitespace(&field.name);
        field.description = field.description.trim().to_string();
        if field.name.is_empty() || field.name.chars().count() > MAX_TEMPLATE_FIELD_NAME_CHARS {
            return Err(CommandError::new(
                "invalid_template_field_name",
                format!(
                    "Template field names must contain 1 to {MAX_TEMPLATE_FIELD_NAME_CHARS} characters."
                ),
            ));
        }
        if field.description.chars().count() > MAX_TEMPLATE_FIELD_DESCRIPTION_CHARS {
            return Err(CommandError::new(
                "invalid_template_field_description",
                format!(
                    "Template field descriptions cannot exceed {MAX_TEMPLATE_FIELD_DESCRIPTION_CHARS} characters."
                ),
            ));
        }
        let normalized = normalize_template_name(&field.name);
        if !seen.insert(normalized) {
            return Err(CommandError::new(
                "duplicate_template_field",
                "Template field names must be unique.",
            ));
        }
        validated.push(field);
    }
    Ok(validated)
}

fn validate_template_rules(
    rules: Vec<CardTemplateRuleInput>,
    field_ids: &BTreeSet<String>,
) -> Result<Vec<CardTemplateRuleInput>, CommandError> {
    if rules.is_empty() || rules.len() > MAX_TEMPLATE_RULES {
        return Err(CommandError::new(
            "invalid_template_rules",
            format!("Templates must define 1 to {MAX_TEMPLATE_RULES} generated-card rules."),
        ));
    }
    let mut seen = BTreeSet::new();
    let mut validated = Vec::with_capacity(rules.len());
    for mut rule in rules {
        if let Some(rule_id) = rule.id.as_deref() {
            validate_entity_id(rule_id, "template rule")?;
        }
        rule.name = collapse_whitespace(&rule.name);
        if rule.name.is_empty() || rule.name.chars().count() > MAX_TEMPLATE_RULE_NAME_CHARS {
            return Err(CommandError::new(
                "invalid_template_rule_name",
                format!(
                    "Generated-card names must contain 1 to {MAX_TEMPLATE_RULE_NAME_CHARS} characters."
                ),
            ));
        }
        let normalized = normalize_template_name(&rule.name);
        if !seen.insert(normalized) {
            return Err(CommandError::new(
                "duplicate_template_rule",
                "Generated-card names must be unique within a template.",
            ));
        }
        validate_template_document(&rule.front_doc, field_ids, "question")?;
        validate_template_document(&rule.back_doc, field_ids, "answer")?;
        validated.push(rule);
    }
    Ok(validated)
}

fn validate_template_document(
    document: &Value,
    field_ids: &BTreeSet<String>,
    side: &str,
) -> Result<(), CommandError> {
    let json = serde_json::to_string(document).map_err(CommandError::serialization)?;
    if json.len() > MAX_TEMPLATE_DOCUMENT_BYTES {
        return Err(CommandError::new(
            "template_document_too_large",
            format!("Template {side} layouts cannot exceed {MAX_TEMPLATE_DOCUMENT_BYTES} bytes."),
        ));
    }
    let root = document.as_object().ok_or_else(|| {
        CommandError::new(
            "invalid_template_layout",
            format!("The template {side} layout must be a rich-text document."),
        )
    })?;
    if root.get("type").and_then(Value::as_str) != Some("doc") {
        return Err(CommandError::new(
            "invalid_template_layout",
            format!("The template {side} layout must use a document root."),
        ));
    }
    let mut found = BTreeSet::new();
    collect_template_placeholders(document, field_ids, &mut found)?;
    if found.is_empty() {
        return Err(CommandError::new(
            "missing_template_placeholder",
            format!("The template {side} layout must include at least one field placeholder."),
        ));
    }
    Ok(())
}

fn collect_template_placeholders(
    value: &Value,
    field_ids: &BTreeSet<String>,
    found: &mut BTreeSet<String>,
) -> Result<(), CommandError> {
    match value {
        Value::Array(values) => {
            for value in values {
                collect_template_placeholders(value, field_ids, found)?;
            }
        }
        Value::Object(object) => {
            if let Some(node_type) = object.get("type").and_then(Value::as_str) {
                if matches!(node_type, TEMPLATE_INLINE_NODE | TEMPLATE_BLOCK_NODE) {
                    let attrs = object
                        .get("attrs")
                        .and_then(Value::as_object)
                        .ok_or_else(|| invalid_template_placeholder("missing attributes"))?;
                    let field_id = attrs
                        .get("fieldId")
                        .and_then(Value::as_str)
                        .ok_or_else(|| invalid_template_placeholder("missing field identifier"))?;
                    validate_entity_id(field_id, "template field")?;
                    if !field_ids.contains(field_id) {
                        return Err(CommandError::new(
                            "unknown_template_field",
                            "A card layout refers to a field that does not exist in this template.",
                        ));
                    }
                    found.insert(field_id.to_string());
                }
            }
            for value in object.values() {
                collect_template_placeholders(value, field_ids, found)?;
            }
        }
        _ => {}
    }
    Ok(())
}

fn invalid_template_placeholder(reason: &str) -> CommandError {
    CommandError::new(
        "invalid_template_placeholder",
        format!("A template field placeholder is invalid: {reason}."),
    )
}

fn insert_template_fields(
    connection: &Connection,
    template_id: &str,
    fields: &[CardTemplateFieldInput],
    now: i64,
) -> Result<(), CommandError> {
    for (position, field) in fields.iter().enumerate() {
        let field_id = match field.id.as_deref() {
            Some(field_id) => field_id.to_string(),
            None => generate_id(connection)?,
        };
        connection
            .execute(
                "INSERT INTO card_template_fields (
                     id, template_id, name, description, position, created_at, updated_at
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?6)",
                params![
                    field_id,
                    template_id,
                    field.name,
                    field.description,
                    i64::try_from(position).unwrap_or(i64::MAX),
                    now
                ],
            )
            .map_err(CommandError::database)?;
    }
    Ok(())
}

fn insert_template_rules(
    connection: &Connection,
    template_id: &str,
    rules: &[CardTemplateRuleInput],
    now: i64,
) -> Result<(), CommandError> {
    for (position, rule) in rules.iter().enumerate() {
        let rule_id = match rule.id.as_deref() {
            Some(rule_id) => rule_id.to_string(),
            None => generate_id(connection)?,
        };
        let front_doc =
            serde_json::to_string(&rule.front_doc).map_err(CommandError::serialization)?;
        let back_doc = serde_json::to_string(&rule.back_doc).map_err(CommandError::serialization)?;
        connection
            .execute(
                "INSERT INTO card_template_rules (
                     id, template_id, name, front_doc, back_doc,
                     position, created_at, updated_at
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?7)",
                params![
                    rule_id,
                    template_id,
                    rule.name,
                    front_doc,
                    back_doc,
                    i64::try_from(position).unwrap_or(i64::MAX),
                    now
                ],
            )
            .map_err(CommandError::database)?;
    }
    Ok(())
}

fn template_asset_ids_from_rules(
    rules: &[CardTemplateRuleInput],
) -> Result<Vec<String>, CommandError> {
    let mut asset_ids = BTreeSet::new();
    for rule in rules {
        asset_ids.extend(collect_asset_ids(&rule.front_doc)?);
        asset_ids.extend(collect_asset_ids(&rule.back_doc)?);
    }
    Ok(asset_ids.into_iter().collect())
}

fn replace_template_assets(
    connection: &Connection,
    template_id: &str,
    asset_ids: &[String],
    now: i64,
) -> Result<(), CommandError> {
    connection
        .execute(
            "DELETE FROM card_template_assets WHERE template_id = ?1",
            [template_id],
        )
        .map_err(CommandError::database)?;
    for (position, asset_id) in asset_ids.iter().enumerate() {
        connection
            .execute(
                "INSERT INTO card_template_assets (
                     template_id, asset_id, position, created_at
                 ) VALUES (?1, ?2, ?3, ?4)",
                params![
                    template_id,
                    asset_id,
                    i64::try_from(position).unwrap_or(i64::MAX),
                    now
                ],
            )
            .map_err(CommandError::database)?;
    }
    Ok(())
}

fn ensure_template_revision(
    template: &StoredTemplate,
    expected_revision: i64,
) -> Result<(), CommandError> {
    if expected_revision < 1 || template.dto.revision != expected_revision {
        return Err(CommandError::new(
            "template_changed",
            "This card template changed. Reload it before previewing or generating cards.",
        ));
    }
    Ok(())
}

fn validate_template_values(
    connection: &Connection,
    fields: &[CardTemplateFieldDto],
    values: Vec<TemplateValueInput>,
) -> Result<HashMap<String, ValidatedTemplateValue>, CommandError> {
    if values.len() != fields.len() {
        return Err(CommandError::new(
            "missing_template_value",
            "Fill in every template field before generating cards.",
        ));
    }
    let allowed_ids = fields
        .iter()
        .map(|field| field.id.clone())
        .collect::<BTreeSet<_>>();
    let mut validated = HashMap::new();
    for value in values {
        validate_entity_id(&value.field_id, "template field")?;
        if !allowed_ids.contains(&value.field_id) || validated.contains_key(&value.field_id) {
            return Err(CommandError::new(
                "invalid_template_value",
                "Template values must match the selected template exactly once.",
            ));
        }
        if value.text.chars().count() > MAX_TEMPLATE_VALUE_CHARS {
            return Err(CommandError::new(
                "template_value_too_large",
                format!("Template field text cannot exceed {MAX_TEMPLATE_VALUE_CHARS} characters."),
            ));
        }
        let asset_ids = collect_asset_ids(&value.doc)?;
        ensure_assets_exist(connection, &asset_ids)?;
        validated.insert(
            value.field_id,
            ValidatedTemplateValue {
                doc: value.doc,
                asset_ids,
            },
        );
    }
    Ok(validated)
}

fn render_template_previews(
    connection: &Connection,
    paths: &AppPaths,
    template: &StoredTemplate,
    values: Vec<TemplateValueInput>,
) -> Result<Vec<TemplateCardPreviewDto>, CommandError> {
    let rendered = render_template_cards(connection, template, values)?;
    rendered
        .into_iter()
        .map(|card| {
            let asset_ids = content_asset_ids(&card.content);
            Ok(TemplateCardPreviewDto {
                rule_id: card.rule_id,
                rule_name: card.rule_name,
                question_doc: card.content.front_doc,
                answer_doc: card.content.back_doc,
                question_text: card.content.front_text,
                answer_text: card.content.back_text,
                assets: load_assets_by_ids(connection, paths, &asset_ids)?,
            })
        })
        .collect()
}

fn render_template_cards(
    connection: &Connection,
    template: &StoredTemplate,
    values: Vec<TemplateValueInput>,
) -> Result<Vec<RenderedTemplateCard>, CommandError> {
    let values = validate_template_values(connection, &template.dto.fields, values)?;
    template
        .dto
        .rules
        .iter()
        .map(|rule| {
            let front_doc = substitute_template_document(&rule.front_doc, &values)?;
            let back_doc = substitute_template_document(&rule.back_doc, &values)?;
            let content = validate_content(
                front_doc,
                back_doc,
                String::new(),
                String::new(),
            )?;
            Ok(RenderedTemplateCard {
                rule_id: rule.id.clone(),
                rule_name: rule.name.clone(),
                content,
            })
        })
        .collect()
}

fn generate_template_cards_in_connection(
    connection: &mut Connection,
    _paths: &AppPaths,
    input: GenerateTemplateCardsInput,
) -> Result<GenerateTemplateCardsResultDto, CommandError> {
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(CommandError::database)?;
    let template = load_stored_template(&transaction, _paths, &input.template_id, true)?;
    ensure_template_revision(&template, input.template_revision)?;
    let rendered_cards = render_template_cards(&transaction, &template, input.values)?;
    let tags = validate_tags(input.tags)?;
    ensure_active_deck(&transaction, &input.deck_id)?;
    let now = unix_time_millis()?;
    let mut created_note_ids = Vec::new();
    let mut skipped_duplicates = 0_i64;
    let mut seen = BTreeSet::new();
    for rendered in rendered_cards {
        let normalized = normalized_pair(
            &rendered.content.front_text,
            &rendered.content.back_text,
        );
        let hash = duplicate_hash(&normalized.0, &normalized.1);
        if !seen.insert(normalized.clone())
            || has_exact_duplicate(
                &transaction,
                &input.deck_id,
                &hash,
                &normalized.0,
                &normalized.1,
                None,
            )?
        {
            skipped_duplicates += 1;
            continue;
        }
        ensure_assets_exist(&transaction, &content_asset_ids(&rendered.content))?;
        let note_id = insert_organized_card(
            &transaction,
            &input.deck_id,
            rendered.content,
            &tags,
            &hash,
            now,
        )?;
        created_note_ids.push(note_id);
    }
    transaction.commit().map_err(CommandError::database)?;
    Ok(GenerateTemplateCardsResultDto {
        created_count: i64::try_from(created_note_ids.len()).unwrap_or(i64::MAX),
        skipped_duplicates,
        created_note_ids,
    })
}

fn substitute_template_document(
    layout: &Value,
    values: &HashMap<String, ValidatedTemplateValue>,
) -> Result<Value, CommandError> {
    let substituted = substitute_template_value(layout, values)?;
    if substituted.len() != 1 {
        return Err(CommandError::new(
            "invalid_template_layout",
            "The template layout could not be rendered as one document.",
        ));
    }
    Ok(substituted.into_iter().next().expect("length checked"))
}

fn substitute_template_value(
    value: &Value,
    values: &HashMap<String, ValidatedTemplateValue>,
) -> Result<Vec<Value>, CommandError> {
    let Some(object) = value.as_object() else {
        return Ok(vec![value.clone()]);
    };
    let node_type = object.get("type").and_then(Value::as_str);
    if matches!(node_type, Some(TEMPLATE_INLINE_NODE | TEMPLATE_BLOCK_NODE)) {
        let field_id = object
            .get("attrs")
            .and_then(Value::as_object)
            .and_then(|attrs| attrs.get("fieldId"))
            .and_then(Value::as_str)
            .ok_or_else(|| invalid_template_placeholder("missing field identifier"))?;
        let value = values.get(field_id).ok_or_else(|| {
            CommandError::new(
                "missing_template_value",
                "Fill in every template field before generating cards.",
            )
        })?;
        return match node_type {
            Some(TEMPLATE_INLINE_NODE) => inline_nodes_from_value(value),
            Some(TEMPLATE_BLOCK_NODE) => block_nodes_from_value(value),
            _ => unreachable!(),
        };
    }
    let mut cloned = object.clone();
    if let Some(content) = object.get("content") {
        let content = content.as_array().ok_or_else(|| {
            CommandError::new(
                "invalid_template_layout",
                "Template content must be represented as a list.",
            )
        })?;
        let mut rendered = Vec::new();
        for child in content {
            rendered.extend(substitute_template_value(child, values)?);
        }
        cloned.insert("content".to_string(), Value::Array(rendered));
    }
    Ok(vec![Value::Object(cloned)])
}

fn inline_nodes_from_value(value: &ValidatedTemplateValue) -> Result<Vec<Value>, CommandError> {
    let blocks = value
        .doc
        .get("content")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            CommandError::new(
                "invalid_template_value",
                "Inline template values must use a single paragraph.",
            )
        })?;
    if blocks.len() != 1 || blocks[0].get("type").and_then(Value::as_str) != Some("paragraph") {
        return Err(CommandError::new(
            "invalid_template_value",
            "Inline template values must use a single paragraph.",
        ));
    }
    Ok(blocks[0]
        .get("content")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default())
}

fn block_nodes_from_value(value: &ValidatedTemplateValue) -> Result<Vec<Value>, CommandError> {
    value
        .doc
        .get("content")
        .and_then(Value::as_array)
        .cloned()
        .ok_or_else(|| {
            CommandError::new(
                "invalid_template_value",
                "Block template values must use a rich-text document.",
            )
        })
}

fn content_asset_ids(content: &ValidatedContent) -> Vec<String> {
    let mut ids = content
        .front_asset_ids
        .iter()
        .chain(&content.back_asset_ids)
        .cloned()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    ids.sort();
    ids
}

fn load_assets_by_ids(
    connection: &Connection,
    paths: &AppPaths,
    asset_ids: &[String],
) -> Result<Vec<AssetDto>, CommandError> {
    if asset_ids.is_empty() {
        return Ok(Vec::new());
    }
    let wanted = asset_ids.iter().cloned().collect::<BTreeSet<_>>();
    let mut statement = connection
        .prepare(
            "SELECT id, original_filename, mime_type, byte_size,
                    relative_path, created_at
             FROM assets
             WHERE deleted_at IS NULL AND lifecycle_state = 'ready'
             ORDER BY created_at, id",
        )
        .map_err(CommandError::database)?;
    let assets = statement
        .query_map([], |row| {
            let relative_path: String = row.get(4)?;
            Ok(AssetDto {
                id: row.get(0)?,
                original_filename: row.get(1)?,
                mime_type: row.get(2)?,
                byte_size: row.get(3)?,
                file_path: paths
                    .resolve_relative(&relative_path)
                    .to_string_lossy()
                    .into_owned(),
                created_at: row.get(5)?,
            })
        })
        .map_err(CommandError::database)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(CommandError::database)?;
    Ok(assets
        .into_iter()
        .filter(|asset| wanted.contains(&asset.id))
        .collect())
}
