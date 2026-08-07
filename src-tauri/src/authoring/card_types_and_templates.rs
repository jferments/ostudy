use super::*;
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

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ClozeMode {
    #[default]
    OneAtATime,
    AllTogether,
}

impl ClozeMode {
    fn as_str(self) -> &'static str {
        match self {
            Self::OneAtATime => "one_at_a_time",
            Self::AllTogether => "all_together",
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(
    tag = "cardType",
    rename_all = "snake_case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum CreateGeneratedCardInput {
    Basic {
        deck_id: String,
        #[serde(default)]
        reverse_enabled: bool,
        front_doc: Value,
        back_doc: Value,
        #[serde(default)]
        tags: Vec<String>,
        #[serde(default)]
        allow_duplicate: bool,
        #[serde(default)]
        asset_draft_id: Option<String>,
    },
    Cloze {
        deck_id: String,
        cloze_source: String,
        #[serde(default)]
        cloze_mode: ClozeMode,
        #[serde(default)]
        tags: Vec<String>,
        #[serde(default)]
        allow_duplicate: bool,
        #[serde(default)]
        asset_draft_id: Option<String>,
    },
}

#[derive(Debug, Deserialize)]
#[serde(
    tag = "cardType",
    rename_all = "snake_case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum UpdateGeneratedCardInput {
    Basic {
        note_id: String,
        deck_id: String,
        revision: i64,
        #[serde(default)]
        reverse_enabled: bool,
        front_doc: Value,
        back_doc: Value,
        #[serde(default)]
        tags: Vec<String>,
        #[serde(default)]
        allow_duplicate: bool,
        #[serde(default)]
        asset_draft_id: Option<String>,
    },
    Cloze {
        note_id: String,
        deck_id: String,
        revision: i64,
        cloze_source: String,
        #[serde(default)]
        cloze_mode: ClozeMode,
        #[serde(default)]
        tags: Vec<String>,
        #[serde(default)]
        allow_duplicate: bool,
        #[serde(default)]
        asset_draft_id: Option<String>,
    },
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
    #[serde(default)]
    pub asset_draft_id: Option<String>,
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
    #[serde(default)]
    pub asset_draft_id: Option<String>,
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
    #[serde(default)]
    pub asset_draft_id: Option<String>,
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
pub fn get_card_template(
    database: State<'_, Database>,
    paths: State<'_, AppPaths>,
    template_id: String,
) -> Result<CardTemplateDto, CommandError> {
    validate_entity_id(&template_id, "card template")?;
    database.with_connection(|connection| {
        let template = load_stored_template(connection, &paths, &template_id, true)?.dto;
        normalize_card_template_dto_for_editing(template)
    })
}

#[tauri::command]
pub fn create_card_template(
    database: State<'_, Database>,
    paths: State<'_, AppPaths>,
    input: CreateCardTemplateInput,
) -> Result<CardTemplateDto, CommandError> {
    database.with_connection(|connection| {
        let input = prepare_automatic_create_template_input(connection, input)?;
        let draft_id = input.asset_draft_id.clone();
        let asset_ids = collect_template_input_asset_ids(&input.rules)?;
        let created = create_card_template_in_connection(connection, &paths, input)?;
        finalize_assets_after_committed_write(
            connection,
            &paths,
            &asset_ids,
            draft_id.as_deref(),
            &created.id,
            "create_card_template",
        );
        let template = match load_stored_template(connection, &paths, &created.id, true) {
            Ok(stored) => stored.dto,
            Err(error) => {
                crate::logging::error(
                    "committed_template_refresh_failed",
                    format!("template_id={} error={}", created.id, error.message),
                );
                created
            }
        };
        normalize_card_template_dto_for_editing(template)
    })
}

#[tauri::command]
pub fn update_card_template(
    database: State<'_, Database>,
    paths: State<'_, AppPaths>,
    input: UpdateCardTemplateInput,
) -> Result<CardTemplateDto, CommandError> {
    database.with_connection(|connection| {
        let input = prepare_automatic_update_template_input(connection, input)?;
        let draft_id = input.asset_draft_id.clone();
        let asset_ids = collect_template_input_asset_ids(&input.rules)?;
        let updated = update_card_template_in_connection(connection, &paths, input)?;
        finalize_assets_after_committed_write(
            connection,
            &paths,
            &asset_ids,
            draft_id.as_deref(),
            &updated.id,
            "update_card_template",
        );
        let template = match load_stored_template(connection, &paths, &updated.id, true) {
            Ok(stored) => stored.dto,
            Err(error) => {
                crate::logging::error(
                    "committed_template_refresh_failed",
                    format!("template_id={} error={}", updated.id, error.message),
                );
                updated
            }
        };
        normalize_card_template_dto_for_editing(template)
    })
}

#[tauri::command]
pub fn delete_card_template(
    database: State<'_, Database>,
    template_id: String,
) -> Result<(), CommandError> {
    validate_entity_id(&template_id, "card template")?;
    database
        .with_connection(|connection| delete_card_template_in_connection(connection, &template_id))
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
        render_automatic_template_previews(connection, &paths, &template, input.values)
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
    let draft_id = input.asset_draft_id.clone();
    let owner_id = input.deck_id.clone();
    let asset_ids = collect_template_value_asset_ids(&input.values)?;
    database.with_connection(|connection| {
        let result = generate_automatic_template_cards_in_connection(connection, &paths, input)?;
        finalize_assets_after_committed_write(
            connection,
            &paths,
            &asset_ids,
            draft_id.as_deref(),
            &owner_id,
            "generate_cards_from_template",
        );
        Ok(result)
    })
}

fn finalize_assets_after_committed_write(
    connection: &mut Connection,
    paths: &AppPaths,
    asset_ids: &[String],
    draft_id: Option<&str>,
    owner_id: &str,
    operation: &str,
) {
    let transaction = match connection.transaction_with_behavior(TransactionBehavior::Immediate) {
        Ok(transaction) => transaction,
        Err(error) => {
            crate::logging::error(
                "committed_asset_finalization_start_failed",
                format!("operation={operation} owner_id={owner_id} error={error}"),
            );
            return;
        }
    };
    let finalization =
        match finalize_assets_for_owner(&transaction, paths, asset_ids, draft_id, Some(owner_id)) {
            Ok(finalization) => finalization,
            Err(error) => {
                crate::logging::error(
                    "committed_asset_finalization_failed",
                    format!(
                        "operation={operation} owner_id={owner_id} error={}",
                        error.message
                    ),
                );
                return;
            }
        };
    if let Err(error) = transaction.commit() {
        crate::logging::error(
            "committed_asset_finalization_commit_failed",
            format!("operation={operation} owner_id={owner_id} error={error}"),
        );
        return;
    }
    finalization.cleanup();
}

fn collect_template_input_asset_ids(
    rules: &[CardTemplateRuleInput],
) -> Result<Vec<String>, CommandError> {
    let mut ids = BTreeSet::new();
    for rule in rules {
        ids.extend(crate::cards::collect_asset_ids(&rule.front_doc)?);
        ids.extend(crate::cards::collect_asset_ids(&rule.back_doc)?);
    }
    Ok(ids.into_iter().collect())
}

fn collect_template_value_asset_ids(
    values: &[TemplateValueInput],
) -> Result<Vec<String>, CommandError> {
    let mut ids = BTreeSet::new();
    for value in values {
        ids.extend(crate::cards::collect_asset_ids(&value.doc)?);
    }
    Ok(ids.into_iter().collect())
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

enum CardNoteDraft {
    Basic {
        reverse_enabled: bool,
        front_doc: Value,
        back_doc: Value,
    },
    Cloze {
        source: String,
        mode: ClozeMode,
    },
}

struct GeneratedCardWrite {
    note_id: Option<String>,
    deck_id: String,
    revision: Option<i64>,
    draft: CardNoteDraft,
    tags: Vec<String>,
    allow_duplicate: bool,
    asset_draft_id: Option<String>,
}

fn create_generated_card_in_database(
    database: State<'_, Database>,
    paths: State<'_, AppPaths>,
    input: CreateGeneratedCardInput,
) -> Result<OrganizedCardDto, CommandError> {
    let write = match input {
        CreateGeneratedCardInput::Basic {
            deck_id,
            reverse_enabled,
            front_doc,
            back_doc,
            tags,
            allow_duplicate,
            asset_draft_id,
        } => GeneratedCardWrite {
            note_id: None,
            deck_id,
            revision: None,
            draft: CardNoteDraft::Basic {
                reverse_enabled,
                front_doc,
                back_doc,
            },
            tags,
            allow_duplicate,
            asset_draft_id,
        },
        CreateGeneratedCardInput::Cloze {
            deck_id,
            cloze_source,
            cloze_mode,
            tags,
            allow_duplicate,
            asset_draft_id,
        } => GeneratedCardWrite {
            note_id: None,
            deck_id,
            revision: None,
            draft: CardNoteDraft::Cloze {
                source: cloze_source,
                mode: cloze_mode,
            },
            tags,
            allow_duplicate,
            asset_draft_id,
        },
    };
    create_prepared_card_note(database, paths, write)
}

fn create_prepared_card_note(
    database: State<'_, Database>,
    paths: State<'_, AppPaths>,
    write: GeneratedCardWrite,
) -> Result<OrganizedCardDto, CommandError> {
    let GeneratedCardWrite {
        note_id: _,
        deck_id,
        revision: _,
        draft,
        tags,
        allow_duplicate,
        asset_draft_id,
    } = write;
    validate_entity_id(&deck_id, "deck")?;
    if let Some(draft_id) = asset_draft_id.as_deref() {
        validate_entity_id(draft_id, "asset draft")?;
    }
    let prepared = prepare_card_note(draft)?;
    let asset_ids = content_asset_ids(&prepared.content);
    let tags = validate_tags(tags)?;
    let normalized = normalized_pair(&prepared.content.front_text, &prepared.content.back_text);
    let hash = duplicate_hash(&normalized.0, &normalized.1);

    let note_id = database.with_connection(|connection| {
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(CommandError::database)?;
        ensure_active_deck(&transaction, &deck_id)?;
        ensure_assets_available(&transaction, &paths, &asset_ids, asset_draft_id.as_deref())?;
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
        sync_generated_cards(&transaction, &note_id, &deck_id, &prepared.cards, now)?;
        replace_note_assets(&transaction, &note_id, &prepared.content)?;
        replace_note_tags(&transaction, &note_id, &tags, now)?;
        let finalization = finalize_assets_for_owner(
            &transaction,
            &paths,
            &asset_ids,
            asset_draft_id.as_deref(),
            Some(&note_id),
        )?;
        transaction.commit().map_err(CommandError::database)?;
        finalization.cleanup();
        Ok(note_id)
    })?;

    database.with_connection(|connection| hydrate_organized_card(connection, &paths, &note_id))
}

fn update_generated_card_in_database(
    database: State<'_, Database>,
    paths: State<'_, AppPaths>,
    input: UpdateGeneratedCardInput,
) -> Result<OrganizedCardDto, CommandError> {
    let write = match input {
        UpdateGeneratedCardInput::Basic {
            note_id,
            deck_id,
            revision,
            reverse_enabled,
            front_doc,
            back_doc,
            tags,
            allow_duplicate,
            asset_draft_id,
        } => GeneratedCardWrite {
            note_id: Some(note_id),
            deck_id,
            revision: Some(revision),
            draft: CardNoteDraft::Basic {
                reverse_enabled,
                front_doc,
                back_doc,
            },
            tags,
            allow_duplicate,
            asset_draft_id,
        },
        UpdateGeneratedCardInput::Cloze {
            note_id,
            deck_id,
            revision,
            cloze_source,
            cloze_mode,
            tags,
            allow_duplicate,
            asset_draft_id,
        } => GeneratedCardWrite {
            note_id: Some(note_id),
            deck_id,
            revision: Some(revision),
            draft: CardNoteDraft::Cloze {
                source: cloze_source,
                mode: cloze_mode,
            },
            tags,
            allow_duplicate,
            asset_draft_id,
        },
    };
    update_prepared_card_note(database, paths, write)
}

fn update_prepared_card_note(
    database: State<'_, Database>,
    paths: State<'_, AppPaths>,
    write: GeneratedCardWrite,
) -> Result<OrganizedCardDto, CommandError> {
    let GeneratedCardWrite {
        note_id,
        deck_id,
        revision,
        draft,
        tags,
        allow_duplicate,
        asset_draft_id,
    } = write;
    let note_id = note_id
        .ok_or_else(|| CommandError::new("card_not_found", "That flashcard no longer exists."))?;
    let revision = revision.unwrap_or_default();
    validate_entity_id(&note_id, "note")?;
    validate_entity_id(&deck_id, "deck")?;
    if let Some(draft_id) = asset_draft_id.as_deref() {
        validate_entity_id(draft_id, "asset draft")?;
    }
    if revision < 1 {
        return Err(CommandError::new(
            "invalid_revision",
            "The card revision is invalid.",
        ));
    }
    let prepared = prepare_card_note(draft)?;
    let asset_ids = content_asset_ids(&prepared.content);
    let tags = validate_tags(tags)?;
    let normalized = normalized_pair(&prepared.content.front_text, &prepared.content.back_text);
    let hash = duplicate_hash(&normalized.0, &normalized.1);

    database.with_connection(|connection| {
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(CommandError::database)?;
        ensure_active_deck(&transaction, &deck_id)?;
        ensure_assets_available(&transaction, &paths, &asset_ids, asset_draft_id.as_deref())?;
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
        if display_card_type(&existing_type) != prepared.note_type {
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
        sync_generated_cards(&transaction, &note_id, &deck_id, &prepared.cards, now)?;
        replace_note_assets(&transaction, &note_id, &prepared.content)?;
        replace_note_tags(&transaction, &note_id, &tags, now)?;
        let finalization = finalize_assets_for_owner(
            &transaction,
            &paths,
            &asset_ids,
            asset_draft_id.as_deref(),
            Some(&note_id),
        )?;
        transaction.commit().map_err(CommandError::database)?;
        finalization.cleanup();
        Ok(())
    })?;

    database.with_connection(|connection| hydrate_organized_card(connection, &paths, &note_id))
}

fn prepare_basic_card_content(
    front_doc: Value,
    back_doc: Value,
) -> Result<ValidatedContent, CommandError> {
    validate_content(front_doc, back_doc, String::new(), String::new())
}

fn prepare_card_note(input: CardNoteDraft) -> Result<PreparedGeneratedNote, CommandError> {
    match input {
        CardNoteDraft::Basic {
            reverse_enabled,
            front_doc,
            back_doc,
        } => {
            let content = prepare_basic_card_content(front_doc, back_doc)?;
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
                cloze_mode: ClozeMode::OneAtATime.as_str().to_string(),
                content,
                cards,
            })
        }
        CardNoteDraft::Cloze { source, mode } => {
            let parsed = parse_cloze(&source)?;
            let target = if mode == ClozeMode::AllTogether {
                None
            } else {
                parsed.groups.first().copied()
            };
            let question = render_cloze_question(&parsed, target);
            let answer = render_cloze_answer(&parsed, target);
            let content = validate_content(
                plain_text_document(&question),
                plain_text_document(&answer),
                String::new(),
                String::new(),
            )?;
            Ok(PreparedGeneratedNote {
                note_type: "cloze",
                reverse_enabled: false,
                cloze_source: source,
                cloze_mode: mode.as_str().to_string(),
                content,
                cards: vec![GeneratedCardSpec {
                    direction: "forward",
                    template_ordinal: 0,
                }],
            })
        }
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
        let rows = statement
            .query_map([note_id], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, i64>(2)?,
                    row.get::<_, Option<i64>>(3)?,
                ))
            })
            .map_err(CommandError::database)?;
        rows.collect::<Result<Vec<_>, _>>()
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
        if let Some((card_id, _)) = by_key.get(&(spec.direction.to_string(), spec.template_ordinal))
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
        let close = after_separator
            .find("}}")
            .ok_or_else(invalid_cloze_marker)?;
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

pub(crate) struct ProjectedPresentationInput<'a> {
    pub direction: &'a str,
    pub template_ordinal: i64,
    pub note_type: &'a str,
    pub front_doc_json: &'a str,
    pub back_doc_json: &'a str,
    pub front_text: &'a str,
    pub back_text: &'a str,
    pub cloze_source: &'a str,
    pub cloze_mode: &'a str,
    pub reps: i64,
    pub presentation_variant: Option<&'a str>,
}

