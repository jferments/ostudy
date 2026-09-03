//! Shared card-presentation transport and rich-text validation.
//!
//! The frontend editor sends canonical JSON documents across the Tauri IPC
//! boundary. This module treats those documents as untrusted input, validates
//! every supported node/mark/attribute, and derives authoritative plain text in
//! Rust for search, duplicate detection, import, export, and backup behavior.

use std::collections::BTreeSet;

#[cfg(test)]
use rusqlite::params;
use serde::Serialize;
use serde_json::Value;

use crate::{assets::AssetDto, error::CommandError, util::validate_entity_id};

#[cfg(test)]
use crate::{database::Database, util::unix_time_millis};

const MAX_DOCUMENT_BYTES: usize = 1_000_000;
const MAX_PLAIN_TEXT_CHARS: usize = 200_000;
const MAX_DOCUMENT_DEPTH: usize = 128;
const MAX_RICH_SCHEMA_VERSION: i64 = 2;

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CardPresentationDto {
    #[serde(rename = "cardId")]
    pub id: String,
    pub note_id: String,
    pub deck_id: String,
    pub direction: String,
    pub front_doc: Value,
    pub back_doc: Value,
    pub front_text: String,
    pub back_text: String,
    pub assets: Vec<AssetDto>,
    pub created_at: i64,
    pub updated_at: i64,
    #[serde(rename = "noteRevision")]
    pub revision: i64,
}

pub(crate) struct ValidatedContent {
    pub(crate) front_doc_json: String,
    pub(crate) back_doc_json: String,
    pub(crate) front_text: String,
    pub(crate) back_text: String,
    pub(crate) front_asset_ids: Vec<String>,
    pub(crate) back_asset_ids: Vec<String>,
}

pub(crate) fn validate_content(
    front_doc: Value,
    back_doc: Value,
    front_text_snapshot: String,
    back_text_snapshot: String,
) -> Result<ValidatedContent, CommandError> {
    validate_plain_text_snapshot(&front_text_snapshot, "question")?;
    validate_plain_text_snapshot(&back_text_snapshot, "answer")?;
    validate_document(&front_doc)?;
    validate_document(&back_doc)?;
    validate_answer_checklist_pair(&front_doc, &back_doc)?;

    let front_doc_json = serde_json::to_string(&front_doc).map_err(CommandError::serialization)?;
    let back_doc_json = serde_json::to_string(&back_doc).map_err(CommandError::serialization)?;
    if front_doc_json.len() > MAX_DOCUMENT_BYTES || back_doc_json.len() > MAX_DOCUMENT_BYTES {
        return Err(CommandError::new(
            "card_content_too_large",
            "Each side of a flashcard must be smaller than 1 MB before images.",
        ));
    }

    let front_text = extract_plain_text(&front_doc, "question")?;
    let back_text = extract_plain_text(&back_doc, "answer")?;
    let front_asset_ids = collect_asset_ids(&front_doc)?;
    let back_asset_ids = collect_asset_ids(&back_doc)?;
    if front_text.is_empty() && front_asset_ids.is_empty() {
        return Err(CommandError::new(
            "empty_card_front",
            "Add text or an image to the question side.",
        ));
    }
    if back_text.is_empty() && back_asset_ids.is_empty() {
        return Err(CommandError::new(
            "empty_card_back",
            "Add text or an image to the answer side.",
        ));
    }

    Ok(ValidatedContent {
        front_doc_json,
        back_doc_json,
        front_text,
        back_text,
        front_asset_ids,
        back_asset_ids,
    })
}

fn validate_plain_text_snapshot(value: &str, label: &str) -> Result<(), CommandError> {
    if value.chars().count() > MAX_PLAIN_TEXT_CHARS {
        return Err(CommandError::new(
            "card_text_too_large",
            format!("The {label} text cannot exceed {MAX_PLAIN_TEXT_CHARS} characters."),
        ));
    }
    Ok(())
}

pub(crate) fn validate_document(document: &Value) -> Result<(), CommandError> {
    let root = document.as_object().ok_or_else(|| {
        CommandError::new(
            "invalid_card_document",
            "The card editor content is invalid.",
        )
    })?;
    if root.get("type").and_then(Value::as_str) != Some("doc") {
        return Err(CommandError::new(
            "invalid_card_document",
            "The card editor content is missing its document root.",
        ));
    }
    validate_document_node(document, 0)
}

const ALLOWED_NODE_TYPES: &[&str] = &[
    "doc",
    "paragraph",
    "text",
    "heading",
    "blockquote",
    "bulletList",
    "orderedList",
    "listItem",
    "taskList",
    "taskItem",
    "table",
    "tableRow",
    "tableHeader",
    "tableCell",
    "codeBlock",
    "horizontalRule",
    "hardBreak",
    "image",
];
const ALLOWED_MARK_TYPES: &[&str] = &[
    "bold",
    "italic",
    "strike",
    "code",
    "link",
    "underline",
    "highlight",
    "textStyle",
    "subscript",
    "superscript",
];

