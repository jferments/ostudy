use serde_json::{json, Value};

/// Lower ostudy's richer validation schema to the conservative strict subset
/// shared by remote OpenAI-compatible providers that validate schemas server-side.
///
/// Local Rust validation remains authoritative for length/item/range bounds.
/// The wire schema keeps deterministic shape: unions use `anyOf`, constants
/// become one-value enums, every object property is required, and additional
/// properties are forbidden.
pub(super) fn portable_strict_schema(value: &Value) -> Value {
    match value {
        Value::Array(values) => {
            Value::Array(values.iter().map(portable_strict_schema).collect())
        }
        Value::Object(object) => {
            if let Some(constant) = object.get("const") {
                let value_type = match constant {
                    Value::String(_) => "string",
                    Value::Number(number) if number.is_i64() || number.is_u64() => "integer",
                    Value::Number(_) => "number",
                    Value::Bool(_) => "boolean",
                    _ => "string",
                };
                return json!({
                    "type": value_type,
                    "enum": [constant.clone()]
                });
            }

            let mut normalized = serde_json::Map::new();
            for (key, child) in object {
                match key.as_str() {
                    "minLength" | "maxLength" | "minItems" | "maxItems" | "required" => {}
                    "oneOf" => {
                        normalized.insert(
                            "anyOf".to_string(),
                            portable_strict_schema(child),
                        );
                    }
                    _ => {
                        normalized.insert(key.clone(), portable_strict_schema(child));
                    }
                }
            }
            if let Some(properties) = normalized.get("properties").and_then(Value::as_object) {
                let required = properties
                    .keys()
                    .cloned()
                    .map(Value::String)
                    .collect::<Vec<_>>();
                normalized.insert("required".to_string(), Value::Array(required));
                normalized.insert("additionalProperties".to_string(), Value::Bool(false));
            }
            Value::Object(normalized)
        }
        _ => value.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn portable_strict_schema_normalizes_union_constants_and_object_requirements() {
        let schema = portable_strict_schema(&json!({
            "type": "object",
            "properties": {
                "cards": {
                    "type": "array",
                    "maxItems": 4,
                    "items": {
                        "oneOf": [{
                            "type": "object",
                            "properties": {
                                "type": {"const": "basic"},
                                "front": {"type": "string", "maxLength": 100},
                                "tags": {"type": "array", "maxItems": 4}
                            },
                            "required": ["type", "front"]
                        }]
                    }
                }
            },
            "required": ["cards"]
        }));
        assert!(schema.pointer("/properties/cards/items/oneOf").is_none());
        assert!(schema.pointer("/properties/cards/items/anyOf").is_some());
        assert!(schema.pointer("/properties/cards/maxItems").is_none());
        let required = schema
            .pointer("/properties/cards/items/anyOf/0/required")
            .and_then(Value::as_array)
            .expect("strict object schema should require every declared property");
        for property in ["front", "tags", "type"] {
            assert!(required.contains(&json!(property)));
        }
        assert_eq!(
            schema.pointer("/properties/cards/items/anyOf/0/properties/type/enum"),
            Some(&json!(["basic"]))
        );
    }
}
