//! Schema-3 rich math/media compatibility layered over the released schema-2 validator.
//!
//! New nodes are validated as typed data, then replaced in a validation-only shadow
//! document so the mature schema-2 validator remains authoritative for every older
//! table/checklist/mark/style invariant. The original schema-3 JSON is persisted.

use std::collections::BTreeSet;

use serde_json::{json, Map, Value};

use super::content;
use crate::{error::CommandError, util::validate_entity_id};

const MAX_DOCUMENT_BYTES: usize = 1_000_000;
const MAX_MATH_CHARS: usize = 64 * 1024;
const MAX_MEDIA_TITLE_CHARS: usize = 500;
const MAX_RICH_SCHEMA_VERSION: i64 = 3;

pub(crate) use content::ValidatedContent;

pub(crate) fn validate_content(
    front_doc: Value,
    back_doc: Value,
    front_text_snapshot: String,
    back_text_snapshot: String,
) -> Result<ValidatedContent, CommandError> {
    let front_shadow = validation_shadow(&front_doc)?;
    let back_shadow = validation_shadow(&back_doc)?;
    let mut validated = content::validate_content(
        front_shadow,
        back_shadow,
        front_text_snapshot,
        back_text_snapshot,
    )?;
    let front_json = serde_json::to_string(&front_doc).map_err(CommandError::serialization)?;
    let back_json = serde_json::to_string(&back_doc).map_err(CommandError::serialization)?;
    if front_json.len() > MAX_DOCUMENT_BYTES || back_json.len() > MAX_DOCUMENT_BYTES {
        return Err(CommandError::new(
            "card_content_too_large",
            "Each side of a flashcard must be smaller than 1 MB before managed assets.",
        ));
    }
    validated.front_doc_json = front_json;
    validated.back_doc_json = back_json;
    validated.front_asset_ids = collect_asset_ids(&front_doc)?;
    validated.back_asset_ids = collect_asset_ids(&back_doc)?;
    if validated.front_text.is_empty() && validated.front_asset_ids.is_empty() {
        return Err(CommandError::new(
            "empty_card_front",
            "Add text, math, or media to the question side.",
        ));
    }
    if validated.back_text.is_empty() && validated.back_asset_ids.is_empty() {
        return Err(CommandError::new(
            "empty_card_back",
            "Add text, math, or media to the answer side.",
        ));
    }
    Ok(validated)
}

pub(crate) fn validate_document(document: &Value) -> Result<(), CommandError> {
    content::validate_document(&validation_shadow(document)?)
}

pub(crate) fn collect_asset_ids(document: &Value) -> Result<Vec<String>, CommandError> {
    let mut ids = BTreeSet::new();
    collect_assets(document, &mut ids)?;
    Ok(ids.into_iter().collect())
}

fn collect_assets(value: &Value, ids: &mut BTreeSet<String>) -> Result<(), CommandError> {
    match value {
        Value::Array(values) => {
            for value in values {
                collect_assets(value, ids)?;
            }
        }
        Value::Object(object) => {
            let node_type = object.get("type").and_then(Value::as_str).unwrap_or_default();
            if matches!(node_type, "image" | "audio" | "video") {
                let id = object
                    .get("attrs")
                    .and_then(Value::as_object)
                    .and_then(|attrs| attrs.get("assetId"))
                    .and_then(Value::as_str)
                    .ok_or_else(|| invalid_asset_reference(node_type))?;
                validate_entity_id(id, "asset")?;
                ids.insert(id.to_string());
            }
            for nested in object.values() {
                collect_assets(nested, ids)?;
            }
        }
        _ => {}
    }
    Ok(())
}

fn validation_shadow(document: &Value) -> Result<Value, CommandError> {
    let mut root = document.clone();
    let object = root.as_object_mut().ok_or_else(|| {
        CommandError::new("invalid_card_document", "The card editor content is invalid.")
    })?;
    if object.get("type").and_then(Value::as_str) != Some("doc") {
        return Err(CommandError::new(
            "invalid_card_document",
            "The card editor content is missing its document root.",
        ));
    }
    if let Some(attrs) = object.get_mut("attrs") {
        let attrs = attrs.as_object_mut().ok_or_else(|| {
            CommandError::new(
                "invalid_card_document",
                "Card node attributes must be a JSON object.",
            )
        })?;
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
            attrs.insert("schemaVersion".into(), Value::from(version.min(2)));
        }
    }
    transform_children(&mut root, ParentKind::Block)?;
    Ok(root)
}

