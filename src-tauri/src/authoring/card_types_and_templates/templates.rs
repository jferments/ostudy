use super::*;

pub(super) fn create_card_template_in_connection(
    connection: &mut Connection,
    paths: &AppPaths,
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
    let asset_ids = template_rule_asset_ids(&rules)?;
    ensure_assets_exist(&transaction, &asset_ids)?;
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
    replace_template_assets(&transaction, &template_id, &asset_ids)?;
    transaction.commit().map_err(CommandError::database)?;
    load_stored_template(connection, paths, &template_id, true).map(|stored| stored.dto)
}

pub(super) fn update_card_template_in_connection(
    connection: &mut Connection,
    paths: &AppPaths,
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
    let current: Option<i64> = transaction
        .query_row(
            "SELECT revision FROM card_templates
             WHERE id = ?1 AND deleted_at IS NULL",
            [input.template_id.as_str()],
            |row| row.get(0),
        )
        .optional()
        .map_err(CommandError::database)?;
    let Some(current_revision) = current else {
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
    ensure_template_name_available(&transaction, &normalized_name, Some(&input.template_id))?;
    let fields = validate_template_fields(&transaction, &input.template_id, input.fields, true)?;
    let rules =
        validate_template_rules(&transaction, &input.template_id, input.rules, &fields, true)?;
    let asset_ids = template_rule_asset_ids(&rules)?;
    ensure_assets_exist(&transaction, &asset_ids)?;
    let now = unix_time_millis()?;
    let changed = transaction
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
    if changed != 1 {
        return Err(CommandError::new(
            "template_changed",
            "This template changed in another window. Reload it and try again.",
        ));
    }
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
    replace_template_assets(&transaction, &input.template_id, &asset_ids)?;
    transaction.commit().map_err(CommandError::database)?;
    load_stored_template(connection, paths, &input.template_id, true).map(|stored| stored.dto)
}

pub(super) fn delete_card_template_in_connection(
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

pub(super) fn load_stored_template(
    connection: &Connection,
    paths: &AppPaths,
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
        let rows = statement
            .query_map([template_id], |row| {
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
    let stored_rules = {
        let mut statement = connection
            .prepare(
                "SELECT id, name, front_doc, back_doc, position
                 FROM card_template_rules
                 WHERE template_id = ?1 ORDER BY position ASC",
            )
            .map_err(CommandError::database)?;
        let rows = statement
            .query_map([template_id], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, Option<String>>(2)?,
                    row.get::<_, Option<String>>(3)?,
                    row.get::<_, i64>(4)?,
                ))
            })
            .map_err(CommandError::database)?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(CommandError::database)?
    };
    let rules = stored_rules
        .into_iter()
        .map(|(id, name, front_json, back_json, position)| {
            let front_json = front_json.ok_or_else(|| {
                CommandError::new(
                    "invalid_stored_template",
                    "A saved template question layout is missing.",
                )
            })?;
            let back_json = back_json.ok_or_else(|| {
                CommandError::new(
                    "invalid_stored_template",
                    "A saved template answer layout is missing.",
                )
            })?;
            Ok(CardTemplateRuleDto {
                id,
                name,
                front_doc: serde_json::from_str(&front_json)
                    .map_err(CommandError::serialization)?,
                back_doc: serde_json::from_str(&back_json)
                    .map_err(CommandError::serialization)?,
                position,
            })
        })
        .collect::<Result<Vec<_>, CommandError>>()?;
    validate_stored_template(&fields, &rules)?;
    let assets = load_template_assets(connection, paths, template_id)?;
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

fn validate_template_fields(
    connection: &Connection,
    template_id: &str,
    inputs: Vec<CardTemplateFieldInput>,
    updating: bool,
) -> Result<Vec<CardTemplateFieldDto>, CommandError> {
    if inputs.is_empty() || inputs.len() > MAX_TEMPLATE_FIELDS {
        return Err(CommandError::new(
            "invalid_template_fields",
            format!("A template must define 1 to {MAX_TEMPLATE_FIELDS} fields."),
        ));
    }
    let mut names = BTreeSet::new();
    let mut ids = BTreeSet::new();
    let mut fields = Vec::with_capacity(inputs.len());
    for (position, input) in inputs.into_iter().enumerate() {
        let name = validate_template_field_name(input.name)?;
        let normalized = normalize_template_name(&name);
        if !names.insert(normalized) {
            return Err(CommandError::new(
                "duplicate_template_field",
                "Template field names must be unique, ignoring capitalization and spacing.",
            ));
        }
        let id = resolve_template_child_id(
            connection,
            "card_template_fields",
            template_id,
            input.id,
            updating,
            "template field",
        )?;
        if !ids.insert(id.clone()) {
            return Err(CommandError::new(
                "invalid_template_fields",
                "A template field appears more than once.",
            ));
        }
        fields.push(CardTemplateFieldDto {
            id,
            name,
            description: validate_bounded_visible_text(
                input.description,
                MAX_TEMPLATE_FIELD_DESCRIPTION_CHARS,
                "Template field descriptions",
                true,
            )?,
            position: i64::try_from(position).unwrap_or(i64::MAX),
        });
    }
    Ok(fields)
}

fn validate_template_rules(
    connection: &Connection,
    template_id: &str,
    inputs: Vec<CardTemplateRuleInput>,
    fields: &[CardTemplateFieldDto],
    updating: bool,
) -> Result<Vec<CardTemplateRuleDto>, CommandError> {
    if inputs.is_empty() || inputs.len() > MAX_TEMPLATE_RULES {
        return Err(CommandError::new(
            "invalid_template_rules",
            format!("A template must define 1 to {MAX_TEMPLATE_RULES} card rules."),
        ));
    }
    let mut names = BTreeSet::new();
    let mut ids = BTreeSet::new();
    let mut rules = Vec::with_capacity(inputs.len());
    for (position, input) in inputs.into_iter().enumerate() {
        let name = validate_bounded_visible_text(
            input.name,
            MAX_TEMPLATE_RULE_NAME_CHARS,
            "Template rule names",
            false,
        )?;
        if !names.insert(normalize_template_name(&name)) {
            return Err(CommandError::new(
                "duplicate_template_rule",
                "Template rule names must be unique, ignoring capitalization and spacing.",
            ));
        }
        let id = resolve_template_child_id(
            connection,
            "card_template_rules",
            template_id,
            input.id,
            updating,
            "template rule",
        )?;
        if !ids.insert(id.clone()) {
            return Err(CommandError::new(
                "invalid_template_rules",
                "A template rule appears more than once.",
            ));
        }
        let front_placeholders = validate_template_layout(&input.front_doc, fields, "question")?;
        let back_placeholders = validate_template_layout(&input.back_doc, fields, "answer")?;
        if front_placeholders + back_placeholders == 0 {
            return Err(CommandError::new(
                "template_rule_without_field",
                "Each card rule must use at least one template field placeholder.",
            ));
        }
        rules.push(CardTemplateRuleDto {
            id,
            name,
            front_doc: input.front_doc,
            back_doc: input.back_doc,
            position: i64::try_from(position).unwrap_or(i64::MAX),
        });
    }
    Ok(rules)
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
        let placeholders = validate_template_layout(&rule.front_doc, fields, "question")?
            + validate_template_layout(&rule.back_doc, fields, "answer")?;
        if placeholders == 0 {
            return Err(CommandError::new(
                "invalid_stored_template",
                "A saved card rule does not use any fields.",
            ));
        }
    }
    Ok(())
}

fn resolve_template_child_id(
    connection: &Connection,
    table: &str,
    template_id: &str,
    supplied: Option<String>,
    updating: bool,
    label: &str,
) -> Result<String, CommandError> {
    let Some(id) = supplied else {
        return generate_id(connection);
    };
    validate_entity_id(&id, label)?;
    let sql = match table {
        "card_template_fields" => {
            "SELECT template_id FROM card_template_fields WHERE id = ?1"
        }
        "card_template_rules" => {
            "SELECT template_id FROM card_template_rules WHERE id = ?1"
        }
        _ => {
            return Err(CommandError::new(
                "invalid_template_child",
                "The template contains an unsupported child record.",
            ));
        }
    };
    let owner: Option<String> = connection
        .query_row(sql, [id.as_str()], |row| row.get(0))
        .optional()
        .map_err(CommandError::database)?;
    match (updating, owner) {
        (true, Some(owner)) if owner == template_id => Ok(id),
        (true, Some(_)) | (false, Some(_)) => Err(CommandError::new(
            "invalid_template_child",
            "A template field or rule identifier is already in use.",
        )),
        (true, None) => generate_id(connection),
        (false, None) => Ok(id),
    }
}

fn insert_template_children(
    connection: &Connection,
    template_id: &str,
    fields: &[CardTemplateFieldDto],
    rules: &[CardTemplateRuleDto],
) -> Result<(), CommandError> {
    for field in fields {
        connection
            .execute(
                "INSERT INTO card_template_fields (
                     id, template_id, name, normalized_name, description, position
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![
                    field.id,
                    template_id,
                    field.name,
                    normalize_template_name(&field.name),
                    field.description,
                    field.position,
                ],
            )
            .map_err(CommandError::database)?;
    }
    for rule in rules {
        let front_json =
            serde_json::to_string(&rule.front_doc).map_err(CommandError::serialization)?;
        let back_json =
            serde_json::to_string(&rule.back_doc).map_err(CommandError::serialization)?;
        connection
            .execute(
                "INSERT INTO card_template_rules (
                     id, template_id, name, front_doc, back_doc, position
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![
                    rule.id,
                    template_id,
                    rule.name,
                    front_json,
                    back_json,
                    rule.position,
                ],
            )
            .map_err(CommandError::database)?;
    }
    Ok(())
}

fn ensure_template_name_available(
    connection: &Connection,
    normalized_name: &str,
    excluded_id: Option<&str>,
) -> Result<(), CommandError> {
    let exists: i64 = connection
        .query_row(
            "SELECT EXISTS(
                 SELECT 1 FROM card_templates
                 WHERE normalized_name = ?1
                   AND deleted_at IS NULL
                   AND (?2 IS NULL OR id != ?2)
             )",
            params![normalized_name, excluded_id],
            |row| row.get(0),
        )
        .map_err(CommandError::database)?;
    if exists == 1 {
        return Err(CommandError::new(
            "duplicate_template_name",
            "A card template with that name already exists.",
        ));
    }
    Ok(())
}

