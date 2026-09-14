//! Schema-3 admission around the current rich-document validator.
//!
//! Existing schema-1/2 documents use the original validator unchanged. New
//! atoms are checked structurally before a validation-only projection is passed
//! through the same table/checklist/style rules. Only the ORIGINAL document is
//! stored. The projection is never sent to the editor or used as a migration.

use std::collections::BTreeSet;

use serde_json::{json, Map, Value};

use super::{content, math::validate_latex};
use crate::{error::CommandError, util::validate_entity_id};

pub(crate) use content::ValidatedContent;
const MAX_BYTES: usize = 1_000_000;
const MAX_VALUES: usize = 100_000;
const MAX_DEPTH: usize = 128;

pub(crate) fn validate_content(
    front: Value,
    back: Value,
    front_snapshot: String,
    back_snapshot: String,
) -> Result<ValidatedContent, CommandError> {
    let front_json = bounded_json(&front)?;
    let back_json = bounded_json(&back)?;
    let mut result = content::validate_content(
        validation_projection(&front)?,
        validation_projection(&back)?,
        front_snapshot,
        back_snapshot,
    )?;
    result.front_doc_json = front_json;
    result.back_doc_json = back_json;
    result.front_asset_ids = collect_asset_ids(&front)?;
    result.back_asset_ids = collect_asset_ids(&back)?;
    Ok(result)
}

pub(crate) fn validate_document(document: &Value) -> Result<(), CommandError> {
    bounded_json(document)?;
    content::validate_document(&validation_projection(document)?)
}

/// This collector is also used on template/draft graphs, before their complete
/// content validation. Do not impose ordinary-card structure on those callers.
pub(crate) fn collect_asset_ids(document: &Value) -> Result<Vec<String>, CommandError> {
    let mut ids = BTreeSet::new();
    visit_bounded(document, |value| {
        if matches!(
            value.get("type").and_then(Value::as_str),
            Some("image" | "audio" | "video")
        ) {
            let id = value
                .get("attrs")
                .and_then(|attrs| attrs.get("assetId"))
                .and_then(Value::as_str)
                .ok_or_else(|| invalid("A managed asset has no identity."))?;
            validate_entity_id(id, "asset")?;
            ids.insert(id.to_string());
        }
        Ok(())
    })?;
    Ok(ids.into_iter().collect())
}

fn bounded_json(document: &Value) -> Result<String, CommandError> {
    visit_bounded(document, |_| Ok(()))?;
    let bytes = serde_json::to_string(document).map_err(CommandError::serialization)?;
    if bytes.len() > MAX_BYTES {
        return Err(CommandError::new(
            "card_content_too_large",
            "Each side must be smaller than 1 MB before managed assets.",
        ));
    }
    Ok(bytes)
}

fn visit_bounded(
    document: &Value,
    mut visitor: impl FnMut(&Value) -> Result<(), CommandError>,
) -> Result<(), CommandError> {
    let mut stack = vec![(document, 0_usize)];
    let mut count = 0_usize;
    while let Some((value, depth)) = stack.pop() {
        count += 1;
        // Count JSON containers as well as nodes. Preserve the existing
        // 128-node content depth, which the current validator still checks.
        if depth > MAX_DEPTH * 3 + 8 || count > MAX_VALUES {
            return Err(invalid(
                "The rich document exceeds its depth or node budget.",
            ));
        }
        visitor(value)?;
        match value {
            Value::String(text) if text.len() > MAX_BYTES => {
                return Err(invalid("A rich-document string exceeds its byte budget."));
            }
            Value::Array(values) => {
                if values.len() > MAX_VALUES.saturating_sub(count + stack.len()) {
                    return Err(invalid("The rich document contains too many values."));
                }
                stack.extend(values.iter().map(|value| (value, depth + 1)));
            }
            Value::Object(object) => {
                if object.len() > MAX_VALUES.saturating_sub(count + stack.len()) {
                    return Err(invalid("The rich document contains too many values."));
                }
                stack.extend(object.values().map(|value| (value, depth + 1)));
            }
            _ => {}
        }
    }
    Ok(())
}

fn validation_projection(document: &Value) -> Result<Value, CommandError> {
    let version = match document
        .get("attrs")
        .and_then(|attrs| attrs.get("schemaVersion"))
    {
        None => 1,
        Some(value) => value
            .as_i64()
            .ok_or_else(|| invalid("The rich-document version must be an integer."))?,
    };
    if !(1..=3).contains(&version) {
        return Err(CommandError::new(
            "unsupported_card_content",
            "This rich-document version is not supported. Update oStudy before editing it.",
        ));
    }
    let mut projection = document.clone();
    // The old validator still validates root keys, attribute object shape,
    // ordinary node structure and every existing checklist/table/style rule.
    if version == 3 {
        projection["attrs"]["schemaVersion"] = Value::from(2);
    }
    project(&mut projection, "", 0, version)?;
    Ok(projection)
}