fn validate_document_node(value: &Value, depth: usize) -> Result<(), CommandError> {
    if depth > MAX_DOCUMENT_DEPTH {
        return Err(CommandError::new(
            "invalid_card_document",
            "The card document is nested too deeply.",
        ));
    }
    let object = value.as_object().ok_or_else(|| {
        CommandError::new(
            "invalid_card_document",
            "Every card document node must be a JSON object.",
        )
    })?;
    let node_type = object.get("type").and_then(Value::as_str).ok_or_else(|| {
        CommandError::new(
            "invalid_card_document",
            "A card document node is missing its type.",
        )
    })?;
    if !ALLOWED_NODE_TYPES.contains(&node_type) {
        return Err(CommandError::new(
            "unsupported_card_content",
            format!("The card contains an unsupported content node: {node_type}."),
        ));
    }

    validate_node_keys(node_type, object)?;
    validate_node_attributes(node_type, object.get("attrs"))?;

    if node_type == "text" {
        let text = object.get("text").and_then(Value::as_str).ok_or_else(|| {
            CommandError::new(
                "invalid_card_document",
                "A text node is missing its text value.",
            )
        })?;
        if text.chars().count() > MAX_PLAIN_TEXT_CHARS {
            return Err(CommandError::new(
                "card_text_too_large",
                "A card text node is too large.",
            ));
        }
        if object.contains_key("content") {
            return Err(CommandError::new(
                "invalid_card_document",
                "Text nodes cannot contain nested content.",
            ));
        }
    } else if object.contains_key("text") {
        return Err(CommandError::new(
            "invalid_card_document",
            "Only text nodes may contain a text value.",
        ));
    }

    if let Some(marks) = object.get("marks") {
        if node_type != "text" {
            return Err(CommandError::new(
                "invalid_card_document",
                "Only text may contain formatting marks.",
            ));
        }
        let marks = marks.as_array().ok_or_else(|| {
            CommandError::new(
                "invalid_card_document",
                "Card text formatting must be represented as a list.",
            )
        })?;
        let mut mark_types = BTreeSet::new();
        for mark in marks {
            let mark_type = validate_mark(mark)?;
            if !mark_types.insert(mark_type.clone()) {
                return Err(CommandError::new(
                    "invalid_card_document",
                    "A text node contains the same formatting mark more than once.",
                ));
            }
        }
        if mark_types.contains("subscript") && mark_types.contains("superscript") {
            return Err(CommandError::new(
                "invalid_card_style",
                "Text cannot be both subscript and superscript.",
            ));
        }
    }

    let content = match object.get("content") {
        Some(value) => Some(value.as_array().ok_or_else(|| {
            CommandError::new(
                "invalid_card_document",
                "Card node content must be represented as a list.",
            )
        })?),
        None => None,
    };
    if let Some(children) = content {
        for child in children {
            validate_document_node(child, depth + 1)?;
        }
    }
    validate_child_structure(node_type, content)?;
    Ok(())
}

fn validate_node_keys(
    node_type: &str,
    object: &serde_json::Map<String, Value>,
) -> Result<(), CommandError> {
    for key in object.keys() {
        if !matches!(
            key.as_str(),
            "type" | "attrs" | "content" | "marks" | "text"
        ) {
            return Err(CommandError::new(
                "invalid_card_document",
                format!("The {node_type} node contains an unsupported property: {key}."),
            ));
        }
    }
    Ok(())
}

fn validate_child_structure(
    node_type: &str,
    content: Option<&Vec<Value>>,
) -> Result<(), CommandError> {
    let child_types = content
        .into_iter()
        .flat_map(|children| children.iter())
        .map(|child| {
            child
                .get("type")
                .and_then(Value::as_str)
                .unwrap_or_default()
        })
        .collect::<Vec<_>>();

    let valid = match node_type {
        "doc" => !child_types.is_empty() && child_types.iter().all(|child| is_block_node(child)),
        "paragraph" | "heading" => child_types
            .iter()
            .all(|child| matches!(*child, "text" | "hardBreak")),
        "blockquote" | "tableCell" | "tableHeader" => {
            !child_types.is_empty() && child_types.iter().all(|child| is_block_node(child))
        }
        "bulletList" | "orderedList" => {
            !child_types.is_empty() && child_types.iter().all(|child| *child == "listItem")
        }
        "taskList" => {
            !child_types.is_empty() && child_types.iter().all(|child| *child == "taskItem")
        }
        "listItem" | "taskItem" => {
            !child_types.is_empty()
                && child_types.first() == Some(&"paragraph")
                && child_types.iter().all(|child| is_block_node(child))
        }
        "table" => !child_types.is_empty() && child_types.iter().all(|child| *child == "tableRow"),
        "tableRow" => {
            !child_types.is_empty()
                && child_types
                    .iter()
                    .all(|child| matches!(*child, "tableHeader" | "tableCell"))
        }
        "codeBlock" => child_types.iter().all(|child| *child == "text"),
        "text" | "horizontalRule" | "hardBreak" | "image" => content.is_none(),
        _ => false,
    };

    if !valid {
        return Err(CommandError::new(
            "invalid_card_document",
            format!("The {node_type} node contains invalid nested content."),
        ));
    }
    Ok(())
}

fn is_block_node(node_type: &str) -> bool {
    matches!(
        node_type,
        "paragraph"
            | "heading"
            | "blockquote"
            | "bulletList"
            | "orderedList"
            | "taskList"
            | "table"
            | "codeBlock"
            | "horizontalRule"
            | "image"
    )
}