fn validate_template_name(value: String) -> Result<String, CommandError> {
    validate_bounded_visible_text(
        value,
        MAX_TEMPLATE_NAME_CHARS,
        "Template names",
        false,
    )
}

fn validate_template_field_name(value: String) -> Result<String, CommandError> {
    let name = validate_bounded_visible_text(
        value,
        MAX_TEMPLATE_FIELD_NAME_CHARS,
        "Template field names",
        false,
    )?;
    if name.contains("{{") || name.contains("}}") {
        return Err(CommandError::new(
            "invalid_template_field",
            "Template field names cannot contain placeholder braces.",
        ));
    }
    Ok(name)
}

fn validate_bounded_visible_text(
    value: String,
    maximum: usize,
    label: &str,
    allow_empty: bool,
) -> Result<String, CommandError> {
    let normalized = if allow_empty {
        value.trim().to_string()
    } else {
        collapse_whitespace(&value)
    };
    if (!allow_empty && normalized.is_empty())
        || normalized.chars().count() > maximum
        || normalized
            .chars()
            .any(|character| character.is_control() && character != '\n' && character != '\t')
    {
        return Err(CommandError::new(
            "invalid_template_text",
            format!("{label} must contain at most {maximum} visible characters."),
        ));
    }
    Ok(normalized)
}

