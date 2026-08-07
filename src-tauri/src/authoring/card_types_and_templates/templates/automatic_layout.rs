use super::*;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum TemplateValuePlacement {
    Inline,
    Block,
}

#[derive(Clone, Debug)]
struct AutomaticTemplateField {
    id: String,
    name: String,
}

pub(super) struct AutomaticTemplateFieldLookup {
    by_id: HashMap<String, AutomaticTemplateField>,
    by_name: HashMap<String, AutomaticTemplateField>,
}

pub(in super::super) fn prepare_automatic_create_template_input(
    connection: &Connection,
    mut input: CreateCardTemplateInput,
) -> Result<CreateCardTemplateInput, CommandError> {
    assign_missing_template_input_ids(connection, &mut input.fields, &mut input.rules)?;
    let lookup = automatic_template_field_lookup_from_inputs(&input.fields)?;
    normalize_template_rule_inputs(&mut input.rules, &lookup)?;
    Ok(input)
}

pub(in super::super) fn prepare_automatic_update_template_input(
    connection: &Connection,
    mut input: UpdateCardTemplateInput,
) -> Result<UpdateCardTemplateInput, CommandError> {
    assign_missing_template_input_ids(connection, &mut input.fields, &mut input.rules)?;
    let lookup = automatic_template_field_lookup_from_inputs(&input.fields)?;
    normalize_template_rule_inputs(&mut input.rules, &lookup)?;
    Ok(input)
}

fn assign_missing_template_input_ids(
    connection: &Connection,
    fields: &mut [CardTemplateFieldInput],
    rules: &mut [CardTemplateRuleInput],
) -> Result<(), CommandError> {
    for field in fields {
        if field.id.is_none() {
            field.id = Some(generate_id(connection)?);
        }
        if let Some(field_id) = field.id.as_deref() {
            validate_entity_id(field_id, "template field")?;
        }
    }
    for rule in rules {
        if rule.id.is_none() {
            rule.id = Some(generate_id(connection)?);
        }
        if let Some(rule_id) = rule.id.as_deref() {
            validate_entity_id(rule_id, "template rule")?;
        }
    }
    Ok(())
}

fn normalize_template_rule_inputs(
    rules: &mut [CardTemplateRuleInput],
    lookup: &AutomaticTemplateFieldLookup,
) -> Result<(), CommandError> {
    for rule in rules {
        rule.front_doc = normalize_typed_template_layout(&rule.front_doc, lookup, "question")?;
        rule.back_doc = normalize_typed_template_layout(&rule.back_doc, lookup, "answer")?;
    }
    Ok(())
}

pub(in super::super) fn normalize_card_template_dto_for_editing(
    mut template: CardTemplateDto,
) -> Result<CardTemplateDto, CommandError> {
    let lookup = automatic_template_field_lookup_from_dtos(&template.fields)?;
    for rule in &mut template.rules {
        rule.front_doc = normalize_typed_template_layout(&rule.front_doc, &lookup, "question")?;
        rule.back_doc = normalize_typed_template_layout(&rule.back_doc, &lookup, "answer")?;
    }
    Ok(template)
}

fn automatic_template_field_lookup_from_inputs(
    fields: &[CardTemplateFieldInput],
) -> Result<AutomaticTemplateFieldLookup, CommandError> {
    let mut by_id = HashMap::new();
    let mut by_name = HashMap::new();
    for field in fields {
        let id = field.id.as_deref().ok_or_else(|| {
            CommandError::new(
                "invalid_template_field",
                "Every template field must have a stable identifier.",
            )
        })?;
        validate_entity_id(id, "template field")?;
        let name = collapse_whitespace(&field.name);
        if name.is_empty() {
            return Err(CommandError::new(
                "invalid_template_field",
                "Give every template field a name before using it in a card layout.",
            ));
        }
        let reference = AutomaticTemplateField {
            id: id.to_string(),
            name: name.clone(),
        };
        if by_id.insert(id.to_string(), reference.clone()).is_some()
            || by_name
                .insert(normalize_template_name(&name), reference)
                .is_some()
        {
            return Err(CommandError::new(
                "duplicate_template_field",
                "Template field names and identifiers must be unique.",
            ));
        }
    }
    Ok(AutomaticTemplateFieldLookup { by_id, by_name })
}

