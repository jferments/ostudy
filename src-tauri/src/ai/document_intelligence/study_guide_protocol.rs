//! Request-only guide protocol. Short aliases lower output cost; native unit
//! identities, provenance and durable guide formats are unchanged.
use super::{output_error, study_guide_section_schema};
use crate::{ai::structured_output, error::CommandError, knowledge::authoring::KnowledgeCatalog};
use serde_json::{json, Value};

pub(super) fn alias(index: usize) -> String {
    format!("K{index}")
}

pub(super) fn catalog_prompt(knowledge: &KnowledgeCatalog) -> Result<String, CommandError> {
    let mut value: Value =
        serde_json::from_str(&knowledge.prompt()?).map_err(CommandError::serialization)?;
    let units = value
        .get_mut("units")
        .and_then(Value::as_array_mut)
        .ok_or_else(|| output_error("The native guide catalog is missing its units."))?;
    for (index, unit) in units.iter_mut().enumerate() {
        unit["id"] = json!(alias(index));
    }
    serde_json::to_string(&value).map_err(CommandError::serialization)
}

pub(super) fn schema(knowledge: &KnowledgeCatalog, images_attached: bool) -> Value {
    let mut content = study_guide_section_schema();
    if !images_attached {
        content["properties"]["visualExplanations"]["maxItems"] = json!(0);
        content["properties"]["visualExplanations"]["description"] =
            json!("No images are attached. Always return [].");
    }
    json!({"type":"object","additionalProperties":false,"required":["content","knowledgeUnitIds"],
        "properties":{"content":content,"knowledgeUnitIds":{
            "type":"array","minItems":1,"maxItems":knowledge.units.len().min(64),"uniqueItems":true,
            "items":{"type":"string","enum":(0..knowledge.units.len()).map(alias).collect::<Vec<_>>()}}}})
}

pub(super) fn resolve(
    ids: &[String],
    knowledge: &KnowledgeCatalog,
) -> Result<Vec<String>, CommandError> {
    ids.iter().map(|id| {
        if let Some(index) = id.strip_prefix('K').and_then(|number| number.parse::<usize>().ok()) {
            if alias(index) == *id {
                return knowledge.units.get(index).map(|unit| unit.id.clone())
                    .ok_or_else(|| output_error("The guide selected a knowledge alias outside its supplied catalog."));
            }
        }
        // Exact IDs from older callers remain a lossless compatibility form;
        // native selection below still rejects unknown and duplicate targets.
        if knowledge.units.iter().any(|unit| unit.id == *id) { Ok(id.clone()) }
        else { Err(output_error("The guide selected a knowledge ID outside its supplied catalog.")) }
    }).collect()
}

pub(super) fn normalize_content(mut value: Value) -> Value {
    structured_output::empty_optional_arrays(
        &mut value,
        &[
            "explanations",
            "importantTerms",
            "processSteps",
            "comparisons",
            "commonMisconceptions",
            "callouts",
            "tables",
            "workedExamples",
            "retrievalChecks",
            "visualExplanations",
        ],
    );
    value
}

/// Serialize complete JSON values. Omitted context is explicit and never a
/// clipped sentence, partial object, or malformed string handed to the model.
pub(super) fn bound_context(
    mut value: Value,
    maximum_bytes: usize,
) -> Result<String, CommandError> {
    let original_topics = value["topics"].as_array().map_or(0, Vec::len);
    loop {
        let remaining = value["topics"].as_array().map_or(0, Vec::len);
        value["omittedTopicCount"] = json!(original_topics.saturating_sub(remaining));
        let serialized = serde_json::to_string(&value).map_err(CommandError::serialization)?;
        if serialized.len() <= maximum_bytes {
            return Ok(serialized);
        }
        // The caller placed the active topic first. Remove remote background
        // topics before the active topic's context and document-level metadata.
        if let Some(topics) = value["topics"].as_array_mut() {
            if topics.len() > 1 {
                topics.pop();
                continue;
            }
        }
        let mut trimmed = false;
        for field in ["keyConcepts", "learningObjectives"] {
            if let Some(items) = value[field].as_array_mut() {
                if !items.is_empty() {
                    items.pop();
                    trimmed = true;
                    break;
                }
            }
        }
        if trimmed {
            continue;
        }
        if value.get("summary").is_some() {
            if let Some(object) = value.as_object_mut() {
                object.remove("summary");
            }
            value["summaryOmitted"] = json!(true);
            continue;
        }
        return Err(output_error("The active learning topic exceeds the guide context budget; no source text was truncated."));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn aliases_resolve_to_native_ids_and_do_not_accept_guessing_or_duplicate_targets() {
        let knowledge = crate::knowledge::authoring::tests::catalog_fixture();
        assert_eq!(
            resolve(&["K0".into()], &knowledge).unwrap(),
            vec![knowledge.units[0].id.clone()]
        );
        for id in ["K999", "K00", "K-1", "invented"] {
            assert!(resolve(&[id.into()], &knowledge).is_err());
        }
        let ids = resolve(&["K0".into(), knowledge.units[0].id.clone()], &knowledge).unwrap();
        assert!(knowledge.select(&ids, 64).is_err());
        let value: Value = serde_json::from_str(&catalog_prompt(&knowledge).unwrap()).unwrap();
        assert_eq!(value["units"][0]["id"], "K0");
        assert_eq!(
            schema(&knowledge, false)["properties"]["knowledgeUnitIds"]["items"]["enum"],
            json!(["K0"])
        );
    }
    #[test]
    fn compact_context_stays_valid_json_preserves_active_topic_and_reports_omissions() {
        let topics=(0..60).map(|i|json!({"title":format!("{i}: αβ \\\""),"summary":"Do not remove this qualification. ".repeat(30)})).collect::<Vec<_>>();
        let value = json!({"topics":topics,"summary":"Overview ".repeat(1000),"learningObjectives":["Long objective"],"keyConcepts":[]});
        let compact = bound_context(value, 2200).unwrap();
        assert!(compact.len() <= 2200);
        let parsed: Value = serde_json::from_str(&compact).unwrap();
        assert!(parsed["topics"][0]["title"]
            .as_str()
            .unwrap()
            .starts_with("0:"));
        assert!(parsed["omittedTopicCount"].as_u64().unwrap() > 0);
        assert!(parsed["topics"][0]["summary"]
            .as_str()
            .unwrap()
            .ends_with("qualification. "));
    }
    #[test]
    fn optional_empty_guide_categories_do_not_invent_required_key_points() {
        let value = normalize_content(json!({"overview":"Summary","explanations":null}));
        assert!(value["explanations"].as_array().unwrap().is_empty());
        assert!(value.get("keyPoints").is_none());
    }
}
