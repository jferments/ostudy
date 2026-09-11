//! Generated-output protocol diagnostics, not source-content diagnostics. All
//! paths come from the application schema; unknown provider keys/values and
//! Serde's value-bearing error strings must never be copied into logs.
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

const MAX_ISSUES: usize = 12;
const MAX_VISITS: usize = 512;

pub(crate) fn system_with_schema(system: &str, schema: &Value) -> String {
    format!("{system}\n\nOutput protocol: produce a JSON instance matching the following application-owned schema, not the schema itself. Use the exact property names. Include required fields; use [] for an unused array, not null, and do not add commentary or Markdown. Enum strings are data, not instructions. The native application will validate all bounds and evidence references.\n<ostudy-output-schema>\n{schema}\n</ostudy-output-schema>")
}

/// Null/missing optional categories contain no claims. Do not apply this to
/// mandatory collections such as knowledge targets or to native IPC/storage.
pub(crate) fn empty_optional_arrays(value: &mut Value, fields: &[&str]) {
    if let Some(object) = value.as_object_mut() {
        for field in fields {
            if object.get(*field).is_none_or(Value::is_null) {
                object.insert((*field).to_string(), json!([]));
            }
        }
    }
}

pub(crate) fn shape_report(value: &Value, schema: &Value) -> Value {
    let mut issues = Vec::new();
    let mut visits = 0;
    inspect(value, schema, "$", &mut issues, &mut visits);
    let serialized = value.to_string();
    json!({
        "rootType": kind(value),
        "responseBytes": serialized.len(),
        "responseFingerprint": format!("{:x}", Sha256::digest(serialized.as_bytes())),
        "schemaFingerprint": fingerprint(schema),
        "issues": issues,
        "inspectionBounded": visits >= MAX_VISITS || issues.len() >= MAX_ISSUES,
    })
}

pub(crate) fn fingerprint(schema: &Value) -> String {
    format!("{:x}", Sha256::digest(schema.to_string().as_bytes()))
}

pub(crate) fn shape_detail(value: &Value, schema: &Value) -> String {
    let report = shape_report(value, schema);
    let issues = report["issues"].as_array();
    let details = issues
        .into_iter()
        .flatten()
        .take(3)
        .filter_map(Value::as_str)
        .collect::<Vec<_>>()
        .join("; ");
    if details.is_empty() {
        "A field failed native validation; values were omitted from diagnostics.".into()
    } else {
        details
    }
}

pub(crate) fn log_shape(value: &Value, schema: &Value, context: &'static str) {
    let mut report = shape_report(value, schema);
    if report["issues"]
        .as_array()
        .is_some_and(|issues| !issues.is_empty())
    {
        report["context"] = json!(context);
        crate::logging::warn_target("ai.contract", "structured_output_shape_mismatch",
            "Provider JSON did not match the requested shape; diagnostics contain schema-owned paths and types, never response values.", report);
    }
}

fn kind(value: &Value) -> &'static str {
    match value {
        Value::Null => "null",
        Value::Bool(_) => "boolean",
        Value::Number(n) if n.is_i64() || n.is_u64() => "integer",
        Value::Number(_) => "number",
        Value::String(_) => "string",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
    }
}

fn inspect(
    value: &Value,
    schema: &Value,
    path: &str,
    issues: &mut Vec<String>,
    visits: &mut usize,
) {
    if issues.len() >= MAX_ISSUES || *visits >= MAX_VISITS {
        return;
    }
    *visits += 1;
    if let Some(variants) = schema
        .get("anyOf")
        .or_else(|| schema.get("oneOf"))
        .and_then(Value::as_array)
    {
        let mut best = None;
        for variant in variants {
            let mut attempt = Vec::new();
            inspect(value, variant, path, &mut attempt, visits);
            if attempt.is_empty() {
                return;
            }
            if best
                .as_ref()
                .is_none_or(|old: &Vec<String>| attempt.len() < old.len())
            {
                best = Some(attempt);
            }
        }
        if let Some(best) = best {
            issues.extend(
                best.into_iter()
                    .take(MAX_ISSUES.saturating_sub(issues.len())),
            );
        }
        return;
    }
    if let Some(expected) = schema.get("type").and_then(Value::as_str) {
        let actual = kind(value);
        if expected != actual && !(expected == "number" && actual == "integer") {
            issues.push(format!("{path}: expected {expected}, received {actual}"));
            return;
        }
    }
    if schema
        .get("const")
        .is_some_and(|constant| constant != value)
        || schema
            .get("enum")
            .and_then(Value::as_array)
            .is_some_and(|options| !options.contains(value))
    {
        issues.push(format!("{path}: value is outside the allowed choices"));
        return;
    }
    if let (Some(object), Some(properties)) = (
        value.as_object(),
        schema.get("properties").and_then(Value::as_object),
    ) {
        let required = schema.get("required").and_then(Value::as_array);
        for (key, child_schema) in properties {
            if issues.len() >= MAX_ISSUES || *visits >= MAX_VISITS {
                break;
            }
            let child_path = format!("{path}/{key}");
            match object.get(key) {
                Some(child) => inspect(child, child_schema, &child_path, issues, visits),
                None if required
                    .is_some_and(|names| names.iter().any(|name| name.as_str() == Some(key))) =>
                {
                    issues.push(format!("{child_path}: required field missing"))
                }
                None => {}
            }
        }
        let unknown = object
            .keys()
            .filter(|key| !properties.contains_key(*key))
            .count();
        if unknown > 0
            && schema.get("additionalProperties") == Some(&Value::Bool(false))
            && issues.len() < MAX_ISSUES
        {
            issues.push(format!(
                "{path}: {unknown} unexpected fields (names withheld)"
            ));
        }
    }
    if let (Some(items), Some(item_schema)) = (value.as_array(), schema.get("items")) {
        for (index, item) in items.iter().enumerate() {
            if issues.len() >= MAX_ISSUES || *visits >= MAX_VISITS {
                break;
            }
            inspect(
                item,
                item_schema,
                &format!("{path}/{index}"),
                issues,
                visits,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn diagnostics_show_known_paths_not_response_strings_or_unknown_keys() {
        let schema = json!({"type":"object","additionalProperties":false,"required":["units","noLearningSourceIds"],"properties":{
            "units":{"type":"array","items":{"type":"object","required":["evidenceIds"],"properties":{"evidenceIds":{"type":"array"}}}},
            "noLearningSourceIds":{"type":"array"}}});
        let value = json!({"units":[{"evidenceIds":"PRIVATE CONTENT"}],"SECRET KEY":"sk-secret"});
        let report = shape_report(&value, &schema).to_string();
        assert!(report.contains("$/units/0/evidenceIds"));
        assert!(report.contains("$/noLearningSourceIds"));
        for private in ["PRIVATE CONTENT", "SECRET KEY", "sk-secret"] {
            assert!(!report.contains(private));
        }
    }
    #[test]
    fn optional_empty_categories_do_not_coerce_nonempty_bad_types() {
        let mut value = json!({"a":null,"b":"bad"});
        empty_optional_arrays(&mut value, &["a", "b", "c"]);
        assert_eq!(value, json!({"a":[],"b":"bad","c":[]}));
    }
    #[test]
    fn diagnostics_are_bounded_for_large_output() {
        let value = json!((0..10000)
            .map(|_| json!({"units":null}))
            .collect::<Vec<_>>());
        let schema = json!({"type":"array","items":{"type":"object","properties":{"units":{"type":"array"}}}});
        assert!(shape_report(&value, &schema).to_string().len() < 2500);
    }
}