fn validate_node_attributes(node_type: &str, attrs: Option<&Value>) -> Result<(), CommandError> {
    let Some(attrs) = attrs else {
        return if node_type == "image" {
            Err(CommandError::new(
                "invalid_image_reference",
                "Every card image must refer to a locally imported image.",
            ))
        } else {
            Ok(())
        };
    };
    let attrs = attrs.as_object().ok_or_else(|| {
        CommandError::new(
            "invalid_card_document",
            "Card node attributes must be a JSON object.",
        )
    })?;
    validate_node_attribute_keys(node_type, attrs)?;

    if node_type == "doc" {
        if let Some(version) = attrs.get("schemaVersion") {
            let version = version.as_i64().ok_or_else(|| {
                CommandError::new(
                    "invalid_card_document",
                    "The rich-document schema version is invalid.",
                )
            })?;
            if !(1..=MAX_RICH_SCHEMA_VERSION).contains(&version) {
                return Err(CommandError::new(
                    "unsupported_card_content",
                    "The card was created by a newer rich-document schema.",
                ));
            }
        }
    }

    if node_type == "image" {
        let asset_id = attrs
            .get("assetId")
            .and_then(Value::as_str)
            .ok_or_else(|| {
                CommandError::new(
                    "invalid_image_reference",
                    "Every card image must refer to a locally imported image.",
                )
            })?;
        validate_entity_id(asset_id, "asset")?;
        let expected_src = format!("study-asset://{asset_id}");
        if attrs.get("src").and_then(Value::as_str) != Some(expected_src.as_str()) {
            return Err(CommandError::new(
                "invalid_image_reference",
                "The card image source does not match its local asset identifier.",
            ));
        }
        validate_optional_attribute_text(attrs.get("alt"), 500, "image alternative text")?;
        validate_optional_attribute_text(attrs.get("title"), 500, "image title")?;
        validate_optional_image_dimension(attrs.get("width"), "width")?;
        validate_optional_image_dimension(attrs.get("height"), "height")?;
    }

    if node_type == "heading" {
        let level = attrs.get("level").and_then(Value::as_i64).ok_or_else(|| {
            CommandError::new(
                "invalid_card_style",
                "Every card heading must specify a valid heading level.",
            )
        })?;
        if !(1..=4).contains(&level) {
            return Err(CommandError::new(
                "invalid_card_style",
                "Card headings must use levels one through four.",
            ));
        }
    }

    if let Some(indent) = attrs.get("indent") {
        if !indent.is_null() {
            let indent = indent.as_i64().ok_or_else(|| {
                CommandError::new(
                    "invalid_card_style",
                    "The card indentation level is invalid.",
                )
            })?;
            if !(0..=6).contains(&indent) {
                return Err(CommandError::new(
                    "invalid_card_style",
                    "Card indentation must be between zero and six levels.",
                ));
            }
        }
    }
    if let Some(alignment) = optional_string(attrs.get("textAlign"), "text alignment")? {
        if !matches!(alignment, "left" | "center" | "right" | "justify") {
            return Err(CommandError::new(
                "invalid_card_style",
                "The card text alignment is invalid.",
            ));
        }
    }
    if node_type == "orderedList" {
        if let Some(start) = attrs.get("start") {
            let start = start.as_i64().ok_or_else(|| {
                CommandError::new(
                    "invalid_card_document",
                    "The ordered-list start is invalid.",
                )
            })?;
            if !(1..=1_000_000).contains(&start) {
                return Err(CommandError::new(
                    "invalid_card_document",
                    "The ordered-list start is outside the supported range.",
                ));
            }
        }
        validate_optional_attribute_text(attrs.get("type"), 20, "ordered-list type")?;
    }
    if node_type == "codeBlock" {
        validate_optional_attribute_text(attrs.get("language"), 100, "code language")?;
    }
    if matches!(node_type, "tableCell" | "tableHeader") {
        validate_table_span(attrs.get("colspan"), "column span")?;
        validate_table_span(attrs.get("rowspan"), "row span")?;
        validate_column_widths(attrs.get("colwidth"))?;
    }
    if node_type == "taskList" {
        let mode = attrs
            .get("mode")
            .and_then(Value::as_str)
            .unwrap_or("ordinary");
        if !matches!(mode, "ordinary" | "answer_key") {
            return Err(CommandError::new(
                "invalid_card_document",
                "The checklist mode is invalid.",
            ));
        }
    }
    if node_type == "taskItem" {
        if !attrs.get("checked").is_some_and(Value::is_boolean) {
            return Err(CommandError::new(
                "invalid_card_document",
                "Every checklist item must have a checked state.",
            ));
        }
        if let Some(item_id) = attrs.get("itemId") {
            if !item_id.is_null() {
                let item_id = item_id.as_str().ok_or_else(|| {
                    CommandError::new(
                        "invalid_card_document",
                        "The checklist item identity is invalid.",
                    )
                })?;
                validate_entity_id(item_id, "checklist item")?;
            }
        }
    }

    Ok(())
}