pub(super) fn automatic_template_field_lookup_from_dtos(
    fields: &[CardTemplateFieldDto],
) -> Result<AutomaticTemplateFieldLookup, CommandError> {
    let inputs = fields
        .iter()
        .map(|field| CardTemplateFieldInput {
            id: Some(field.id.clone()),
            name: field.name.clone(),
            description: field.description.clone(),
        })
        .collect::<Vec<_>>();
    automatic_template_field_lookup_from_inputs(&inputs)
}

pub(super) fn normalize_typed_template_layout(
    document: &Value,
    lookup: &AutomaticTemplateFieldLookup,
    side: &str,
) -> Result<Value, CommandError> {
    let mut placeholder_count = 0_usize;
    let normalized =
        normalize_template_layout_node(document, None, lookup, &mut placeholder_count)?;
    if normalized.len() != 1 || normalized[0].get("type").and_then(Value::as_str) != Some("doc") {
        return Err(CommandError::new(
            "invalid_template_layout",
            format!("The template {side} layout must have one document root."),
        ));
    }
    Ok(normalized.into_iter().next().expect("length checked"))
}

fn normalize_template_layout_node(
    value: &Value,
    parent_type: Option<&str>,
    lookup: &AutomaticTemplateFieldLookup,
    placeholder_count: &mut usize,
) -> Result<Vec<Value>, CommandError> {
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

    if node_type == "text" && matches!(parent_type, Some("paragraph") | Some("heading")) {
        return split_typed_template_text_node(object, lookup, placeholder_count);
    }

    if matches!(node_type, TEMPLATE_INLINE_NODE | TEMPLATE_BLOCK_NODE) {
        let field = resolve_automatic_placeholder_field(value, lookup)?;
        *placeholder_count += 1;
        let placeholder = automatic_inline_placeholder(field, object.get("marks"));
        if node_type == TEMPLATE_INLINE_NODE {
            if !matches!(parent_type, Some("paragraph") | Some("heading")) {
                return Err(CommandError::new(
                    "invalid_template_placeholder",
                    "Template field placeholders must be typed inside a paragraph or heading.",
                ));
            }
            return Ok(vec![placeholder]);
        }
        if !matches!(
            parent_type,
            Some("doc") | Some("blockquote") | Some("listItem")
        ) {
            return Err(CommandError::new(
                "invalid_template_placeholder",
                "A legacy block field placeholder is in an unsupported location.",
            ));
        }
        return Ok(vec![json!({
            "type": "paragraph",
            "content": [placeholder]
        })]);
    }

    let mut normalized = object.clone();
    if let Some(content) = object.get("content") {
        let children = content.as_array().ok_or_else(|| {
            CommandError::new(
                "invalid_template_layout",
                "Template layout content must be represented as a list.",
            )
        })?;
        let mut normalized_children = Vec::new();
        for child in children {
            normalized_children.extend(normalize_template_layout_node(
                child,
                Some(node_type),
                lookup,
                placeholder_count,
            )?);
        }
        normalized.insert("content".to_string(), Value::Array(normalized_children));
    }
    Ok(vec![Value::Object(normalized)])
}

