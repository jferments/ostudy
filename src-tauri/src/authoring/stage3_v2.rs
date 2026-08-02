use std::collections::{BTreeMap, HashMap};

const MAX_CLOZE_SOURCE_CHARS: usize = 200_000;
const MAX_CLOZE_GROUP: u32 = 10_000;
const MAX_TEMPLATE_NAME_CHARS: usize = 80;
const MAX_TEMPLATE_DESCRIPTION_CHARS: usize = 2_000;
const MAX_TEMPLATE_FIELDS: usize = 32;
const MAX_TEMPLATE_FIELD_NAME_CHARS: usize = 64;
const MAX_TEMPLATE_FIELD_DESCRIPTION_CHARS: usize = 500;
const MAX_TEMPLATE_RULES: usize = 32;
const MAX_TEMPLATE_RULE_NAME_CHARS: usize = 80;
const MAX_TEMPLATE_DOCUMENT_BYTES: usize = 1_000_000;
const MAX_TEMPLATE_VALUE_CHARS: usize = 200_000;
const TEMPLATE_INLINE_NODE: &str = "templateFieldInline";
const TEMPLATE_BLOCK_NODE: &str = "templateFieldBlock";

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CardTypeInfoDto {
    pub card_type: String,
    pub reverse_enabled: bool,
    pub cloze_source: String,
    pub cloze_mode: String,
    pub generated_card_count: i64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CreateGeneratedCardInput {
    pub deck_id: String,
    pub card_type: String,
    #[serde(default)]
    pub reverse_enabled: bool,
    #[serde(default)]
    pub cloze_source: String,
    #[serde(default = "default_stage3_cloze_mode")]
    pub cloze_mode: String,
    pub front_doc: Value,
    pub back_doc: Value,
    pub front_text: String,
    pub back_text: String,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub allow_duplicate: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UpdateGeneratedCardInput {
    pub note_id: String,
    pub deck_id: String,
    pub revision: i64,
    pub card_type: String,
    #[serde(default)]
    pub reverse_enabled: bool,
    #[serde(default)]
    pub cloze_source: String,
    #[serde(default = "default_stage3_cloze_mode")]
    pub cloze_mode: String,
    pub front_doc: Value,
    pub back_doc: Value,
    pub front_text: String,
    pub back_text: String,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub allow_duplicate: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CardTemplateFieldInput {
    pub id: Option<String>,
    pub name: String,
    #[serde(default)]
    pub description: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CardTemplateRuleInput {
    pub id: Option<String>,
    pub name: String,
    pub front_doc: Value,
    pub back_doc: Value,
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

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CardTemplateFieldDto {
    pub id: String,
    pub name: String,
    pub description: String,
    pub position: i64,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CardTemplateRuleDto {
    pub id: String,
    pub name: String,
    pub front_doc: Value,
    pub back_doc: Value,
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
    pub assets: Vec<AssetDto>,
    pub created_at: i64,
    pub updated_at: i64,
    pub revision: i64,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TemplateValueInput {
    pub field_id: String,
    pub doc: Value,
    pub text: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PreviewTemplateCardsInput {
    pub template_id: String,
    pub template_revision: i64,
    pub values: Vec<TemplateValueInput>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GenerateTemplateCardsInput {
    pub deck_id: String,
    pub template_id: String,
    pub template_revision: i64,
    pub values: Vec<TemplateValueInput>,
    #[serde(default)]
    pub tags: Vec<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TemplateCardPreviewDto {
    pub rule_id: String,
    pub rule_name: String,
    pub question_doc: Value,
    pub answer_doc: Value,
    pub question_text: String,
    pub answer_text: String,
    pub assets: Vec<AssetDto>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GenerateTemplateCardsResultDto {
    pub created_count: i64,
    pub skipped_duplicates: i64,
    pub created_note_ids: Vec<String>,
}

pub(crate) struct RenderedCardPresentation {
    pub(crate) card_type: String,
    pub(crate) card_label: String,
    pub(crate) front_doc: Value,
    pub(crate) back_doc: Value,
    pub(crate) front_text: String,
    pub(crate) back_text: String,
}

struct PreparedGeneratedNote {
    note_type: &'static str,
    reverse_enabled: bool,
    cloze_source: String,
    cloze_mode: String,
    content: ValidatedContent,
    cards: Vec<GeneratedCardSpec>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
struct GeneratedCardSpec {
    direction: &'static str,
    template_ordinal: i64,
}

#[derive(Clone)]
struct ParsedCloze {
    segments: Vec<ClozeSegment>,
    groups: Vec<u32>,
    answers_by_group: BTreeMap<u32, Vec<String>>,
    full_text: String,
}

#[derive(Clone)]
enum ClozeSegment {
    Text(String),
    Blank { group: u32, text: String },
}

struct StoredTemplate {
    dto: CardTemplateDto,
}

struct ValidatedTemplateValue {
    doc: Value,
    text: String,
    asset_ids: Vec<String>,
}

struct RenderedTemplateCard {
    rule_id: String,
    rule_name: String,
    content: ValidatedContent,
}

#[tauri::command]
pub fn get_card_type_info(
    database: State<'_, Database>,
    note_id: String,
) -> Result<CardTypeInfoDto, CommandError> {
    validate_entity_id(&note_id, "note")?;
    database.with_connection(|connection| load_card_type_info(connection, &note_id))
}

#[tauri::command]
pub fn create_generated_card(
    database: State<'_, Database>,
    paths: State<'_, AppPaths>,
    input: CreateGeneratedCardInput,
) -> Result<OrganizedCardDto, CommandError> {
    create_generated_card_in_database(database, paths, input)
}

#[tauri::command]
pub fn update_generated_card(
    database: State<'_, Database>,
    paths: State<'_, AppPaths>,
    input: UpdateGeneratedCardInput,
) -> Result<OrganizedCardDto, CommandError> {
    update_generated_card_in_database(database, paths, input)
}

#[tauri::command]
pub fn list_card_templates(
    database: State<'_, Database>,
    paths: State<'_, AppPaths>,
) -> Result<Vec<CardTemplateDto>, CommandError> {
    database.with_connection(|connection| list_card_templates_in_connection(connection, &paths))
}

#[tauri::command]
pub fn get_card_template(
    database: State<'_, Database>,
    paths: State<'_, AppPaths>,
    template_id: String,
) -> Result<CardTemplateDto, CommandError> {
    validate_entity_id(&template_id, "card template")?;
    database.with_connection(|connection| {
        Ok(load_stored_template(connection, &paths, &template_id, true)?.dto)
    })
}

#[tauri::command]
pub fn create_card_template(
    database: State<'_, Database>,
    paths: State<'_, AppPaths>,
    input: CreateCardTemplateInput,
) -> Result<CardTemplateDto, CommandError> {
    database.with_connection(|connection| {
        create_card_template_in_connection(connection, &paths, input)
    })
}

#[tauri::command]
pub fn update_card_template(
    database: State<'_, Database>,
    paths: State<'_, AppPaths>,
    input: UpdateCardTemplateInput,
) -> Result<CardTemplateDto, CommandError> {
    database.with_connection(|connection| {
        update_card_template_in_connection(connection, &paths, input)
    })
}

#[tauri::command]
pub fn delete_card_template(
    database: State<'_, Database>,
    template_id: String,
) -> Result<(), CommandError> {
    validate_entity_id(&template_id, "card template")?;
    database.with_connection(|connection| {
        delete_card_template_in_connection(connection, &template_id)
    })
}

#[tauri::command]
pub fn preview_template_cards(
    database: State<'_, Database>,
    paths: State<'_, AppPaths>,
    input: PreviewTemplateCardsInput,
) -> Result<Vec<TemplateCardPreviewDto>, CommandError> {
    validate_entity_id(&input.template_id, "card template")?;
    database.with_connection(|connection| {
        let template = load_stored_template(connection, &paths, &input.template_id, true)?;
        ensure_template_revision(&template, input.template_revision)?;
        render_template_previews(connection, &paths, &template, input.values)
    })
}

#[tauri::command]
pub fn generate_cards_from_template(
    database: State<'_, Database>,
    input: GenerateTemplateCardsInput,
) -> Result<GenerateTemplateCardsResultDto, CommandError> {
    validate_entity_id(&input.deck_id, "deck")?;
    validate_entity_id(&input.template_id, "card template")?;
    database.with_connection(|connection| {
        generate_template_cards_in_connection(connection, input)
    })
}

fn default_stage3_cloze_mode() -> String {
    "one_at_a_time".to_string()
}

fn display_card_type(note_type: &str) -> &str {
    if note_type == "cloze" {
        "cloze"
    } else {
        "basic"
    }
}

fn load_card_type_info(
    connection: &Connection,
    note_id: &str,
) -> Result<CardTypeInfoDto, CommandError> {
    connection
        .query_row(
            "SELECT n.note_type, n.reverse_enabled, n.cloze_source, n.cloze_mode,
                    (SELECT COUNT(*) FROM cards c
                     WHERE c.note_id = n.id AND c.deleted_at IS NULL)
             FROM notes n
             JOIN decks d ON d.id = n.deck_id
             WHERE n.id = ?1
               AND n.deleted_at IS NULL
               AND d.deleted_at IS NULL
               AND d.archived_at IS NULL",
            [note_id],
            |row| {
                Ok(CardTypeInfoDto {
                    card_type: display_card_type(row.get::<_, String>(0)?.as_str()).to_string(),
                    reverse_enabled: row.get::<_, i64>(1)? == 1,
                    cloze_source: row.get(2)?,
                    cloze_mode: row.get(3)?,
                    generated_card_count: row.get(4)?,
                })
            },
        )
        .optional()
        .map_err(CommandError::database)?
        .ok_or_else(|| CommandError::new("card_not_found", "That flashcard no longer exists."))
}

struct GeneratedNoteDraft {
    card_type: String,
    reverse_enabled: bool,
    cloze_source: String,
    cloze_mode: String,
    front_doc: Value,
    back_doc: Value,
    front_text: String,
    back_text: String,
}

fn create_generated_card_in_database(
    database: State<'_, Database>,
    paths: State<'_, AppPaths>,
    input: CreateGeneratedCardInput,
) -> Result<OrganizedCardDto, CommandError> {
    let CreateGeneratedCardInput {
        deck_id,
        card_type,
        reverse_enabled,
        cloze_source,
        cloze_mode,
        front_doc,
        back_doc,
        front_text,
        back_text,
        tags,
        allow_duplicate,
    } = input;
    validate_entity_id(&deck_id, "deck")?;
    let prepared = prepare_generated_note(GeneratedNoteDraft {
        card_type,
        reverse_enabled,
        cloze_source,
        cloze_mode,
        front_doc,
        back_doc,
        front_text,
        back_text,
    })?;
    let tags = validate_tags(tags)?;
    let normalized = normalized_pair(&prepared.content.front_text, &prepared.content.back_text);
    let hash = duplicate_hash(&normalized.0, &normalized.1);

    let note_id = database.with_connection(|connection| {
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(CommandError::database)?;
        ensure_active_deck(&transaction, &deck_id)?;
        ensure_assets_exist(&transaction, &content_asset_ids(&prepared.content))?;
        if !allow_duplicate
            && duplicate_checkable(&prepared.content.front_text, &prepared.content.back_text)
            && has_exact_duplicate(
                &transaction,
                &deck_id,
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
                     reverse_enabled, cloze_source, cloze_mode,
                     created_at, updated_at, revision
                 ) VALUES (
                     ?1, ?2, ?3, 'tiptap-json', 1,
                     ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?12, 1
                 )",
                params![
                    note_id,
                    deck_id,
                    prepared.note_type,
                    prepared.content.front_doc_json,
                    prepared.content.back_doc_json,
                    prepared.content.front_text,
                    prepared.content.back_text,
                    hash,
                    crate::util::bool_to_integer(prepared.reverse_enabled),
                    prepared.cloze_source,
                    prepared.cloze_mode,
                    now,
                ],
            )
            .map_err(CommandError::database)?;
        sync_generated_cards(
            &transaction,
            &note_id,
            &deck_id,
            &prepared.cards,
            now,
        )?;
        replace_note_assets(&transaction, &note_id, &prepared.content)?;
        replace_note_tags(&transaction, &note_id, &tags, now)?;
        transaction.commit().map_err(CommandError::database)?;
        Ok(note_id)
    })?;

    database.with_connection(|connection| hydrate_organized_card(connection, &paths, &note_id))
}

fn update_generated_card_in_database(
    database: State<'_, Database>,
    paths: State<'_, AppPaths>,
    input: UpdateGeneratedCardInput,
) -> Result<OrganizedCardDto, CommandError> {
    let UpdateGeneratedCardInput {
        note_id,
        deck_id,
        revision,
        card_type,
        reverse_enabled,
        cloze_source,
        cloze_mode,
        front_doc,
        back_doc,
        front_text,
        back_text,
        tags,
        allow_duplicate,
    } = input;
    validate_entity_id(&note_id, "note")?;
    validate_entity_id(&deck_id, "deck")?;
    if revision < 1 {
        return Err(CommandError::new(
            "invalid_revision",
            "The card revision is invalid.",
        ));
    }
    let prepared = prepare_generated_note(GeneratedNoteDraft {
        card_type,
        reverse_enabled,
        cloze_source,
        cloze_mode,
        front_doc,
        back_doc,
        front_text,
        back_text,
    })?;
    let tags = validate_tags(tags)?;
    let normalized = normalized_pair(&prepared.content.front_text, &prepared.content.back_text);
    let hash = duplicate_hash(&normalized.0, &normalized.1);

    database.with_connection(|connection| {
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(CommandError::database)?;
        ensure_active_deck(&transaction, &deck_id)?;
        ensure_assets_exist(&transaction, &content_asset_ids(&prepared.content))?;
        let existing_type: Option<String> = transaction
            .query_row(
                "SELECT note_type FROM notes WHERE id = ?1 AND deleted_at IS NULL",
                [note_id.as_str()],
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
        if display_card_type(&existing_type) != display_card_type(prepared.note_type) {
            return Err(CommandError::new(
                "card_type_locked",
                "A saved card cannot be converted between Basic and Fill in the blank. Create a new card instead.",
            ));
        }
        if !allow_duplicate
            && duplicate_checkable(&prepared.content.front_text, &prepared.content.back_text)
            && has_exact_duplicate(
                &transaction,
                &deck_id,
                &hash,
                &normalized.0,
                &normalized.1,
                Some(&note_id),
            )?
        {
            return Err(duplicate_error());
        }

        let now = unix_time_millis()?;
        let changed = transaction
            .execute(
                "UPDATE notes
                 SET deck_id = ?1, note_type = ?2,
                     front_doc = ?3, back_doc = ?4,
                     front_text = ?5, back_text = ?6, duplicate_hash = ?7,
                     reverse_enabled = ?8, cloze_source = ?9, cloze_mode = ?10,
                     updated_at = ?11, revision = revision + 1
                 WHERE id = ?12 AND revision = ?13 AND deleted_at IS NULL",
                params![
                    deck_id,
                    prepared.note_type,
                    prepared.content.front_doc_json,
                    prepared.content.back_doc_json,
                    prepared.content.front_text,
                    prepared.content.back_text,
                    hash,
                    crate::util::bool_to_integer(prepared.reverse_enabled),
                    prepared.cloze_source,
                    prepared.cloze_mode,
                    now,
                    note_id,
                    revision,
                ],
            )
            .map_err(CommandError::database)?;
        if changed != 1 {
            return Err(CommandError::new(
                "card_changed",
                "This card changed in another window. Reload it and try again.",
            ));
        }
        sync_generated_cards(
            &transaction,
            &note_id,
            &deck_id,
            &prepared.cards,
            now,
        )?;
        replace_note_assets(&transaction, &note_id, &prepared.content)?;
        replace_note_tags(&transaction, &note_id, &tags, now)?;
        transaction.commit().map_err(CommandError::database)
    })?;

    database.with_connection(|connection| {
        hydrate_organized_card(connection, &paths, &note_id)
    })
}

fn prepare_generated_note(
    input: GeneratedNoteDraft,
) -> Result<PreparedGeneratedNote, CommandError> {
    let GeneratedNoteDraft {
        card_type,
        reverse_enabled,
        cloze_source,
        cloze_mode,
        front_doc,
        back_doc,
        front_text,
        back_text,
    } = input;
    match card_type.as_str() {
        "basic" => {
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
            Ok(PreparedGeneratedNote {
                note_type: "basic",
                reverse_enabled,
                cloze_source: String::new(),
                cloze_mode: "one_at_a_time".to_string(),
                content,
                cards,
            })
        }
        "cloze" => {
            validate_cloze_mode(&cloze_mode)?;
            let parsed = parse_cloze(&cloze_source)?;
            let target = if cloze_mode == "all_together" {
                None
            } else {
                parsed.groups.first().copied()
            };
            let question = render_cloze_question(&parsed, target);
            let answer = render_cloze_answer(&parsed, target);
            let content = validate_content(
                plain_text_document(&question),
                plain_text_document(&answer),
                question,
                answer,
            )?;
            Ok(PreparedGeneratedNote {
                note_type: "cloze",
                reverse_enabled: false,
                cloze_source,
                cloze_mode,
                content,
                cards: vec![GeneratedCardSpec {
                    direction: "forward",
                    template_ordinal: 0,
                }],
            })
        }
        _ => Err(CommandError::new(
            "invalid_card_type",
            "Choose Basic or Fill in the blank as the card type.",
        )),
    }
}

fn sync_generated_cards(
    connection: &Connection,
    note_id: &str,
    deck_id: &str,
    desired: &[GeneratedCardSpec],
    now: i64,
) -> Result<(), CommandError> {
    if desired.is_empty() {
        return Err(CommandError::new(
            "missing_generated_card",
            "A note must generate at least one study card.",
        ));
    }
    let existing = {
        let mut statement = connection
            .prepare(
                "SELECT id, direction, template_ordinal, deleted_at
                 FROM cards WHERE note_id = ?1
                 ORDER BY direction, template_ordinal, created_at",
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
    let mut by_key = BTreeMap::new();
    for (card_id, direction, ordinal, deleted_at) in &existing {
        by_key.insert(
            (direction.clone(), *ordinal),
            (card_id.clone(), *deleted_at),
        );
    }
    let desired_keys = desired
        .iter()
        .map(|spec| (spec.direction.to_string(), spec.template_ordinal))
        .collect::<BTreeSet<_>>();

    for spec in desired {
        if let Some((card_id, _)) =
            by_key.get(&(spec.direction.to_string(), spec.template_ordinal))
        {
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
                    [card_id.as_str()],
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
            format!(
                "Fill-in-the-blank text must contain 1 to {MAX_CLOZE_SOURCE_CHARS} characters."
            ),
        ));
    }
    let mut segments = Vec::new();
    let mut groups = BTreeSet::new();
    let mut answers_by_group = BTreeMap::<u32, Vec<String>>::new();
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
        if group_text.is_empty()
            || !group_text
                .chars()
                .all(|character| character.is_ascii_digit())
        {
            return Err(invalid_cloze_marker());
        }
        let group: u32 = group_text.parse().map_err(|_| invalid_cloze_marker())?;
        if group == 0 || group > MAX_CLOZE_GROUP {
            return Err(invalid_cloze_marker());
        }
        let after_separator = &marker[separator + 2..];
        let close = after_separator.find("}}").ok_or_else(invalid_cloze_marker)?;
        let raw_blank = &after_separator[..close];
        let blank = raw_blank.trim();
        if blank.is_empty() || raw_blank.contains("{{") || raw_blank.contains("}}") {
            return Err(invalid_cloze_marker());
        }
        groups.insert(group);
        answers_by_group
            .entry(group)
            .or_default()
            .push(blank.to_string());
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
    if groups.is_empty() {
        return Err(CommandError::new(
            "missing_cloze_blank",
            "Mark at least one word or phrase as a blank.",
        ));
    }
    Ok(ParsedCloze {
        segments,
        groups: groups.into_iter().collect(),
        answers_by_group,
        full_text,
    })
}

fn invalid_cloze_marker() -> CommandError {
    CommandError::new(
        "invalid_cloze_marker",
        "A fill-in-the-blank marker is incomplete or nested. Select text and use Mark as blank instead of typing marker braces manually.",
    )
}

fn render_cloze_question(parsed: &ParsedCloze, target: Option<u32>) -> String {
    parsed
        .segments
        .iter()
        .map(|segment| match segment {
            ClozeSegment::Text(text) => text.clone(),
            ClozeSegment::Blank { group, text } => {
                if target.is_none() || target == Some(*group) {
                    "[…]".to_string()
                } else {
                    text.clone()
                }
            }
        })
        .collect()
}

fn render_cloze_answer(parsed: &ParsedCloze, target: Option<u32>) -> String {
    let answers = match target {
        Some(group) => parsed
            .answers_by_group
            .get(&group)
            .cloned()
            .unwrap_or_default(),
        None => parsed
            .groups
            .iter()
            .flat_map(|group| {
                parsed
                    .answers_by_group
                    .get(group)
                    .cloned()
                    .unwrap_or_default()
            })
            .collect(),
    };
    format!(
        "{}\n\nComplete statement: {}",
        answers.join(" · "),
        parsed.full_text
    )
}

pub(crate) fn presentation_variant_for_card(
    connection: &Connection,
    card_id: &str,
) -> Result<String, CommandError> {
    let stored = connection
        .query_row(
            "SELECT n.note_type, n.cloze_source, n.cloze_mode, s.reps
             FROM cards c
             JOIN notes n ON n.id = c.note_id
             JOIN card_schedules s ON s.card_id = c.id
             WHERE c.id = ?1 AND c.deleted_at IS NULL AND n.deleted_at IS NULL",
            [card_id],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, i64>(3)?,
                ))
            },
        )
        .optional()
        .map_err(CommandError::database)?
        .ok_or_else(|| {
            CommandError::new(
                "practice_card_not_found",
                "That generated card is no longer available.",
            )
        })?;
    if stored.0 != "cloze" {
        return Ok(String::new());
    }
    validate_cloze_mode(&stored.2)?;
    if stored.2 == "all_together" {
        return Ok("cloze:all".to_string());
    }
    let parsed = parse_cloze(&stored.1)?;
    let index = usize::try_from(stored.3.max(0)).unwrap_or(0) % parsed.groups.len();
    Ok(format!("cloze:{}", parsed.groups[index]))
}

fn cloze_target_from_variant(
    parsed: &ParsedCloze,
    mode: &str,
    variant: Option<&str>,
    reps: i64,
) -> Result<Option<u32>, CommandError> {
    if mode == "all_together" {
        return Ok(None);
    }
    if let Some(variant) = variant {
        if variant == "cloze:all" {
            return Err(CommandError::new(
                "invalid_cloze_variant",
                "This Fill in the blank card is not configured to hide every blank together.",
            ));
        }
        let group = variant
            .strip_prefix("cloze:")
            .and_then(|value| value.parse::<u32>().ok())
            .filter(|group| parsed.groups.contains(group))
            .ok_or_else(|| {
                CommandError::new(
                    "invalid_cloze_variant",
                    "The selected Fill in the blank presentation no longer exists.",
                )
            })?;
        return Ok(Some(group));
    }
    let index = usize::try_from(reps.max(0)).unwrap_or(0) % parsed.groups.len();
    Ok(Some(parsed.groups[index]))
}

pub(crate) fn render_stored_card_presentation(
    connection: &Connection,
    card_id: &str,
) -> Result<RenderedCardPresentation, CommandError> {
    render_stored_card_presentation_for_variant(connection, card_id, None)
}

pub(crate) fn render_stored_card_presentation_for_variant(
    connection: &Connection,
    card_id: &str,
    presentation_variant: Option<&str>,
) -> Result<RenderedCardPresentation, CommandError> {
    let stored = connection
        .query_row(
            "SELECT c.direction, c.template_ordinal,
                    n.note_type, n.front_doc, n.back_doc, n.front_text, n.back_text,
                    n.cloze_source, n.cloze_mode, s.reps
             FROM cards c
             JOIN notes n ON n.id = c.note_id
             JOIN decks d ON d.id = c.deck_id
             JOIN card_schedules s ON s.card_id = c.id
             WHERE c.id = ?1
               AND c.deleted_at IS NULL
               AND n.deleted_at IS NULL
               AND d.deleted_at IS NULL
               AND d.archived_at IS NULL",
            [card_id],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, i64>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, String>(5)?,
                    row.get::<_, String>(6)?,
                    row.get::<_, String>(7)?,
                    row.get::<_, String>(8)?,
                    row.get::<_, i64>(9)?,
                ))
            },
        )
        .optional()
        .map_err(CommandError::database)?
        .ok_or_else(|| {
            CommandError::new(
                "practice_card_not_found",
                "That generated card is no longer available.",
            )
        })?;

    let (
        direction,
        ordinal,
        note_type,
        front_json,
        back_json,
        front_text,
        back_text,
        cloze_source,
        cloze_mode,
        reps,
    ) = stored;
    if note_type == "basic" {
        let front_doc =
            serde_json::from_str(&front_json).map_err(CommandError::serialization)?;
        let back_doc = serde_json::from_str(&back_json).map_err(CommandError::serialization)?;
        return match direction.as_str() {
            "forward" => Ok(RenderedCardPresentation {
                card_type: "basic".to_string(),
                card_label: "Basic".to_string(),
                front_doc,
                back_doc,
                front_text,
                back_text,
            }),
            "reverse" => Ok(RenderedCardPresentation {
                card_type: "basic".to_string(),
                card_label: "Reverse".to_string(),
                front_doc: back_doc,
                back_doc: front_doc,
                front_text: back_text,
                back_text: front_text,
            }),
            _ => Err(CommandError::new(
                "invalid_generated_card",
                "A Basic note contains an unsupported generated-card direction.",
            )),
        };
    }
    if note_type != "cloze" || direction != "forward" || ordinal != 0 {
        return Err(CommandError::new(
            "invalid_generated_card",
            "The Fill in the blank note contains an unsupported active card identity.",
        ));
    }
    validate_cloze_mode(&cloze_mode)?;
    let parsed = parse_cloze(&cloze_source)?;
    let target = cloze_target_from_variant(&parsed, &cloze_mode, presentation_variant, reps)?;
    let question = render_cloze_question(&parsed, target);
    let answer = render_cloze_answer(&parsed, target);
    let label = if cloze_mode == "all_together" {
        "Fill in the blank · all blanks".to_string()
    } else {
        format!(
            "Fill in the blank · {} alternating {}",
            parsed.groups.len(),
            if parsed.groups.len() == 1 { "blank" } else { "blanks" }
        )
    };
    Ok(RenderedCardPresentation {
        card_type: "cloze".to_string(),
        card_label: label,
        front_doc: plain_text_document(&question),
        back_doc: plain_text_document(&answer),
        front_text: question,
        back_text: answer,
    })
}

pub(crate) fn get_generated_card_for_practice(
    connection: &Connection,
    paths: &AppPaths,
    card_id: &str,
    presentation_variant: &str,
) -> Result<CardDto, CommandError> {
    let raw = connection
        .query_row(
            "SELECT c.id, c.note_id, c.deck_id, c.direction,
                    n.created_at, n.updated_at, n.revision
             FROM cards c
             JOIN notes n ON n.id = c.note_id
             JOIN decks d ON d.id = c.deck_id
             WHERE c.id = ?1
               AND c.deleted_at IS NULL
               AND c.suspended_at IS NULL
               AND n.deleted_at IS NULL
               AND d.deleted_at IS NULL
               AND d.archived_at IS NULL",
            [card_id],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, i64>(4)?,
                    row.get::<_, i64>(5)?,
                    row.get::<_, i64>(6)?,
                ))
            },
        )
        .optional()
        .map_err(CommandError::database)?
        .ok_or_else(|| {
            CommandError::new(
                "practice_card_not_found",
                "That generated card is no longer available.",
            )
        })?;
    let variant = (!presentation_variant.is_empty()).then_some(presentation_variant);
    let rendered = render_stored_card_presentation_for_variant(connection, card_id, variant)?;
    let assets = crate::assets::get_assets_for_note(connection, paths, &raw.1)?;
    Ok(CardDto {
        id: raw.0,
        note_id: raw.1,
        deck_id: raw.2,
        direction: raw.3,
        front_doc: rendered.front_doc,
        back_doc: rendered.back_doc,
        front_text: rendered.front_text,
        back_text: rendered.back_text,
        assets,
        created_at: raw.4,
        updated_at: raw.5,
        revision: raw.6,
    })
}

fn plain_text_document(value: &str) -> Value {
    let content = value
        .split('\n')
        .map(|line| {
            if line.is_empty() {
                json!({"type": "paragraph"})
            } else {
                json!({
                    "type": "paragraph",
                    "content": [{"type": "text", "text": line}]
                })
            }
        })
        .collect::<Vec<_>>();
    json!({"type": "doc", "content": content})
}

fn list_card_templates_in_connection(
    connection: &mut Connection,
    paths: &AppPaths,
) -> Result<Vec<CardTemplateDto>, CommandError> {
    let ids = {
        let mut statement = connection
            .prepare(
                "SELECT id FROM card_templates
                 WHERE deleted_at IS NULL
                 ORDER BY name COLLATE NOCASE ASC, created_at ASC",
            )
            .map_err(CommandError::database)?;
        statement
            .query_map([], |row| row.get::<_, String>(0))
            .map_err(CommandError::database)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(CommandError::database)?
    };
    ids.into_iter()
        .map(|id| Ok(load_stored_template(connection, paths, &id, true)?.dto))
        .collect()
}

fn create_card_template_in_connection(
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

fn update_card_template_in_connection(
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
            .map_err(CommandError::database)?
    };
    let rules = {
        let mut statement = connection
            .prepare(
                "SELECT id, name, front_doc, back_doc, position
                 FROM card_template_rules
                 WHERE template_id = ?1 ORDER BY position ASC",
            )
            .map_err(CommandError::database)?;
        statement
            .query_map([template_id], |row| {
                let front_json: Option<String> = row.get(2)?;
                let back_json: Option<String> = row.get(3)?;
                let front_doc = front_json
                    .as_deref()
                    .and_then(|value| serde_json::from_str(value).ok())
                    .unwrap_or_else(empty_rich_document);
                let back_doc = back_json
                    .as_deref()
                    .and_then(|value| serde_json::from_str(value).ok())
                    .unwrap_or_else(empty_rich_document);
                Ok(CardTemplateRuleDto {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    front_doc,
                    back_doc,
                    position: row.get(4)?,
                })
            })
            .map_err(CommandError::database)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(CommandError::database)?
    };
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
        let front_json =
            serde_json::to_string(&rule.front_doc).map_err(CommandError::serialization)?;
        let back_json =
            serde_json::to_string(&rule.back_doc).map_err(CommandError::serialization)?;
        connection
            .execute(
                "INSERT INTO card_template_rules (
                     id, template_id, name, front_pattern, back_pattern,
                     front_doc, back_doc, position
                 ) VALUES (?1, ?2, ?3, '[rich document]', '[rich document]', ?4, ?5, ?6)",
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
        for key in object.keys() {
            if !matches!(key.as_str(), "type" | "attrs") {
                return Err(CommandError::new(
                    "invalid_template_placeholder",
                    "A template field placeholder contains unsupported data.",
                ));
            }
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
        if attrs.len() != 1 {
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
        match node_type {
            TEMPLATE_INLINE_NODE
                if matches!(parent_type, Some("paragraph") | Some("heading")) => {}
            TEMPLATE_BLOCK_NODE
                if matches!(parent_type, Some("doc") | Some("blockquote")) => {}
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
        let text = crate::cards::extract_plain_text(&value.doc, "template field")?;
        let asset_ids = crate::cards::collect_asset_ids(&value.doc)?;
        if text.is_empty() && asset_ids.is_empty() {
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
                text,
                asset_ids,
            },
        );
    }
    ensure_assets_exist(connection, &all_assets.into_iter().collect::<Vec<_>>())?;
    Ok(result)
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
        let child_type = child.get("type").and_then(Value::as_str).unwrap_or_default();
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
    input: GenerateTemplateCardsInput,
) -> Result<GenerateTemplateCardsResultDto, CommandError> {
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(CommandError::database)?;
    let empty_paths = AppPaths::new(std::env::temp_dir().join("ostudy-template-generation"))
        .map_err(|message| CommandError::new("path_error", message))?;
    let template = load_stored_template(&transaction, &empty_paths, &input.template_id, true)?;
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

fn ensure_template_revision(
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

fn empty_rich_document() -> Value {
    json!({"type": "doc", "content": [{"type": "paragraph"}]})
}

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
        statement
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                ))
            })
            .map_err(CommandError::database)?
            .collect::<Result<Vec<_>, _>>()
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
            statement
                .query_map([template_id.as_str()], |row| {
                    Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
                })
                .map_err(CommandError::database)?
                .collect::<Result<Vec<_>, _>>()
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
                "attrs": {"fieldId": field_id}
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
            let field_name = normalize_template_name(&line[after_open..close]);
            let field_id = fields.get(&field_name).ok_or_else(|| {
                CommandError::new(
                    "unknown_template_field",
                    "A legacy template rule refers to a field that no longer exists.",
                )
            })?;
            inline.push(json!({
                "type": TEMPLATE_INLINE_NODE,
                "attrs": {"fieldId": field_id}
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
        statement
            .query_map([], |row| row.get::<_, String>(0))
            .map_err(CommandError::database)?
            .collect::<Result<Vec<_>, _>>()
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
            statement
                .query_map([note_id.as_str()], |row| {
                    Ok((
                        row.get::<_, i64>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, i64>(3)?,
                        row.get::<_, Option<i64>>(4)?,
                    ))
                })
                .map_err(CommandError::database)?
                .collect::<Result<Vec<_>, _>>()
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

#[cfg(test)]
mod stage3_v2_tests {
    use super::*;

    #[test]
    fn inserts_block_values_without_flattening_lists() {
        let field_id = "11111111111111111111111111111111".to_string();
        let layout = json!({
            "type": "doc",
            "content": [
                {"type": "paragraph", "content": [{"type": "text", "text": "Effects:"}]},
                {"type": TEMPLATE_BLOCK_NODE, "attrs": {"fieldId": field_id}}
            ]
        });
        let value_doc = json!({
            "type": "doc",
            "content": [{
                "type": "orderedList",
                "attrs": {"start": 1},
                "content": [{
                    "type": "listItem",
                    "content": [{
                        "type": "paragraph",
                        "content": [{"type": "text", "text": "First"}]
                    }]
                }]
            }]
        });
        let values = HashMap::from([(
            field_id,
            ValidatedTemplateValue {
                doc: value_doc,
                text: "First".to_string(),
                asset_ids: Vec::new(),
            },
        )]);
        let rendered = render_template_document(&layout, &values).expect("render should work");
        assert_eq!(
            rendered["content"][1]["type"],
            Value::String("orderedList".to_string())
        );
    }

    #[test]
    fn rejects_block_values_inside_sentences() {
        let field_id = "11111111111111111111111111111111".to_string();
        let layout = json!({
            "type": "doc",
            "content": [{
                "type": "paragraph",
                "content": [{"type": TEMPLATE_INLINE_NODE, "attrs": {"fieldId": field_id}}]
            }]
        });
        let value_doc = json!({
            "type": "doc",
            "content": [{"type": "image", "attrs": {
                "assetId": "22222222222222222222222222222222",
                "src": "study-asset://22222222222222222222222222222222"
            }}]
        });
        let values = HashMap::from([(
            field_id,
            ValidatedTemplateValue {
                doc: value_doc,
                text: String::new(),
                asset_ids: vec!["22222222222222222222222222222222".to_string()],
            },
        )]);
        let error = render_template_document(&layout, &values)
            .expect_err("block content should fail inline insertion");
        assert_eq!(error.code, "template_block_content_inline");
    }

    #[test]
    fn cloze_rotation_uses_one_card_schedule_review_count() {
        let parsed = parse_cloze("{{c1::Alpha}} and {{c2::Beta}}")
            .expect("cloze should parse");
        assert_eq!(
            cloze_target_from_variant(&parsed, "one_at_a_time", None, 0)
                .expect("first target"),
            Some(1)
        );
        assert_eq!(
            cloze_target_from_variant(&parsed, "one_at_a_time", None, 1)
                .expect("second target"),
            Some(2)
        );
        assert_eq!(
            cloze_target_from_variant(&parsed, "one_at_a_time", None, 2)
                .expect("wrapped target"),
            Some(1)
        );
    }
}