fn validate_table_span(value: Option<&Value>, label: &str) -> Result<(), CommandError> {
    let span = value.and_then(Value::as_i64).unwrap_or(1);
    if !(1..=64).contains(&span) {
        return Err(CommandError::new(
            "invalid_card_document",
            format!("The table {label} must be between one and 64."),
        ));
    }
    Ok(())
}

fn validate_column_widths(value: Option<&Value>) -> Result<(), CommandError> {
    let Some(value) = value else {
        return Ok(());
    };
    if value.is_null() {
        return Ok(());
    }
    let widths = value.as_array().ok_or_else(|| {
        CommandError::new(
            "invalid_card_document",
            "Table column widths must be a list or null.",
        )
    })?;
    if widths.len() > 64
        || widths.iter().any(|width| {
            !width
                .as_i64()
                .is_some_and(|width| (1..=10_000).contains(&width))
        })
    {
        return Err(CommandError::new(
            "invalid_card_document",
            "A table column width is outside the supported range.",
        ));
    }
    Ok(())
}

fn validate_optional_image_dimension(
    value: Option<&Value>,
    label: &str,
) -> Result<(), CommandError> {
    let Some(value) = value else {
        return Ok(());
    };
    if value.is_null() {
        return Ok(());
    }
    let dimension = value.as_f64().ok_or_else(|| {
        CommandError::new(
            "invalid_card_style",
            format!("The image {label} must be a number or null."),
        )
    })?;
    if !dimension.is_finite() || !(1.0..=10_000.0).contains(&dimension) {
        return Err(CommandError::new(
            "invalid_card_style",
            format!("The image {label} must be between 1 and 10,000 pixels."),
        ));
    }
    Ok(())
}

fn validate_node_attribute_keys(
    node_type: &str,
    attrs: &serde_json::Map<String, Value>,
) -> Result<(), CommandError> {
    let allowed: &[&str] = match node_type {
        "doc" => &["schemaVersion"],
        "paragraph" => &["textAlign", "indent"],
        "heading" => &["textAlign", "indent", "level"],
        "image" => &["src", "alt", "title", "assetId", "width", "height"],
        "orderedList" => &["start", "type"],
        "codeBlock" => &["language"],
        "tableCell" | "tableHeader" => &["colspan", "rowspan", "colwidth"],
        "taskList" => &["mode"],
        "taskItem" => &["checked", "itemId"],
        _ => &[],
    };
    for key in attrs.keys() {
        if !allowed.contains(&key.as_str()) {
            return Err(CommandError::new(
                "invalid_card_document",
                format!("The {node_type} node contains an unsupported attribute: {key}."),
            ));
        }
    }
    Ok(())
}

fn validate_mark(value: &Value) -> Result<String, CommandError> {
    let object = value.as_object().ok_or_else(|| {
        CommandError::new(
            "invalid_card_document",
            "Every text formatting mark must be a JSON object.",
        )
    })?;
    let mark_type = object.get("type").and_then(Value::as_str).ok_or_else(|| {
        CommandError::new(
            "invalid_card_document",
            "A text formatting mark is missing its type.",
        )
    })?;
    for key in object.keys() {
        if !matches!(key.as_str(), "type" | "attrs") {
            return Err(CommandError::new(
                "invalid_card_document",
                format!("The text formatting mark contains an unsupported property: {key}."),
            ));
        }
    }
    if !ALLOWED_MARK_TYPES.contains(&mark_type) {
        return Err(CommandError::new(
            "unsupported_card_content",
            format!("The card contains unsupported text formatting: {mark_type}."),
        ));
    }

    let attrs = match object.get("attrs") {
        Some(Value::Object(attrs)) => Some(attrs),
        Some(Value::Null) | None => None,
        Some(_) => {
            return Err(CommandError::new(
                "invalid_card_document",
                "Text formatting attributes must be a JSON object.",
            ));
        }
    };
    validate_mark_attribute_keys(mark_type, attrs)?;

    if mark_type == "link" {
        let href = attrs
            .and_then(|value| value.get("href"))
            .and_then(Value::as_str)
            .ok_or_else(|| CommandError::new("invalid_link", "The card link has no address."))?;
        validate_link(href)?;
        if let Some(target) =
            optional_string(attrs.and_then(|value| value.get("target")), "link target")?
        {
            if target != "_blank" {
                return Err(CommandError::new(
                    "invalid_link",
                    "Card links may only open in a new external window.",
                ));
            }
        }
        validate_optional_attribute_text(
            attrs.and_then(|value| value.get("rel")),
            200,
            "link relationship",
        )?;
        validate_optional_attribute_text(
            attrs.and_then(|value| value.get("class")),
            200,
            "link class",
        )?;
    }
    if mark_type == "highlight" {
        if let Some(color) = optional_string(
            attrs.and_then(|value| value.get("color")),
            "highlight color",
        )? {
            validate_css_color(color, "highlight color")?;
        }
    }
    if mark_type == "textStyle" {
        if let Some(attrs) = attrs {
            if let Some(font_family) = optional_string(attrs.get("fontFamily"), "font family")? {
                validate_font_family(font_family)?;
            }
            if let Some(font_size) = optional_string(attrs.get("fontSize"), "font size")? {
                validate_font_size(font_size)?;
            }
            if let Some(color) = optional_string(attrs.get("color"), "text color")? {
                validate_css_color(color, "text color")?;
            }
            if let Some(background) =
                optional_string(attrs.get("backgroundColor"), "background color")?
            {
                validate_css_color(background, "background color")?;
            }
            if let Some(line_height) = optional_string(attrs.get("lineHeight"), "line height")? {
                validate_line_height(line_height)?;
            }
        }
    }

    Ok(mark_type.to_string())
}

