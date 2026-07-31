//! Phase 2 standalone flashcard core.
//!
//! This module owns note templates, generated cards, tags, FTS5 browsing,
//! editor drafts, duplicate checks, and card-state actions. Card content and
//! imported deck data are untrusted. Rich documents are accepted only after
//! the canonical validator in `cards` succeeds. Templates are deliberately
//! non-executable: literal text plus `{{field name}}` placeholders. Fill-in-the-
//! blank notes use literal `[[answer]]` markers. Rendered values become JSON
//! text nodes and are never interpreted as HTML, JavaScript, CSS, or SQL.

use std::collections::{BTreeMap, BTreeSet, HashMap};

use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tauri::State;

use crate::{
    assets::{ensure_assets_exist, get_assets_for_note, AssetDto},
    cards::{validate_content, CardDto, ValidatedContent},
    database::Database,
    error::CommandError,
    paths::AppPaths,
    scheduler::{insert_new_schedule, new_schedule, update_schedule},
    util::{generate_id, unix_time_millis, validate_entity_id},
};

const MAX_TAGS_PER_NOTE: usize = 100;
const MAX_TEMPLATE_FIELDS: usize = 64;
const MAX_TEMPLATE_CARDS: usize = 64;
const MAX_TEMPLATE_PATTERN_CHARS: usize = 20_000;
const MAX_FIELD_VALUE_CHARS: usize = 100_000;
const MAX_BULK_ENTRIES: usize = 500;
const MAX_DRAFT_BYTES: usize = 4_000_000;
const PHARMACOLOGY_TEMPLATE_ID: &str = "fffffffffffffffffffffffffffffff1";

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TemplateCardSpec {
    pub name: String,
    pub front: String,
    pub back: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CardTemplateDto {
    pub id: String,
    pub name: String,
    pub description: String,
    pub fields: Vec<String>,
    pub cards: Vec<TemplateCardSpec>,
    pub is_builtin: bool,
    pub created_at: i64,
    pub updated_at: i64,
    pub revision: i64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveTemplateInput {
    pub template_id: Option<String>,
    pub revision: Option<i64>,
    pub name: String,
    #[serde(default)]
    pub description: String,
    pub fields: Vec<String>,
    pub cards: Vec<TemplateCardSpec>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveNoteInput {
    pub note_id: Option<String>,
    pub revision: Option<i64>,
    pub deck_id: String,
    pub mode: String,
    pub front_doc: Value,
    pub back_doc: Value,
    #[serde(default)]
    pub front_text: String,
    #[serde(default)]
    pub back_text: String,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub create_reverse: bool,
    pub fill_blank_text: Option<String>,
    pub template_id: Option<String>,
    #[serde(default)]
    pub fields: BTreeMap<String, String>,
    #[serde(default)]
    pub allow_duplicate: bool,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NoteEditorDto {
    pub note_id: String,
    pub deck_id: String,
    pub mode: String,
    pub front_doc: Value,
    pub back_doc: Value,
    pub front_text: String,
    pub back_text: String,
    pub tags: Vec<String>,
    pub create_reverse: bool,
    pub fill_blank_text: Option<String>,
    pub template_id: Option<String>,
    pub fields: BTreeMap<String, String>,
    pub assets: Vec<AssetDto>,
    pub generated_card_count: i64,
    pub revision: i64,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BrowserCardDto {
    pub note_id: String,
    pub deck_id: String,
    pub mode: String,
    pub front_text: String,
    pub back_text: String,
    pub tags: Vec<String>,
    pub generated_card_count: i64,
    pub due_at: Option<i64>,
    pub is_new: bool,
    pub suspended: bool,
    pub buried_until: Option<i64>,
    pub struggling: bool,
    pub struggling_reason: Option<String>,
    pub has_images: bool,
    pub created_at: i64,
    pub updated_at: i64,
    pub revision: i64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BrowseCardsInput {
    pub deck_id: String,
    #[serde(default)]
    pub query: String,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default = "default_all")]
    pub status: String,
    #[serde(default = "default_all")]
    pub mode: String,
    #[serde(default = "default_updated_desc")]
    pub sort: String,
    #[serde(default = "default_page_limit")]
    pub limit: usize,
    #[serde(default)]
    pub offset: usize,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BrowseCardsResultDto {
    pub cards: Vec<BrowserCardDto>,
    pub total: usize,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DuplicateNoteDto {
    pub note_id: String,
    pub deck_id: String,
    pub deck_name: String,
    pub front_text: String,
    pub back_text: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NoteStateInput {
    pub note_id: String,
    pub value: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BuryNoteInput {
    pub note_id: String,
    pub buried_until: Option<i64>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RescheduleNoteInput {
    pub note_id: String,
    pub due_at: i64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MoveCopyNoteInput {
    pub note_id: String,
    pub destination_deck_id: String,
    pub operation: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MoveCopyResultDto {
    pub note_id: String,
    pub destination_deck_id: String,
    pub copied: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BulkEntryRow {
    pub question: String,
    pub answer: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BulkCreateInput {
    pub deck_id: String,
    pub entries: Vec<BulkEntryRow>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub create_reverse: bool,
    #[serde(default = "default_true")]
    pub skip_duplicates: bool,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BulkCreateResultDto {
    pub created_count: usize,
    pub skipped_duplicates: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveDraftInput {
    pub draft_key: String,
    pub deck_id: String,
    pub note_id: Option<String>,
    pub payload: Value,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EditorDraftDto {
    pub draft_key: String,
    pub deck_id: String,
    pub note_id: Option<String>,
    pub payload: Value,
    pub updated_at: i64,
}

#[derive(Clone, Debug)]
struct GeneratedCardSpec {
    direction: &'static str,
    ordinal: i64,
    display_name: String,
}

struct PreparedNote {
    note_type: &'static str,
    template_id: Option<String>,
    fields_json: String,
    content: ValidatedContent,
    generated: Vec<GeneratedCardSpec>,
    duplicate_key: String,
}

#[tauri::command]
pub fn list_card_templates(
    database: State<'_, Database>,
) -> Result<Vec<CardTemplateDto>, CommandError> {
    database.with_connection(list_templates_from_connection)
}

#[tauri::command]
pub fn save_card_template(
    database: State<'_, Database>,
    input: SaveTemplateInput,
) -> Result<CardTemplateDto, CommandError> {
    database.with_connection(|connection| save_template_in_connection(connection, input))
}

#[tauri::command]
pub fn delete_card_template(
    database: State<'_, Database>,
    template_id: String,
) -> Result<(), CommandError> {
    validate_entity_id(&template_id, "card template")?;
    database.with_connection(|connection| delete_template_in_connection(connection, &template_id))
}

#[tauri::command]
pub fn save_advanced_note(
    database: State<'_, Database>,
    paths: State<'_, AppPaths>,
    input: SaveNoteInput,
) -> Result<NoteEditorDto, CommandError> {
    validate_entity_id(&input.deck_id, "deck")?;
    if let Some(note_id) = &input.note_id {
        validate_entity_id(note_id, "note")?;
    }
    database.with_connection(|connection| save_note_in_connection(connection, &paths, input))
}

#[tauri::command]
pub fn get_note_for_editing(
    database: State<'_, Database>,
    paths: State<'_, AppPaths>,
    note_id: String,
) -> Result<NoteEditorDto, CommandError> {
    validate_entity_id(&note_id, "note")?;
    database.with_connection(|connection| get_note_editor(connection, &paths, &note_id))
}

#[tauri::command]
pub fn browse_cards(
    database: State<'_, Database>,
    input: BrowseCardsInput,
) -> Result<BrowseCardsResultDto, CommandError> {
    validate_entity_id(&input.deck_id, "deck")?;
    database.with_connection(|connection| browse_cards_in_connection(connection, input))
}

#[tauri::command]
pub fn find_note_duplicates(
    database: State<'_, Database>,
    deck_id: String,
    front_text: String,
    back_text: String,
    exclude_note_id: Option<String>,
) -> Result<Vec<DuplicateNoteDto>, CommandError> {
    validate_entity_id(&deck_id, "deck")?;
    if let Some(note_id) = &exclude_note_id {
        validate_entity_id(note_id, "note")?;
    }
    let key = duplicate_key("basic", &front_text, &back_text, "{}");
    database.with_connection(|connection| {
        load_duplicates(connection, &deck_id, &key, exclude_note_id.as_deref())
    })
}

#[tauri::command]
pub fn render_generated_card(
    database: State<'_, Database>,
    paths: State<'_, AppPaths>,
    card_id: String,
) -> Result<CardDto, CommandError> {
    validate_entity_id(&card_id, "card")?;
    database.with_connection(|connection| render_card_from_connection(connection, &paths, &card_id))
}

#[tauri::command]
pub fn set_note_suspended(
    database: State<'_, Database>,
    input: NoteStateInput,
) -> Result<(), CommandError> {
    validate_entity_id(&input.note_id, "note")?;
    database.with_connection(|connection| {
        set_suspended_in_connection(connection, &input.note_id, input.value)
    })
}

#[tauri::command]
pub fn bury_note(
    database: State<'_, Database>,
    input: BuryNoteInput,
) -> Result<(), CommandError> {
    validate_entity_id(&input.note_id, "note")?;
    if let Some(until) = input.buried_until {
        validate_timestamp(until, "bury time")?;
    }
    database.with_connection(|connection| {
        bury_note_in_connection(connection, &input.note_id, input.buried_until)
    })
}

#[tauri::command]
pub fn reset_note_schedule(
    database: State<'_, Database>,
    note_id: String,
) -> Result<(), CommandError> {
    validate_entity_id(&note_id, "note")?;
    database.with_connection(|connection| reset_note_in_connection(connection, &note_id))
}

#[tauri::command]
pub fn reschedule_note(
    database: State<'_, Database>,
    input: RescheduleNoteInput,
) -> Result<(), CommandError> {
    validate_entity_id(&input.note_id, "note")?;
    validate_timestamp(input.due_at, "due date")?;
    database.with_connection(|connection| {
        reschedule_note_in_connection(connection, &input.note_id, input.due_at)
    })
}

#[tauri::command]
pub fn move_or_copy_note(
    database: State<'_, Database>,
    input: MoveCopyNoteInput,
) -> Result<MoveCopyResultDto, CommandError> {
    validate_entity_id(&input.note_id, "note")?;
    validate_entity_id(&input.destination_deck_id, "deck")?;
    database.with_connection(|connection| move_or_copy_in_connection(connection, input))
}

#[tauri::command]
pub fn bulk_create_notes(
    database: State<'_, Database>,
    input: BulkCreateInput,
) -> Result<BulkCreateResultDto, CommandError> {
    validate_entity_id(&input.deck_id, "deck")?;
    database.with_connection(|connection| bulk_create_in_connection(connection, input))
}

#[tauri::command]
pub fn save_editor_draft(
    database: State<'_, Database>,
    input: SaveDraftInput,
) -> Result<EditorDraftDto, CommandError> {
    validate_draft_key(&input.draft_key)?;
    validate_entity_id(&input.deck_id, "deck")?;
    if let Some(note_id) = &input.note_id {
        validate_entity_id(note_id, "note")?;
    }
    database.with_connection(|connection| save_draft_in_connection(connection, input))
}

#[tauri::command]
pub fn get_editor_draft(
    database: State<'_, Database>,
    draft_key: String,
) -> Result<Option<EditorDraftDto>, CommandError> {
    validate_draft_key(&draft_key)?;
    database.with_connection(|connection| get_draft_from_connection(connection, &draft_key))
}

#[tauri::command]
pub fn delete_editor_draft(
    database: State<'_, Database>,
    draft_key: String,
) -> Result<(), CommandError> {
    validate_draft_key(&draft_key)?;
    database.with_connection(|connection| {
        connection
            .execute("DELETE FROM editor_drafts WHERE draft_key = ?1", [draft_key])
            .map_err(CommandError::database)?;
        Ok(())
    })
}

pub(crate) fn render_card_from_connection(
    connection: &Connection,
    paths: &AppPaths,
    card_id: &str,
) -> Result<CardDto, CommandError> {
    let raw: Option<(
        String,
        String,
        String,
        String,
        String,
        i64,
        i64,
        i64,
        String,
        String,
        String,
        String,
        String,
        Option<String>,
        String,
        String,
        i64,
    )> = connection
        .query_row(
            "SELECT c.id, c.note_id, c.deck_id, c.direction, c.display_name, c.template_ordinal,\n\
                    c.created_at, c.updated_at, n.front_doc, n.back_doc,\n\
                    n.front_text, n.back_text, n.note_type, n.template_id,\n\
                    n.fields_json, n.content_format, n.revision\n\
             FROM cards c\n\
             JOIN notes n ON n.id = c.note_id\n\
             JOIN decks d ON d.id = c.deck_id\n\
             WHERE c.id = ?1\n\
               AND c.deleted_at IS NULL\n\
               AND n.deleted_at IS NULL\n\
               AND d.deleted_at IS NULL\n\
               AND d.archived_at IS NULL",
            [card_id],
            |row| {
                Ok((
                    row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?,
                    row.get(5)?, row.get(6)?, row.get(7)?, row.get(8)?, row.get(9)?,
                    row.get(10)?, row.get(11)?, row.get(12)?, row.get(13)?, row.get(14)?,
                    row.get(15)?, row.get(16)?,
                ))
            },
        )
        .optional()
        .map_err(CommandError::database)?;
    let Some((
        id,
        note_id,
        deck_id,
        direction,
        display_name,
        ordinal,
        created_at,
        updated_at,
        front_doc_json,
        back_doc_json,
        stored_front_text,
        stored_back_text,
        note_type,
        template_id,
        fields_json,
        content_format,
        revision,
    )) = raw
    else {
        return Err(CommandError::new(
            "card_not_found",
            "That flashcard no longer exists.",
        ));
    };
    if content_format != "tiptap-json" {
        return Err(CommandError::new(
            "unsupported_card_content",
            "This card uses an unsupported content format.",
        ));
    }

    let stored_front: Value =
        serde_json::from_str(&front_doc_json).map_err(CommandError::serialization)?;
    let stored_back: Value =
        serde_json::from_str(&back_doc_json).map_err(CommandError::serialization)?;
    let (front_doc, back_doc, front_text, back_text) = if let Some(template_id) = template_id {
        let template = load_template(connection, &template_id)?;
        let fields = parse_fields_json(&fields_json)?;
        let spec = template
            .cards
            .get(usize::try_from(ordinal).unwrap_or(usize::MAX))
            .ok_or_else(|| {
                CommandError::new(
                    "invalid_generated_card",
                    "This generated card no longer matches its template.",
                )
            })?;
        let front_text = render_pattern(&spec.front, &fields)?;
        let back_text = render_pattern(&spec.back, &fields)?;
        (
            plain_document(&front_text),
            plain_document(&back_text),
            front_text,
            back_text,
        )
    } else if note_type == "cloze" || direction == "cloze" {
        let fields = parse_fields_json(&fields_json)?;
        let source = fields
            .get("fill in the blank")
            .cloned()
            .unwrap_or(stored_front_text.clone());
        let blanks = parse_fill_blanks(&source)?;
        let index = usize::try_from(ordinal).unwrap_or(usize::MAX);
        let target = blanks.get(index).ok_or_else(|| {
            CommandError::new(
                "invalid_generated_card",
                "This fill-in-the-blank card no longer matches its note.",
            )
        })?;
        let question = render_fill_question(&source, index)?;
        (
            plain_document(&question),
            plain_document(target),
            question,
            target.clone(),
        )
    } else if direction == "reverse" {
        (
            stored_back,
            stored_front,
            stored_back_text,
            stored_front_text,
        )
    } else {
        (
            stored_front,
            stored_back,
            stored_front_text,
            stored_back_text,
        )
    };

    let assets = get_assets_for_note(connection, paths, &note_id)?;
    Ok(CardDto {
        id,
        note_id,
        deck_id,
        direction,
        display_name,
        front_doc,
        back_doc,
        front_text,
        back_text,
        assets,
        created_at,
        updated_at,
        revision,
    })
}

fn save_note_in_connection(
    connection: &mut Connection,
    paths: &AppPaths,
    input: SaveNoteInput,
) -> Result<NoteEditorDto, CommandError> {
    ensure_active_deck(connection, &input.deck_id)?;
    let prepared = prepare_note(connection, &input)?;
    let normalized_tags = validate_tags(input.tags)?;
    let excluded = input.note_id.as_deref();
    if !input.allow_duplicate {
        let duplicates = load_duplicates(
            connection,
            &input.deck_id,
            &prepared.duplicate_key,
            excluded,
        )?;
        if let Some(first) = duplicates.first() {
            return Err(CommandError::new(
                "duplicate_card",
                format!(
                    "A matching card already exists in “{}”. Choose Save anyway to keep both.",
                    first.deck_name
                ),
            ));
        }
    }

    let now = unix_time_millis()?;
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(CommandError::database)?;
    ensure_assets_exist(&transaction, &all_asset_ids(&prepared.content))?;

    let note_id = if let Some(note_id) = input.note_id {
        let revision = input.revision.filter(|value| *value >= 1).ok_or_else(|| {
            CommandError::new("invalid_revision", "The card revision is invalid.")
        })?;
        let changed = transaction
            .execute(
                "UPDATE notes SET\n\
                     note_type = ?1, template_id = ?2, fields_json = ?3,\n\
                     front_doc = ?4, back_doc = ?5, front_text = ?6, back_text = ?7,\n\
                     duplicate_key = ?8, updated_at = ?9, revision = revision + 1\n\
                 WHERE id = ?10 AND deck_id = ?11 AND revision = ?12 AND deleted_at IS NULL",
                params![
                    prepared.note_type,
                    prepared.template_id,
                    prepared.fields_json,
                    prepared.content.front_doc_json,
                    prepared.content.back_doc_json,
                    prepared.content.front_text,
                    prepared.content.back_text,
                    prepared.duplicate_key,
                    now,
                    note_id,
                    input.deck_id,
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
        synchronize_generated_cards(&transaction, &note_id, &input.deck_id, &prepared.generated, now)?;
        replace_note_assets(&transaction, &note_id, &prepared.content)?;
        replace_note_tags(&transaction, &note_id, &normalized_tags, now)?;
        refresh_note_fts(&transaction, &note_id)?;
        note_id
    } else {
        let note_id = generate_id(&transaction)?;
        transaction
            .execute(
                "INSERT INTO notes (\n\
                     id, deck_id, note_type, content_format, content_schema_version,\n\
                     front_doc, back_doc, front_text, back_text, created_at, updated_at, revision,\n\
                     template_id, fields_json, duplicate_key\n\
                 ) VALUES (?1, ?2, ?3, 'tiptap-json', 1, ?4, ?5, ?6, ?7, ?8, ?8, 1, ?9, ?10, ?11)",
                params![
                    note_id,
                    input.deck_id,
                    prepared.note_type,
                    prepared.content.front_doc_json,
                    prepared.content.back_doc_json,
                    prepared.content.front_text,
                    prepared.content.back_text,
                    now,
                    prepared.template_id,
                    prepared.fields_json,
                    prepared.duplicate_key,
                ],
            )
            .map_err(CommandError::database)?;
        synchronize_generated_cards(&transaction, &note_id, &input.deck_id, &prepared.generated, now)?;
        replace_note_assets(&transaction, &note_id, &prepared.content)?;
        replace_note_tags(&transaction, &note_id, &normalized_tags, now)?;
        refresh_note_fts(&transaction, &note_id)?;
        note_id
    };

    transaction.commit().map_err(CommandError::database)?;
    get_note_editor(connection, paths, &note_id)
}

fn prepare_note(connection: &Connection, input: &SaveNoteInput) -> Result<PreparedNote, CommandError> {
    match input.mode.as_str() {
        "basic" | "reverse" => {
            let content = validate_content(
                input.front_doc.clone(),
                input.back_doc.clone(),
                input.front_text.clone(),
                input.back_text.clone(),
            )?;
            let reverse = input.mode == "reverse" || input.create_reverse;
            let mut generated = vec![GeneratedCardSpec {
                direction: "forward",
                ordinal: 0,
                display_name: "Forward".to_string(),
            }];
            if reverse {
                generated.push(GeneratedCardSpec {
                    direction: "reverse",
                    ordinal: 0,
                    display_name: "Reverse".to_string(),
                });
            }
            let fields_json = "{}".to_string();
            let key = duplicate_key(
                if reverse { "reverse" } else { "basic" },
                &content.front_text,
                &content.back_text,
                &fields_json,
            );
            Ok(PreparedNote {
                note_type: "basic",
                template_id: None,
                fields_json,
                content,
                generated,
                duplicate_key: key,
            })
        }
        "fill_blank" => {
            let source = input
                .fill_blank_text
                .as_deref()
                .unwrap_or_default()
                .trim()
                .to_string();
            let blanks = parse_fill_blanks(&source)?;
            let answers = blanks.join(" · ");
            let front_doc = plain_document(&source);
            let back_doc = plain_document(&answers);
            let content = validate_content(
                front_doc,
                back_doc,
                source.clone(),
                answers.clone(),
            )?;
            let fields = BTreeMap::from([("fill in the blank".to_string(), source.clone())]);
            let fields_json = serde_json::to_string(&fields).map_err(CommandError::serialization)?;
            let generated = blanks
                .iter()
                .enumerate()
                .map(|(index, _)| GeneratedCardSpec {
                    direction: "cloze",
                    ordinal: i64::try_from(index).unwrap_or(i64::MAX),
                    display_name: format!("Blank {}", index + 1),
                })
                .collect::<Vec<_>>();
            let key = duplicate_key("fill_blank", &source, &answers, &fields_json);
            Ok(PreparedNote {
                note_type: "cloze",
                template_id: None,
                fields_json,
                content,
                generated,
                duplicate_key: key,
            })
        }
        "template" => {
            let template_id = input.template_id.as_deref().ok_or_else(|| {
                CommandError::new("template_required", "Choose a card template.")
            })?;
            validate_entity_id(template_id, "card template")?;
            let template = load_template(connection, template_id)?;
            let fields = normalize_template_values(&template.fields, &input.fields)?;
            let mut generated = Vec::with_capacity(template.cards.len());
            let mut rendered = Vec::with_capacity(template.cards.len());
            for (index, spec) in template.cards.iter().enumerate() {
                let front = render_pattern(&spec.front, &fields)?;
                let back = render_pattern(&spec.back, &fields)?;
                if front.trim().is_empty() || back.trim().is_empty() {
                    return Err(CommandError::new(
                        "empty_template_card",
                        format!("The template card “{}” rendered an empty side.", spec.name),
                    ));
                }
                rendered.push((front, back));
                generated.push(GeneratedCardSpec {
                    direction: "forward",
                    ordinal: i64::try_from(index).unwrap_or(i64::MAX),
                    display_name: spec.name.clone(),
                });
            }
            let Some((first_front, first_back)) = rendered.first() else {
                return Err(CommandError::new(
                    "invalid_template",
                    "A card template must generate at least one card.",
                ));
            };
            let content = validate_content(
                plain_document(first_front),
                plain_document(first_back),
                first_front.clone(),
                first_back.clone(),
            )?;
            let fields_json = serde_json::to_string(&fields).map_err(CommandError::serialization)?;
            let key = duplicate_key("template", first_front, first_back, &fields_json);
            Ok(PreparedNote {
                note_type: "basic",
                template_id: Some(template.id),
                fields_json,
                content,
                generated,
                duplicate_key: key,
            })
        }
        _ => Err(CommandError::new(
            "invalid_card_mode",
            "Choose Basic, Reverse, Fill in the blank, or Template.",
        )),
    }
}

fn synchronize_generated_cards(
    connection: &Connection,
    note_id: &str,
    deck_id: &str,
    desired: &[GeneratedCardSpec],
    now: i64,
) -> Result<(), CommandError> {
    let mut statement = connection
        .prepare(
            "SELECT id, direction, template_ordinal, deleted_at\n\
             FROM cards WHERE note_id = ?1",
        )
        .map_err(CommandError::database)?;
    let existing_rows = statement
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
        .map_err(CommandError::database)?;
    drop(statement);

    let existing = existing_rows
        .into_iter()
        .map(|row| ((row.1, row.2), (row.0, row.3)))
        .collect::<HashMap<_, _>>();
    let desired_keys = desired
        .iter()
        .map(|spec| (spec.direction.to_string(), spec.ordinal))
        .collect::<BTreeSet<_>>();

    for spec in desired {
        let key = (spec.direction.to_string(), spec.ordinal);
        if let Some((card_id, _)) = existing.get(&key) {
            connection
                .execute(
                    "UPDATE cards SET deck_id = ?1, display_name = ?2,\n\
                         deleted_at = NULL, updated_at = ?3\n\
                     WHERE id = ?4",
                    params![deck_id, spec.display_name, now, card_id],
                )
                .map_err(CommandError::database)?;
            let has_schedule: i64 = connection
                .query_row(
                    "SELECT EXISTS(SELECT 1 FROM card_schedules WHERE card_id = ?1)",
                    [card_id],
                    |row| row.get(0),
                )
                .map_err(CommandError::database)?;
            if has_schedule == 0 {
                insert_new_schedule(connection, card_id, now)?;
            }
        } else {
            let card_id = generate_id(connection)?;
            connection
                .execute(
                    "INSERT INTO cards (\n\
                         id, note_id, deck_id, direction, template_ordinal,\n\
                         created_at, updated_at, display_name\n\
                     ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?6, ?7)",
                    params![
                        card_id,
                        note_id,
                        deck_id,
                        spec.direction,
                        spec.ordinal,
                        now,
                        spec.display_name,
                    ],
                )
                .map_err(CommandError::database)?;
            insert_new_schedule(connection, &card_id, now)?;
        }
    }

    for ((direction, ordinal), (card_id, deleted_at)) in existing {
        if !desired_keys.contains(&(direction, ordinal)) && deleted_at.is_none() {
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

fn get_note_editor(
    connection: &Connection,
    paths: &AppPaths,
    note_id: &str,
) -> Result<NoteEditorDto, CommandError> {
    let row: Option<(
        String,
        String,
        String,
        String,
        String,
        String,
        Option<String>,
        String,
        i64,
    )> = connection
        .query_row(
            "SELECT deck_id, note_type, front_doc, back_doc, front_text, back_text,\n\
                    template_id, fields_json, revision\n\
             FROM notes WHERE id = ?1 AND deleted_at IS NULL",
            [note_id],
            |row| {
                Ok((
                    row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?,
                    row.get(5)?, row.get(6)?, row.get(7)?, row.get(8)?,
                ))
            },
        )
        .optional()
        .map_err(CommandError::database)?;
    let Some((
        deck_id,
        note_type,
        front_doc_json,
        back_doc_json,
        front_text,
        back_text,
        template_id,
        fields_json,
        revision,
    )) = row
    else {
        return Err(CommandError::new(
            "card_not_found",
            "That flashcard no longer exists.",
        ));
    };
    let generated_card_count: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM cards WHERE note_id = ?1 AND deleted_at IS NULL",
            [note_id],
            |row| row.get(0),
        )
        .map_err(CommandError::database)?;
    let has_reverse: i64 = connection
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM cards\n\
             WHERE note_id = ?1 AND direction = 'reverse' AND deleted_at IS NULL)",
            [note_id],
            |row| row.get(0),
        )
        .map_err(CommandError::database)?;
    let fields = parse_fields_json(&fields_json)?;
    let mode = if template_id.is_some() {
        "template"
    } else if note_type == "cloze" {
        "fill_blank"
    } else if has_reverse != 0 {
        "reverse"
    } else {
        "basic"
    };
    Ok(NoteEditorDto {
        note_id: note_id.to_string(),
        deck_id,
        mode: mode.to_string(),
        front_doc: serde_json::from_str(&front_doc_json).map_err(CommandError::serialization)?,
        back_doc: serde_json::from_str(&back_doc_json).map_err(CommandError::serialization)?,
        front_text,
        back_text,
        tags: load_tags(connection, note_id)?,
        create_reverse: has_reverse != 0,
        fill_blank_text: fields.get("fill in the blank").cloned(),
        template_id,
        fields,
        assets: get_assets_for_note(connection, paths, note_id)?,
        generated_card_count,
        revision,
    })
}

fn browse_cards_in_connection(
    connection: &mut Connection,
    input: BrowseCardsInput,
) -> Result<BrowseCardsResultDto, CommandError> {
    ensure_active_deck(connection, &input.deck_id)?;
    let candidate_ids = if input.query.trim().is_empty() {
        None
    } else {
        let query = build_fts_query(&input.query)?;
        let mut statement = connection
            .prepare(
                "SELECT note_id FROM notes_fts\n\
                 WHERE notes_fts MATCH ?1 AND deck_id = ?2\n\
                 ORDER BY bm25(notes_fts) LIMIT 5000",
            )
            .map_err(CommandError::database)?;
        let ids = statement
            .query_map(params![query, input.deck_id], |row| row.get::<_, String>(0))
            .map_err(CommandError::database)?
            .collect::<Result<BTreeSet<_>, _>>()
            .map_err(CommandError::database)?;
        Some(ids)
    };

    let now = unix_time_millis()?;
    let mut statement = connection
        .prepare(
            "SELECT n.id, n.deck_id, n.note_type, n.template_id, n.front_text, n.back_text,\n\
                    n.created_at, n.updated_at, n.revision,\n\
                    COUNT(c.id) AS generated_count,\n\
                    MIN(s.due_at) AS due_at,\n\
                    MIN(s.state) AS min_state,\n\
                    SUM(CASE WHEN c.suspended_at IS NOT NULL THEN 1 ELSE 0 END) AS suspended_count,\n\
                    MAX(c.buried_until) AS buried_until,\n\
                    MAX(CASE WHEN c.struggling_since IS NOT NULL THEN 1 ELSE 0 END) AS struggling,\n\
                    MAX(c.struggling_reason) AS struggling_reason,\n\
                    EXISTS(SELECT 1 FROM note_assets na WHERE na.note_id = n.id) AS has_images,\n\
                    EXISTS(SELECT 1 FROM cards cr WHERE cr.note_id = n.id AND cr.direction = 'reverse' AND cr.deleted_at IS NULL) AS has_reverse\n\
             FROM notes n\n\
             JOIN cards c ON c.note_id = n.id AND c.deleted_at IS NULL\n\
             LEFT JOIN card_schedules s ON s.card_id = c.id\n\
             WHERE n.deck_id = ?1 AND n.deleted_at IS NULL\n\
             GROUP BY n.id\n\
             LIMIT 10000",
        )
        .map_err(CommandError::database)?;
    let rows = statement
        .query_map([input.deck_id.as_str()], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, Option<String>>(3)?,
                row.get::<_, String>(4)?,
                row.get::<_, String>(5)?,
                row.get::<_, i64>(6)?,
                row.get::<_, i64>(7)?,
                row.get::<_, i64>(8)?,
                row.get::<_, i64>(9)?,
                row.get::<_, Option<i64>>(10)?,
                row.get::<_, Option<i64>>(11)?,
                row.get::<_, i64>(12)?,
                row.get::<_, Option<i64>>(13)?,
                row.get::<_, i64>(14)?,
                row.get::<_, Option<String>>(15)?,
                row.get::<_, i64>(16)?,
                row.get::<_, i64>(17)?,
            ))
        })
        .map_err(CommandError::database)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(CommandError::database)?;
    drop(statement);

    let requested_tags = validate_tags(input.tags)?
        .into_iter()
        .map(|tag| tag.normalized)
        .collect::<BTreeSet<_>>();
    let mut cards = Vec::new();
    for row in rows {
        if let Some(ids) = &candidate_ids {
            if !ids.contains(&row.0) {
                continue;
            }
        }
        let tags = load_tags(connection, &row.0)?;
        let normalized = tags
            .iter()
            .map(|tag| normalize_tag_name(tag))
            .collect::<BTreeSet<_>>();
        if !requested_tags.is_subset(&normalized) {
            continue;
        }
        let mode = if row.3.is_some() {
            "template"
        } else if row.2 == "cloze" {
            "fill_blank"
        } else if row.17 != 0 {
            "reverse"
        } else {
            "basic"
        };
        if input.mode != "all" && input.mode != mode {
            continue;
        }
        let suspended = row.9 > 0 && row.12 >= row.9;
        let buried = row.13.is_some_and(|until| until > now);
        let struggling = row.14 != 0;
        let status_matches = match input.status.as_str() {
            "all" => true,
            "active" => !suspended && !buried,
            "suspended" => suspended,
            "buried" => buried,
            "struggling" => struggling,
            "due" => row.10.is_some_and(|due| due <= now) && !suspended && !buried,
            "new" => row.11 == Some(0),
            _ => {
                return Err(CommandError::new(
                    "invalid_card_filter",
                    "Choose a supported card status filter.",
                ))
            }
        };
        if !status_matches {
            continue;
        }
        cards.push(BrowserCardDto {
            note_id: row.0,
            deck_id: row.1,
            mode: mode.to_string(),
            front_text: row.4,
            back_text: row.5,
            tags,
            generated_card_count: row.9,
            due_at: row.10,
            is_new: row.11 == Some(0),
            suspended,
            buried_until: row.13,
            struggling,
            struggling_reason: row.15,
            has_images: row.16 != 0,
            created_at: row.6,
            updated_at: row.7,
            revision: row.8,
        });
    }

    match input.sort.as_str() {
        "updated_desc" => cards.sort_by(|a, b| b.updated_at.cmp(&a.updated_at)),
        "created_asc" => cards.sort_by(|a, b| a.created_at.cmp(&b.created_at)),
        "due_asc" => cards.sort_by_key(|card| card.due_at.unwrap_or(i64::MAX)),
        "question_asc" => cards.sort_by(|a, b| {
            a.front_text
                .to_lowercase()
                .cmp(&b.front_text.to_lowercase())
        }),
        _ => {
            return Err(CommandError::new(
                "invalid_card_sort",
                "Choose a supported card sort order.",
            ))
        }
    }
    let total = cards.len();
    let limit = input.limit.clamp(1, 500);
    let cards = cards.into_iter().skip(input.offset).take(limit).collect();
    Ok(BrowseCardsResultDto { cards, total })
}

fn save_template_in_connection(
    connection: &mut Connection,
    input: SaveTemplateInput,
) -> Result<CardTemplateDto, CommandError> {
    let name = validate_template_name(&input.name)?;
    let description = validate_template_description(&input.description)?;
    let fields = validate_template_fields(input.fields)?;
    let cards = validate_template_cards(&fields, input.cards)?;
    let fields_json = serde_json::to_string(&fields).map_err(CommandError::serialization)?;
    let cards_json = serde_json::to_string(&cards).map_err(CommandError::serialization)?;
    let now = unix_time_millis()?;

    let id = if let Some(id) = input.template_id {
        validate_entity_id(&id, "card template")?;
        if id == PHARMACOLOGY_TEMPLATE_ID {
            return Err(CommandError::new(
                "builtin_template_read_only",
                "Built-in templates cannot be edited.",
            ));
        }
        let revision = input.revision.filter(|value| *value >= 1).ok_or_else(|| {
            CommandError::new("invalid_revision", "The template revision is invalid.")
        })?;
        let changed = connection
            .execute(
                "UPDATE card_templates SET name = ?1, description = ?2, fields_json = ?3,\n\
                     cards_json = ?4, updated_at = ?5, revision = revision + 1\n\
                 WHERE id = ?6 AND revision = ?7 AND deleted_at IS NULL AND is_builtin = 0",
                params![name, description, fields_json, cards_json, now, id, revision],
            )
            .map_err(map_unique_template_error)?;
        if changed != 1 {
            return Err(CommandError::new(
                "template_changed",
                "This template changed in another window. Reload it and try again.",
            ));
        }
        id
    } else {
        let id = generate_id(connection)?;
        connection
            .execute(
                "INSERT INTO card_templates (\n\
                     id, name, description, fields_json, cards_json, is_builtin,\n\
                     created_at, updated_at, revision\n\
                 ) VALUES (?1, ?2, ?3, ?4, ?5, 0, ?6, ?6, 1)",
                params![id, name, description, fields_json, cards_json, now],
            )
            .map_err(map_unique_template_error)?;
        id
    };
    load_template(connection, &id)
}

fn delete_template_in_connection(
    connection: &mut Connection,
    template_id: &str,
) -> Result<(), CommandError> {
    if template_id == PHARMACOLOGY_TEMPLATE_ID {
        return Err(CommandError::new(
            "builtin_template_read_only",
            "Built-in templates cannot be deleted.",
        ));
    }
    let in_use: i64 = connection
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM notes\n\
             WHERE template_id = ?1 AND deleted_at IS NULL)",
            [template_id],
            |row| row.get(0),
        )
        .map_err(CommandError::database)?;
    if in_use != 0 {
        return Err(CommandError::new(
            "template_in_use",
            "This template is still used by one or more cards.",
        ));
    }
    let changed = connection
        .execute(
            "UPDATE card_templates SET deleted_at = ?1, updated_at = ?1, revision = revision + 1\n\
             WHERE id = ?2 AND deleted_at IS NULL AND is_builtin = 0",
            params![unix_time_millis()?, template_id],
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

fn list_templates_from_connection(
    connection: &mut Connection,
) -> Result<Vec<CardTemplateDto>, CommandError> {
    let mut statement = connection
        .prepare(
            "SELECT id, name, description, fields_json, cards_json, is_builtin,\n\
                    created_at, updated_at, revision\n\
             FROM card_templates WHERE deleted_at IS NULL\n\
             ORDER BY is_builtin DESC, name COLLATE NOCASE ASC",
        )
        .map_err(CommandError::database)?;
    statement
        .query_map([], template_from_row)
        .map_err(CommandError::database)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(CommandError::database)
}

fn load_template(connection: &Connection, id: &str) -> Result<CardTemplateDto, CommandError> {
    connection
        .query_row(
            "SELECT id, name, description, fields_json, cards_json, is_builtin,\n\
                    created_at, updated_at, revision\n\
             FROM card_templates WHERE id = ?1 AND deleted_at IS NULL",
            [id],
            template_from_row,
        )
        .optional()
        .map_err(CommandError::database)?
        .ok_or_else(|| {
            CommandError::new(
                "template_not_found",
                "That card template no longer exists.",
            )
        })
}

fn template_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<CardTemplateDto> {
    let fields_json: String = row.get(3)?;
    let cards_json: String = row.get(4)?;
    let fields = serde_json::from_str(&fields_json).map_err(|error| {
        rusqlite::Error::FromSqlConversionFailure(
            3,
            rusqlite::types::Type::Text,
            Box::new(error),
        )
    })?;
    let cards = serde_json::from_str(&cards_json).map_err(|error| {
        rusqlite::Error::FromSqlConversionFailure(
            4,
            rusqlite::types::Type::Text,
            Box::new(error),
        )
    })?;
    Ok(CardTemplateDto {
        id: row.get(0)?,
        name: row.get(1)?,
        description: row.get(2)?,
        fields,
        cards,
        is_builtin: row.get::<_, i64>(5)? != 0,
        created_at: row.get(6)?,
        updated_at: row.get(7)?,
        revision: row.get(8)?,
    })
}

fn validate_template_name(value: &str) -> Result<String, CommandError> {
    let normalized = value.split_whitespace().collect::<Vec<_>>().join(" ");
    if normalized.is_empty()
        || normalized.chars().count() > 120
        || normalized.chars().any(|character| character.is_control())
    {
        return Err(CommandError::new(
            "invalid_template_name",
            "Template names must contain 1 to 120 ordinary characters.",
        ));
    }
    Ok(normalized)
}

fn validate_template_description(value: &str) -> Result<String, CommandError> {
    let normalized = value.trim().to_string();
    let has_unsupported_control = normalized.chars().any(|character| {
        character.is_control() && !matches!(character, '\n' | '\r' | '\t')
    });
    if normalized.chars().count() > 2_000 || has_unsupported_control {
        return Err(CommandError::new(
            "invalid_template_description",
            "Template descriptions may contain up to 2,000 ordinary characters.",
        ));
    }
    Ok(normalized)
}

fn validate_template_fields(values: Vec<String>) -> Result<Vec<String>, CommandError> {
    if values.is_empty() || values.len() > MAX_TEMPLATE_FIELDS {
        return Err(CommandError::new(
            "invalid_template_fields",
            "A template must contain between 1 and 64 fields.",
        ));
    }
    let mut seen = BTreeSet::new();
    let mut fields = Vec::with_capacity(values.len());
    for value in values {
        let field = normalize_field_name(&value)?;
        if !seen.insert(field.clone()) {
            return Err(CommandError::new(
                "duplicate_template_field",
                format!("The template field “{field}” is listed more than once."),
            ));
        }
        fields.push(field);
    }
    Ok(fields)
}

fn validate_template_cards(
    fields: &[String],
    cards: Vec<TemplateCardSpec>,
) -> Result<Vec<TemplateCardSpec>, CommandError> {
    if cards.is_empty() || cards.len() > MAX_TEMPLATE_CARDS {
        return Err(CommandError::new(
            "invalid_template_cards",
            "A template must generate between 1 and 64 cards.",
        ));
    }
    let allowed = fields.iter().cloned().collect::<BTreeSet<_>>();
    let mut names = BTreeSet::new();
    let mut validated = Vec::with_capacity(cards.len());
    for card in cards {
        let name = validate_template_name(&card.name)?;
        if !names.insert(name.to_lowercase()) {
            return Err(CommandError::new(
                "duplicate_template_card",
                format!("The generated-card name “{name}” is used more than once."),
            ));
        }
        validate_template_pattern(&card.front, &allowed)?;
        validate_template_pattern(&card.back, &allowed)?;
        validated.push(TemplateCardSpec {
            name,
            front: card.front,
            back: card.back,
        });
    }
    Ok(validated)
}

fn validate_template_pattern(
    pattern: &str,
    allowed_fields: &BTreeSet<String>,
) -> Result<(), CommandError> {
    if pattern.trim().is_empty() || pattern.chars().count() > MAX_TEMPLATE_PATTERN_CHARS {
        return Err(CommandError::new(
            "invalid_template_pattern",
            "Each template side must contain 1 to 20,000 characters.",
        ));
    }
    let fields = parse_pattern_fields(pattern)?;
    if fields.is_empty() {
        return Err(CommandError::new(
            "template_field_required",
            "Each generated card side must use at least one {{field name}} placeholder.",
        ));
    }
    for field in fields {
        if !allowed_fields.contains(&field) {
            return Err(CommandError::new(
                "unknown_template_field",
                format!("The template refers to an unknown field: {{{{{field}}}}}."),
            ));
        }
    }
    Ok(())
}

fn parse_pattern_fields(pattern: &str) -> Result<Vec<String>, CommandError> {
    let chars = pattern.chars().collect::<Vec<_>>();
    let mut fields = Vec::new();
    let mut index = 0;
    while index < chars.len() {
        if chars[index] == '{' {
            if index + 1 >= chars.len() || chars[index + 1] != '{' {
                return Err(invalid_template_syntax());
            }
            let start = index + 2;
            let mut end = start;
            while end + 1 < chars.len() && !(chars[end] == '}' && chars[end + 1] == '}') {
                if chars[end] == '{' || chars[end] == '}' {
                    return Err(invalid_template_syntax());
                }
                end += 1;
            }
            if end + 1 >= chars.len() {
                return Err(invalid_template_syntax());
            }
            let field = normalize_field_name(&chars[start..end].iter().collect::<String>())?;
            fields.push(field);
            index = end + 2;
        } else if chars[index] == '}' {
            return Err(invalid_template_syntax());
        } else {
            index += 1;
        }
    }
    Ok(fields)
}

fn render_pattern(
    pattern: &str,
    fields: &BTreeMap<String, String>,
) -> Result<String, CommandError> {
    let chars = pattern.chars().collect::<Vec<_>>();
    let mut output = String::new();
    let mut index = 0;
    while index < chars.len() {
        if chars[index] == '{' {
            if index + 1 >= chars.len() || chars[index + 1] != '{' {
                return Err(invalid_template_syntax());
            }
            let start = index + 2;
            let mut end = start;
            while end + 1 < chars.len() && !(chars[end] == '}' && chars[end + 1] == '}') {
                if chars[end] == '{' || chars[end] == '}' {
                    return Err(invalid_template_syntax());
                }
                end += 1;
            }
            if end + 1 >= chars.len() {
                return Err(invalid_template_syntax());
            }
            let field = normalize_field_name(&chars[start..end].iter().collect::<String>())?;
            let value = fields.get(&field).ok_or_else(|| {
                CommandError::new(
                    "missing_template_field",
                    format!("Enter a value for “{field}”."),
                )
            })?;
            output.push_str(value);
            index = end + 2;
        } else if chars[index] == '}' {
            return Err(invalid_template_syntax());
        } else {
            output.push(chars[index]);
            index += 1;
        }
    }
    Ok(output)
}

fn normalize_template_values(
    required_fields: &[String],
    values: &BTreeMap<String, String>,
) -> Result<BTreeMap<String, String>, CommandError> {
    let normalized_input = values
        .iter()
        .map(|(key, value)| Ok((normalize_field_name(key)?, value.clone())))
        .collect::<Result<BTreeMap<_, _>, CommandError>>()?;
    let mut output = BTreeMap::new();
    for field in required_fields {
        let value = normalized_input
            .get(field)
            .map(|value| value.trim().to_string())
            .unwrap_or_default();
        if value.is_empty() {
            return Err(CommandError::new(
                "missing_template_field",
                format!("Enter a value for “{field}”."),
            ));
        }
        if value.chars().count() > MAX_FIELD_VALUE_CHARS || value.contains('\0') {
            return Err(CommandError::new(
                "template_field_too_large",
                format!("The value for “{field}” is too large or invalid."),
            ));
        }
        output.insert(field.clone(), value);
    }
    Ok(output)
}

fn normalize_field_name(value: &str) -> Result<String, CommandError> {
    let normalized = value
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase();
    let valid = !normalized.is_empty()
        && normalized.chars().count() <= 80
        && normalized.chars().all(|character| {
            character.is_alphanumeric() || matches!(character, ' ' | '-' | '_' | '/' | '(' | ')')
        });
    if !valid {
        return Err(CommandError::new(
            "invalid_template_field",
            "Field names may use letters, numbers, spaces, hyphens, underscores, slashes, and parentheses.",
        ));
    }
    Ok(normalized)
}

fn invalid_template_syntax() -> CommandError {
    CommandError::new(
        "invalid_template_syntax",
        "Templates support literal text and simple {{field name}} placeholders only.",
    )
}

fn parse_fill_blanks(source: &str) -> Result<Vec<String>, CommandError> {
    if source.trim().is_empty() || source.chars().count() > MAX_TEMPLATE_PATTERN_CHARS {
        return Err(CommandError::new(
            "invalid_fill_blank",
            "Enter up to 20,000 characters and mark at least one answer with [[double brackets]].",
        ));
    }
    let chars = source.chars().collect::<Vec<_>>();
    let mut answers = Vec::new();
    let mut index = 0;
    while index < chars.len() {
        if chars[index] == '[' {
            if index + 1 >= chars.len() || chars[index + 1] != '[' {
                return Err(invalid_fill_syntax());
            }
            let start = index + 2;
            let mut end = start;
            while end + 1 < chars.len() && !(chars[end] == ']' && chars[end + 1] == ']') {
                if chars[end] == '[' || chars[end] == ']' {
                    return Err(invalid_fill_syntax());
                }
                end += 1;
            }
            if end + 1 >= chars.len() {
                return Err(invalid_fill_syntax());
            }
            let answer = chars[start..end].iter().collect::<String>().trim().to_string();
            if answer.is_empty() || answer.chars().count() > 2_000 {
                return Err(CommandError::new(
                    "invalid_fill_answer",
                    "Each fill-in-the-blank answer must contain 1 to 2,000 characters.",
                ));
            }
            answers.push(answer);
            index = end + 2;
        } else if chars[index] == ']' {
            return Err(invalid_fill_syntax());
        } else {
            index += 1;
        }
    }
    if answers.is_empty() || answers.len() > 100 {
        return Err(CommandError::new(
            "invalid_fill_blank",
            "Mark between 1 and 100 answers with [[double brackets]].",
        ));
    }
    Ok(answers)
}

fn render_fill_question(source: &str, target_index: usize) -> Result<String, CommandError> {
    let chars = source.chars().collect::<Vec<_>>();
    let mut output = String::new();
    let mut index = 0;
    let mut blank_index = 0;
    while index < chars.len() {
        if chars[index] == '[' && index + 1 < chars.len() && chars[index + 1] == '[' {
            let start = index + 2;
            let mut end = start;
            while end + 1 < chars.len() && !(chars[end] == ']' && chars[end + 1] == ']') {
                end += 1;
            }
            if end + 1 >= chars.len() {
                return Err(invalid_fill_syntax());
            }
            if blank_index == target_index {
                output.push_str("_____");
            } else {
                output.extend(chars[start..end].iter());
            }
            blank_index += 1;
            index = end + 2;
        } else {
            output.push(chars[index]);
            index += 1;
        }
    }
    Ok(output)
}

fn invalid_fill_syntax() -> CommandError {
    CommandError::new(
        "invalid_fill_syntax",
        "Use [[double brackets]] around each answer. Nested or unmatched brackets are not supported.",
    )
}

fn plain_document(text: &str) -> Value {
    let mut content = Vec::new();
    for line in text.split('\n') {
        if line.is_empty() {
            content.push(json!({"type": "paragraph"}));
        } else {
            content.push(json!({
                "type": "paragraph",
                "content": [{"type": "text", "text": line}]
            }));
        }
    }
    if content.is_empty() {
        content.push(json!({"type": "paragraph"}));
    }
    json!({"type": "doc", "content": content})
}

fn parse_fields_json(value: &str) -> Result<BTreeMap<String, String>, CommandError> {
    serde_json::from_str(value).map_err(CommandError::serialization)
}

#[derive(Clone)]
struct ValidatedTag {
    name: String,
    normalized: String,
}

fn validate_tags(values: Vec<String>) -> Result<Vec<ValidatedTag>, CommandError> {
    if values.len() > MAX_TAGS_PER_NOTE {
        return Err(CommandError::new(
            "too_many_tags",
            "A card can have at most 100 tags.",
        ));
    }
    let mut seen = BTreeSet::new();
    let mut tags = Vec::new();
    for value in values {
        let name = value.split_whitespace().collect::<Vec<_>>().join(" ");
        let normalized = normalize_tag_name(&name);
        if normalized.is_empty()
            || normalized.chars().count() > 80
            || name.chars().any(|character| character.is_control())
        {
            return Err(CommandError::new(
                "invalid_tag",
                "Tags must contain 1 to 80 ordinary characters.",
            ));
        }
        if seen.insert(normalized.clone()) {
            tags.push(ValidatedTag { name, normalized });
        }
    }
    Ok(tags)
}

fn normalize_tag_name(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ").to_lowercase()
}

fn replace_note_tags(
    connection: &Connection,
    note_id: &str,
    tags: &[ValidatedTag],
    now: i64,
) -> Result<(), CommandError> {
    connection
        .execute("DELETE FROM note_tags WHERE note_id = ?1", [note_id])
        .map_err(CommandError::database)?;
    for tag in tags {
        let existing: Option<String> = connection
            .query_row(
                "SELECT id FROM tags WHERE normalized_name = ?1",
                [tag.normalized.as_str()],
                |row| row.get(0),
            )
            .optional()
            .map_err(CommandError::database)?;
        let tag_id = if let Some(id) = existing {
            connection
                .execute(
                    "UPDATE tags SET name = ?1, updated_at = ?2 WHERE id = ?3",
                    params![tag.name, now, id],
                )
                .map_err(CommandError::database)?;
            id
        } else {
            let id = generate_id(connection)?;
            connection
                .execute(
                    "INSERT INTO tags (id, name, normalized_name, created_at, updated_at)\n\
                     VALUES (?1, ?2, ?3, ?4, ?4)",
                    params![id, tag.name, tag.normalized, now],
                )
                .map_err(CommandError::database)?;
            id
        };
        connection
            .execute(
                "INSERT INTO note_tags (note_id, tag_id) VALUES (?1, ?2)",
                params![note_id, tag_id],
            )
            .map_err(CommandError::database)?;
    }
    Ok(())
}

fn load_tags(connection: &Connection, note_id: &str) -> Result<Vec<String>, CommandError> {
    let mut statement = connection
        .prepare(
            "SELECT t.name FROM tags t\n\
             JOIN note_tags nt ON nt.tag_id = t.id\n\
             WHERE nt.note_id = ?1\n\
             ORDER BY t.name COLLATE NOCASE",
        )
        .map_err(CommandError::database)?;
    statement
        .query_map([note_id], |row| row.get(0))
        .map_err(CommandError::database)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(CommandError::database)
}

fn refresh_note_fts(connection: &Connection, note_id: &str) -> Result<(), CommandError> {
    connection
        .execute("DELETE FROM notes_fts WHERE note_id = ?1", [note_id])
        .map_err(CommandError::database)?;
    connection
        .execute(
            "INSERT INTO notes_fts(note_id, deck_id, front_text, back_text, tags, fields)\n\
             SELECT n.id, n.deck_id, n.front_text, n.back_text,\n\
                    COALESCE((SELECT group_concat(t.name, ' ')\n\
                              FROM tags t JOIN note_tags nt ON nt.tag_id = t.id\n\
                              WHERE nt.note_id = n.id), ''),\n\
                    n.fields_json\n\
             FROM notes n WHERE n.id = ?1 AND n.deleted_at IS NULL",
            [note_id],
        )
        .map_err(CommandError::database)?;
    Ok(())
}

fn build_fts_query(value: &str) -> Result<String, CommandError> {
    let mut terms = Vec::new();
    let mut current = String::new();
    for character in value.chars() {
        if character.is_alphanumeric() || matches!(character, '_' | '-') {
            current.push(character);
        } else if !current.is_empty() {
            terms.push(std::mem::take(&mut current));
        }
    }
    if !current.is_empty() {
        terms.push(current);
    }
    terms.retain(|term| !term.trim().is_empty());
    if terms.is_empty() {
        return Err(CommandError::new(
            "invalid_search",
            "Enter at least one letter or number to search.",
        ));
    }
    if terms.len() > 30 {
        terms.truncate(30);
    }
    Ok(terms
        .into_iter()
        .map(|term| format!("\"{}\"*", term.replace('"', "\"\"")))
        .collect::<Vec<_>>()
        .join(" AND "))
}

fn duplicate_key(mode: &str, front: &str, back: &str, fields_json: &str) -> String {
    let canonical = format!(
        "{}\u{1f}{}\u{1f}{}\u{1f}{}",
        normalize_duplicate_text(mode),
        normalize_duplicate_text(front),
        normalize_duplicate_text(back),
        normalize_duplicate_text(fields_json),
    );
    let first = fnv1a64(canonical.as_bytes(), 0xcbf29ce484222325);
    let second = fnv1a64(canonical.as_bytes(), 0x84222325cbf29ce4);
    format!("{first:016x}{second:016x}")
}

fn normalize_duplicate_text(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ").to_lowercase()
}

fn fnv1a64(bytes: &[u8], seed: u64) -> u64 {
    let mut hash = seed;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

fn load_duplicates(
    connection: &Connection,
    deck_id: &str,
    key: &str,
    excluded_note_id: Option<&str>,
) -> Result<Vec<DuplicateNoteDto>, CommandError> {
    let mut statement = connection
        .prepare(
            "SELECT n.id, n.deck_id, d.name, n.front_text, n.back_text\n\
             FROM notes n JOIN decks d ON d.id = n.deck_id\n\
             WHERE n.deck_id = ?1 AND n.duplicate_key = ?2\n\
               AND n.deleted_at IS NULL AND d.deleted_at IS NULL\n\
               AND (?3 IS NULL OR n.id <> ?3)\n\
             ORDER BY n.updated_at DESC LIMIT 20",
        )
        .map_err(CommandError::database)?;
    let mut duplicates = statement
        .query_map(params![deck_id, key, excluded_note_id], |row| {
            Ok(DuplicateNoteDto {
                note_id: row.get(0)?,
                deck_id: row.get(1)?,
                deck_name: row.get(2)?,
                front_text: row.get(3)?,
                back_text: row.get(4)?,
            })
        })
        .map_err(CommandError::database)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(CommandError::database)?;
    drop(statement);

    // Schema 4 cannot calculate the application-owned duplicate hash in SQL.
    // Lazily backfill notes created by earlier releases while checking them so
    // upgraded libraries participate in duplicate detection immediately.
    let mut legacy_statement = connection
        .prepare(
            "SELECT n.id, n.deck_id, d.name, n.front_text, n.back_text, n.note_type,\n\
                    n.template_id, n.fields_json,\n\
                    EXISTS(SELECT 1 FROM cards c WHERE c.note_id = n.id\n\
                           AND c.direction = 'reverse' AND c.deleted_at IS NULL)\n\
             FROM notes n JOIN decks d ON d.id = n.deck_id\n\
             WHERE n.deck_id = ?1 AND n.duplicate_key = ''\n\
               AND n.deleted_at IS NULL AND d.deleted_at IS NULL\n\
               AND (?2 IS NULL OR n.id <> ?2)\n\
             ORDER BY n.updated_at DESC LIMIT 5000",
        )
        .map_err(CommandError::database)?;
    let legacy = legacy_statement
        .query_map(params![deck_id, excluded_note_id], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, String>(4)?,
                row.get::<_, String>(5)?,
                row.get::<_, Option<String>>(6)?,
                row.get::<_, String>(7)?,
                row.get::<_, i64>(8)?,
            ))
        })
        .map_err(CommandError::database)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(CommandError::database)?;
    drop(legacy_statement);
    for row in legacy {
        let mode = if row.6.is_some() {
            "template"
        } else if row.5 == "cloze" {
            "fill_blank"
        } else if row.8 != 0 {
            "reverse"
        } else if row.5 == "image_occlusion" {
            "image_occlusion"
        } else {
            "basic"
        };
        let candidate = duplicate_key(mode, &row.3, &row.4, &row.7);
        connection
            .execute(
                "UPDATE notes SET duplicate_key = ?1 WHERE id = ?2 AND duplicate_key = ''",
                params![candidate, row.0],
            )
            .map_err(CommandError::database)?;
        if candidate == key {
            duplicates.push(DuplicateNoteDto {
                note_id: row.0,
                deck_id: row.1,
                deck_name: row.2,
                front_text: row.3,
                back_text: row.4,
            });
        }
    }
    duplicates.truncate(20);
    Ok(duplicates)
}

fn replace_note_assets(
    connection: &Connection,
    note_id: &str,
    content: &ValidatedContent,
) -> Result<(), CommandError> {
    connection
        .execute("DELETE FROM note_assets WHERE note_id = ?1", [note_id])
        .map_err(CommandError::database)?;
    for asset_id in &content.front_asset_ids {
        connection
            .execute(
                "INSERT INTO note_assets(note_id, asset_id, side) VALUES (?1, ?2, 'front')",
                params![note_id, asset_id],
            )
            .map_err(CommandError::database)?;
    }
    for asset_id in &content.back_asset_ids {
        connection
            .execute(
                "INSERT INTO note_assets(note_id, asset_id, side) VALUES (?1, ?2, 'back')",
                params![note_id, asset_id],
            )
            .map_err(CommandError::database)?;
    }
    Ok(())
}

fn all_asset_ids(content: &ValidatedContent) -> Vec<String> {
    content
        .front_asset_ids
        .iter()
        .chain(&content.back_asset_ids)
        .cloned()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

fn ensure_active_deck(connection: &Connection, deck_id: &str) -> Result<(), CommandError> {
    let exists: i64 = connection
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM decks\n\
             WHERE id = ?1 AND deleted_at IS NULL AND archived_at IS NULL)",
            [deck_id],
            |row| row.get(0),
        )
        .map_err(CommandError::database)?;
    if exists == 0 {
        return Err(CommandError::new(
            "deck_not_found",
            "The selected deck no longer exists.",
        ));
    }
    Ok(())
}

fn set_suspended_in_connection(
    connection: &mut Connection,
    note_id: &str,
    suspended: bool,
) -> Result<(), CommandError> {
    let now = unix_time_millis()?;
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(CommandError::database)?;
    let changed = transaction
        .execute(
            "UPDATE cards SET suspended_at = ?1, updated_at = ?2\n\
             WHERE note_id = ?3 AND deleted_at IS NULL",
            params![if suspended { Some(now) } else { None }, now, note_id],
        )
        .map_err(CommandError::database)?;
    if changed == 0 {
        return Err(CommandError::new(
            "card_not_found",
            "That flashcard no longer exists.",
        ));
    }
    record_state_events(
        &transaction,
        note_id,
        if suspended { "suspend" } else { "resume" },
        json!({}),
        now,
    )?;
    transaction.commit().map_err(CommandError::database)
}

fn bury_note_in_connection(
    connection: &mut Connection,
    note_id: &str,
    buried_until: Option<i64>,
) -> Result<(), CommandError> {
    let now = unix_time_millis()?;
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(CommandError::database)?;
    let changed = transaction
        .execute(
            "UPDATE cards SET buried_until = ?1, updated_at = ?2\n\
             WHERE note_id = ?3 AND deleted_at IS NULL",
            params![buried_until, now, note_id],
        )
        .map_err(CommandError::database)?;
    if changed == 0 {
        return Err(CommandError::new(
            "card_not_found",
            "That flashcard no longer exists.",
        ));
    }
    record_state_events(
        &transaction,
        note_id,
        if buried_until.is_some() { "bury" } else { "unbury" },
        json!({"buriedUntil": buried_until}),
        now,
    )?;
    transaction.commit().map_err(CommandError::database)
}

fn reset_note_in_connection(
    connection: &mut Connection,
    note_id: &str,
) -> Result<(), CommandError> {
    let now = unix_time_millis()?;
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(CommandError::database)?;
    let card_ids = active_card_ids(&transaction, note_id)?;
    if card_ids.is_empty() {
        return Err(CommandError::new(
            "card_not_found",
            "That flashcard no longer exists.",
        ));
    }
    for card_id in &card_ids {
        let schedule = new_schedule(card_id.clone(), now);
        update_schedule(&transaction, &schedule)?;
        transaction
            .execute(
                "UPDATE cards SET buried_until = NULL, struggling_since = NULL,\n\
                     struggling_reason = NULL, updated_at = ?1 WHERE id = ?2",
                params![now, card_id],
            )
            .map_err(CommandError::database)?;
        record_card_state_event(&transaction, card_id, "reset", json!({}), now)?;
    }
    transaction.commit().map_err(CommandError::database)
}

fn reschedule_note_in_connection(
    connection: &mut Connection,
    note_id: &str,
    due_at: i64,
) -> Result<(), CommandError> {
    let now = unix_time_millis()?;
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(CommandError::database)?;
    let card_ids = active_card_ids(&transaction, note_id)?;
    if card_ids.is_empty() {
        return Err(CommandError::new(
            "card_not_found",
            "That flashcard no longer exists.",
        ));
    }
    for card_id in &card_ids {
        transaction
            .execute(
                "UPDATE card_schedules SET due_at = ?1, updated_at = ?2 WHERE card_id = ?3",
                params![due_at, now, card_id],
            )
            .map_err(CommandError::database)?;
        record_card_state_event(
            &transaction,
            card_id,
            "reschedule",
            json!({"dueAt": due_at}),
            now,
        )?;
    }
    transaction.commit().map_err(CommandError::database)
}

fn move_or_copy_in_connection(
    connection: &mut Connection,
    input: MoveCopyNoteInput,
) -> Result<MoveCopyResultDto, CommandError> {
    ensure_active_deck(connection, &input.destination_deck_id)?;
    if !matches!(input.operation.as_str(), "move" | "copy") {
        return Err(CommandError::new(
            "invalid_move_operation",
            "Choose Move or Copy.",
        ));
    }
    let source_deck: String = connection
        .query_row(
            "SELECT deck_id FROM notes WHERE id = ?1 AND deleted_at IS NULL",
            [input.note_id.as_str()],
            |row| row.get(0),
        )
        .optional()
        .map_err(CommandError::database)?
        .ok_or_else(|| {
            CommandError::new("card_not_found", "That flashcard no longer exists.")
        })?;
    if source_deck == input.destination_deck_id {
        return Err(CommandError::new(
            "same_deck",
            "Choose a different destination deck.",
        ));
    }

    let now = unix_time_millis()?;
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(CommandError::database)?;
    if input.operation == "move" {
        transaction
            .execute(
                "UPDATE notes SET deck_id = ?1, updated_at = ?2, revision = revision + 1\n\
                 WHERE id = ?3 AND deleted_at IS NULL",
                params![input.destination_deck_id, now, input.note_id],
            )
            .map_err(CommandError::database)?;
        transaction
            .execute(
                "UPDATE cards SET deck_id = ?1, updated_at = ?2\n\
                 WHERE note_id = ?3 AND deleted_at IS NULL",
                params![input.destination_deck_id, now, input.note_id],
            )
            .map_err(CommandError::database)?;
        refresh_note_fts(&transaction, &input.note_id)?;
        record_state_events(
            &transaction,
            &input.note_id,
            "move",
            json!({"fromDeckId": source_deck, "toDeckId": input.destination_deck_id}),
            now,
        )?;
        transaction.commit().map_err(CommandError::database)?;
        return Ok(MoveCopyResultDto {
            note_id: input.note_id,
            destination_deck_id: input.destination_deck_id,
            copied: false,
        });
    }

    let note: (
        String,
        String,
        i64,
        String,
        String,
        String,
        String,
        i64,
        i64,
        Option<String>,
        String,
        String,
    ) = transaction
        .query_row(
            "SELECT note_type, content_format, content_schema_version, front_doc, back_doc,\n\
                    front_text, back_text, created_at, revision, template_id, fields_json, duplicate_key\n\
             FROM notes WHERE id = ?1 AND deleted_at IS NULL",
            [input.note_id.as_str()],
            |row| {
                Ok((
                    row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?, row.get(5)?,
                    row.get(6)?, row.get(7)?, row.get(8)?, row.get(9)?, row.get(10)?, row.get(11)?,
                ))
            },
        )
        .map_err(CommandError::database)?;
    let new_note_id = generate_id(&transaction)?;
    transaction
        .execute(
            "INSERT INTO notes (\n\
                 id, deck_id, note_type, content_format, content_schema_version,\n\
                 front_doc, back_doc, front_text, back_text, created_at, updated_at, revision,\n\
                 template_id, fields_json, duplicate_key\n\
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, 1, ?12, ?13, ?14)",
            params![
                new_note_id,
                input.destination_deck_id,
                note.0,
                note.1,
                note.2,
                note.3,
                note.4,
                note.5,
                note.6,
                now,
                now,
                note.9,
                note.10,
                note.11,
            ],
        )
        .map_err(CommandError::database)?;
    transaction
        .execute(
            "INSERT INTO note_assets(note_id, asset_id, side)\n\
             SELECT ?1, asset_id, side FROM note_assets WHERE note_id = ?2",
            params![new_note_id, input.note_id],
        )
        .map_err(CommandError::database)?;
    transaction
        .execute(
            "INSERT INTO note_tags(note_id, tag_id)\n\
             SELECT ?1, tag_id FROM note_tags WHERE note_id = ?2",
            params![new_note_id, input.note_id],
        )
        .map_err(CommandError::database)?;

    let mut statement = transaction
        .prepare(
            "SELECT id, direction, template_ordinal, display_name\n\
             FROM cards WHERE note_id = ?1 AND deleted_at IS NULL\n\
             ORDER BY template_ordinal, direction",
        )
        .map_err(CommandError::database)?;
    let source_cards = statement
        .query_map([input.note_id.as_str()], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, i64>(2)?,
                row.get::<_, String>(3)?,
            ))
        })
        .map_err(CommandError::database)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(CommandError::database)?;
    drop(statement);
    for source in source_cards {
        let new_card_id = generate_id(&transaction)?;
        transaction
            .execute(
                "INSERT INTO cards (\n\
                     id, note_id, deck_id, direction, template_ordinal, created_at, updated_at,\n\
                     source_card_id, display_name\n\
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?6, ?7, ?8)",
                params![
                    new_card_id,
                    new_note_id,
                    input.destination_deck_id,
                    source.1,
                    source.2,
                    now,
                    source.0,
                    source.3,
                ],
            )
            .map_err(CommandError::database)?;
        insert_new_schedule(&transaction, &new_card_id, now)?;
        record_card_state_event(
            &transaction,
            &new_card_id,
            "copy",
            json!({"sourceCardId": source.0, "sourceNoteId": input.note_id}),
            now,
        )?;
    }
    refresh_note_fts(&transaction, &new_note_id)?;
    transaction.commit().map_err(CommandError::database)?;
    Ok(MoveCopyResultDto {
        note_id: new_note_id,
        destination_deck_id: input.destination_deck_id,
        copied: true,
    })
}

fn bulk_create_in_connection(
    connection: &mut Connection,
    input: BulkCreateInput,
) -> Result<BulkCreateResultDto, CommandError> {
    ensure_active_deck(connection, &input.deck_id)?;
    if input.entries.is_empty() || input.entries.len() > MAX_BULK_ENTRIES {
        return Err(CommandError::new(
            "invalid_bulk_entry_count",
            "Bulk entry supports between 1 and 500 cards at a time.",
        ));
    }
    let tags = validate_tags(input.tags)?;
    let mut prepared = Vec::with_capacity(input.entries.len());
    for row in input.entries {
        let question = row.question.trim().to_string();
        let answer = row.answer.trim().to_string();
        let content = validate_content(
            plain_document(&question),
            plain_document(&answer),
            question.clone(),
            answer.clone(),
        )?;
        let mut generated = vec![GeneratedCardSpec {
            direction: "forward",
            ordinal: 0,
            display_name: "Forward".to_string(),
        }];
        if input.create_reverse {
            generated.push(GeneratedCardSpec {
                direction: "reverse",
                ordinal: 0,
                display_name: "Reverse".to_string(),
            });
        }
        prepared.push((
            question,
            PreparedNote {
                note_type: "basic",
                template_id: None,
                fields_json: "{}".to_string(),
                duplicate_key: duplicate_key(
                    if input.create_reverse { "reverse" } else { "basic" },
                    &content.front_text,
                    &content.back_text,
                    "{}",
                ),
                content,
                generated,
            },
        ));
    }

    let now = unix_time_millis()?;
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(CommandError::database)?;
    let mut created_count = 0;
    let mut skipped_duplicates = Vec::new();
    for (question, note) in prepared {
        let duplicate = !load_duplicates(
            &transaction,
            &input.deck_id,
            &note.duplicate_key,
            None,
        )?
        .is_empty();
        if duplicate && input.skip_duplicates {
            skipped_duplicates.push(question);
            continue;
        }
        let note_id = generate_id(&transaction)?;
        transaction
            .execute(
                "INSERT INTO notes (\n\
                     id, deck_id, note_type, content_format, content_schema_version,\n\
                     front_doc, back_doc, front_text, back_text, created_at, updated_at, revision,\n\
                     fields_json, duplicate_key\n\
                 ) VALUES (?1, ?2, 'basic', 'tiptap-json', 1, ?3, ?4, ?5, ?6, ?7, ?7, 1, '{}', ?8)",
                params![
                    note_id,
                    input.deck_id,
                    note.content.front_doc_json,
                    note.content.back_doc_json,
                    note.content.front_text,
                    note.content.back_text,
                    now,
                    note.duplicate_key,
                ],
            )
            .map_err(CommandError::database)?;
        synchronize_generated_cards(&transaction, &note_id, &input.deck_id, &note.generated, now)?;
        replace_note_tags(&transaction, &note_id, &tags, now)?;
        refresh_note_fts(&transaction, &note_id)?;
        created_count += 1;
    }
    transaction.commit().map_err(CommandError::database)?;
    Ok(BulkCreateResultDto {
        created_count,
        skipped_duplicates,
    })
}

fn save_draft_in_connection(
    connection: &mut Connection,
    input: SaveDraftInput,
) -> Result<EditorDraftDto, CommandError> {
    ensure_active_deck(connection, &input.deck_id)?;
    let payload_json = serde_json::to_string(&input.payload).map_err(CommandError::serialization)?;
    if payload_json.len() > MAX_DRAFT_BYTES {
        return Err(CommandError::new(
            "draft_too_large",
            "This editor draft is too large to autosave.",
        ));
    }
    let now = unix_time_millis()?;
    connection
        .execute(
            "INSERT INTO editor_drafts(draft_key, deck_id, note_id, payload_json, updated_at)\n\
             VALUES (?1, ?2, ?3, ?4, ?5)\n\
             ON CONFLICT(draft_key) DO UPDATE SET\n\
                 deck_id = excluded.deck_id, note_id = excluded.note_id,\n\
                 payload_json = excluded.payload_json, updated_at = excluded.updated_at",
            params![
                input.draft_key,
                input.deck_id,
                input.note_id,
                payload_json,
                now,
            ],
        )
        .map_err(CommandError::database)?;
    Ok(EditorDraftDto {
        draft_key: input.draft_key,
        deck_id: input.deck_id,
        note_id: input.note_id,
        payload: input.payload,
        updated_at: now,
    })
}

fn get_draft_from_connection(
    connection: &Connection,
    draft_key: &str,
) -> Result<Option<EditorDraftDto>, CommandError> {
    let row: Option<(String, String, Option<String>, String, i64)> = connection
        .query_row(
            "SELECT draft_key, deck_id, note_id, payload_json, updated_at\n\
             FROM editor_drafts WHERE draft_key = ?1",
            [draft_key],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?)),
        )
        .optional()
        .map_err(CommandError::database)?;
    row.map(|(draft_key, deck_id, note_id, payload_json, updated_at)| {
        Ok(EditorDraftDto {
            draft_key,
            deck_id,
            note_id,
            payload: serde_json::from_str(&payload_json).map_err(CommandError::serialization)?,
            updated_at,
        })
    })
    .transpose()
}

fn validate_draft_key(value: &str) -> Result<(), CommandError> {
    if value.is_empty()
        || value.len() > 200
        || value.chars().any(|character| character.is_control())
    {
        return Err(CommandError::new(
            "invalid_draft_key",
            "The editor draft identifier is invalid.",
        ));
    }
    Ok(())
}

fn active_card_ids(connection: &Connection, note_id: &str) -> Result<Vec<String>, CommandError> {
    let mut statement = connection
        .prepare("SELECT id FROM cards WHERE note_id = ?1 AND deleted_at IS NULL")
        .map_err(CommandError::database)?;
    statement
        .query_map([note_id], |row| row.get(0))
        .map_err(CommandError::database)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(CommandError::database)
}

fn record_state_events(
    connection: &Connection,
    note_id: &str,
    action: &str,
    details: Value,
    now: i64,
) -> Result<(), CommandError> {
    for card_id in active_card_ids(connection, note_id)? {
        record_card_state_event(connection, &card_id, action, details.clone(), now)?;
    }
    Ok(())
}

fn record_card_state_event(
    connection: &Connection,
    card_id: &str,
    action: &str,
    details: Value,
    now: i64,
) -> Result<(), CommandError> {
    let id = generate_id(connection)?;
    let details_json = serde_json::to_string(&details).map_err(CommandError::serialization)?;
    connection
        .execute(
            "INSERT INTO card_state_events(id, card_id, action, details_json, created_at)\n\
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![id, card_id, action, details_json, now],
        )
        .map_err(CommandError::database)?;
    Ok(())
}

fn validate_timestamp(value: i64, label: &str) -> Result<(), CommandError> {
    const MAX_TIMESTAMP: i64 = 253_402_300_799_999; // 9999-12-31T23:59:59.999Z
    if !(0..=MAX_TIMESTAMP).contains(&value) {
        return Err(CommandError::new(
            "invalid_timestamp",
            format!("The selected {label} is outside the supported date range."),
        ));
    }
    Ok(())
}

fn map_unique_template_error(error: rusqlite::Error) -> CommandError {
    if error.to_string().contains("UNIQUE constraint failed") {
        CommandError::new(
            "duplicate_template_name",
            "A template with that name already exists.",
        )
    } else {
        CommandError::database(error)
    }
}

/// Validates display metadata before an imported template enters the library.
pub(crate) fn validate_imported_template_metadata(
    name: &str,
    description: &str,
) -> Result<(String, String), CommandError> {
    Ok((
        validate_template_name(name)?,
        validate_template_description(description)?,
    ))
}

/// Shared validation boundary for versioned SQLite deck packages.
pub(crate) fn validate_imported_template(
    fields: Vec<String>,
    cards: Vec<TemplateCardSpec>,
) -> Result<(Vec<String>, Vec<TemplateCardSpec>), CommandError> {
    let fields = validate_template_fields(fields)?;
    let cards = validate_template_cards(&fields, cards)?;
    Ok((fields, cards))
}

/// Normalizes and validates imported field values against a template.
pub(crate) fn validate_imported_template_values(
    required_fields: &[String],
    values: &BTreeMap<String, String>,
) -> Result<BTreeMap<String, String>, CommandError> {
    normalize_template_values(required_fields, values)
}

/// Returns the validated literal answers from imported fill-in-the-blank text.
pub(crate) fn validate_imported_fill_blank(source: &str) -> Result<Vec<String>, CommandError> {
    parse_fill_blanks(source)
}

/// Renders one already-validated template card using inert text substitution.
pub(crate) fn render_imported_template_card(
    card: &TemplateCardSpec,
    fields: &BTreeMap<String, String>,
) -> Result<(String, String), CommandError> {
    Ok((
        render_pattern(&card.front, fields)?,
        render_pattern(&card.back, fields)?,
    ))
}

pub(crate) fn compute_duplicate_key(
    mode: &str,
    front: &str,
    back: &str,
    fields_json: &str,
) -> String {
    duplicate_key(mode, front, back, fields_json)
}

pub(crate) fn refresh_note_search(
    connection: &Connection,
    note_id: &str,
) -> Result<(), CommandError> {
    refresh_note_fts(connection, note_id)
}

fn default_all() -> String {
    "all".to_string()
}

fn default_updated_desc() -> String {
    "updated_desc".to_string()
}

const fn default_page_limit() -> usize {
    100
}

const fn default_true() -> bool {
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::database::Database;

    #[test]
    fn template_language_is_data_only() {
        let fields = BTreeMap::from([("medication name".to_string(), "warfarin".to_string())]);
        assert_eq!(
            render_pattern("What is {{medication name}}?", &fields).unwrap(),
            "What is warfarin?"
        );
        for unsafe_pattern in [
            "{{medication name.constructor}}",
            "{{{medication name}}}",
            "{{}}",
            "{{medication name",
            "javascript:{{medication name}}}",
        ] {
            assert!(render_pattern(unsafe_pattern, &fields).is_err());
        }
    }

    #[test]
    fn fill_blank_parser_generates_one_question_per_marker() {
        let source = "[[Warfarin]] is a [[vitamin K antagonist]].";
        assert_eq!(parse_fill_blanks(source).unwrap().len(), 2);
        assert_eq!(
            render_fill_question(source, 0).unwrap(),
            "_____ is a vitamin K antagonist."
        );
        assert_eq!(
            render_fill_question(source, 1).unwrap(),
            "Warfarin is a _____."
        );
    }

    #[test]
    fn fts_query_quotes_tokens_instead_of_accepting_operators() {
        assert_eq!(
            build_fts_query("warfarin OR javascript:alert(1)").unwrap(),
            "\"warfarin\"* AND \"OR\"* AND \"javascript\"* AND \"alert\"* AND \"1\"*"
        );
    }

    #[test]
    fn migration_seeds_the_pharmacology_template() {
        let database = Database::open_in_memory().expect("database should open");
        let templates = database
            .with_connection(list_templates_from_connection)
            .expect("templates should load");
        assert!(templates.iter().any(|template| {
            template.id == PHARMACOLOGY_TEMPLATE_ID && template.cards.len() == 4
        }));
    }

    #[test]
    fn duplicate_keys_are_stable_and_whitespace_insensitive() {
        assert_eq!(
            duplicate_key("basic", "What is ATP?", "Energy", "{}"),
            duplicate_key(" BASIC ", "  what  is ATP? ", "energy", "{}")
        );
    }
    #[test]
    fn advanced_note_modes_generate_expected_cards_and_search_rows() {
        let database = Database::open_in_memory().expect("database should open");
        let root = temporary_test_directory("advanced-modes");
        let paths = AppPaths::new(root.clone()).expect("test paths should initialize");

        database
            .with_connection(|connection| {
                let deck = crate::decks::create_deck_in_connection(
                    connection,
                    crate::decks::CreateDeckInput {
                        name: "Pharmacology".to_string(),
                        description: None,
                    },
                )?;

                let reverse = save_note_in_connection(
                    connection,
                    &paths,
                    basic_note_input(
                        &deck.id,
                        "reverse",
                        "Warfarin class?",
                        "Vitamin K antagonist",
                        vec!["anticoagulants"],
                    ),
                )?;
                assert_eq!(reverse.generated_card_count, 2);
                assert_eq!(reverse.mode, "reverse");

                let fill = save_note_in_connection(
                    connection,
                    &paths,
                    SaveNoteInput {
                        note_id: None,
                        revision: None,
                        deck_id: deck.id.clone(),
                        mode: "fill_blank".to_string(),
                        front_doc: plain_document(""),
                        back_doc: plain_document(""),
                        front_text: String::new(),
                        back_text: String::new(),
                        tags: vec!["anticoagulants".to_string()],
                        create_reverse: false,
                        fill_blank_text: Some(
                            "[[Warfarin]] inhibits [[vitamin K epoxide reductase]].".to_string(),
                        ),
                        template_id: None,
                        fields: BTreeMap::new(),
                        allow_duplicate: false,
                    },
                )?;
                assert_eq!(fill.generated_card_count, 2);
                assert_eq!(fill.mode, "fill_blank");

                let template = save_note_in_connection(
                    connection,
                    &paths,
                    SaveNoteInput {
                        note_id: None,
                        revision: None,
                        deck_id: deck.id.clone(),
                        mode: "template".to_string(),
                        front_doc: plain_document(""),
                        back_doc: plain_document(""),
                        front_text: String::new(),
                        back_text: String::new(),
                        tags: vec!["anticoagulants".to_string(), "high alert".to_string()],
                        create_reverse: false,
                        fill_blank_text: None,
                        template_id: Some(PHARMACOLOGY_TEMPLATE_ID.to_string()),
                        fields: BTreeMap::from([
                            ("medication name".to_string(), "warfarin".to_string()),
                            ("drug class".to_string(), "vitamin K antagonist".to_string()),
                            ("medical conditions".to_string(), "thromboembolism".to_string()),
                            ("side effects".to_string(), "bleeding".to_string()),
                        ]),
                        allow_duplicate: false,
                    },
                )?;
                assert_eq!(template.generated_card_count, 4);
                assert_eq!(template.mode, "template");

                let search = browse_cards_in_connection(
                    connection,
                    BrowseCardsInput {
                        deck_id: deck.id.clone(),
                        query: "warfarin".to_string(),
                        tags: vec!["anticoagulants".to_string()],
                        status: "all".to_string(),
                        mode: "all".to_string(),
                        sort: "question_asc".to_string(),
                        limit: 100,
                        offset: 0,
                    },
                )?;
                assert_eq!(search.total, 3);

                let template_card_id: String = connection
                    .query_row(
                        "SELECT id FROM cards WHERE note_id = ?1 AND template_ordinal = 2",
                        [template.note_id.as_str()],
                        |row| row.get(0),
                    )
                    .map_err(CommandError::database)?;
                let rendered = render_card_from_connection(connection, &paths, &template_card_id)?;
                assert_eq!(rendered.front_text, "What are the major side effects of warfarin?");
                assert_eq!(rendered.back_text, "bleeding");

                let duplicate = save_note_in_connection(
                    connection,
                    &paths,
                    basic_note_input(
                        &deck.id,
                        "reverse",
                        "Warfarin class?",
                        "Vitamin K antagonist",
                        Vec::new(),
                    ),
                )
                .expect_err("an exact duplicate should require explicit confirmation");
                assert_eq!(duplicate.code, "duplicate_card");
                Ok(())
            })
            .expect("advanced card flows should succeed");

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn move_copy_bulk_entry_and_drafts_preserve_local_invariants() {
        let database = Database::open_in_memory().expect("database should open");
        let root = temporary_test_directory("move-copy-bulk");
        let paths = AppPaths::new(root.clone()).expect("test paths should initialize");

        database
            .with_connection(|connection| {
                let source = crate::decks::create_deck_in_connection(
                    connection,
                    crate::decks::CreateDeckInput {
                        name: "Source".to_string(),
                        description: None,
                    },
                )?;
                let destination = crate::decks::create_deck_in_connection(
                    connection,
                    crate::decks::CreateDeckInput {
                        name: "Destination".to_string(),
                        description: None,
                    },
                )?;
                let note = save_note_in_connection(
                    connection,
                    &paths,
                    basic_note_input(
                        &source.id,
                        "basic",
                        "Question one",
                        "Answer one",
                        vec!["transfer"],
                    ),
                )?;

                let copied = move_or_copy_in_connection(
                    connection,
                    MoveCopyNoteInput {
                        note_id: note.note_id.clone(),
                        destination_deck_id: destination.id.clone(),
                        operation: "copy".to_string(),
                    },
                )?;
                assert!(copied.copied);
                assert_ne!(copied.note_id, note.note_id);

                let moved = move_or_copy_in_connection(
                    connection,
                    MoveCopyNoteInput {
                        note_id: note.note_id.clone(),
                        destination_deck_id: destination.id.clone(),
                        operation: "move".to_string(),
                    },
                )?;
                assert!(!moved.copied);
                let destination_notes: i64 = connection
                    .query_row(
                        "SELECT COUNT(*) FROM notes WHERE deck_id = ?1 AND deleted_at IS NULL",
                        [destination.id.as_str()],
                        |row| row.get(0),
                    )
                    .map_err(CommandError::database)?;
                assert_eq!(destination_notes, 2);

                let first_bulk = bulk_create_in_connection(
                    connection,
                    BulkCreateInput {
                        deck_id: destination.id.clone(),
                        entries: vec![
                            BulkEntryRow {
                                question: "Bulk one".to_string(),
                                answer: "Answer".to_string(),
                            },
                            BulkEntryRow {
                                question: "Bulk two".to_string(),
                                answer: "Answer".to_string(),
                            },
                        ],
                        tags: vec!["bulk".to_string()],
                        create_reverse: true,
                        skip_duplicates: true,
                    },
                )?;
                assert_eq!(first_bulk.created_count, 2);

                let second_bulk = bulk_create_in_connection(
                    connection,
                    BulkCreateInput {
                        deck_id: destination.id.clone(),
                        entries: vec![BulkEntryRow {
                            question: "Bulk one".to_string(),
                            answer: "Answer".to_string(),
                        }],
                        tags: vec!["bulk".to_string()],
                        create_reverse: true,
                        skip_duplicates: true,
                    },
                )?;
                assert_eq!(second_bulk.created_count, 0);
                assert_eq!(
                    second_bulk.skipped_duplicates,
                    vec!["Bulk one".to_string()]
                );

                let saved = save_draft_in_connection(
                    connection,
                    SaveDraftInput {
                        draft_key: "new-destination".to_string(),
                        deck_id: destination.id.clone(),
                        note_id: None,
                        payload: json!({"mode": "basic", "frontText": "Unsaved"}),
                    },
                )?;
                assert_eq!(
                    saved.payload.get("frontText").and_then(Value::as_str),
                    Some("Unsaved")
                );
                let loaded = get_draft_from_connection(connection, "new-destination")?
                    .expect("draft should load");
                assert_eq!(loaded.deck_id, destination.id);
                connection
                    .execute(
                        "DELETE FROM editor_drafts WHERE draft_key = ?1",
                        [saved.draft_key.as_str()],
                    )
                    .map_err(CommandError::database)?;
                assert!(get_draft_from_connection(connection, "new-destination")?.is_none());
                Ok(())
            })
            .expect("move, copy, bulk, and draft flows should succeed");

        let _ = std::fs::remove_dir_all(root);
    }

    fn basic_note_input(
        deck_id: &str,
        mode: &str,
        front: &str,
        back: &str,
        tags: Vec<&str>,
    ) -> SaveNoteInput {
        SaveNoteInput {
            note_id: None,
            revision: None,
            deck_id: deck_id.to_string(),
            mode: mode.to_string(),
            front_doc: plain_document(front),
            back_doc: plain_document(back),
            front_text: front.to_string(),
            back_text: back.to_string(),
            tags: tags.into_iter().map(str::to_string).collect(),
            create_reverse: mode == "reverse",
            fill_blank_text: None,
            template_id: None,
            fields: BTreeMap::new(),
            allow_duplicate: false,
        }
    }

    fn temporary_test_directory(label: &str) -> std::path::PathBuf {
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock should be after epoch")
            .as_nanos();
        std::env::temp_dir().join(format!(
            "ostudy-phase2-{label}-{}-{nonce}",
            std::process::id()
        ))
    }
}
