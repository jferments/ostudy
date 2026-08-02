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
                "Template field guidance",
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
        let front_pattern = validate_pattern(&input.front_pattern, fields, "question")?;
        let back_pattern = validate_pattern(&input.back_pattern, fields, "answer")?;
        if !pattern_has_placeholder(&front_pattern) && !pattern_has_placeholder(&back_pattern) {
            return Err(CommandError::new(
                "template_rule_without_field",
                "Each card rule must use at least one template field placeholder.",
            ));
        }
        rules.push(CardTemplateRuleDto {
            id,
            name,
            front_pattern,
            back_pattern,
            position: i64::try_from(position).unwrap_or(i64::MAX),
        });
    }
    Ok(rules)
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
    if !updating {
        return generate_id(connection);
    }
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
    match owner {
        Some(owner) if owner == template_id => Ok(id),
        Some(_) => Err(CommandError::new(
            "invalid_template_child",
            "A template field or rule belongs to a different template.",
        )),
        None => generate_id(connection),
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
        connection
            .execute(
                "INSERT INTO card_template_rules (
                     id, template_id, name, front_pattern, back_pattern, position
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![
                    rule.id,
                    template_id,
                    rule.name,
                    rule.front_pattern,
                    rule.back_pattern,
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

fn validate_pattern(
    value: &str,
    fields: &[CardTemplateFieldDto],
    side: &str,
) -> Result<String, CommandError> {
    let pattern = value.trim().to_string();
    if pattern.is_empty()
        || pattern.chars().count() > MAX_TEMPLATE_PATTERN_CHARS
        || pattern
            .chars()
            .any(|character| character.is_control() && character != '\n' && character != '\t')
    {
        return Err(CommandError::new(
            "invalid_template_pattern",
            format!(
                "Each template {side} pattern must contain 1 to {MAX_TEMPLATE_PATTERN_CHARS} characters."
            ),
        ));
    }
    let known = fields
        .iter()
        .map(|field| normalize_template_name(&field.name))
        .collect::<BTreeSet<_>>();
    for placeholder in parse_pattern_placeholders(&pattern)? {
        if !known.contains(&normalize_template_name(&placeholder)) {
            return Err(CommandError::new(
                "unknown_template_field",
                format!("The template {side} uses an unknown field: {placeholder}."),
            ));
        }
    }
    Ok(pattern)
}

fn parse_pattern_placeholders(pattern: &str) -> Result<Vec<String>, CommandError> {
    let mut values = Vec::new();
    let mut cursor = 0;
    while let Some(relative_open) = pattern[cursor..].find("{{") {
        let open = cursor + relative_open;
        if pattern[cursor..open].contains("}}") {
            return Err(invalid_template_placeholder());
        }
        let after_open = open + 2;
        let relative_close = pattern[after_open..]
            .find("}}")
            .ok_or_else(invalid_template_placeholder)?;
        let close = after_open + relative_close;
        let raw = &pattern[after_open..close];
        if raw.contains("{{") || raw.trim().is_empty() {
            return Err(invalid_template_placeholder());
        }
        values.push(collapse_whitespace(raw));
        cursor = close + 2;
    }
    if pattern[cursor..].contains("{{") || pattern[cursor..].contains("}}") {
        return Err(invalid_template_placeholder());
    }
    Ok(values)
}

fn invalid_template_placeholder() -> CommandError {
    CommandError::new(
        "invalid_template_placeholder",
        "Template placeholders must use a complete data-only form such as {{name}}.",
    )
}