fn validate_mark_attribute_keys(
    mark_type: &str,
    attrs: Option<&serde_json::Map<String, Value>>,
) -> Result<(), CommandError> {
    let allowed: &[&str] = match mark_type {
        "link" => &["href", "target", "rel", "class"],
        "highlight" => &["color"],
        "textStyle" => &[
            "fontFamily",
            "fontSize",
            "color",
            "backgroundColor",
            "lineHeight",
        ],
        _ => &[],
    };
    if let Some(attrs) = attrs {
        for key in attrs.keys() {
            if !allowed.contains(&key.as_str()) {
                return Err(CommandError::new(
                    "invalid_card_document",
                    format!(
                        "The {mark_type} formatting mark contains an unsupported attribute: {key}."
                    ),
                ));
            }
        }
    }
    Ok(())
}

fn optional_string<'a>(
    value: Option<&'a Value>,
    label: &str,
) -> Result<Option<&'a str>, CommandError> {
    match value {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(value)) => Ok(Some(value)),
        Some(_) => Err(CommandError::new(
            "invalid_card_style",
            format!("The selected {label} is invalid."),
        )),
    }
}

fn validate_optional_attribute_text(
    value: Option<&Value>,
    max_chars: usize,
    label: &str,
) -> Result<(), CommandError> {
    let Some(value) = value else {
        return Ok(());
    };
    if value.is_null() {
        return Ok(());
    }
    let text = value.as_str().ok_or_else(|| {
        CommandError::new(
            "invalid_card_document",
            format!("The {label} must be text."),
        )
    })?;
    if text.chars().count() > max_chars || text.chars().any(char::is_control) {
        return Err(CommandError::new(
            "invalid_card_document",
            format!("The {label} is invalid."),
        ));
    }
    Ok(())
}

fn validate_font_family(value: &str) -> Result<(), CommandError> {
    if value.is_empty()
        || value.len() > 200
        || value
            .chars()
            .any(|character| matches!(character, ';' | '{' | '}' | '<' | '>' | '\n' | '\r' | '\0'))
    {
        return Err(CommandError::new(
            "invalid_card_style",
            "The selected font family is invalid.",
        ));
    }
    Ok(())
}

fn validate_font_size(value: &str) -> Result<(), CommandError> {
    let Some(number) = value.strip_suffix("px") else {
        return Err(CommandError::new(
            "invalid_card_style",
            "Card font sizes must be expressed in pixels.",
        ));
    };
    let parsed = number.parse::<u16>().map_err(|_| {
        CommandError::new(
            "invalid_card_style",
            "The selected card font size is invalid.",
        )
    })?;
    if !(8..=96).contains(&parsed) {
        return Err(CommandError::new(
            "invalid_card_style",
            "Card font sizes must be between 8 and 96 pixels.",
        ));
    }
    Ok(())
}

fn validate_line_height(value: &str) -> Result<(), CommandError> {
    let parsed = value.parse::<f32>().map_err(|_| {
        CommandError::new("invalid_card_style", "The selected line height is invalid.")
    })?;
    if !parsed.is_finite() || !(0.8..=3.0).contains(&parsed) {
        return Err(CommandError::new(
            "invalid_card_style",
            "Card line height must be between 0.8 and 3.0.",
        ));
    }
    Ok(())
}

fn validate_css_color(value: &str, label: &str) -> Result<(), CommandError> {
    let is_hex = value.len() == 7
        && value.starts_with('#')
        && value[1..].bytes().all(|byte| byte.is_ascii_hexdigit());
    if !is_hex {
        return Err(CommandError::new(
            "invalid_card_style",
            format!("The selected {label} is invalid."),
        ));
    }
    Ok(())
}

#[derive(Debug)]
struct AnswerChecklist {
    ids: Vec<String>,
    checked: Vec<bool>,
}

fn validate_answer_checklist_pair(front: &Value, back: &Value) -> Result<(), CommandError> {
    let front_lists = collect_answer_checklists(front)?;
    let back_lists = collect_answer_checklists(back)?;
    if front_lists.is_empty() && back_lists.is_empty() {
        return Ok(());
    }
    if front_lists.len() != back_lists.len() {
        return Err(invalid_answer_checklist(
            "Copy each question answer-checklist to the answer before saving.",
        ));
    }
    for (front, back) in front_lists.iter().zip(back_lists.iter()) {
        if front.ids != back.ids {
            return Err(invalid_answer_checklist(
                "The question and answer checklists must contain the same stable items.",
            ));
        }
        if front.checked.iter().any(|checked| *checked) {
            return Err(invalid_answer_checklist(
                "Question answer-checklist boxes must be empty. Copy the checklist to the answer again.",
            ));
        }
        if !back.checked.iter().any(|checked| *checked) {
            return Err(invalid_answer_checklist(
                "Mark at least one correct answer in each answer checklist.",
            ));
        }
    }
    Ok(())
}