pub(crate) fn projected_presentation_variant(
    note_type: &str,
    cloze_source: &str,
    cloze_mode: &str,
    reps: i64,
) -> Result<String, CommandError> {
    if note_type != "cloze" {
        return Ok(String::new());
    }
    validate_cloze_mode(cloze_mode)?;
    if cloze_mode == "all_together" {
        return Ok("cloze:all".to_string());
    }
    let parsed = parse_cloze(cloze_source)?;
    let index = usize::try_from(reps.max(0)).unwrap_or(0) % parsed.groups.len();
    Ok(format!("cloze:{}", parsed.groups[index]))
}

/// Render a generated-card presentation entirely from an already loaded SQL
/// row. Bounded list and practice-window projections use this function so card
/// count affects CPU work, not SQL statement count.
pub(crate) fn render_projected_presentation(
    input: ProjectedPresentationInput<'_>,
) -> Result<RenderedCardPresentation, CommandError> {
    if input.note_type == "basic" {
        let front_doc =
            serde_json::from_str(input.front_doc_json).map_err(CommandError::serialization)?;
        let back_doc =
            serde_json::from_str(input.back_doc_json).map_err(CommandError::serialization)?;
        return match input.direction {
            "forward" => Ok(RenderedCardPresentation {
                card_type: "basic".to_string(),
                card_label: "Basic".to_string(),
                front_doc,
                back_doc,
                front_text: input.front_text.to_string(),
                back_text: input.back_text.to_string(),
            }),
            "reverse" => Ok(RenderedCardPresentation {
                card_type: "basic".to_string(),
                card_label: "Reverse".to_string(),
                front_doc: back_doc,
                back_doc: front_doc,
                front_text: input.back_text.to_string(),
                back_text: input.front_text.to_string(),
            }),
            _ => Err(CommandError::new(
                "invalid_generated_card",
                "A Basic note contains an unsupported generated-card direction.",
            )),
        };
    }

    if input.note_type != "cloze" || input.direction != "forward" || input.template_ordinal != 0 {
        return Err(CommandError::new(
            "invalid_generated_card",
            "The Fill in the blank note contains an unsupported active card identity.",
        ));
    }
    validate_cloze_mode(input.cloze_mode)?;
    let parsed = parse_cloze(input.cloze_source)?;
    let target = cloze_target_from_variant(
        &parsed,
        input.cloze_mode,
        input.presentation_variant,
        input.reps,
    )?;
    let question = render_cloze_question(&parsed, target);
    let answer = render_cloze_answer(&parsed, target);
    let label = if input.cloze_mode == "all_together" {
        "Fill in the blank · all blanks".to_string()
    } else {
        format!(
            "Fill in the blank · {} alternating {}",
            parsed.groups.len(),
            if parsed.groups.len() == 1 {
                "blank"
            } else {
                "blanks"
            }
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

pub(crate) mod templates;
pub(crate) use templates::apply_stage3_data_repairs;
use templates::{
    create_card_template_in_connection, delete_card_template_in_connection,
    ensure_template_revision, generate_automatic_template_cards_in_connection,
    load_stored_template, normalize_card_template_dto_for_editing,
    prepare_automatic_create_template_input, prepare_automatic_update_template_input,
    render_automatic_template_previews, update_card_template_in_connection,
};

pub(crate) mod generated_import;

#[cfg(test)]
mod authoring_contract_tests {
    use super::*;

    fn paragraph(text: &str) -> Value {
        json!({
            "type": "doc",
            "content": [{"type": "paragraph", "content": [{"type": "text", "text": text}]}]
        })
    }

    #[test]
    fn generated_card_contracts_reject_type_inappropriate_fields() {
        let basic = serde_json::json!({
            "cardType": "basic",
            "deckId": "11111111111111111111111111111111",
            "frontDoc": paragraph("Question"),
            "backDoc": paragraph("Answer"),
            "clozeSource": "should not be accepted"
        });
        assert!(serde_json::from_value::<CreateGeneratedCardInput>(basic).is_err());

        let cloze = serde_json::json!({
            "cardType": "cloze",
            "deckId": "11111111111111111111111111111111",
            "clozeSource": "The {{c1::heart}} pumps blood.",
            "frontDoc": paragraph("should not be accepted")
        });
        assert!(serde_json::from_value::<CreateGeneratedCardInput>(cloze).is_err());
    }

    #[test]
    fn basic_contract_does_not_require_browser_plain_text_snapshots() {
        let input = serde_json::json!({
            "cardType": "basic",
            "deckId": "11111111111111111111111111111111",
            "frontDoc": paragraph("Question"),
            "backDoc": paragraph("Answer")
        });
        assert!(serde_json::from_value::<CreateGeneratedCardInput>(input).is_ok());
    }
}