#[derive(Clone, Copy)]
enum ParentKind {
    Block,
    Inline,
}

fn transform_children(value: &mut Value, parent: ParentKind) -> Result<(), CommandError> {
    let Some(object) = value.as_object_mut() else {
        return Ok(());
    };
    let node_type = object
        .get("type")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();

    match node_type.as_str() {
        "inlineMath" => {
            if !matches!(parent, ParentKind::Inline) {
                return Err(CommandError::new(
                    "invalid_card_document",
                    "Inline math must appear inside a text block.",
                ));
            }
            let latex = validate_math_node(object, "inlineMath")?;
            *value = json!({"type": "text", "text": latex});
            return Ok(());
        }
        "blockMath" => {
            if !matches!(parent, ParentKind::Block) {
                return Err(CommandError::new(
                    "invalid_card_document",
                    "Block math cannot be nested inside a paragraph.",
                ));
            }
            let latex = validate_math_node(object, "blockMath")?;
            *value = json!({
                "type": "paragraph",
                "content": [{"type": "text", "text": latex}]
            });
            return Ok(());
        }
        "audio" | "video" => {
            if !matches!(parent, ParentKind::Block) {
                return Err(CommandError::new(
                    "invalid_card_document",
                    "Managed audio and video must be block content.",
                ));
            }
            let title = validate_media_node(object, node_type.as_str())?;
            let label = if node_type == "audio" { "Audio" } else { "Video" };
            let text = title
                .filter(|value| !value.trim().is_empty())
                .map(|value| format!("[{label}: {value}]"))
                .unwrap_or_else(|| format!("[{label}]"));
            *value = json!({
                "type": "paragraph",
                "content": [{"type": "text", "text": text}]
            });
            return Ok(());
        }
        _ => {}
    }

    let next_parent = if matches!(node_type.as_str(), "paragraph" | "heading") {
        ParentKind::Inline
    } else {
        ParentKind::Block
    };
    if let Some(children) = object.get_mut("content").and_then(Value::as_array_mut) {
        for child in children {
            transform_children(child, next_parent)?;
        }
    }
    Ok(())
}

fn validate_math_node(
    object: &Map<String, Value>,
    node_type: &str,
) -> Result<String, CommandError> {
    validate_atomic_node_keys(object, node_type)?;
    let attrs = exact_attrs(object, node_type, &["latex"])?;
    let latex = attrs.get("latex").and_then(Value::as_str).ok_or_else(|| {
        CommandError::new("invalid_math", "Every math node must contain LaTeX source.")
    })?;
    let trimmed = latex.trim();
    if trimmed.is_empty()
        || trimmed.chars().count() > MAX_MATH_CHARS
        || trimmed
            .chars()
            .any(|character| character.is_control() && !matches!(character, '\n' | '\t'))
    {
        return Err(CommandError::new(
            "invalid_math",
            "The math source is empty, too large, or contains invalid control characters.",
        ));
    }
    Ok(trimmed.to_string())
}

fn validate_media_node<'a>(
    object: &'a Map<String, Value>,
    node_type: &str,
) -> Result<Option<&'a str>, CommandError> {
    validate_atomic_node_keys(object, node_type)?;
    let allowed = if node_type == "video" {
        &["assetId", "src", "title", "width", "height"][..]
    } else {
        &["assetId", "src", "title"][..]
    };
    let attrs = exact_attrs(object, node_type, allowed)?;
    let asset_id = attrs
        .get("assetId")
        .and_then(Value::as_str)
        .ok_or_else(|| invalid_asset_reference(node_type))?;
    validate_entity_id(asset_id, "asset")?;
    let expected_src = format!("study-asset://{asset_id}");
    if attrs.get("src").and_then(Value::as_str) != Some(expected_src.as_str()) {
        return Err(invalid_asset_reference(node_type));
    }
    let title = match attrs.get("title") {
        None | Some(Value::Null) => None,
        Some(Value::String(value)) => {
            if value.chars().count() > MAX_MEDIA_TITLE_CHARS
                || value.chars().any(char::is_control)
            {
                return Err(CommandError::new(
                    "invalid_media_reference",
                    "The media title is invalid.",
                ));
            }
            Some(value.as_str())
        }
        Some(_) => {
            return Err(CommandError::new(
                "invalid_media_reference",
                "The media title must be text or null.",
            ));
        }
    };
    if node_type == "video" {
        validate_dimension(attrs.get("width"), "width")?;
        validate_dimension(attrs.get("height"), "height")?;
    }
    Ok(title)
}