fn split_typed_template_text_node(
    object: &serde_json::Map<String, Value>,
    lookup: &AutomaticTemplateFieldLookup,
    placeholder_count: &mut usize,
) -> Result<Vec<Value>, CommandError> {
    let text = object.get("text").and_then(Value::as_str).ok_or_else(|| {
        CommandError::new(
            "invalid_template_layout",
            "A template text node is missing its text value.",
        )
    })?;
    let mut result = Vec::new();
    let mut cursor = 0_usize;

    while cursor < text.len() {
        let remaining = &text[cursor..];
        let next_open = remaining.find("{{");
        let next_close = remaining.find("}}");
        if let Some(close) = next_close {
            if next_open.is_none_or(|open| close < open) {
                return Err(invalid_typed_template_placeholder());
            }
        }
        let Some(relative_open) = next_open else {
            push_template_text_fragment(&mut result, object, remaining);
            cursor = text.len();
            break;
        };
        let open = cursor + relative_open;
        push_template_text_fragment(&mut result, object, &text[cursor..open]);
        let body_start = open + 2;
        let relative_close = text[body_start..]
            .find("}}")
            .ok_or_else(invalid_typed_template_placeholder)?;
        let close = body_start + relative_close;
        let raw_name = &text[body_start..close];
        if raw_name.contains("{{") || raw_name.contains("}}") {
            return Err(invalid_typed_template_placeholder());
        }
        let normalized_name = normalize_template_name(raw_name);
        let field = lookup.by_name.get(&normalized_name).ok_or_else(|| {
            CommandError::new(
                "unknown_template_field",
                format!(
                    "The placeholder {{{{{}}}}} does not match a field in this template.",
                    collapse_whitespace(raw_name)
                ),
            )
        })?;
        result.push(automatic_inline_placeholder(field, object.get("marks")));
        *placeholder_count += 1;
        cursor = close + 2;
    }

    if cursor == 0 {
        push_template_text_fragment(&mut result, object, text);
    }
    Ok(result)
}

fn push_template_text_fragment(
    output: &mut Vec<Value>,
    template: &serde_json::Map<String, Value>,
    text: &str,
) {
    if text.is_empty() {
        return;
    }
    let mut fragment = template.clone();
    fragment.insert("text".to_string(), Value::String(text.to_string()));
    output.push(Value::Object(fragment));
}

fn automatic_inline_placeholder(field: &AutomaticTemplateField, marks: Option<&Value>) -> Value {
    let mut placeholder = serde_json::Map::new();
    placeholder.insert(
        "type".to_string(),
        Value::String(TEMPLATE_INLINE_NODE.to_string()),
    );
    placeholder.insert(
        "attrs".to_string(),
        json!({"fieldId": field.id, "label": field.name}),
    );
    if let Some(marks) = marks {
        placeholder.insert("marks".to_string(), marks.clone());
    }
    Value::Object(placeholder)
}

fn resolve_automatic_placeholder_field<'a>(
    placeholder: &Value,
    lookup: &'a AutomaticTemplateFieldLookup,
) -> Result<&'a AutomaticTemplateField, CommandError> {
    let attrs = placeholder
        .get("attrs")
        .and_then(Value::as_object)
        .ok_or_else(|| {
            CommandError::new(
                "invalid_template_placeholder",
                "A template field placeholder is missing its attributes.",
            )
        })?;
    if let Some(field_id) = attrs.get("fieldId").and_then(Value::as_str) {
        if let Some(field) = lookup.by_id.get(field_id) {
            return Ok(field);
        }
    }
    if let Some(label) = attrs.get("label").and_then(Value::as_str) {
        if let Some(field) = lookup.by_name.get(&normalize_template_name(label)) {
            return Ok(field);
        }
    }
    Err(CommandError::new(
        "unknown_template_field",
        "A card layout refers to a field that does not exist in this template.",
    ))
}

fn invalid_typed_template_placeholder() -> CommandError {
    CommandError::new(
        "invalid_template_placeholder",
        "Template fields must use complete double braces, such as {{Subject}}.",
    )
}

pub(in super::super) fn template_update_contains_new_children(
    connection: &Connection,
    input: &UpdateCardTemplateInput,
) -> Result<bool, CommandError> {
    for (table, id) in input
        .fields
        .iter()
        .filter_map(|field| field.id.as_deref().map(|id| ("card_template_fields", id)))
        .chain(
            input
                .rules
                .iter()
                .filter_map(|rule| rule.id.as_deref().map(|id| ("card_template_rules", id))),
        )
    {
        let sql = match table {
            "card_template_fields" => {
                "SELECT EXISTS(SELECT 1 FROM card_template_fields WHERE id = ?1)"
            }
            "card_template_rules" => {
                "SELECT EXISTS(SELECT 1 FROM card_template_rules WHERE id = ?1)"
            }
            _ => unreachable!("static table name"),
        };
        let exists: i64 = connection
            .query_row(sql, [id], |row| row.get(0))
            .map_err(CommandError::database)?;
        if exists == 0 {
            return Ok(true);
        }
    }
    Ok(false)
}

