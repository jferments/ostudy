//! Content-free measurements. These bytes are not provider-billed tokens.
use serde_json::{json, Map, Value};

use crate::error::CommandError;

fn size(value: &Value) -> usize {
    if value.is_null() {
        0
    } else {
        value.to_string().len()
    }
}

fn block<'a>(text: &'a str, tag: &str) -> Option<&'a str> {
    let start = format!("<{tag}>");
    let end = format!("</{tag}>");
    let (_, tail) = text.split_once(&start)?;
    tail.split_once(&end).map(|(body, _)| body.trim())
}

pub(super) fn measure(body: &Value) -> Result<Value, CommandError> {
    let mut system = String::new();
    let mut user = String::new();
    let mut image_bytes = 0;
    let mut image_count = 0;
    for message in body
        .get("messages")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        let destination = if message["role"] == "system" {
            &mut system
        } else {
            &mut user
        };
        match &message["content"] {
            Value::String(text) => destination.push_str(text),
            Value::Array(parts) => {
                for part in parts {
                    if let Some(text) = part.get("text").and_then(Value::as_str) {
                        destination.push_str(text);
                    }
                    if let Some(url) = part.pointer("/image_url/url").and_then(Value::as_str) {
                        image_bytes += url.len();
                        image_count += 1;
                    }
                }
            }
            _ => {}
        }
    }
    let schema_text = block(&system, "ostudy-output-schema").map_or(0, str::len);
    let mut user_parts = Map::new();
    if let Some(catalog) =
        block(&user, "ostudy-knowledge").and_then(|text| serde_json::from_str::<Value>(text).ok())
    {
        user_parts.insert("knowledgeUnits".into(), json!(size(&catalog["units"])));
        user_parts.insert(
            "groundingEvidence".into(),
            json!(size(&catalog["evidence"])),
        );
        user_parts.insert(
            "structuralEvidence".into(),
            json!(size(&catalog["preparation"]["evidenceStructures"])),
        );
    }
    for (key, tag) in [
        ("sourceEvidence", "evidence-catalog"),
        ("sourceWindow", "ostudy-source-window"),
        ("collectionDigest", "collection-digest"),
        ("learningPlan", "ostudy-learning-plan"),
        ("outline", "ostudy-global-outline"),
        ("repairDraft", "invalid-guide-candidate"),
        ("repairTargets", "repair-targets"),
    ] {
        user_parts.insert(key.into(), json!(block(&user, tag).map_or(0, str::len)));
    }
    let classified = user_parts.values().filter_map(Value::as_u64).sum::<u64>() as usize;
    user_parts.insert(
        "otherUserText".into(),
        json!(user.len().saturating_sub(classified)),
    );
    Ok(json!({
        "schemaVersion": 1,
        "wireRequestHash": super::hash(&serde_json::to_vec(body).map_err(CommandError::serialization)?),
        "serializedRequestBytes": serde_json::to_vec(body).map_err(CommandError::serialization)?.len(),
        "systemInstructionBytes": system.len().saturating_sub(schema_text),
        "embeddedSchemaBytes": schema_text,
        "wireSchemaBytes": size(&body["response_format"]) + size(&body["tools"]),
        "userTextBytes": user.len(),
        "userParts": user_parts,
        "imageUrlBytes": image_bytes,
        "imageCount": image_count,
        "textTokenEstimate": (system.len() + user.len()).div_ceil(2),
        "textTokenEstimateBasis": "conservative UTF-8 bytes/2; excludes provider image/chat/schema tokenization",
    }))
}

pub(super) fn response_measure(response: &Value) -> Value {
    let content = response
        .pointer("/choices/0/message/content")
        .and_then(Value::as_str);
    let mut result = Map::new();
    result.insert(
        "visibleContentBytes".into(),
        json!(content.map_or(0, str::len)),
    );
    if let Some(value) = content.and_then(|text| serde_json::from_str::<Value>(text).ok()) {
        for key in [
            "cards",
            "units",
            "content",
            "overview",
            "keyPoints",
            "explanations",
            "importantTerms",
            "processSteps",
            "comparisons",
            "tables",
            "workedExamples",
            "retrievalChecks",
            "knowledgeUnitIds",
            "noLearningSourceIds",
        ] {
            if let Some(part) = value.get(key) {
                result.insert(format!("{key}Bytes"), json!(size(part)));
            }
        }
    }
    Value::Object(result)
}