fn project(
    value: &mut Value,
    parent: &str,
    index: usize,
    version: i64,
) -> Result<(), CommandError> {
    let Some(object) = value.as_object() else {
        return Ok(());
    };
    let kind = object
        .get("type")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    if matches!(
        kind.as_str(),
        "inlineMath" | "blockMath" | "audio" | "video"
    ) {
        if version != 3
            || object
                .keys()
                .any(|key| !matches!(key.as_str(), "type" | "attrs"))
        {
            return Err(invalid(
                "Math and media atoms require rich schema 3 and only type/attrs properties.",
            ));
        }
        let inline = kind == "inlineMath";
        let allowed_parent = if inline {
            matches!(parent, "paragraph" | "heading")
        } else {
            matches!(parent, "doc" | "blockquote" | "tableHeader" | "tableCell")
                || (matches!(parent, "listItem" | "taskItem") && index > 0)
        };
        if !allowed_parent {
            return Err(invalid(
                "Math or media is not valid at this document position.",
            ));
        }
        let attrs = object
            .get("attrs")
            .and_then(Value::as_object)
            .ok_or_else(|| invalid("A math/media atom must have attributes."))?;
        let text = if matches!(kind.as_str(), "inlineMath" | "blockMath") {
            exact_attributes(attrs, &["latex"])?;
            let latex = attrs
                .get("latex")
                .and_then(Value::as_str)
                .ok_or_else(|| invalid("A math node must contain LaTeX source."))?;
            validate_latex(latex).map_err(|message| CommandError::new("invalid_math", message))?;
            if inline {
                format!("\\({latex}\\)")
            } else {
                format!("\\[{latex}\\]")
            }
        } else {
            validate_media(attrs, &kind)?;
            // Filenames/titles can reveal an answer. They are not inserted
            // implicitly into visible or evidence text by this projection.
            if kind == "audio" {
                "[Audio]".into()
            } else {
                "[Video]".into()
            }
        };
        *value = if inline {
            json!({"type": "text", "text": text})
        } else {
            json!({"type": "paragraph", "content": [{"type": "text", "text": text}]})
        };
        return Ok(());
    }
    if let Some(children) = value.get_mut("content").and_then(Value::as_array_mut) {
        for (index, child) in children.iter_mut().enumerate() {
            project(child, &kind, index, version)?;
        }
    }
    Ok(())
}

fn exact_attributes(attrs: &Map<String, Value>, allowed: &[&str]) -> Result<(), CommandError> {
    if attrs.keys().any(|key| !allowed.contains(&key.as_str())) {
        return Err(invalid(
            "The math/media node contains unsupported attributes.",
        ));
    }
    Ok(())
}

fn validate_media(attrs: &Map<String, Value>, kind: &str) -> Result<(), CommandError> {
    let allowed = if kind == "video" {
        &["assetId", "src", "title", "width", "height"][..]
    } else {
        &["assetId", "src", "title"][..]
    };
    exact_attributes(attrs, allowed)?;
    let id = attrs
        .get("assetId")
        .and_then(Value::as_str)
        .ok_or_else(|| invalid("A media node must identify a managed local asset."))?;
    validate_entity_id(id, "asset")?;
    let expected_src = format!("study-asset://{id}");
    if attrs.get("src").and_then(Value::as_str) != Some(expected_src.as_str()) {
        return Err(invalid(
            "Media cannot reference remote URLs or local paths.",
        ));
    }
    match attrs.get("title") {
        None | Some(Value::Null) => {}
        Some(Value::String(title))
            if title.chars().count() <= 500 && !title.chars().any(char::is_control) => {}
        _ => return Err(invalid("Media title must be bounded plain text.")),
    }
    for key in ["width", "height"] {
        if let Some(value) = attrs.get(key).filter(|value| !value.is_null()) {
            if !value
                .as_f64()
                .is_some_and(|n| n.is_finite() && (1.0..=10_000.0).contains(&n))
            {
                return Err(invalid(
                    "Video dimensions must be between 1 and 10,000 pixels.",
                ));
            }
        }
    }
    Ok(())
}

fn invalid(message: &str) -> CommandError {
    CommandError::new("invalid_card_document", message)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn doc(content: Value) -> Value {
        json!({"type":"doc", "attrs":{"schemaVersion":3}, "content":content})
    }

    #[test]
    fn retains_atoms_but_derives_bounded_public_text_and_references() {
        let id = "a".repeat(32);
        let front = doc(json!([{
            "type":"audio",
            "attrs":{"assetId":id,"src":format!("study-asset://{id}"),"title":"secret answer"}
        }]));
        let back = doc(json!([{ "type":"blockMath", "attrs":{"latex":"x=2"} }]));
        let result = validate_content(front.clone(), back, String::new(), String::new()).unwrap();
        assert_eq!(
            serde_json::from_str::<Value>(&result.front_doc_json).unwrap(),
            front
        );
        assert_eq!(result.front_text, "[Audio]");
        assert!(!result.front_text.contains("secret"));
        assert_eq!(result.front_asset_ids, vec![id]);
        assert_eq!(result.back_text, r"\[x=2\]");
    }

    #[test]
    fn rejects_legacy_mislabeling_hidden_children_and_wrong_parent_shape() {
        let atom = json!({"type":"inlineMath","attrs":{"latex":"x"}});
        let mut valid = doc(json!([{ "type":"paragraph", "content":[atom.clone()] }]));
        validate_document(&valid).unwrap();
        valid["attrs"]["schemaVersion"] = json!(2);
        assert!(validate_document(&valid).is_err());
        assert!(validate_document(&doc(json!([atom]))).is_err());
        let nested = doc(json!([{
            "type":"blockMath", "attrs":{"latex":"x"}, "content":[]
        }]));
        assert!(validate_document(&nested).is_err());
        let code = doc(json!([{
            "type":"codeBlock", "content":[{"type":"inlineMath", "attrs":{"latex":"x"}}]
        }]));
        assert!(validate_document(&code).is_err());
    }

    #[test]
    fn retains_current_table_and_checklist_validation() {
        let invalid = doc(json!([{
            "type":"table", "content":[{"type":"paragraph"}]
        }]));
        assert!(validate_document(&invalid).is_err());
        let legacy = json!({
            "type":"doc", "content":[{
                "type":"paragraph", "content":[{"type":"text","text":"before"}]
            }]
        });
        validate_document(&legacy).unwrap();
        let mut future = legacy;
        future["attrs"] = json!({"schemaVersion":4});
        assert!(validate_document(&future).is_err());
    }
}