fn template_value_placement(value: &ValidatedTemplateValue) -> TemplateValuePlacement {
    if template_document_is_inline(&value.doc, &value.asset_ids) {
        TemplateValuePlacement::Inline
    } else {
        TemplateValuePlacement::Block
    }
}

fn raw_template_value_is_inline(document: &Value) -> bool {
    let Ok(asset_ids) = crate::cards::collect_asset_ids(document) else {
        return false;
    };
    template_document_is_inline(document, &asset_ids)
}

fn template_document_is_inline(document: &Value, asset_ids: &[String]) -> bool {
    if !asset_ids.is_empty() {
        return false;
    }
    let Some(blocks) = document.get("content").and_then(Value::as_array) else {
        return false;
    };
    if blocks.len() != 1 || blocks[0].get("type").and_then(Value::as_str) != Some("paragraph") {
        return false;
    }
    blocks[0]
        .get("content")
        .and_then(Value::as_array)
        .is_none_or(|content| {
            content.iter().all(|node| {
                matches!(
                    node.get("type").and_then(Value::as_str),
                    Some("text") | Some("hardBreak")
                )
            })
        })
}

pub(in super::super) fn render_automatic_template_previews(
    connection: &Connection,
    paths: &AppPaths,
    template: &StoredTemplate,
    values: Vec<TemplateValueInput>,
) -> Result<Vec<TemplateCardPreviewDto>, CommandError> {
    let validated_values =
        validate_template_values(connection, &template.dto.fields, values.clone())?;
    let materialized = materialize_automatic_template(template, &validated_values)?;
    render_template_previews(connection, paths, &materialized, values)
}

fn materialize_automatic_template(
    template: &StoredTemplate,
    values: &HashMap<String, ValidatedTemplateValue>,
) -> Result<StoredTemplate, CommandError> {
    let lookup = automatic_template_field_lookup_from_dtos(&template.dto.fields)?;
    let mut dto = template.dto.clone();
    for rule in &mut dto.rules {
        rule.front_doc =
            materialize_automatic_template_document(&rule.front_doc, &lookup, values, "question")?;
        rule.back_doc =
            materialize_automatic_template_document(&rule.back_doc, &lookup, values, "answer")?;
    }
    Ok(StoredTemplate { dto })
}

pub(super) fn materialize_automatic_template_document(
    layout: &Value,
    lookup: &AutomaticTemplateFieldLookup,
    values: &HashMap<String, ValidatedTemplateValue>,
    side: &str,
) -> Result<Value, CommandError> {
    let normalized = normalize_typed_template_layout(layout, lookup, side)?;
    let root = normalized.as_object().ok_or_else(|| {
        CommandError::new("invalid_template_layout", "The template layout is invalid.")
    })?;
    let content = root
        .get("content")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            CommandError::new(
                "invalid_template_layout",
                "The template layout must contain at least one block.",
            )
        })?;
    let mut rendered = root.clone();
    rendered.insert(
        "content".to_string(),
        Value::Array(materialize_template_block_children(content, values)?),
    );
    Ok(Value::Object(rendered))
}

fn materialize_template_block_children(
    children: &[Value],
    values: &HashMap<String, ValidatedTemplateValue>,
) -> Result<Vec<Value>, CommandError> {
    let mut rendered = Vec::new();
    for child in children {
        let object = child.as_object().ok_or_else(|| {
            CommandError::new(
                "invalid_template_layout",
                "Every template layout node must be a JSON object.",
            )
        })?;
        let node_type = object
            .get("type")
            .and_then(Value::as_str)
            .unwrap_or_default();
        if matches!(node_type, "paragraph" | "heading") {
            rendered.extend(materialize_template_text_block(object, values)?);
            continue;
        }
        if node_type == TEMPLATE_INLINE_NODE {
            return Err(CommandError::new(
                "invalid_template_placeholder",
                "Template field placeholders must be typed inside a paragraph or heading.",
            ));
        }
        let mut next = object.clone();
        if node_type != "codeBlock" {
            if let Some(content) = object.get("content") {
                let content = content.as_array().ok_or_else(|| {
                    CommandError::new(
                        "invalid_template_layout",
                        "Template layout content must be represented as a list.",
                    )
                })?;
                next.insert(
                    "content".to_string(),
                    Value::Array(materialize_template_block_children(content, values)?),
                );
            }
        }
        rendered.push(Value::Object(next));
    }
    Ok(rendered)
}