fn normalize_template_name(value: &str) -> String {
    collapse_whitespace(value).to_lowercase()
}

fn validate_template_layout(
    document: &Value,
    fields: &[CardTemplateFieldDto],
    side: &str,
) -> Result<usize, CommandError> {
    let bytes = serde_json::to_vec(document).map_err(CommandError::serialization)?;
    if bytes.len() > MAX_TEMPLATE_DOCUMENT_BYTES {
        return Err(CommandError::new(
            "template_layout_too_large",
            format!("The template {side} layout must be smaller than 1 MB before images."),
        ));
    }
    let field_ids = fields
        .iter()
        .map(|field| field.id.as_str())
        .collect::<BTreeSet<_>>();
    let mut count = 0;
    let sanitized = sanitize_template_node(document, None, &field_ids, &mut count)?;
    crate::cards::validate_document(&sanitized)?;
    Ok(count)
}

fn sanitize_template_node(
    value: &Value,
    parent_type: Option<&str>,
    field_ids: &BTreeSet<&str>,
    placeholder_count: &mut usize,
) -> Result<Value, CommandError> {
    let object = value.as_object().ok_or_else(|| {
        CommandError::new(
            "invalid_template_layout",
            "Every template layout node must be a JSON object.",
        )
    })?;
    let node_type = object.get("type").and_then(Value::as_str).ok_or_else(|| {
        CommandError::new(
            "invalid_template_layout",
            "A template layout node is missing its type.",
        )
    })?;
    if matches!(node_type, TEMPLATE_INLINE_NODE | TEMPLATE_BLOCK_NODE) {
        if object
            .keys()
            .any(|key| !matches!(key.as_str(), "type" | "attrs"))
        {
            return Err(CommandError::new(
                "invalid_template_placeholder",
                "A template field placeholder contains unsupported data.",
            ));
        }
        let attrs = object
            .get("attrs")
            .and_then(Value::as_object)
            .ok_or_else(|| {
                CommandError::new(
                    "invalid_template_placeholder",
                    "A template field placeholder is missing its field identifier.",
                )
            })?;
        if attrs
            .keys()
            .any(|key| !matches!(key.as_str(), "fieldId" | "label"))
        {
            return Err(CommandError::new(
                "invalid_template_placeholder",
                "A template field placeholder contains unsupported attributes.",
            ));
        }
        let field_id = attrs
            .get("fieldId")
            .and_then(Value::as_str)
            .ok_or_else(|| {
                CommandError::new(
                    "invalid_template_placeholder",
                    "A template field placeholder is missing its field identifier.",
                )
            })?;
        validate_entity_id(field_id, "template field")?;
        if !field_ids.contains(field_id) {
            return Err(CommandError::new(
                "unknown_template_field",
                "A card layout refers to a field that does not exist in this template.",
            ));
        }
        if let Some(label) = attrs.get("label") {
            let label = label.as_str().ok_or_else(|| {
                CommandError::new(
                    "invalid_template_placeholder",
                    "A template field placeholder label is invalid.",
                )
            })?;
            if label.chars().count() > MAX_TEMPLATE_FIELD_NAME_CHARS {
                return Err(CommandError::new(
                    "invalid_template_placeholder",
                    "A template field placeholder label is too long.",
                ));
            }
        }
        match node_type {
            TEMPLATE_INLINE_NODE if matches!(parent_type, Some("paragraph") | Some("heading")) => {}
            TEMPLATE_BLOCK_NODE if matches!(parent_type, Some("doc") | Some("blockquote")) => {}
            TEMPLATE_INLINE_NODE => {
                return Err(CommandError::new(
                    "invalid_template_placeholder",
                    "Inline field placeholders must be placed inside a paragraph or heading.",
                ));
            }
            _ => {
                return Err(CommandError::new(
                    "invalid_template_placeholder",
                    "Block field placeholders must be placed on their own line.",
                ));
            }
        }
        *placeholder_count += 1;
        return Ok(if node_type == TEMPLATE_INLINE_NODE {
            json!({"type": "text", "text": "field"})
        } else {
            json!({
                "type": "paragraph",
                "content": [{"type": "text", "text": "field"}]
            })
        });
    }

    let mut sanitized = object.clone();
    if let Some(content) = object.get("content") {
        let children = content.as_array().ok_or_else(|| {
            CommandError::new(
                "invalid_template_layout",
                "Template layout content must be represented as a list.",
            )
        })?;
        let sanitized_children = children
            .iter()
            .map(|child| {
                sanitize_template_node(child, Some(node_type), field_ids, placeholder_count)
            })
            .collect::<Result<Vec<_>, _>>()?;
        sanitized.insert("content".to_string(), Value::Array(sanitized_children));
    }
    Ok(Value::Object(sanitized))
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
    let field_ids = fields
        .iter()
        .map(|field| field.id.as_str())
        .collect::<BTreeSet<_>>();
    let mut result = HashMap::new();
    let mut all_assets = BTreeSet::new();
    for value in values {
        validate_entity_id(&value.field_id, "template field")?;
        if !field_ids.contains(value.field_id.as_str()) || result.contains_key(&value.field_id) {
            return Err(CommandError::new(
                "invalid_template_value",
                "A template value does not match one unique field.",
            ));
        }
        crate::cards::validate_document(&value.doc)?;
        let serialized = serde_json::to_vec(&value.doc).map_err(CommandError::serialization)?;
        if serialized.len() > MAX_TEMPLATE_DOCUMENT_BYTES {
            return Err(CommandError::new(
                "template_value_too_large",
                "Each template field must be smaller than 1 MB before images.",
            ));
        }
        if value.text.chars().count() > MAX_TEMPLATE_VALUE_CHARS {
            return Err(CommandError::new(
                "invalid_template_value",
                format!(
                    "Each template field can contain at most {MAX_TEMPLATE_VALUE_CHARS} text characters."
                ),
            ));
        }
        let derived_text = extract_template_plain_text(&value.doc)?;
        let asset_ids = crate::cards::collect_asset_ids(&value.doc)?;
        if derived_text.is_empty() && asset_ids.is_empty() {
            return Err(CommandError::new(
                "missing_template_value",
                "Fill in every template field before generating cards.",
            ));
        }
        for asset_id in &asset_ids {
            all_assets.insert(asset_id.clone());
        }
        result.insert(
            value.field_id,
            ValidatedTemplateValue {
                doc: value.doc,
                asset_ids,
            },
        );
    }
    ensure_assets_exist(connection, &all_assets.into_iter().collect::<Vec<_>>())?;
    Ok(result)
}

