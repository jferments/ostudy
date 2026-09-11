//! Bounded syntax recovery shared by content and tool-result transports. This
//! never completes truncated JSON, guesses keys/quotes, or accepts source claims.
use serde_json::{json, Value};

use super::ProviderOutputError;

const MAX_JSON_BYTES: usize = 2 * 1024 * 1024;

pub(super) fn parse(raw: &str) -> Result<Value, ProviderOutputError> {
    if raw.len() > MAX_JSON_BYTES {
        return Err(failure("The structured response exceeded its JSON size limit."));
    }
    let normalized = unwrap_fence(raw);
    let (mut value, repaired) = match serde_json::from_str::<Value>(normalized) {
        Ok(value) => (value, normalized != raw),
        Err(original) => {
            let candidate = repair_syntax(normalized);
            match candidate
                .as_deref()
                .and_then(|text| serde_json::from_str(text).ok())
            {
                Some(value) => (value, true),
                None => {
                    log_failure(raw, &original);
                    return Err(failure("The AI provider returned structured document content that was not valid JSON. The parse category and location are recorded in diagnostics; no incomplete content was accepted."));
                }
            }
        }
    };
    // Some adapters serialize their JSON object twice. Decode exactly one extra
    // string layer, not arbitrary nested wrappers or prose containing braces.
    let mut encoded = false;
    if let Value::String(text) = &value {
        let inner = unwrap_fence(text);
        if inner.starts_with('{') || inner.starts_with('[') {
            value = serde_json::from_str(inner).map_err(|error| {
                log_failure(raw, &error);
                failure("The AI provider returned an invalid JSON-encoded structured result.")
            })?;
            encoded = true;
        }
    }
    if repaired || encoded {
        crate::logging::debug_target(
            "ai.contract",
            "structured_json_normalized",
            "Normalized JSON formatting before unchanged domain validation; no content is logged.",
            json!({"responseBytes":raw.len(),"encodedObject":encoded,
                "responseFingerprint":crate::ai::structured_output::fingerprint(&json!(raw))}),
        );
    }
    Ok(value)
}

fn failure(message: &'static str) -> ProviderOutputError {
    ProviderOutputError { message }
}

fn log_failure(raw: &str, error: &serde_json::Error) {
    crate::logging::warn_target(
        "ai.contract",
        "structured_json_parse_failed",
        "A structured response failed JSON parsing; diagnostics omit response values and parser text.",
        json!({"category":format!("{:?}",error.classify()),"line":error.line(),
            "column":error.column(),"responseBytes":raw.len(),
            "responseFingerprint":crate::ai::structured_output::fingerprint(&json!(raw))}),
    );
}

fn unwrap_fence(raw: &str) -> &str {
    let text = raw.trim().trim_start_matches('\u{feff}').trim();
    let Some((opening, body)) = text.split_once('\n') else {
        return text;
    };
    if !["```", "```json", "```JSON"].contains(&opening.trim()) {
        return text;
    }
    body.trim_end()
        .strip_suffix("```")
        .map(str::trim)
        .unwrap_or(text)
}

/// Only unambiguous lexical errors: trailing commas OUTSIDE strings, literal
/// controls INSIDE strings, and invalid non-Unicode backslash escapes. Preserve
/// every string character; do not infer missing delimiters or damaged Unicode.
fn repair_syntax(text: &str) -> Option<String> {
    let mut result = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    let mut in_string = false;
    let mut changed = false;
    while let Some(character) = chars.next() {
        match character {
            '"' => {
                in_string = !in_string;
                result.push(character);
            }
            '\\' if in_string => {
                result.push('\\');
                let next = chars.next()?;
                if !matches!(next, '"' | '\\' | '/' | 'b' | 'f' | 'n' | 'r' | 't' | 'u') {
                    result.push('\\');
                    changed = true;
                }
                if next.is_control() {
                    result.push_str(&format!("\\u{:04x}", u32::from(next)));
                } else {
                    result.push(next);
                }
            }
            value if in_string && value <= '\u{1f}' => {
                result.push_str(&format!("\\u{:04x}", u32::from(value)));
                changed = true;
            }
            ',' if !in_string => {
                let next = chars.clone().find(|c| !c.is_ascii_whitespace());
                if matches!(next, Some('}' | ']')) {
                    changed = true;
                } else {
                    result.push(',');
                }
            }
            _ => result.push(character),
        }
        if result.len() > MAX_JSON_BYTES {
            return None;
        }
    }
    (changed && !in_string).then_some(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tool_and_content_json_accept_the_same_whole_fences_and_encoded_object() {
        let expected = json!({"units":[],"noLearningSourceIds":[]});
        for text in [
            expected.to_string(),
            format!("\u{feff}```json\n{expected}\n```"),
            serde_json::to_string(&expected.to_string()).unwrap(),
        ] {
            assert_eq!(parse(&text).unwrap(), expected);
        }
    }

    #[test]
    fn formatting_recovery_preserves_text_and_never_edits_inside_string_commas() {
        let raw = "{\"text\":\"Line one\nLine two \\alpha ,] café\",\"ids\":[\"E0\",],}";
        let value = parse(raw).unwrap();
        assert_eq!(value["text"], "Line one\nLine two \\alpha ,] café");
        assert_eq!(value["ids"], json!(["E0"]));
        assert_eq!(
            parse(r#"{"path":"C:\\temp","escaped":"\"","comma":",}"}"#).unwrap()["path"],
            "C:\\temp"
        );
    }

    #[test]
    fn does_not_guess_truncation_quotes_missing_separators_or_unicode() {
        for raw in [
            r#"{"units":["#,
            r#"{"a":"unterminated}"#,
            r#"{"a":1 "b":2}"#,
            r#"{"text":"\uZZZZ"}"#,
            r#"{"a":"first "second" third"}"#,
            "Here is your JSON: {\"a\":1}",
            "{}{}",
            "```json\n{}",
        ] {
            let error = parse(raw).unwrap_err();
            assert!(!error.message.contains(raw));
        }
        assert!(parse(&" ".repeat(MAX_JSON_BYTES + 1)).is_err());
    }
}