fn materialize_template_text_block(
    block: &serde_json::Map<String, Value>,
    values: &HashMap<String, ValidatedTemplateValue>,
) -> Result<Vec<Value>, CommandError> {
    let children = block
        .get("content")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let mut rendered = Vec::new();
    let mut inline = Vec::new();
    let mut split_by_block_value = false;

    for child in children {
        if child.get("type").and_then(Value::as_str) != Some(TEMPLATE_INLINE_NODE) {
            inline.push(child);
            continue;
        }
        let field_id = child
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
        let value = values.get(field_id).ok_or_else(|| {
            CommandError::new(
                "missing_template_value",
                "Fill in every template field before generating cards.",
            )
        })?;
        match template_value_placement(value) {
            TemplateValuePlacement::Inline => {
                inline.extend(inline_nodes_for_automatic_value(value)?);
            }
            TemplateValuePlacement::Block => {
                split_by_block_value = true;
                push_materialized_text_block(&mut rendered, block, &mut inline, false);
                rendered.extend(block_nodes_for_automatic_value(value)?);
            }
        }
    }

    push_materialized_text_block(&mut rendered, block, &mut inline, !split_by_block_value);
    Ok(rendered)
}

fn inline_nodes_for_automatic_value(
    value: &ValidatedTemplateValue,
) -> Result<Vec<Value>, CommandError> {
    if template_value_placement(value) != TemplateValuePlacement::Inline {
        return Err(CommandError::new(
            "invalid_template_value",
            "A structured template field cannot be inserted inline.",
        ));
    }
    Ok(value
        .doc
        .get("content")
        .and_then(Value::as_array)
        .and_then(|blocks| blocks.first())
        .and_then(|paragraph| paragraph.get("content"))
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default())
}

fn block_nodes_for_automatic_value(
    value: &ValidatedTemplateValue,
) -> Result<Vec<Value>, CommandError> {
    value
        .doc
        .get("content")
        .and_then(Value::as_array)
        .cloned()
        .ok_or_else(|| {
            CommandError::new(
                "invalid_template_value",
                "A template field contains an invalid document.",
            )
        })
}

fn push_materialized_text_block(
    output: &mut Vec<Value>,
    template: &serde_json::Map<String, Value>,
    inline: &mut Vec<Value>,
    preserve_empty: bool,
) {
    let content = std::mem::take(inline);
    if !preserve_empty && !inline_content_is_visible(&content) {
        return;
    }
    let mut block = template.clone();
    if content.is_empty() {
        block.remove("content");
    } else {
        block.insert("content".to_string(), Value::Array(content));
    }
    output.push(Value::Object(block));
}

fn inline_content_is_visible(content: &[Value]) -> bool {
    content
        .iter()
        .any(|node| match node.get("type").and_then(Value::as_str) {
            Some("text") => node
                .get("text")
                .and_then(Value::as_str)
                .is_some_and(|text| !text.trim().is_empty()),
            Some("hardBreak") => true,
            Some(_) => true,
            None => false,
        })
}

pub(in super::super) fn generate_automatic_template_cards_in_connection(
    connection: &mut Connection,
    paths: &AppPaths,
    input: GenerateTemplateCardsInput,
) -> Result<GenerateTemplateCardsResultDto, CommandError> {
    if input
        .values
        .iter()
        .all(|value| raw_template_value_is_inline(&value.doc))
    {
        return generate_template_cards_in_connection(connection, paths, input);
    }

    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(CommandError::database)?;
    let template = load_stored_template(&transaction, paths, &input.template_id, true)?;
    ensure_template_revision(&template, input.template_revision)?;
    let validated_values =
        validate_template_values(&transaction, &template.dto.fields, input.values.clone())?;
    let materialized = materialize_automatic_template(&template, &validated_values)?;
    let rendered_cards = render_template_cards(&transaction, &materialized, input.values)?;
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