fn extract_template_plain_text(document: &Value) -> Result<String, CommandError> {
    fn visit(value: &Value, output: &mut String) -> Result<(), CommandError> {
        let object = value.as_object().ok_or_else(|| {
            CommandError::new(
                "invalid_template_value",
                "A template field contains an invalid document node.",
            )
        })?;
        let node_type = object
            .get("type")
            .and_then(Value::as_str)
            .unwrap_or_default();
        if node_type == "text" {
            if let Some(text) = object.get("text").and_then(Value::as_str) {
                output.push_str(text);
            }
            return Ok(());
        }
        if node_type == "hardBreak" {
            output.push('\n');
            return Ok(());
        }
        let before = output.len();
        if let Some(content) = object.get("content") {
            let content = content.as_array().ok_or_else(|| {
                CommandError::new(
                    "invalid_template_value",
                    "A template field contains invalid nested content.",
                )
            })?;
            for child in content {
                visit(child, output)?;
            }
        }
        if matches!(
            node_type,
            "paragraph" | "heading" | "blockquote" | "listItem" | "codeBlock"
        ) && output.len() > before
            && !output.ends_with('\n')
        {
            output.push('\n');
        }
        Ok(())
    }

    let mut text = String::new();
    visit(document, &mut text)?;
    Ok(text.trim().to_string())
}