fn validate_atomic_node_keys(
    object: &Map<String, Value>,
    node_type: &str,
) -> Result<(), CommandError> {
    for key in object.keys() {
        if !matches!(key.as_str(), "type" | "attrs") {
            return Err(CommandError::new(
                "invalid_card_document",
                format!("The {node_type} node contains an unsupported property: {key}."),
            ));
        }
    }
    Ok(())
}

fn exact_attrs<'a>(
    object: &'a Map<String, Value>,
    node_type: &str,
    allowed: &[&str],
) -> Result<&'a Map<String, Value>, CommandError> {
    let attrs = object
        .get("attrs")
        .and_then(Value::as_object)
        .ok_or_else(|| {
            CommandError::new(
                "invalid_card_document",
                format!("The {node_type} node is missing its attributes."),
            )
        })?;
    for key in attrs.keys() {
        if !allowed.contains(&key.as_str()) {
            return Err(CommandError::new(
                "invalid_card_document",
                format!("The {node_type} node contains an unsupported attribute: {key}."),
            ));
        }
    }
    Ok(attrs)
}

fn validate_dimension(value: Option<&Value>, label: &str) -> Result<(), CommandError> {
    let Some(value) = value else {
        return Ok(());
    };
    if value.is_null() {
        return Ok(());
    }
    let dimension = value.as_f64().ok_or_else(|| {
        CommandError::new(
            "invalid_card_style",
            format!("The video {label} must be a number or null."),
        )
    })?;
    if !dimension.is_finite() || !(1.0..=10_000.0).contains(&dimension) {
        return Err(CommandError::new(
            "invalid_card_style",
            format!("The video {label} must be between 1 and 10,000 pixels."),
        ));
    }
    Ok(())
}

fn invalid_asset_reference(node_type: &str) -> CommandError {
    CommandError::new(
        "invalid_media_reference",
        format!("Every {node_type} node must refer to one managed local asset."),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_schema_three_math_and_managed_media() {
        let asset = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
        let front = json!({
            "type": "doc", "attrs": {"schemaVersion": 3}, "content": [
                {"type": "paragraph", "content": [
                    {"type": "text", "text": "Solve "},
                    {"type": "inlineMath", "attrs": {"latex": "x^2=4"}}
                ]},
                {"type": "audio", "attrs": {
                    "assetId": asset, "src": format!("study-asset://{asset}"), "title": "Pronunciation"
                }}
            ]
        });
        let back = json!({
            "type": "doc", "attrs": {"schemaVersion": 3}, "content": [
                {"type": "blockMath", "attrs": {"latex": "x=\\pm2"}}
            ]
        });
        let validated = validate_content(front, back, String::new(), String::new())
            .expect("schema-three content should validate");
        assert!(validated.front_text.contains("x^2=4"));
        assert!(validated.front_text.contains("Audio: Pronunciation"));
        assert!(validated.back_text.contains("x=\\pm2"));
        assert_eq!(validated.front_asset_ids, vec![asset.to_string()]);
    }

    #[test]
    fn rejects_remote_media_and_oversized_math() {
        let asset = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
        let remote = json!({
            "type": "doc", "attrs": {"schemaVersion": 3}, "content": [{
                "type": "video", "attrs": {"assetId": asset, "src": "https://example.org/a.mp4"}
            }]
        });
        assert!(validate_document(&remote).is_err());
        let huge = json!({
            "type": "doc", "attrs": {"schemaVersion": 3}, "content": [{
                "type": "blockMath", "attrs": {"latex": "x".repeat(MAX_MATH_CHARS + 1)}
            }]
        });
        assert!(validate_document(&huge).is_err());
    }
}
