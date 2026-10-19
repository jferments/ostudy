use serde_json::{json, Value};

/// Lower schema nodes, not arbitrary JSON objects. In particular, property names
/// (including `required` or `const`) and literal enum/default objects are data.
/// Rust remains authoritative for omitted provider-incompatible bounds.
pub(super) fn portable_strict_schema(value: &Value) -> Value {
    let Some(object) = value.as_object() else {
        return value.clone();
    };
    let mut normalized = serde_json::Map::new();
    for (key, child) in object {
        match key.as_str() {
            "minLength" | "maxLength" | "minItems" | "maxItems" | "minimum" | "maximum"
            | "exclusiveMinimum" | "exclusiveMaximum" | "uniqueItems" | "required" => {}
            "properties" | "$defs" | "definitions" => {
                if let Some(entries) = child.as_object() {
                    normalized.insert(
                        key.clone(),
                        Value::Object(
                            entries
                                .iter()
                                .map(|(name, schema)| {
                                    (name.clone(), portable_strict_schema(schema))
                                })
                                .collect(),
                        ),
                    );
                }
            }
            "oneOf" | "anyOf" | "allOf" => {
                if let Some(variants) = child.as_array() {
                    normalized.insert(
                        if key == "oneOf" {
                            "anyOf".into()
                        } else {
                            key.clone()
                        },
                        Value::Array(variants.iter().map(portable_strict_schema).collect()),
                    );
                }
            }
            "items" | "additionalProperties" => {
                normalized.insert(key.clone(), portable_strict_schema(child));
            }
            "const" => {
                normalized.insert("enum".into(), json!([child]));
                if !object.contains_key("type") {
                    let value_type = match child {
                        Value::Null => "null",
                        Value::Bool(_) => "boolean",
                        Value::Number(n) if n.is_i64() || n.is_u64() => "integer",
                        Value::Number(_) => "number",
                        Value::Array(_) => "array",
                        Value::Object(_) => "object",
                        Value::String(_) => "string",
                    };
                    normalized.insert("type".into(), json!(value_type));
                }
            }
            _ => {
                normalized.insert(key.clone(), child.clone());
            }
        }
    }
    if let Some(properties) = normalized.get("properties").and_then(Value::as_object) {
        let required = properties.keys().cloned().map(Value::String).collect();
        normalized.insert("required".into(), Value::Array(required));
        normalized.insert("additionalProperties".into(), Value::Bool(false));
    }
    Value::Object(normalized)
}

/// DeepSeek strict function calling is a smaller JSON-Schema language than the
/// provider-neutral contract. Incompatible contracts must use JSON object mode
/// and keep Rust/native admission authoritative instead of sending a schema the
/// provider will reject before inference begins.
pub(super) fn deepseek_strict_schema_compatible(value: &Value) -> bool {
    deepseek_node_compatible(&portable_strict_schema(value))
}

fn deepseek_node_compatible(value: &Value) -> bool {
    let Some(object) = value.as_object() else {
        return true;
    };
    if let Some(kind) = object.get("type") {
        if !matches!(
            kind.as_str(),
            Some("object" | "string" | "number" | "integer" | "boolean" | "array")
        ) {
            return false;
        }
    }
    if [
        "allOf",
        "not",
        "if",
        "then",
        "else",
        "contains",
        "prefixItems",
        "patternProperties",
        "dependentSchemas",
        "unevaluatedProperties",
    ]
    .iter()
    .any(|key| object.contains_key(*key))
    {
        return false;
    }
    if object
        .get("enum")
        .and_then(Value::as_array)
        .is_some_and(|values| values.iter().any(Value::is_null))
    {
        return false;
    }
    if object
        .get("additionalProperties")
        .is_some_and(|value| value != &Value::Bool(false))
    {
        return false;
    }
    for group in ["properties", "$defs", "definitions"] {
        if object
            .get(group)
            .and_then(Value::as_object)
            .is_some_and(|entries| {
                entries
                    .values()
                    .any(|child| !deepseek_node_compatible(child))
            })
        {
            return false;
        }
    }
    if object
        .get("items")
        .is_some_and(|child| !deepseek_node_compatible(child))
    {
        return false;
    }
    if object
        .get("anyOf")
        .and_then(Value::as_array)
        .is_some_and(|variants| {
            variants
                .iter()
                .any(|child| !deepseek_node_compatible(child))
        })
    {
        return false;
    }
    true
}

/// Anthropic's documented structured-output limits: at most this many
/// parameters may use a union (`anyOf` or a type array) across a request.
const CLAUDE_MAX_UNION_PARAMETERS: usize = 16;

/// Lower a contract to Claude's grammar-constrained JSON-Schema subset, or
/// return `None` when it falls outside that subset. Every portable object
/// already lists all properties as required, so the optional-parameter limit
/// never applies. References are refused outright because Claude rejects
/// recursive schemas, and complex enum values because enums must be scalar.
pub(super) fn claude_strict_schema(value: &Value) -> Option<Value> {
    let schema = portable_strict_schema(value);
    let mut unions = 0_usize;
    claude_node_compatible(&schema, &mut unions)
        .then_some(())
        .filter(|()| unions <= CLAUDE_MAX_UNION_PARAMETERS)
        .map(|()| schema)
}