fn render_template_document(
    layout: &Value,
    values: &HashMap<String, ValidatedTemplateValue>,
) -> Result<Value, CommandError> {
    let root = layout.as_object().ok_or_else(|| {
        CommandError::new("invalid_template_layout", "The template layout is invalid.")
    })?;
    if root.get("type").and_then(Value::as_str) != Some("doc") {
        return Err(CommandError::new(
            "invalid_template_layout",
            "The template layout is missing its document root.",
        ));
    }
    let mut rendered = root.clone();
    let content = root
        .get("content")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            CommandError::new(
                "invalid_template_layout",
                "The template layout must contain at least one block.",
            )
        })?;
    rendered.insert(
        "content".to_string(),
        Value::Array(render_template_children("doc", content, values)?),
    );
    Ok(Value::Object(rendered))
}

fn render_template_children(
    parent_type: &str,
    children: &[Value],
    values: &HashMap<String, ValidatedTemplateValue>,
) -> Result<Vec<Value>, CommandError> {
    let mut rendered = Vec::new();
    for child in children {
        let child_type = child
            .get("type")
            .and_then(Value::as_str)
            .unwrap_or_default();
        if child_type == TEMPLATE_BLOCK_NODE {
            if !matches!(parent_type, "doc" | "blockquote") {
                return Err(CommandError::new(
                    "invalid_template_placeholder",
                    "Block field placeholders must be placed on their own line.",
                ));
            }
            let field = template_value_for_placeholder(child, values)?;
            let blocks = field
                .doc
                .get("content")
                .and_then(Value::as_array)
                .ok_or_else(|| {
                    CommandError::new(
                        "invalid_template_value",
                        "A template field contains an invalid document.",
                    )
                })?;
            rendered.extend(blocks.iter().cloned());
            continue;
        }
        if child_type == TEMPLATE_INLINE_NODE {
            if !matches!(parent_type, "paragraph" | "heading") {
                return Err(CommandError::new(
                    "invalid_template_placeholder",
                    "Inline field placeholders must be placed inside a paragraph or heading.",
                ));
            }
            let field = template_value_for_placeholder(child, values)?;
            rendered.extend(inline_nodes_for_template_value(field)?);
            continue;
        }
        let object = child.as_object().ok_or_else(|| {
            CommandError::new(
                "invalid_template_layout",
                "Every template layout node must be a JSON object.",
            )
        })?;
        let mut next = object.clone();
        if let Some(content) = object.get("content") {
            let content = content.as_array().ok_or_else(|| {
                CommandError::new(
                    "invalid_template_layout",
                    "Template layout content must be represented as a list.",
                )
            })?;
            next.insert(
                "content".to_string(),
                Value::Array(render_template_children(child_type, content, values)?),
            );
        }
        rendered.push(Value::Object(next));
    }
    Ok(rendered)
}

