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
    #[serde(default)]
    pub id: Option<String>,
    pub name: String,
    #[serde(default)]
    pub description: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CardTemplateRuleInput {
    #[serde(default)]
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
    paths: State<'_, AppPaths>,
    input: GenerateTemplateCardsInput,
) -> Result<GenerateTemplateCardsResultDto, CommandError> {
    validate_entity_id(&input.deck_id, "deck")?;
    validate_entity_id(&input.template_id, "card template")?;
    database.with_connection(|connection| {
        generate_template_cards_in_connection(connection, &paths, input)
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
