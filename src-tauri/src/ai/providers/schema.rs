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
}