fn template_value_for_placeholder<'a>(
    placeholder: &Value,
    values: &'a HashMap<String, ValidatedTemplateValue>,
) -> Result<&'a ValidatedTemplateValue, CommandError> {
    let field_id = placeholder
        .get("attrs")
        .and_then(Value::as_object)
        .and_then(|attrs| attrs.get("fieldId"))
        .and_then(Value::as_str)
        .ok_or_else(|| {
            CommandError::new(
                "invalid_template_placeholder",
                "A template field placeholder is missing its field identifier.",
            )
        })?;
    values.get(field_id).ok_or_else(|| {
        CommandError::new(
            "missing_template_value",
            "Fill in every template field before generating cards.",
        )
    })
}

fn inline_nodes_for_template_value(
    value: &ValidatedTemplateValue,
) -> Result<Vec<Value>, CommandError> {
    let blocks = value
        .doc
        .get("content")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            CommandError::new(
                "invalid_template_value",
                "A template field contains an invalid document.",
            )
        })?;
    if blocks.len() != 1
        || blocks[0].get("type").and_then(Value::as_str) != Some("paragraph")
        || !value.asset_ids.is_empty()
    {
        return Err(CommandError::new(
            "template_block_content_inline",
            "A field containing a list, image, heading, or multiple paragraphs must be inserted as a block on its own line.",
        ));
    }
    let content = blocks[0]
        .get("content")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    if content.iter().any(|node| {
        !matches!(
            node.get("type").and_then(Value::as_str),
            Some("text") | Some("hardBreak")
        )
    }) {
        return Err(CommandError::new(
            "template_block_content_inline",
            "A field containing block content must be inserted as a block on its own line.",
        ));
    }
    Ok(content)
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
            let front_doc = render_template_document(&rule.front_doc, &values)?;
            let back_doc = render_template_document(&rule.back_doc, &values)?;
            let content = validate_content(front_doc, back_doc, String::new(), String::new())?;
            Ok(RenderedTemplateCard {
                rule_id: rule.id.clone(),
                rule_name: rule.name.clone(),
                content,
            })
        })
        .collect()
}

