fn pattern_has_placeholder(pattern: &str) -> bool {
    pattern.contains("{{")
}

fn render_template_previews(
    template: &StoredTemplate,
    values: Vec<TemplateValueInput>,
) -> Result<Vec<TemplateCardPreviewDto>, CommandError> {
    let values = validate_template_values(&template.dto.fields, values)?;
    template
        .dto
        .rules
        .iter()
        .map(|rule| {
            let question = render_pattern(
                &rule.front_pattern,
                &template.normalized_fields,
                &values,
            )?;
            let answer = render_pattern(
                &rule.back_pattern,
                &template.normalized_fields,
                &values,
            )?;
            if question.trim().is_empty() || answer.trim().is_empty() {
                return Err(CommandError::new(
                    "empty_template_card",
                    format!("The template rule “{}” produced an empty side.", rule.name),
                ));
            }
            Ok(TemplateCardPreviewDto {
                rule_id: rule.id.clone(),
                rule_name: rule.name.clone(),
                question,
                answer,
            })
        })
        .collect()
}

fn validate_template_values(
    fields: &[CardTemplateFieldDto],
    values: Vec<TemplateValueInput>,
) -> Result<HashMap<String, String>, CommandError> {
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
    for value in values {
        validate_entity_id(&value.field_id, "template field")?;
        if !field_ids.contains(value.field_id.as_str()) || result.contains_key(&value.field_id) {
            return Err(CommandError::new(
                "invalid_template_value",
                "A template value does not match one unique field.",
            ));
        }
        let trimmed = value.value.trim().to_string();
        if trimmed.is_empty()
            || trimmed.chars().count() > MAX_TEMPLATE_VALUE_CHARS
            || trimmed
                .chars()
                .any(|character| character.is_control() && character != '\n' && character != '\t')
        {
            return Err(CommandError::new(
                "invalid_template_value",
                format!(
                    "Each template field must contain 1 to {MAX_TEMPLATE_VALUE_CHARS} characters."
                ),
            ));
        }
        result.insert(value.field_id, trimmed);
    }
    Ok(result)
}

fn render_pattern(
    pattern: &str,
    normalized_fields: &HashMap<String, String>,
    values: &HashMap<String, String>,
) -> Result<String, CommandError> {
    let mut rendered = String::with_capacity(pattern.len());
    let mut cursor = 0;
    while let Some(relative_open) = pattern[cursor..].find("{{") {
        let open = cursor + relative_open;
        rendered.push_str(&pattern[cursor..open]);
        let after_open = open + 2;
        let relative_close = pattern[after_open..]
            .find("}}")
            .ok_or_else(invalid_template_placeholder)?;
        let close = after_open + relative_close;
        let placeholder = normalize_template_name(&pattern[after_open..close]);
        let field_id = normalized_fields.get(&placeholder).ok_or_else(|| {
            CommandError::new(
                "unknown_template_field",
                "A saved template refers to a field that no longer exists.",
            )
        })?;
        let value = values.get(field_id).ok_or_else(|| {
            CommandError::new(
                "missing_template_value",
                "Fill in every template field before generating cards.",
            )
        })?;
        rendered.push_str(value);
        if rendered.chars().count() > MAX_TEMPLATE_OUTPUT_CHARS {
            return Err(CommandError::new(
                "template_output_too_large",
                format!(
                    "A generated card side cannot exceed {MAX_TEMPLATE_OUTPUT_CHARS} characters."
                ),
            ));
        }
        cursor = close + 2;
    }
    rendered.push_str(&pattern[cursor..]);
    if rendered.chars().count() > MAX_TEMPLATE_OUTPUT_CHARS {
        return Err(CommandError::new(
            "template_output_too_large",
            format!(
                "A generated card side cannot exceed {MAX_TEMPLATE_OUTPUT_CHARS} characters."
            ),
        ));
    }
    Ok(rendered)
}

fn generate_template_cards_in_connection(
    connection: &mut Connection,
    input: GenerateTemplateCardsInput,
) -> Result<GenerateTemplateCardsResultDto, CommandError> {
    let template = load_stored_template(connection, &input.template_id, true)?;
    let previews = render_template_previews(&template, input.values)?;
    let tags = validate_tags(input.tags)?;
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(CommandError::database)?;
    ensure_active_deck(&transaction, &input.deck_id)?;
    let now = unix_time_millis()?;
    let mut created_note_ids = Vec::new();
    let mut skipped_duplicates = 0_i64;
    let mut seen = BTreeSet::new();

    for preview in previews {
        let content = validate_content(
            plain_text_document(&preview.question),
            plain_text_document(&preview.answer),
            preview.question,
            preview.answer,
        )?;
        let normalized = normalized_pair(&content.front_text, &content.back_text);
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
        let note_id = insert_organized_card(
            &transaction,
            &input.deck_id,
            content,
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