fn collect_answer_checklists(document: &Value) -> Result<Vec<AnswerChecklist>, CommandError> {
    let mut output = Vec::new();
    let mut all_ids = BTreeSet::new();
    collect_answer_checklists_from_value(document, &mut output, &mut all_ids)?;
    Ok(output)
}

fn collect_answer_checklists_from_value(
    value: &Value,
    output: &mut Vec<AnswerChecklist>,
    all_ids: &mut BTreeSet<String>,
) -> Result<(), CommandError> {
    let Some(object) = value.as_object() else {
        return Ok(());
    };
    if object.get("type").and_then(Value::as_str) == Some("taskList")
        && object
            .get("attrs")
            .and_then(Value::as_object)
            .and_then(|attrs| attrs.get("mode"))
            .and_then(Value::as_str)
            == Some("answer_key")
    {
        let mut ids = Vec::new();
        let mut checked = Vec::new();
        for item in object
            .get("content")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            let attrs = item
                .get("attrs")
                .and_then(Value::as_object)
                .ok_or_else(|| {
                    invalid_answer_checklist("Every answer-checklist item needs attributes.")
                })?;
            let id = attrs.get("itemId").and_then(Value::as_str).ok_or_else(|| {
                invalid_answer_checklist("Every answer-checklist item needs a stable identity.")
            })?;
            validate_entity_id(id, "checklist item")?;
            if !all_ids.insert(id.to_string()) {
                return Err(invalid_answer_checklist(
                    "Answer-checklist item identities must be unique.",
                ));
            }
            ids.push(id.to_string());
            checked.push(
                attrs
                    .get("checked")
                    .and_then(Value::as_bool)
                    .unwrap_or(false),
            );
        }
        output.push(AnswerChecklist { ids, checked });
        return Ok(());
    }
    if let Some(content) = object.get("content").and_then(Value::as_array) {
        for child in content {
            collect_answer_checklists_from_value(child, output, all_ids)?;
        }
    }
    Ok(())
}

fn invalid_answer_checklist(message: &str) -> CommandError {
    CommandError::new("invalid_answer_checklist", message)
}

fn extract_plain_text(document: &Value, label: &str) -> Result<String, CommandError> {
    let mut output = String::new();
    collect_plain_text(document, &mut output);
    let normalized = output
        .lines()
        .map(str::trim_end)
        .collect::<Vec<_>>()
        .join("\n")
        .trim()
        .to_string();
    if normalized.chars().count() > MAX_PLAIN_TEXT_CHARS {
        return Err(CommandError::new(
            "card_text_too_large",
            format!("The {label} text cannot exceed {MAX_PLAIN_TEXT_CHARS} characters."),
        ));
    }
    Ok(normalized)
}

fn collect_plain_text(value: &Value, output: &mut String) {
    let Some(object) = value.as_object() else {
        return;
    };
    let node_type = object
        .get("type")
        .and_then(Value::as_str)
        .unwrap_or_default();
    if node_type == "text" {
        if let Some(text) = object.get("text").and_then(Value::as_str) {
            output.push_str(text);
        }
        return;
    }
    if node_type == "hardBreak" {
        output.push('\n');
        return;
    }
    if node_type == "taskItem" {
        let checked = object
            .get("attrs")
            .and_then(Value::as_object)
            .and_then(|attrs| attrs.get("checked"))
            .and_then(Value::as_bool)
            .unwrap_or(false);
        output.push_str(if checked { "[x] " } else { "[ ] " });
    }

    if let Some(content) = object.get("content").and_then(Value::as_array) {
        for (index, child) in content.iter().enumerate() {
            collect_plain_text(child, output);
            if matches!(node_type, "tableRow")
                && index + 1 < content.len()
                && !output.ends_with('\t')
            {
                output.push('\t');
            }
        }
    }

    if matches!(
        node_type,
        "paragraph"
            | "heading"
            | "blockquote"
            | "listItem"
            | "taskItem"
            | "codeBlock"
            | "tableRow"
            | "table"
    ) && !output.ends_with('\n')
    {
        output.push('\n');
    }
}

fn validate_link(href: &str) -> Result<(), CommandError> {
    if href.is_empty()
        || href != href.trim()
        || href.len() > 2_000
        || href
            .chars()
            .any(|character| character.is_control() || character.is_whitespace())
    {
        return Err(invalid_link());
    }

    let lower = href.to_ascii_lowercase();
    let valid = if lower.starts_with("https://") {
        has_http_authority(href, "https://".len())
    } else if lower.starts_with("http://") {
        has_http_authority(href, "http://".len())
    } else if lower.starts_with("mailto:") {
        !href["mailto:".len()..].is_empty()
    } else if lower.starts_with("tel:") {
        !href["tel:".len()..].is_empty()
    } else {
        false
    };
    if !valid {
        return Err(invalid_link());
    }
    Ok(())
}

fn has_http_authority(href: &str, scheme_length: usize) -> bool {
    href.get(scheme_length..)
        .and_then(|remainder| remainder.split(['/', '?', '#']).next())
        .is_some_and(|authority| !authority.is_empty())
}