fn render_template_previews(
    connection: &Connection,
    paths: &AppPaths,
    template: &StoredTemplate,
    values: Vec<TemplateValueInput>,
) -> Result<Vec<TemplateCardPreviewDto>, CommandError> {
    render_template_cards(connection, template, values)?
        .into_iter()
        .map(|rendered| {
            let front_doc: Value = serde_json::from_str(&rendered.content.front_doc_json)
                .map_err(CommandError::serialization)?;
            let back_doc: Value = serde_json::from_str(&rendered.content.back_doc_json)
                .map_err(CommandError::serialization)?;
            let asset_ids = content_asset_ids(&rendered.content);
            let assets = load_assets_by_ids(connection, paths, &asset_ids)?;
            Ok(TemplateCardPreviewDto {
                rule_id: rendered.rule_id,
                rule_name: rendered.rule_name,
                question_doc: front_doc,
                answer_doc: back_doc,
                question_text: rendered.content.front_text,
                answer_text: rendered.content.back_text,
                assets,
            })
        })
        .collect()
}

fn generate_template_cards_in_connection(
    connection: &mut Connection,
    paths: &AppPaths,
    input: GenerateTemplateCardsInput,
) -> Result<GenerateTemplateCardsResultDto, CommandError> {
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(CommandError::database)?;
    let template = load_stored_template(&transaction, paths, &input.template_id, true)?;
    ensure_template_revision(&template, input.template_revision)?;
    let rendered_cards = render_template_cards(&transaction, &template, input.values)?;
    let tags = validate_tags(input.tags)?;
    ensure_active_deck(&transaction, &input.deck_id)?;
    let now = unix_time_millis()?;
    let mut created_note_ids = Vec::new();
    let mut skipped_duplicates = 0_i64;
    let mut seen = BTreeSet::new();

    for rendered in rendered_cards {
        let normalized = normalized_pair(&rendered.content.front_text, &rendered.content.back_text);
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

pub(super) fn ensure_template_revision(
    template: &StoredTemplate,
    expected_revision: i64,
) -> Result<(), CommandError> {
    if expected_revision < 1 || template.dto.revision != expected_revision {
        return Err(CommandError::new(
            "template_changed",
            "This template changed after the entry form was opened. Reload it and review the generated cards again.",
        ));
    }
    Ok(())
}

fn template_rule_asset_ids(rules: &[CardTemplateRuleDto]) -> Result<Vec<String>, CommandError> {
    let mut ids = BTreeSet::new();
    for rule in rules {
        ids.extend(crate::cards::collect_asset_ids(&rule.front_doc)?);
        ids.extend(crate::cards::collect_asset_ids(&rule.back_doc)?);
    }
    Ok(ids.into_iter().collect())
}

fn replace_template_assets(
    connection: &Connection,
    template_id: &str,
    asset_ids: &[String],
) -> Result<(), CommandError> {
    connection
        .execute(
            "DELETE FROM card_template_assets WHERE template_id = ?1",
            [template_id],
        )
        .map_err(CommandError::database)?;
    for asset_id in asset_ids {
        connection
            .execute(
                "INSERT INTO card_template_assets (template_id, asset_id) VALUES (?1, ?2)",
                params![template_id, asset_id],
            )
            .map_err(CommandError::database)?;
    }
    Ok(())
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
             FROM assets a
             JOIN card_template_assets cta ON cta.asset_id = a.id
             WHERE cta.template_id = ?1 AND a.deleted_at IS NULL
             ORDER BY a.created_at ASC, a.id ASC",
        )
        .map_err(CommandError::database)?;
    let rows = statement
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
        .map_err(CommandError::database)?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(CommandError::database)
}

fn load_assets_by_ids(
    connection: &Connection,
    paths: &AppPaths,
    asset_ids: &[String],
) -> Result<Vec<AssetDto>, CommandError> {
    let mut assets = Vec::new();
    for asset_id in asset_ids {
        let asset = connection
            .query_row(
                "SELECT id, original_filename, mime_type, byte_size, relative_path, created_at
                 FROM assets WHERE id = ?1 AND deleted_at IS NULL",
                [asset_id],
                |row| {
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
                },
            )
            .optional()
            .map_err(CommandError::database)?
            .ok_or_else(|| {
                CommandError::new(
                    "asset_not_found",
                    "One of the images used by the generated card is no longer available.",
                )
            })?;
        assets.push(asset);
    }
    Ok(assets)
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CardTemplateSummaryDto {
    pub id: String,
    pub name: String,
    pub description: String,
    pub field_count: i64,
    pub generated_card_rule_count: i64,
    pub image_count: i64,
    pub updated_at: i64,
    pub revision: i64,
}

#[tauri::command]
pub fn list_card_template_summaries(
    database: State<'_, Database>,
) -> Result<Vec<CardTemplateSummaryDto>, CommandError> {
    database.with_connection(list_card_template_summaries_in_connection)
}

fn list_card_template_summaries_in_connection(
    connection: &mut Connection,
) -> Result<Vec<CardTemplateSummaryDto>, CommandError> {
    let mut statement = connection
        .prepare(
            "SELECT
                 ct.id,
                 ct.name,
                 ct.description,
                 COUNT(DISTINCT ctf.id),
                 COUNT(DISTINCT ctr.id),
                 COUNT(DISTINCT CASE
                     WHEN a.deleted_at IS NULL AND a.lifecycle_state = 'ready' THEN a.id
                 END),
                 ct.updated_at,
                 ct.revision
             FROM card_templates ct
             LEFT JOIN card_template_fields ctf ON ctf.template_id = ct.id
             LEFT JOIN card_template_rules ctr ON ctr.template_id = ct.id
             LEFT JOIN card_template_assets cta ON cta.template_id = ct.id
             LEFT JOIN assets a ON a.id = cta.asset_id
             WHERE ct.deleted_at IS NULL
             GROUP BY ct.id, ct.name, ct.description, ct.updated_at, ct.revision
             ORDER BY ct.name COLLATE NOCASE ASC, ct.created_at ASC, ct.id ASC",
        )
        .map_err(CommandError::database)?;
    let summaries = statement
        .query_map([], |row| {
            Ok(CardTemplateSummaryDto {
                id: row.get(0)?,
                name: row.get(1)?,
                description: row.get(2)?,
                field_count: row.get(3)?,
                generated_card_rule_count: row.get(4)?,
                image_count: row.get(5)?,
                updated_at: row.get(6)?,
                revision: row.get(7)?,
            })
        })
        .map_err(CommandError::database)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(CommandError::database)?;
    Ok(summaries)
}

#[cfg(test)]
mod template_summary_tests {
    use super::*;

    #[test]
    fn empty_template_library_returns_an_empty_summary_list() {
        let database = Database::open_in_memory().expect("database should open");
        let summaries = database
            .with_connection(list_card_template_summaries_in_connection)
            .expect("summaries should load");
        assert!(summaries.is_empty());
    }
}

mod automatic_layout;
mod repair;
mod stable_update;

use automatic_layout::{
    automatic_template_field_lookup_from_dtos, normalize_typed_template_layout,
    AutomaticTemplateFieldLookup,
};
#[cfg(test)]
use automatic_layout::materialize_automatic_template_document;
pub(super) use automatic_layout::{
    generate_automatic_template_cards_in_connection, normalize_card_template_dto_for_editing,
    prepare_automatic_create_template_input, prepare_automatic_update_template_input,
    render_automatic_template_previews, template_update_contains_new_children,
};
pub(super) use repair::apply_stage3_data_repairs;
pub(super) use stable_update::update_card_template_with_stable_children_in_connection;

#[cfg(test)]
mod asset_tests;
#[cfg(test)]
mod tests;