fn claude_node_compatible(value: &Value, unions: &mut usize) -> bool {
    let Some(object) = value.as_object() else {
        return true;
    };
    if [
        "$ref",
        "not",
        "if",
        "then",
        "else",
        "patternProperties",
        "prefixItems",
        "contains",
    ]
    .iter()
    .any(|key| object.contains_key(*key))
    {
        return false;
    }
    if object.contains_key("anyOf") || object.get("type").is_some_and(Value::is_array) {
        *unions = unions.saturating_add(1);
    }
    if object
        .get("enum")
        .and_then(Value::as_array)
        .is_some_and(|values| {
            values
                .iter()
                .any(|value| value.is_object() || value.is_array())
        })
    {
        return false;
    }
    if object
        .get("additionalProperties")
        .is_some_and(|value| value != &Value::Bool(false))
    {
        return false;
    }
    for group in ["properties", "$defs", "definitions"] {
        if object
            .get(group)
            .and_then(Value::as_object)
            .is_some_and(|entries| {
                entries
                    .values()
                    .any(|child| !claude_node_compatible(child, unions))
            })
        {
            return false;
        }
    }
    if object
        .get("items")
        .is_some_and(|child| !claude_node_compatible(child, unions))
    {
        return false;
    }
    for group in ["anyOf", "allOf"] {
        if object
            .get(group)
            .and_then(Value::as_array)
            .is_some_and(|variants| {
                variants
                    .iter()
                    .any(|child| !claude_node_compatible(child, unions))
            })
        {
            return false;
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn portable_strict_schema_normalizes_union_constants_and_object_requirements() {
        let schema = portable_strict_schema(&json!({"type":"object","properties":{"cards":{
            "type":"array","maxItems":4,"items":{"oneOf":[{"type":"object","properties":{
                "type":{"const":"basic"},"front":{"type":"string","maxLength":100},
                "tags":{"type":"array","maxItems":4}},"required":["type","front"]}]} }},"required":["cards"]}));
        assert!(schema.pointer("/properties/cards/items/oneOf").is_none());
        assert!(schema.pointer("/properties/cards/items/anyOf").is_some());
        assert!(schema.pointer("/properties/cards/maxItems").is_none());
        let required = schema
            .pointer("/properties/cards/items/anyOf/0/required")
            .and_then(Value::as_array)
            .unwrap();
        for property in ["front", "tags", "type"] {
            assert!(required.contains(&json!(property)));
        }
        assert_eq!(
            schema.pointer("/properties/cards/items/anyOf/0/properties/type/enum"),
            Some(&json!(["basic"]))
        );
    }
    #[test]
    fn keyword_named_properties_and_literal_objects_are_not_rewritten() {
        let literal = json!({"const":"do not rewrite","required":["x"],"minItems":2});
        let schema = portable_strict_schema(&json!({"type":"object","properties":{
            "const":{"type":"string"},"required":{"type":"array","items":{"type":"string"}},
            "minItems":{"enum":[literal.clone()]},"nullable":{"const":null}}}));
        for name in ["const", "required", "minItems", "nullable"] {
            assert!(schema["properties"].get(name).is_some());
        }
        assert_eq!(schema["properties"]["minItems"]["enum"], json!([literal]));
        assert_eq!(schema["properties"]["nullable"]["enum"], json!([null]));
    }
    #[test]
    fn deepseek_strict_compatibility_rejects_nullable_and_composition_not_supported_by_server() {
        assert!(deepseek_strict_schema_compatible(&json!({
            "type":"object","properties":{"name":{"type":"string"}},"required":["name"]
        })));
        assert!(!deepseek_strict_schema_compatible(&json!({
            "type":"object","properties":{"explanation":{"type":["string","null"]}}
        })));
        assert!(!deepseek_strict_schema_compatible(&json!({
            "allOf":[{"type":"object","properties":{}}]
        })));
        assert!(!deepseek_strict_schema_compatible(&json!({
            "type":"object","properties":{"value":{"const":null}}
        })));
    }
    #[test]
    fn claude_strict_schema_accepts_portable_contracts_and_refuses_its_unsupported_subset() {
        let schema = claude_strict_schema(&json!({
            "type":"object","properties":{"name":{"type":"string","maxLength":20},
            "kind":{"oneOf":[{"const":"a"},{"const":"b"}]}},"required":["name"]
        }))
        .expect("simple contracts are enforced");
        assert!(schema.pointer("/properties/name/maxLength").is_none());
        assert_eq!(schema["additionalProperties"], json!(false));
        assert!(claude_strict_schema(&json!({
            "type":"object","properties":{"node":{"$ref":"#"}}
        }))
        .is_none());
        assert!(claude_strict_schema(&json!({
            "type":"object","properties":{"value":{"const":{"nested":true}}}
        }))
        .is_none());
        let many_unions = (0..17)
            .map(|index| (format!("p{index}"), json!({"type":["string","null"]})))
            .collect::<serde_json::Map<_, _>>();
        assert!(claude_strict_schema(&json!({"type":"object","properties":many_unions})).is_none());
    }
}