fn invalid_link() -> CommandError {
    CommandError::new(
        "invalid_link",
        "Enter a complete http, https, mailto, or tel address.",
    )
}

pub(crate) fn collect_asset_ids(document: &Value) -> Result<Vec<String>, CommandError> {
    let mut ids = BTreeSet::new();
    collect_asset_ids_from_value(document, &mut ids)?;
    Ok(ids.into_iter().collect())
}

fn collect_asset_ids_from_value(
    value: &Value,
    ids: &mut BTreeSet<String>,
) -> Result<(), CommandError> {
    match value {
        Value::Array(values) => {
            for item in values {
                collect_asset_ids_from_value(item, ids)?;
            }
        }
        Value::Object(object) => {
            if object.get("type").and_then(Value::as_str) == Some("image") {
                let id = object
                    .get("attrs")
                    .and_then(Value::as_object)
                    .and_then(|attrs| attrs.get("assetId"))
                    .and_then(Value::as_str)
                    .ok_or_else(|| {
                        CommandError::new(
                            "invalid_image_reference",
                            "A card image is missing its local asset identifier.",
                        )
                    })?;
                validate_entity_id(id, "asset")?;
                ids.insert(id.to_string());
            }
            for nested in object.values() {
                collect_asset_ids_from_value(nested, ids)?;
            }
        }
        _ => {}
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn paragraph(text: &str) -> Value {
        serde_json::json!({
            "type": "doc",
            "content": [{
                "type": "paragraph",
                "content": [{"type": "text", "text": text}]
            }]
        })
    }

    #[test]
    fn accepts_complete_supported_links() {
        for href in [
            "https://example.org",
            "http://localhost:1420/path?query=1#fragment",
            "mailto:student@example.org",
            "tel:+1-555-0100",
        ] {
            validate_link(href).unwrap_or_else(|error| panic!("{href} should be valid: {error:?}"));
        }
    }

    #[test]
    fn rejects_incomplete_or_unsafe_links() {
        for href in [
            "",
            " https://example.org",
            "https://example.org ",
            "https://",
            "http:///missing-host",
            "https://example .org",
            "mailto:",
            "tel:",
            "javascript:alert(1)",
            "file:///tmp/private",
        ] {
            assert!(validate_link(href).is_err(), "{href} should be rejected");
        }
    }

    #[test]
    fn accepts_versioned_tables_tasks_and_subscripts() {
        let document = serde_json::json!({
            "type": "doc",
            "attrs": {"schemaVersion": 2},
            "content": [
                {
                    "type": "paragraph",
                    "content": [{
                        "type": "text",
                        "text": "H2O",
                        "marks": [{"type": "subscript"}]
                    }]
                },
                {
                    "type": "table",
                    "content": [{
                        "type": "tableRow",
                        "content": [
                            {"type": "tableHeader", "attrs": {"colspan": 1, "rowspan": 1, "colwidth": null}, "content": [{"type": "paragraph", "content": [{"type": "text", "text": "Drug"}]}]},
                            {"type": "tableCell", "attrs": {"colspan": 1, "rowspan": 1, "colwidth": null}, "content": [{"type": "paragraph", "content": [{"type": "text", "text": "Dose"}]}]}
                        ]
                    }]
                },
                {
                    "type": "taskList",
                    "attrs": {"mode": "ordinary"},
                    "content": [{
                        "type": "taskItem",
                        "attrs": {"checked": true, "itemId": null},
                        "content": [{"type": "paragraph", "content": [{"type": "text", "text": "Review"}]}]
                    }]
                }
            ]
        });
        validate_document(&document).expect("versioned rich content should validate");
        let text = extract_plain_text(&document, "question").expect("text should derive");
        assert!(text.contains("Drug\tDose"));
        assert!(text.contains("[x] Review"));
    }

    #[test]
    fn validates_matching_answer_checklists() {
        let item_a = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
        let item_b = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
        let make = |first_checked: bool, second_checked: bool| {
            serde_json::json!({
                "type": "doc",
                "attrs": {"schemaVersion": 2},
                "content": [{
                    "type": "taskList",
                    "attrs": {"mode": "answer_key"},
                    "content": [
                        {"type": "taskItem", "attrs": {"checked": first_checked, "itemId": item_a}, "content": [{"type": "paragraph", "content": [{"type": "text", "text": "First"}]}]},
                        {"type": "taskItem", "attrs": {"checked": second_checked, "itemId": item_b}, "content": [{"type": "paragraph", "content": [{"type": "text", "text": "Second"}]}]}
                    ]
                }]
            })
        };
        validate_content(
            make(false, false),
            make(true, false),
            "First Second".to_string(),
            "First Second".to_string(),
        )
        .expect("matching answer checklist should validate");
    }

    #[test]
    fn rejects_mismatched_answer_checklists() {
        let front = serde_json::json!({
            "type": "doc", "attrs": {"schemaVersion": 2}, "content": [{
                "type": "taskList", "attrs": {"mode": "answer_key"}, "content": [{
                    "type": "taskItem", "attrs": {"checked": false, "itemId": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"},
                    "content": [{"type": "paragraph", "content": [{"type": "text", "text": "First"}]}]
                }]
            }]
        });
        let back = paragraph("Different answer");
        let error = validate_content(
            front,
            back,
            "First".to_string(),
            "Different answer".to_string(),
        )
        .expect_err("unsynchronized checklist should fail");
        assert_eq!(error.code, "invalid_answer_checklist");
    }

    #[test]
    fn deletes_a_card_note_and_its_presentation() {
        let database = Database::open_in_memory().expect("database should open");
        let deck = database
            .with_connection(|connection| {
                crate::decks::create_deck_in_connection(
                    connection,
                    crate::decks::CreateDeckInput {
                        name: "Biology".to_string(),
                        description: None,
                    },
                )
            })
            .expect("deck should be created");

        let (note_id, card_id) = database
            .with_connection(|connection| {
                let content = validate_content(
                    paragraph("What is ATP?"),
                    paragraph("The cell's immediately usable energy currency."),
                    "What is ATP?".to_string(),
                    "The cell's immediately usable energy currency.".to_string(),
                )?;
                let now = unix_time_millis()?;
                let note_id = crate::util::generate_id(connection)?;
                let card_id = crate::util::generate_id(connection)?;
                connection.execute(
                    "INSERT INTO notes (
                         id, deck_id, note_type, content_format, content_schema_version,
                         front_doc, back_doc, front_text, back_text,
                         created_at, updated_at, revision
                     ) VALUES (?1, ?2, 'basic', 'tiptap-json', 1, ?3, ?4, ?5, ?6, ?7, ?7, 1)",
                    params![note_id, deck.id, content.front_doc_json, content.back_doc_json, content.front_text, content.back_text, now],
                ).map_err(CommandError::database)?;
                connection.execute(
                    "INSERT INTO cards (id, note_id, deck_id, direction, template_ordinal, created_at, updated_at)
                     VALUES (?1, ?2, ?3, 'forward', 0, ?4, ?4)",
                    params![card_id, note_id, deck.id, now],
                ).map_err(CommandError::database)?;
                crate::scheduler::insert_new_schedule(connection, &card_id, now)?;
                Ok((note_id, card_id))
            })
            .expect("card fixture should be created");

        let deletion = database
            .with_connection(|connection| {
                crate::deletion::delete_cards(
                    connection,
                    vec![note_id.clone()],
                    "Flashcard".to_string(),
                )
            })
            .expect("card should delete");
        assert_eq!(deletion.affected_count, 1);
        database
            .with_connection(|connection| {
                let active_note: i64 = connection
                    .query_row(
                        "SELECT COUNT(*) FROM notes WHERE id = ?1 AND deleted_at IS NULL",
                        [note_id.as_str()],
                        |row| row.get(0),
                    )
                    .map_err(CommandError::database)?;
                let active_card: i64 = connection
                    .query_row(
                        "SELECT COUNT(*) FROM cards WHERE id = ?1 AND deleted_at IS NULL",
                        [card_id.as_str()],
                        |row| row.get(0),
                    )
                    .map_err(CommandError::database)?;
                assert_eq!(active_note, 0);
                assert_eq!(active_card, 0);
                Ok(())
            })
            .expect("deletion tombstones should be readable");
    }

    #[test]
    fn rejects_javascript_links() {
        let document = serde_json::json!({
            "type": "doc", "content": [{"type": "paragraph", "content": [{
                "type": "text", "text": "bad",
                "marks": [{"type": "link", "attrs": {"href": "javascript:alert(1)"}}]
            }]}]
        });
        let error = validate_document(&document).expect_err("link should fail");
        assert_eq!(error.code, "invalid_link");
    }

    #[test]
    fn derives_search_text_from_the_validated_document() {
        let validated = validate_content(
            paragraph("Visible question"),
            paragraph("Visible answer"),
            "spoofed question".to_string(),
            "spoofed answer".to_string(),
        )
        .expect("valid card should be accepted");
        assert_eq!(validated.front_text, "Visible question");
        assert_eq!(validated.back_text, "Visible answer");
    }

    #[test]
    fn rejects_unknown_nodes_and_noncanonical_images() {
        let unknown = serde_json::json!({"type": "doc", "content": [{"type": "iframe"}]});
        let error = validate_document(&unknown).expect_err("unknown node should fail");
        assert_eq!(error.code, "unsupported_card_content");

        let image = serde_json::json!({
            "type": "doc", "content": [{"type": "image", "attrs": {
                "assetId": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa", "src": "https://example.org/image.png",
                "alt": "diagram", "title": null, "width": null, "height": 320
            }}]
        });
        let error = validate_document(&image).expect_err("remote image should fail");
        assert_eq!(error.code, "invalid_image_reference");
    }

    #[test]
    fn accepts_supported_rich_text_and_local_image_nodes() {
        let document = serde_json::json!({
            "type": "doc", "content": [
                {"type": "heading", "attrs": {"level": 2, "textAlign": "center", "indent": 1},
                 "content": [{"type": "text", "text": "Card heading", "marks": [{"type": "superscript"}]}]},
                {"type": "image", "attrs": {
                    "assetId": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
                    "src": "study-asset://aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
                    "alt": "diagram", "title": "Local image"
                }}
            ]
        });
        validate_document(&document).expect("supported rich document should validate");
    }
}
