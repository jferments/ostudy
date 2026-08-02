//! Phase 2 Stage 3 card types and safe, data-only templates.
//!
//! A saved template is only an authoring shortcut. Filling it expands each rule
//! into an ordinary Basic note/card in the selected deck. Generated cards do not
//! retain a template foreign key, and editing or deleting the template cannot
//! mutate cards that already exist. Reverse and cloze presentations are different:
//! they are durable presentations of one source note and therefore keep stable
//! card identities and independent schedules.

use std::collections::{BTreeMap, BTreeSet, HashMap};

const MAX_TEMPLATES: usize = 10_000;
const MAX_TEMPLATE_NAME_CHARS: usize = 80;
const MAX_TEMPLATE_DESCRIPTION_CHARS: usize = 2_000;
const MAX_TEMPLATE_FIELDS: usize = 32;
const MAX_TEMPLATE_RULES: usize = 32;
const MAX_TEMPLATE_FIELD_NAME_CHARS: usize = 64;
const MAX_TEMPLATE_FIELD_DESCRIPTION_CHARS: usize = 500;
const MAX_TEMPLATE_RULE_NAME_CHARS: usize = 80;
const MAX_TEMPLATE_PATTERN_CHARS: usize = 10_000;
const MAX_TEMPLATE_VALUE_CHARS: usize = 200_000;
const MAX_TEMPLATE_VALUE_TOTAL_CHARS: usize = 500_000;
const MAX_CLOZE_SOURCE_CHARS: usize = 200_000;
const MAX_CLOZE_GROUPS: usize = 64;

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CardTemplateFieldDto {
    pub id: String,
    pub name: String,
    pub description: String,
    pub position: i64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CardTemplateRuleDto {
    pub id: String,
    pub name: String,
    pub front_pattern: String,
    pub back_pattern: String,
    pub position: i64,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CardTemplateDto {
    pub id: String,
    pub name: String,
    pub description: String,
    pub fields: Vec<CardTemplateFieldDto>,
    pub rules: Vec<CardTemplateRuleDto>,
    pub created_at: i64,
    pub updated_at: i64,
    pub revision: i64,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CardTemplateFieldInput {
    pub id: Option<String>,
    pub name: String,
    #[serde(default)]
    pub description: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CardTemplateRuleInput {
    pub id: Option<String>,
    pub name: String,
    pub front_pattern: String,
    pub back_pattern: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CreateCardTemplateInput {
    pub name: String,
    #[serde(default)]
    pub description: String,
    pub fields: Vec<CardTemplateFieldInput>,
    pub rules: Vec<CardTemplateRuleInput>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UpdateCardTemplateInput {
    pub template_id: String,
    pub revision: i64,
    pub name: String,
    #[serde(default)]
    pub description: String,
    pub fields: Vec<CardTemplateFieldInput>,
    pub rules: Vec<CardTemplateRuleInput>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TemplateValueInput {
    pub field_id: String,
    pub value: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PreviewTemplateCardsInput {
    pub template_id: String,
    pub values: Vec<TemplateValueInput>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TemplateCardPreviewDto {
    pub rule_id: String,
    pub rule_name: String,
    pub question: String,
    pub answer: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GenerateTemplateCardsInput {
    pub deck_id: String,
    pub template_id: String,
    pub values: Vec<TemplateValueInput>,
    #[serde(default)]
    pub tags: Vec<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GenerateTemplateCardsResultDto {
    pub created_count: i64,
    pub skipped_duplicates: i64,
    pub created_note_ids: Vec<String>,
}

#[derive(Clone)]
struct StoredTemplate {
    id: String,
    name: String,
    description: String,
    fields: Vec<CardTemplateFieldDto>,
    rules: Vec<CardTemplateRuleDto>,
    created_at: i64,
    updated_at: i64,
    revision: i64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum TypedCardKind {
    Basic,
    Cloze,
}

impl TypedCardKind {
    fn parse(value: &str) -> Result<Self, CommandError> {
        match value {
            "basic" => Ok(Self::Basic),
            "cloze" => Ok(Self::Cloze),
            _ => Err(CommandError::new(
                "invalid_card_type",
                "Choose Basic or Fill in the blank. Saved templates generate ordinary Basic cards.",
            )),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct GeneratedCardSpec {
    pub(crate) direction: &'static str,
    pub(crate) template_ordinal: i64,
}

pub(crate) struct PreparedTypedNote {
    pub(crate) note_type: String,
    pub(crate) reverse_enabled: bool,
    pub(crate) cloze_text: String,
    pub(crate) cloze_mode: String,
    pub(crate) content: ValidatedContent,
    pub(crate) cards: Vec<GeneratedCardSpec>,
}

#[derive(Clone, Debug)]
pub(crate) struct RenderedCardPresentation {
    pub(crate) front_doc: Value,
    pub(crate) back_doc: Value,
    pub(crate) front_text: String,
    pub(crate) back_text: String,
    pub(crate) card_type: String,
    pub(crate) card_label: String,
}

#[derive(Clone, Debug)]
enum ClozeSegment {
    Text(String),
    Blank { group: u32, text: String },
}

#[derive(Clone, Debug)]
struct ParsedCloze {
    segments: Vec<ClozeSegment>,
    groups: Vec<u32>,
    full_text: String,
    answers: Vec<String>,
}

#[tauri::command]
pub fn list_card_templates(
    database: State<'_, Database>,
) -> Result<Vec<CardTemplateDto>, CommandError> {
    database.with_connection(list_card_templates_in_connection)
}

#[tauri::command]
pub fn get_card_template(
    database: State<'_, Database>,
    template_id: String,
) -> Result<CardTemplateDto, CommandError> {
    validate_entity_id(&template_id, "template")?;
    database.with_connection(|connection| hydrate_card_template(connection, &template_id))
}

#[tauri::command]
pub fn create_card_template(
    database: State<'_, Database>,
    input: CreateCardTemplateInput,
) -> Result<CardTemplateDto, CommandError> {
    database.with_connection(|connection| create_card_template_in_connection(connection, input))
}

#[tauri::command]
pub fn update_card_template(
    database: State<'_, Database>,
    input: UpdateCardTemplateInput,
) -> Result<CardTemplateDto, CommandError> {
    validate_entity_id(&input.template_id, "template")?;
    if input.revision < 1 {
        return Err(CommandError::new(
            "invalid_revision",
            "The template revision is invalid.",
        ));
    }
    database.with_connection(|connection| update_card_template_in_connection(connection, input))
}

#[tauri::command]
pub fn delete_card_template(
    database: State<'_, Database>,
    template_id: String,
) -> Result<(), CommandError> {
    validate_entity_id(&template_id, "template")?;
    database.with_connection(|connection| delete_card_template_in_connection(connection, &template_id))
}

#[tauri::command]
pub fn preview_template_cards(
    database: State<'_, Database>,
    input: PreviewTemplateCardsInput,
) -> Result<Vec<TemplateCardPreviewDto>, CommandError> {
    validate_entity_id(&input.template_id, "template")?;
    database.with_connection(|connection| {
        let template = load_stored_template(connection, &input.template_id, true)?;
        render_template_previews(&template, input.values)
    })
}

#[tauri::command]
pub fn generate_cards_from_template(
    database: State<'_, Database>,
    input: GenerateTemplateCardsInput,
) -> Result<GenerateTemplateCardsResultDto, CommandError> {
    validate_entity_id(&input.deck_id, "deck")?;
    validate_entity_id(&input.template_id, "template")?;
    database.with_connection(|connection| generate_template_cards_in_connection(connection, input))
}

fn list_card_templates_in_connection(
    connection: &Connection,
) -> Result<Vec<CardTemplateDto>, CommandError> {
    let mut statement = connection
        .prepare(
            "SELECT id FROM card_templates
             WHERE deleted_at IS NULL
             ORDER BY name COLLATE NOCASE ASC, created_at ASC
             LIMIT ?1",
        )
        .map_err(CommandError::database)?;
    let ids = statement
        .query_map([i64::try_from(MAX_TEMPLATES).unwrap_or(i64::MAX)], |row| {
            row.get::<_, String>(0)
        })
        .map_err(CommandError::database)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(CommandError::database)?;
    ids.into_iter()
        .map(|template_id| hydrate_card_template(connection, &template_id))
        .collect()
}

fn create_card_template_in_connection(
    connection: &mut Connection,
    input: CreateCardTemplateInput,
) -> Result<CardTemplateDto, CommandError> {
    let name = validate_template_name(input.name)?;
    let normalized_name = normalize_template_name(&name);
    let description = validate_template_description(input.description)?;
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(CommandError::database)?;
    ensure_template_name_available(&transaction, &normalized_name, None)?;
    let fields = prepare_template_fields(&transaction, input.fields)?;
    let rules = prepare_template_rules(&transaction, input.rules, &fields)?;
    let fields_json = serde_json::to_string(&fields).map_err(CommandError::serialization)?;
    let rules_json = serde_json::to_string(&rules).map_err(CommandError::serialization)?;
    let template_id = generate_id(&transaction)?;
    let now = unix_time_millis()?;
    transaction
        .execute(
            "INSERT INTO card_templates (
                 id, name, normalized_name, description, fields_json, rules_json,
                 created_at, updated_at, revision
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?7, 1)",
            params![
                template_id,
                name,
                normalized_name,
                description,
                fields_json,
                rules_json,
                now,
            ],
        )
        .map_err(CommandError::database)?;
    transaction.commit().map_err(CommandError::database)?;
    hydrate_card_template(connection, &template_id)
}

fn update_card_template_in_connection(
    connection: &mut Connection,
    input: UpdateCardTemplateInput,
) -> Result<CardTemplateDto, CommandError> {
    let name = validate_template_name(input.name)?;
    let normalized_name = normalize_template_name(&name);
    let description = validate_template_description(input.description)?;
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(CommandError::database)?;
    load_stored_template(&transaction, &input.template_id, true)?;
    ensure_template_name_available(
        &transaction,
        &normalized_name,
        Some(&input.template_id),
    )?;
    let fields = prepare_template_fields(&transaction, input.fields)?;
    let rules = prepare_template_rules(&transaction, input.rules, &fields)?;
    let fields_json = serde_json::to_string(&fields).map_err(CommandError::serialization)?;
    let rules_json = serde_json::to_string(&rules).map_err(CommandError::serialization)?;
    let now = unix_time_millis()?;
    let changed = transaction
        .execute(
            "UPDATE card_templates
             SET name = ?1, normalized_name = ?2, description = ?3,
                 fields_json = ?4, rules_json = ?5,
                 updated_at = ?6, revision = revision + 1
             WHERE id = ?7 AND revision = ?8 AND deleted_at IS NULL",
            params![
                name,
                normalized_name,
                description,
                fields_json,
                rules_json,
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
    transaction.commit().map_err(CommandError::database)?;
    hydrate_card_template(connection, &input.template_id)
}

fn delete_card_template_in_connection(
    connection: &mut Connection,
    template_id: &str,
) -> Result<(), CommandError> {
    // Templates do not own generated cards. Soft-deleting this definition cannot
    // affect any Basic cards that were previously generated from it.
    let now = unix_time_millis()?;
    let changed = connection
        .execute(
            "UPDATE card_templates
             SET deleted_at = ?1, updated_at = ?1, revision = revision + 1
             WHERE id = ?2 AND deleted_at IS NULL",
            params![now, template_id],
        )
        .map_err(CommandError::database)?;
    if changed != 1 {
        return Err(CommandError::new(
            "template_not_found",
            "That card template no longer exists.",
        ));
    }
    Ok(())
}

fn hydrate_card_template(
    connection: &Connection,
    template_id: &str,
) -> Result<CardTemplateDto, CommandError> {
    let template = load_stored_template(connection, template_id, true)?;
    Ok(CardTemplateDto {
        id: template.id,
        name: template.name,
        description: template.description,
        fields: template.fields,
        rules: template.rules,
        created_at: template.created_at,
        updated_at: template.updated_at,
        revision: template.revision,
    })
}

fn load_stored_template(
    connection: &Connection,
    template_id: &str,
    require_active: bool,
) -> Result<StoredTemplate, CommandError> {
    let mut sql = String::from(
        "SELECT id, name, description, fields_json, rules_json,
                created_at, updated_at, revision
         FROM card_templates WHERE id = ?1",
    );
    if require_active {
        sql.push_str(" AND deleted_at IS NULL");
    }
    let raw: Option<(String, String, String, String, String, i64, i64, i64)> = connection
        .query_row(&sql, [template_id], |row| {
            Ok((
                row.get(0)?,
                row.get(1)?,
                row.get(2)?,
                row.get(3)?,
                row.get(4)?,
                row.get(5)?,
                row.get(6)?,
                row.get(7)?,
            ))
        })
        .optional()
        .map_err(CommandError::database)?;
    let Some((id, name, description, fields_json, rules_json, created_at, updated_at, revision)) = raw else {
        return Err(CommandError::new(
            "template_not_found",
            "That card template no longer exists.",
        ));
    };
    let fields = parse_template_fields(&fields_json)?;
    let rules = parse_template_rules(&rules_json, &fields)?;
    Ok(StoredTemplate {
        id,
        name,
        description,
        fields,
        rules,
        created_at,
        updated_at,
        revision,
    })
}

fn parse_template_fields(value: &str) -> Result<Vec<CardTemplateFieldDto>, CommandError> {
    let fields: Vec<CardTemplateFieldDto> = serde_json::from_str(value).map_err(|_| {
        CommandError::new(
            "invalid_stored_template",
            "A saved template has invalid field data.",
        )
    })?;
    validate_stored_fields(&fields)?;
    Ok(fields)
}

fn parse_template_rules(
    value: &str,
    fields: &[CardTemplateFieldDto],
) -> Result<Vec<CardTemplateRuleDto>, CommandError> {
    let rules: Vec<CardTemplateRuleDto> = serde_json::from_str(value).map_err(|_| {
        CommandError::new(
            "invalid_stored_template",
            "A saved template has invalid card-rule data.",
        )
    })?;
    validate_stored_rules(&rules, fields)?;
    Ok(rules)
}

fn prepare_template_fields(
    connection: &Connection,
    inputs: Vec<CardTemplateFieldInput>,
) -> Result<Vec<CardTemplateFieldDto>, CommandError> {
    if inputs.is_empty() || inputs.len() > MAX_TEMPLATE_FIELDS {
        return Err(CommandError::new(
            "invalid_template_fields",
            format!("A template must contain 1 to {MAX_TEMPLATE_FIELDS} fields."),
        ));
    }
    let mut ids = BTreeSet::new();
    let mut names = BTreeSet::new();
    let mut fields = Vec::with_capacity(inputs.len());
    for (position, input) in inputs.into_iter().enumerate() {
        let id = match input.id {
            Some(id) => {
                validate_entity_id(&id, "template field")?;
                id
            }
            None => generate_id(connection)?,
        };
        if !ids.insert(id.clone()) {
            return Err(CommandError::new(
                "invalid_template_fields",
                "A template field appears more than once.",
            ));
        }
        let name = validate_template_field_name(input.name)?;
        if !names.insert(normalize_field_name(&name)) {
            return Err(CommandError::new(
                "duplicate_template_field",
                "Template field names must be unique.",
            ));
        }
        let description = validate_bounded_text(
            input.description,
            MAX_TEMPLATE_FIELD_DESCRIPTION_CHARS,
            "Template field descriptions",
        )?;
        fields.push(CardTemplateFieldDto {
            id,
            name,
            description,
            position: i64::try_from(position).unwrap_or(i64::MAX),
        });
    }
    Ok(fields)
}

fn prepare_template_rules(
    connection: &Connection,
    inputs: Vec<CardTemplateRuleInput>,
    fields: &[CardTemplateFieldDto],
) -> Result<Vec<CardTemplateRuleDto>, CommandError> {
    if inputs.is_empty() || inputs.len() > MAX_TEMPLATE_RULES {
        return Err(CommandError::new(
            "invalid_template_rules",
            format!("A template must contain 1 to {MAX_TEMPLATE_RULES} card rules."),
        ));
    }
    let mut ids = BTreeSet::new();
    let mut names = BTreeSet::new();
    let mut rules = Vec::with_capacity(inputs.len());
    for (position, input) in inputs.into_iter().enumerate() {
        let id = match input.id {
            Some(id) => {
                validate_entity_id(&id, "template rule")?;
                id
            }
            None => generate_id(connection)?,
        };
        if !ids.insert(id.clone()) {
            return Err(CommandError::new(
                "invalid_template_rules",
                "A template rule appears more than once.",
            ));
        }
        let name = validate_bounded_text(
            input.name,
            MAX_TEMPLATE_RULE_NAME_CHARS,
            "Template rule names",
        )?;
        if name.is_empty() || !names.insert(name.to_lowercase()) {
            return Err(CommandError::new(
                "duplicate_template_rule",
                "Template rule names must be non-empty and unique.",
            ));
        }
        let front_pattern = validate_pattern(input.front_pattern, fields, "question")?;
        let back_pattern = validate_pattern(input.back_pattern, fields, "answer")?;
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

fn validate_stored_fields(fields: &[CardTemplateFieldDto]) -> Result<(), CommandError> {
    if fields.is_empty() || fields.len() > MAX_TEMPLATE_FIELDS {
        return Err(CommandError::new(
            "invalid_stored_template",
            "A saved template has an invalid number of fields.",
        ));
    }
    let mut ids = BTreeSet::new();
    let mut names = BTreeSet::new();
    for (position, field) in fields.iter().enumerate() {
        validate_entity_id(&field.id, "template field")?;
        if !ids.insert(field.id.clone())
            || !names.insert(normalize_field_name(&field.name))
            || field.position != i64::try_from(position).unwrap_or(i64::MAX)
        {
            return Err(CommandError::new(
                "invalid_stored_template",
                "A saved template contains duplicate or out-of-order fields.",
            ));
        }
        validate_template_field_name(field.name.clone())?;
        validate_bounded_text(
            field.description.clone(),
            MAX_TEMPLATE_FIELD_DESCRIPTION_CHARS,
            "Template field descriptions",
        )?;
    }
    Ok(())
}

fn validate_stored_rules(
    rules: &[CardTemplateRuleDto],
    fields: &[CardTemplateFieldDto],
) -> Result<(), CommandError> {
    if rules.is_empty() || rules.len() > MAX_TEMPLATE_RULES {
        return Err(CommandError::new(
            "invalid_stored_template",
            "A saved template has an invalid number of card rules.",
        ));
    }
    let mut ids = BTreeSet::new();
    let mut names = BTreeSet::new();
    for (position, rule) in rules.iter().enumerate() {
        validate_entity_id(&rule.id, "template rule")?;
        if !ids.insert(rule.id.clone())
            || !names.insert(rule.name.to_lowercase())
            || rule.position != i64::try_from(position).unwrap_or(i64::MAX)
        {
            return Err(CommandError::new(
                "invalid_stored_template",
                "A saved template contains duplicate or out-of-order card rules.",
            ));
        }
        if validate_bounded_text(
            rule.name.clone(),
            MAX_TEMPLATE_RULE_NAME_CHARS,
            "Template rule names",
        )?
        .is_empty()
        {
            return Err(CommandError::new(
                "invalid_stored_template",
                "A saved template contains an empty card-rule name.",
            ));
        }
        validate_pattern(rule.front_pattern.clone(), fields, "question")?;
        validate_pattern(rule.back_pattern.clone(), fields, "answer")?;
    }
    Ok(())
}

fn validate_template_name(value: String) -> Result<String, CommandError> {
    let name = collapse_whitespace(&value);
    if name.is_empty()
        || name.chars().count() > MAX_TEMPLATE_NAME_CHARS
        || name.chars().any(char::is_control)
    {
        return Err(CommandError::new(
            "invalid_template_name",
            format!("Template names must contain 1 to {MAX_TEMPLATE_NAME_CHARS} visible characters."),
        ));
    }
    Ok(name)
}

fn normalize_template_name(value: &str) -> String {
    value.to_lowercase()
}

fn validate_template_description(value: String) -> Result<String, CommandError> {
    validate_bounded_text(
        value,
        MAX_TEMPLATE_DESCRIPTION_CHARS,
        "Template descriptions",
    )
}

fn validate_template_field_name(value: String) -> Result<String, CommandError> {
    let name = collapse_whitespace(&value);
    if name.is_empty()
        || name.chars().count() > MAX_TEMPLATE_FIELD_NAME_CHARS
        || name.chars().any(char::is_control)
        || name.contains("{{")
        || name.contains("}}")
    {
        return Err(CommandError::new(
            "invalid_template_field",
            format!(
                "Template field names must contain 1 to {MAX_TEMPLATE_FIELD_NAME_CHARS} visible characters and cannot contain braces."
            ),
        ));
    }
    Ok(name)
}

fn normalize_field_name(value: &str) -> String {
    collapse_whitespace(value).to_lowercase()
}

fn validate_bounded_text(
    value: String,
    maximum: usize,
    label: &str,
) -> Result<String, CommandError> {
    let value = value.trim().to_string();
    if value.chars().count() > maximum || value.chars().any(|character| character == '\0') {
        return Err(CommandError::new(
            "invalid_template_text",
            format!("{label} cannot exceed {maximum} characters."),
        ));
    }
    Ok(value)
}

fn ensure_template_name_available(
    connection: &Connection,
    normalized_name: &str,
    exclude_id: Option<&str>,
) -> Result<(), CommandError> {
    let exists: i64 = connection
        .query_row(
            "SELECT EXISTS(
                 SELECT 1 FROM card_templates
                 WHERE normalized_name = ?1
                   AND deleted_at IS NULL
                   AND (?2 IS NULL OR id != ?2)
             )",
            params![normalized_name, exclude_id],
            |row| row.get(0),
        )
        .map_err(CommandError::database)?;
    if exists == 1 {
        return Err(CommandError::new(
            "duplicate_template_name",
            "A template with that name already exists.",
        ));
    }
    Ok(())
}

fn validate_pattern(
    value: String,
    fields: &[CardTemplateFieldDto],
    side: &'static str,
) -> Result<String, CommandError> {
    let value = value.trim().to_string();
    if value.is_empty() || value.chars().count() > MAX_TEMPLATE_PATTERN_CHARS {
        return Err(CommandError::new(
            "invalid_template_pattern",
            format!(
                "Each template {side} must contain 1 to {MAX_TEMPLATE_PATTERN_CHARS} characters."
            ),
        ));
    }
    let field_names = fields
        .iter()
        .map(|field| normalize_field_name(&field.name))
        .collect::<BTreeSet<_>>();
    let placeholders = parse_pattern_placeholders(&value)?;
    if placeholders.is_empty() {
        return Err(CommandError::new(
            "invalid_template_pattern",
            format!("The template {side} must include at least one field placeholder."),
        ));
    }
    for placeholder in placeholders {
        if !field_names.contains(&normalize_field_name(&placeholder)) {
            return Err(CommandError::new(
                "unknown_template_field",
                format!("The template {side} references an unknown field: {placeholder}."),
            ));
        }
    }
    Ok(value)
}

fn parse_pattern_placeholders(value: &str) -> Result<Vec<String>, CommandError> {
    let mut placeholders = Vec::new();
    let mut remainder = value;
    while let Some(open) = remainder.find("{{") {
        let before = &remainder[..open];
        if before.contains("}}") {
            return Err(invalid_pattern_braces());
        }
        let after_open = &remainder[open + 2..];
        let close = after_open.find("}}").ok_or_else(invalid_pattern_braces)?;
        let name = collapse_whitespace(&after_open[..close]);
        if name.is_empty() || name.contains("{{") || name.contains("}}") {
            return Err(invalid_pattern_braces());
        }
        placeholders.push(name);
        remainder = &after_open[close + 2..];
    }
    if remainder.contains("{{") || remainder.contains("}}") {
        return Err(invalid_pattern_braces());
    }
    Ok(placeholders)
}

fn invalid_pattern_braces() -> CommandError {
    CommandError::new(
        "invalid_template_pattern",
        "Template placeholders must use the exact form {{field name}}.",
    )
}

fn render_pattern(
    pattern: &str,
    fields: &[CardTemplateFieldDto],
    values: &BTreeMap<String, String>,
) -> Result<String, CommandError> {
    let field_by_name = fields
        .iter()
        .map(|field| (normalize_field_name(&field.name), field))
        .collect::<HashMap<_, _>>();
    let mut rendered = String::new();
    let mut remainder = pattern;
    while let Some(open) = remainder.find("{{") {
        rendered.push_str(&remainder[..open]);
        let after_open = &remainder[open + 2..];
        let close = after_open.find("}}").ok_or_else(invalid_pattern_braces)?;
        let field_name = collapse_whitespace(&after_open[..close]);
        let field = field_by_name
            .get(&normalize_field_name(&field_name))
            .ok_or_else(|| {
                CommandError::new(
                    "unknown_template_field",
                    format!("The template references an unknown field: {field_name}."),
                )
            })?;
        let value = values.get(&field.id).ok_or_else(|| {
            CommandError::new(
                "missing_template_value",
                format!("Enter a value for {}.", field.name),
            )
        })?;
        rendered.push_str(value);
        remainder = &after_open[close + 2..];
    }
    rendered.push_str(remainder);
    Ok(rendered)
}

fn validate_template_values(
    fields: &[CardTemplateFieldDto],
    values: Vec<TemplateValueInput>,
) -> Result<BTreeMap<String, String>, CommandError> {
    if values.len() != fields.len() {
        return Err(CommandError::new(
            "missing_template_value",
            "Enter a value for every template field.",
        ));
    }
    let field_ids = fields
        .iter()
        .map(|field| field.id.as_str())
        .collect::<BTreeSet<_>>();
    let mut seen = BTreeSet::new();
    let mut total = 0_usize;
    let mut map = BTreeMap::new();
    for submitted in values {
        validate_entity_id(&submitted.field_id, "template field")?;
        if !field_ids.contains(submitted.field_id.as_str())
            || !seen.insert(submitted.field_id.clone())
        {
            return Err(CommandError::new(
                "invalid_template_value",
                "A submitted template value does not match this template.",
            ));
        }
        let value = submitted.value.trim().to_string();
        let count = value.chars().count();
        if count == 0 {
            let field_name = fields
                .iter()
                .find(|field| field.id == submitted.field_id)
                .map(|field| field.name.as_str())
                .unwrap_or("this field");
            return Err(CommandError::new(
                "missing_template_value",
                format!("Enter a value for {field_name}."),
            ));
        }
        if count > MAX_TEMPLATE_VALUE_CHARS {
            return Err(CommandError::new(
                "template_value_too_large",
                format!("Each template value must be at most {MAX_TEMPLATE_VALUE_CHARS} characters."),
            ));
        }
        total = total.saturating_add(count);
        if total > MAX_TEMPLATE_VALUE_TOTAL_CHARS {
            return Err(CommandError::new(
                "template_values_too_large",
                format!(
                    "Template values must total at most {MAX_TEMPLATE_VALUE_TOTAL_CHARS} characters."
                ),
            ));
        }
        map.insert(submitted.field_id, value);
    }
    Ok(map)
}

fn render_template_previews(
    template: &StoredTemplate,
    values: Vec<TemplateValueInput>,
) -> Result<Vec<TemplateCardPreviewDto>, CommandError> {
    let values = validate_template_values(&template.fields, values)?;
    template
        .rules
        .iter()
        .map(|rule| {
            let question = render_pattern(&rule.front_pattern, &template.fields, &values)?;
            let answer = render_pattern(&rule.back_pattern, &template.fields, &values)?;
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

pub(crate) fn prepare_typed_note(
    card_type: &str,
    reverse_enabled: bool,
    cloze_text: String,
    cloze_mode: String,
    front_doc: Value,
    back_doc: Value,
    front_text: String,
    back_text: String,
) -> Result<PreparedTypedNote, CommandError> {
    match TypedCardKind::parse(card_type)? {
        TypedCardKind::Basic => {
            let content = validate_content(front_doc, back_doc, front_text, back_text)?;
            let mut cards = vec![GeneratedCardSpec {
                direction: "forward",
                template_ordinal: 0,
            }];
            if reverse_enabled {
                cards.push(GeneratedCardSpec {
                    direction: "reverse",
                    template_ordinal: 0,
                });
            }
            Ok(PreparedTypedNote {
                note_type: "basic".to_string(),
                reverse_enabled,
                cloze_text: String::new(),
                cloze_mode: "one_at_a_time".to_string(),
                content,
                cards,
            })
        }
        TypedCardKind::Cloze => {
            validate_cloze_mode(&cloze_mode)?;
            let parsed = parse_cloze(&cloze_text)?;
            let answer_index = parsed.answers.join(" · ");
            let content = validate_content(
                plain_text_document(&parsed.full_text),
                plain_text_document(&answer_index),
                parsed.full_text.clone(),
                answer_index,
            )?;
            let cards = if cloze_mode == "all_together" {
                vec![GeneratedCardSpec {
                    direction: "cloze",
                    template_ordinal: 0,
                }]
            } else {
                parsed
                    .groups
                    .iter()
                    .map(|group| GeneratedCardSpec {
                        direction: "cloze",
                        template_ordinal: i64::from(*group),
                    })
                    .collect()
            };
            Ok(PreparedTypedNote {
                note_type: "cloze".to_string(),
                reverse_enabled: false,
                cloze_text,
                cloze_mode,
                content,
                cards,
            })
        }
    }
}

fn validate_cloze_mode(value: &str) -> Result<(), CommandError> {
    if matches!(value, "one_at_a_time" | "all_together") {
        Ok(())
    } else {
        Err(CommandError::new(
            "invalid_cloze_mode",
            "Choose whether to hide one blank at a time or all blanks together.",
        ))
    }
}

fn parse_cloze(value: &str) -> Result<ParsedCloze, CommandError> {
    if value.trim().is_empty() || value.chars().count() > MAX_CLOZE_SOURCE_CHARS {
        return Err(CommandError::new(
            "invalid_cloze_text",
            format!("Fill-in-the-blank text must contain 1 to {MAX_CLOZE_SOURCE_CHARS} characters."),
        ));
    }
    let mut segments = Vec::new();
    let mut groups = BTreeSet::new();
    let mut answers = Vec::new();
    let mut full_text = String::new();
    let mut remainder = value;

    while let Some(open) = remainder.find("{{c") {
        let before = &remainder[..open];
        if before.contains("}}") {
            return Err(invalid_cloze_marker());
        }
        if !before.is_empty() {
            segments.push(ClozeSegment::Text(before.to_string()));
            full_text.push_str(before);
        }
        let marker = &remainder[open + 3..];
        let separator = marker.find("::").ok_or_else(invalid_cloze_marker)?;
        let group_text = &marker[..separator];
        if group_text.is_empty() || !group_text.chars().all(|character| character.is_ascii_digit()) {
            return Err(invalid_cloze_marker());
        }
        let group: u32 = group_text.parse().map_err(|_| invalid_cloze_marker())?;
        if group == 0 || group > 10_000 {
            return Err(invalid_cloze_marker());
        }
        let after_separator = &marker[separator + 2..];
        let close = after_separator.find("}}").ok_or_else(invalid_cloze_marker)?;
        let blank = after_separator[..close].trim();
        if blank.is_empty() || blank.contains("{{") || blank.contains("}}") {
            return Err(invalid_cloze_marker());
        }
        groups.insert(group);
        answers.push(blank.to_string());
        segments.push(ClozeSegment::Blank {
            group,
            text: blank.to_string(),
        });
        full_text.push_str(blank);
        remainder = &after_separator[close + 2..];
    }

    if remainder.contains("{{") || remainder.contains("}}") {
        return Err(invalid_cloze_marker());
    }
    if !remainder.is_empty() {
        segments.push(ClozeSegment::Text(remainder.to_string()));
        full_text.push_str(remainder);
    }
    if groups.is_empty() || groups.len() > MAX_CLOZE_GROUPS {
        return Err(CommandError::new(
            "invalid_cloze_text",
            format!("Mark between 1 and {MAX_CLOZE_GROUPS} blank groups."),
        ));
    }

    Ok(ParsedCloze {
        segments,
        groups: groups.into_iter().collect(),
        full_text,
        answers,
    })
}

fn invalid_cloze_marker() -> CommandError {
    CommandError::new(
        "invalid_cloze_text",
        "Blank markers must use the exact form {{c1::hidden words}}.",
    )
}

fn render_cloze_front(parsed: &ParsedCloze, target_group: Option<u32>) -> String {
    let mut rendered = String::new();
    for segment in &parsed.segments {
        match segment {
            ClozeSegment::Text(text) => rendered.push_str(text),
            ClozeSegment::Blank { group, text } => {
                if target_group.is_none() || target_group == Some(*group) {
                    rendered.push_str("[…]");
                } else {
                    rendered.push_str(text);
                }
            }
        }
    }
    rendered
}

fn sync_generated_cards(
    connection: &Connection,
    note_id: &str,
    deck_id: &str,
    desired: &[GeneratedCardSpec],
    now: i64,
) -> Result<(), CommandError> {
    let existing = {
        let mut statement = connection
            .prepare(
                "SELECT id, direction, template_ordinal, deleted_at
                 FROM cards WHERE note_id = ?1",
            )
            .map_err(CommandError::database)?;
        statement
            .query_map([note_id], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, i64>(2)?,
                    row.get::<_, Option<i64>>(3)?,
                ))
            })
            .map_err(CommandError::database)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(CommandError::database)?
    };
    let by_key = existing
        .iter()
        .map(|(id, direction, ordinal, deleted_at)| {
            ((direction.clone(), *ordinal), (id.clone(), *deleted_at))
        })
        .collect::<HashMap<_, _>>();
    let desired_keys = desired
        .iter()
        .map(|spec| (spec.direction.to_string(), spec.template_ordinal))
        .collect::<BTreeSet<_>>();

    for spec in desired {
        let key = (spec.direction.to_string(), spec.template_ordinal);
        if let Some((card_id, _)) = by_key.get(&key) {
            connection
                .execute(
                    "UPDATE cards
                     SET deck_id = ?1, deleted_at = NULL, updated_at = ?2
                     WHERE id = ?3",
                    params![deck_id, now, card_id],
                )
                .map_err(CommandError::database)?;
            let schedule_exists: i64 = connection
                .query_row(
                    "SELECT EXISTS(SELECT 1 FROM card_schedules WHERE card_id = ?1)",
                    [card_id],
                    |row| row.get(0),
                )
                .map_err(CommandError::database)?;
            if schedule_exists == 0 {
                insert_new_schedule(connection, card_id, now)?;
            }
        } else {
            let card_id = generate_id(connection)?;
            connection
                .execute(
                    "INSERT INTO cards (
                         id, note_id, deck_id, direction, template_ordinal,
                         created_at, updated_at
                     ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?6)",
                    params![
                        card_id,
                        note_id,
                        deck_id,
                        spec.direction,
                        spec.template_ordinal,
                        now,
                    ],
                )
                .map_err(CommandError::database)?;
            insert_new_schedule(connection, &card_id, now)?;
        }
    }

    for (card_id, direction, ordinal, deleted_at) in existing {
        if deleted_at.is_none() && !desired_keys.contains(&(direction, ordinal)) {
            connection
                .execute(
                    "UPDATE cards SET deleted_at = ?1, updated_at = ?1 WHERE id = ?2",
                    params![now, card_id],
                )
                .map_err(CommandError::database)?;
        }
    }
    Ok(())
}

fn create_typed_card(
    database: State<'_, Database>,
    paths: State<'_, AppPaths>,
    input: CreateOrganizedCardInput,
) -> Result<OrganizedCardDto, CommandError> {
    validate_entity_id(&input.deck_id, "deck")?;
    let prepared = prepare_typed_note(
        &input.card_type,
        input.reverse_enabled,
        input.cloze_text,
        input.cloze_mode,
        input.front_doc,
        input.back_doc,
        input.front_text,
        input.back_text,
    )?;
    let tags = validate_tags(input.tags)?;
    let normalized = normalized_pair(&prepared.content.front_text, &prepared.content.back_text);
    let hash = duplicate_hash(&normalized.0, &normalized.1);

    let note_id = database.with_connection(|connection| {
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(CommandError::database)?;
        ensure_active_deck(&transaction, &input.deck_id)?;
        ensure_assets_exist(&transaction, &content_asset_ids(&prepared.content))?;
        if !input.allow_duplicate
            && duplicate_checkable(&prepared.content.front_text, &prepared.content.back_text)
            && has_exact_duplicate(
                &transaction,
                &input.deck_id,
                &hash,
                &normalized.0,
                &normalized.1,
                None,
            )?
        {
            return Err(duplicate_error());
        }
        let now = unix_time_millis()?;
        let note_id = generate_id(&transaction)?;
        transaction
            .execute(
                "INSERT INTO notes (
                     id, deck_id, note_type, content_format, content_schema_version,
                     front_doc, back_doc, front_text, back_text, duplicate_hash,
                     reverse_enabled, cloze_text, cloze_mode,
                     created_at, updated_at, revision
                 ) VALUES (
                     ?1, ?2, ?3, 'tiptap-json', 1,
                     ?4, ?5, ?6, ?7, ?8,
                     ?9, ?10, ?11, ?12, ?12, 1
                 )",
                params![
                    note_id,
                    input.deck_id,
                    prepared.note_type,
                    prepared.content.front_doc_json,
                    prepared.content.back_doc_json,
                    prepared.content.front_text,
                    prepared.content.back_text,
                    hash,
                    bool_to_integer(prepared.reverse_enabled),
                    prepared.cloze_text,
                    prepared.cloze_mode,
                    now,
                ],
            )
            .map_err(CommandError::database)?;
        sync_generated_cards(
            &transaction,
            &note_id,
            &input.deck_id,
            &prepared.cards,
            now,
        )?;
        replace_note_assets(&transaction, &note_id, &prepared.content)?;
        replace_note_tags(&transaction, &note_id, &tags, now)?;
        transaction.commit().map_err(CommandError::database)?;
        Ok(note_id)
    })?;

    database.with_connection(|connection| hydrate_typed_note(connection, &paths, &note_id))
}

fn update_typed_card(
    database: State<'_, Database>,
    paths: State<'_, AppPaths>,
    input: UpdateOrganizedCardInput,
) -> Result<OrganizedCardDto, CommandError> {
    validate_entity_id(&input.note_id, "note")?;
    validate_entity_id(&input.deck_id, "deck")?;
    if input.revision < 1 {
        return Err(CommandError::new(
            "invalid_revision",
            "The card revision is invalid.",
        ));
    }
    let prepared = prepare_typed_note(
        &input.card_type,
        input.reverse_enabled,
        input.cloze_text,
        input.cloze_mode,
        input.front_doc,
        input.back_doc,
        input.front_text,
        input.back_text,
    )?;
    let tags = validate_tags(input.tags)?;
    let normalized = normalized_pair(&prepared.content.front_text, &prepared.content.back_text);
    let hash = duplicate_hash(&normalized.0, &normalized.1);

    database.with_connection(|connection| {
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(CommandError::database)?;
        ensure_active_deck(&transaction, &input.deck_id)?;
        ensure_assets_exist(&transaction, &content_asset_ids(&prepared.content))?;
        let existing_type: Option<String> = transaction
            .query_row(
                "SELECT note_type FROM notes WHERE id = ?1 AND deleted_at IS NULL",
                [input.note_id.as_str()],
                |row| row.get(0),
            )
            .optional()
            .map_err(CommandError::database)?;
        let Some(existing_type) = existing_type else {
            return Err(CommandError::new(
                "card_not_found",
                "That flashcard no longer exists.",
            ));
        };
        if existing_type != prepared.note_type {
            return Err(CommandError::new(
                "card_type_locked",
                "A saved note cannot be converted between Basic and Fill in the blank. Create a new card instead.",
            ));
        }
        if !input.allow_duplicate
            && duplicate_checkable(&prepared.content.front_text, &prepared.content.back_text)
            && has_exact_duplicate(
                &transaction,
                &input.deck_id,
                &hash,
                &normalized.0,
                &normalized.1,
                Some(&input.note_id),
            )?
        {
            return Err(duplicate_error());
        }
        let now = unix_time_millis()?;
        let changed = transaction
            .execute(
                "UPDATE notes
                 SET deck_id = ?1, front_doc = ?2, back_doc = ?3,
                     front_text = ?4, back_text = ?5, duplicate_hash = ?6,
                     reverse_enabled = ?7, cloze_text = ?8, cloze_mode = ?9,
                     updated_at = ?10, revision = revision + 1
                 WHERE id = ?11 AND revision = ?12 AND deleted_at IS NULL",
                params![
                    input.deck_id,
                    prepared.content.front_doc_json,
                    prepared.content.back_doc_json,
                    prepared.content.front_text,
                    prepared.content.back_text,
                    hash,
                    bool_to_integer(prepared.reverse_enabled),
                    prepared.cloze_text,
                    prepared.cloze_mode,
                    now,
                    input.note_id,
                    input.revision,
                ],
            )
            .map_err(CommandError::database)?;
        if changed != 1 {
            return Err(CommandError::new(
                "card_changed",
                "This card changed in another window. Reload it and try again.",
            ));
        }
        transaction
            .execute(
                "UPDATE cards SET deck_id = ?1, updated_at = ?2 WHERE note_id = ?3",
                params![input.deck_id, now, input.note_id],
            )
            .map_err(CommandError::database)?;
        sync_generated_cards(
            &transaction,
            &input.note_id,
            &input.deck_id,
            &prepared.cards,
            now,
        )?;
        replace_note_assets(&transaction, &input.note_id, &prepared.content)?;
        replace_note_tags(&transaction, &input.note_id, &tags, now)?;
        transaction.commit().map_err(CommandError::database)
    })?;

    database.with_connection(|connection| hydrate_typed_note(connection, &paths, &input.note_id))
}

pub(crate) fn render_stored_card_presentation(
    connection: &Connection,
    card_id: &str,
) -> Result<RenderedCardPresentation, CommandError> {
    let raw: Option<(
        String,
        i64,
        String,
        String,
        String,
        String,
        String,
        String,
        String,
    )> = connection
        .query_row(
            "SELECT c.direction, c.template_ordinal, n.note_type,
                    n.front_doc, n.back_doc, n.front_text, n.back_text,
                    n.cloze_text, n.cloze_mode
             FROM cards c
             JOIN notes n ON n.id = c.note_id
             WHERE c.id = ?1
               AND c.deleted_at IS NULL
               AND n.deleted_at IS NULL",
            [card_id],
            |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                    row.get(5)?,
                    row.get(6)?,
                    row.get(7)?,
                    row.get(8)?,
                ))
            },
        )
        .optional()
        .map_err(CommandError::database)?;
    let Some((
        direction,
        ordinal,
        note_type,
        front_doc_json,
        back_doc_json,
        front_text,
        back_text,
        cloze_text,
        cloze_mode,
    )) = raw else {
        return Err(CommandError::new(
            "card_not_found",
            "That flashcard no longer exists.",
        ));
    };

    if note_type == "cloze" || direction == "cloze" {
        validate_cloze_mode(&cloze_mode)?;
        let parsed = parse_cloze(&cloze_text)?;
        let target = if cloze_mode == "all_together" {
            None
        } else {
            Some(u32::try_from(ordinal).map_err(|_| {
                CommandError::new(
                    "invalid_cloze_card",
                    "This Fill in the blank card has an invalid blank group.",
                )
            })?)
        };
        if let Some(group) = target {
            if !parsed.groups.contains(&group) {
                return Err(CommandError::new(
                    "invalid_cloze_card",
                    "This Fill in the blank card no longer matches its source note.",
                ));
            }
        }
        let question = render_cloze_front(&parsed, target);
        return Ok(RenderedCardPresentation {
            front_doc: plain_text_document(&question),
            back_doc: plain_text_document(&parsed.full_text),
            front_text: question,
            back_text: parsed.full_text,
            card_type: "cloze".to_string(),
            card_label: if target.is_none() {
                "All blanks".to_string()
            } else {
                format!("Blank {}", target.unwrap_or_default())
            },
        });
    }

    let stored_front: Value =
        serde_json::from_str(&front_doc_json).map_err(CommandError::serialization)?;
    let stored_back: Value =
        serde_json::from_str(&back_doc_json).map_err(CommandError::serialization)?;
    if direction == "reverse" {
        Ok(RenderedCardPresentation {
            front_doc: stored_back,
            back_doc: stored_front,
            front_text: back_text,
            back_text: front_text,
            card_type: "basic".to_string(),
            card_label: "Reverse".to_string(),
        })
    } else if direction == "forward" && ordinal == 0 {
        Ok(RenderedCardPresentation {
            front_doc: stored_front,
            back_doc: stored_back,
            front_text,
            back_text,
            card_type: "basic".to_string(),
            card_label: "Basic".to_string(),
        })
    } else {
        Err(CommandError::new(
            "invalid_generated_card",
            "This card has unsupported generated-card metadata.",
        ))
    }
}

pub(crate) fn hydrate_typed_note(
    connection: &Connection,
    paths: &AppPaths,
    note_id: &str,
) -> Result<OrganizedCardDto, CommandError> {
    let source: Option<(String, i64, String, String, String)> = connection
        .query_row(
            "SELECT note_type, reverse_enabled, cloze_text, cloze_mode, deck_id
             FROM notes WHERE id = ?1 AND deleted_at IS NULL",
            [note_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?)),
        )
        .optional()
        .map_err(CommandError::database)?;
    let Some((note_type, reverse_enabled, cloze_text, cloze_mode, deck_id)) = source else {
        return Err(CommandError::new(
            "card_not_found",
            "That flashcard no longer exists.",
        ));
    };
    let primary_card_id: String = connection
        .query_row(
            "SELECT id FROM cards
             WHERE note_id = ?1 AND deleted_at IS NULL
             ORDER BY CASE direction WHEN 'forward' THEN 0 WHEN 'cloze' THEN 1 ELSE 2 END,
                      template_ordinal ASC
             LIMIT 1",
            [note_id],
            |row| row.get(0),
        )
        .map_err(CommandError::database)?;
    let card = get_card_by_id_from_connection(connection, paths, &primary_card_id)?;
    let tags = load_tag_names(connection, note_id)?;
    let due_at = connection
        .query_row(
            "SELECT due_at FROM card_schedules WHERE card_id = ?1",
            [primary_card_id.as_str()],
            |row| row.get(0),
        )
        .map_err(CommandError::database)?;
    let generated_card_count = connection
        .query_row(
            "SELECT COUNT(*) FROM cards WHERE note_id = ?1 AND deleted_at IS NULL",
            [note_id],
            |row| row.get(0),
        )
        .map_err(CommandError::database)?;

    Ok(OrganizedCardDto {
        id: card.id,
        note_id: card.note_id,
        deck_id,
        direction: card.direction,
        card_type: if note_type == "cloze" {
            "cloze".to_string()
        } else {
            "basic".to_string()
        },
        card_label: if note_type == "cloze" {
            "Fill in the blank".to_string()
        } else {
            "Basic".to_string()
        },
        reverse_enabled: reverse_enabled == 1,
        cloze_text,
        cloze_mode,
        front_doc: card.front_doc,
        back_doc: card.back_doc,
        front_text: card.front_text,
        back_text: card.back_text,
        assets: card.assets,
        tags,
        due_at,
        generated_card_count,
        created_at: card.created_at,
        updated_at: card.updated_at,
        revision: card.revision,
    })
}

#[cfg(test)]
mod stage3_tests {
    use super::*;
    use crate::decks::{create_deck_in_connection, CreateDeckInput};

    fn create_test_deck(database: &Database) -> String {
        database
            .with_connection(|connection| {
                create_deck_in_connection(
                    connection,
                    CreateDeckInput {
                        name: "Pharmacology".to_string(),
                        description: None,
                    },
                )
            })
            .expect("deck should be created")
            .id
    }

    fn pharmacology_template(connection: &mut Connection) -> CardTemplateDto {
        create_card_template_in_connection(
            connection,
            CreateCardTemplateInput {
                name: "Pharmacology".to_string(),
                description: "Generate common medication cards.".to_string(),
                fields: vec![
                    CardTemplateFieldInput {
                        id: None,
                        name: "name".to_string(),
                        description: String::new(),
                    },
                    CardTemplateFieldInput {
                        id: None,
                        name: "drug class".to_string(),
                        description: String::new(),
                    },
                ],
                rules: vec![CardTemplateRuleInput {
                    id: None,
                    name: "Drug class".to_string(),
                    front_pattern: "What is the drug class of {{name}}?".to_string(),
                    back_pattern: "{{drug class}}".to_string(),
                }],
            },
        )
        .expect("template should be created")
    }

    #[test]
    fn parses_one_at_a_time_and_all_together_cloze_sources() {
        let parsed = parse_cloze(
            "{{c1::ACE inhibitors}} block formation of {{c2::angiotensin II}}.",
        )
        .expect("cloze should parse");
        assert_eq!(parsed.groups, vec![1, 2]);
        assert_eq!(
            render_cloze_front(&parsed, Some(1)),
            "[…] block formation of angiotensin II."
        );
        assert_eq!(
            render_cloze_front(&parsed, None),
            "[…] block formation of […]."
        );
    }

    #[test]
    fn deleting_a_template_does_not_delete_generated_basic_cards() {
        let database = Database::open_in_memory().expect("database should open");
        let deck_id = create_test_deck(&database);
        let (template_id, generated_note_id) = database
            .with_connection(|connection| {
                let template = pharmacology_template(connection);
                let values = template
                    .fields
                    .iter()
                    .map(|field| TemplateValueInput {
                        field_id: field.id.clone(),
                        value: if field.name == "name" {
                            "lisinopril".to_string()
                        } else {
                            "ACE inhibitor".to_string()
                        },
                    })
                    .collect();
                let generated = generate_template_cards_in_connection(
                    connection,
                    GenerateTemplateCardsInput {
                        deck_id: deck_id.clone(),
                        template_id: template.id.clone(),
                        values,
                        tags: vec!["Medication".to_string()],
                    },
                )?;
                let note_id = generated.created_note_ids[0].clone();
                delete_card_template_in_connection(connection, &template.id)?;
                Ok((template.id, note_id))
            })
            .expect("generation and template deletion should succeed");

        database
            .with_connection(|connection| {
                let template_active: i64 = connection
                    .query_row(
                        "SELECT EXISTS(SELECT 1 FROM card_templates WHERE id = ?1 AND deleted_at IS NULL)",
                        [template_id],
                        |row| row.get(0),
                    )
                    .map_err(CommandError::database)?;
                let generated: (String, i64) = connection
                    .query_row(
                        "SELECT note_type,
                                (SELECT COUNT(*) FROM cards WHERE note_id = notes.id AND deleted_at IS NULL)
                         FROM notes WHERE id = ?1 AND deleted_at IS NULL",
                        [generated_note_id],
                        |row| Ok((row.get(0)?, row.get(1)?)),
                    )
                    .map_err(CommandError::database)?;
                assert_eq!(template_active, 0);
                assert_eq!(generated.0, "basic");
                assert_eq!(generated.1, 1);
                Ok(())
            })
            .expect("generated Basic card should remain");
    }

    #[test]
    fn reverse_card_reactivation_reuses_its_schedule() {
        let database = Database::open_in_memory().expect("database should open");
        let deck_id = create_test_deck(&database);
        database
            .with_connection(|connection| {
                let now = unix_time_millis()?;
                let note_id = generate_id(connection)?;
                connection
                    .execute(
                        "INSERT INTO notes (
                             id, deck_id, note_type, content_format, content_schema_version,
                             front_doc, back_doc, front_text, back_text, duplicate_hash,
                             reverse_enabled, cloze_text, cloze_mode,
                             created_at, updated_at, revision
                         ) VALUES (?1, ?2, 'basic', 'tiptap-json', 1,
                                   ?3, ?4, 'Q', 'A', '', 1, '', 'one_at_a_time', ?5, ?5, 1)",
                        params![
                            note_id,
                            deck_id,
                            serde_json::to_string(&plain_text_document("Q"))
                                .map_err(CommandError::serialization)?,
                            serde_json::to_string(&plain_text_document("A"))
                                .map_err(CommandError::serialization)?,
                            now,
                        ],
                    )
                    .map_err(CommandError::database)?;
                let desired = vec![
                    GeneratedCardSpec {
                        direction: "forward",
                        template_ordinal: 0,
                    },
                    GeneratedCardSpec {
                        direction: "reverse",
                        template_ordinal: 0,
                    },
                ];
                sync_generated_cards(connection, &note_id, &deck_id, &desired, now)?;
                let reverse: (String, i64) = connection
                    .query_row(
                        "SELECT c.id, s.due_at FROM cards c
                         JOIN card_schedules s ON s.card_id = c.id
                         WHERE c.note_id = ?1 AND c.direction = 'reverse'",
                        [note_id.as_str()],
                        |row| Ok((row.get(0)?, row.get(1)?)),
                    )
                    .map_err(CommandError::database)?;
                sync_generated_cards(connection, &note_id, &deck_id, &desired[..1], now + 1)?;
                sync_generated_cards(connection, &note_id, &deck_id, &desired, now + 2)?;
                let revived: (String, i64) = connection
                    .query_row(
                        "SELECT c.id, s.due_at FROM cards c
                         JOIN card_schedules s ON s.card_id = c.id
                         WHERE c.note_id = ?1 AND c.direction = 'reverse' AND c.deleted_at IS NULL",
                        [note_id],
                        |row| Ok((row.get(0)?, row.get(1)?)),
                    )
                    .map_err(CommandError::database)?;
                assert_eq!(revived, reverse);
                Ok(())
            })
            .expect("reverse schedule should survive disable and re-enable");
    }
}
