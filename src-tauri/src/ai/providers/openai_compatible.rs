use serde_json::{json, Value};

use crate::ai::gateway::{error, normalized_usage, AiGatewayError};

use super::CompletionOutcome;

pub(super) fn chat_body(
    model_id: &str,
    messages: Value,
    temperature: f64,
    max_tokens: usize,
) -> Value {
    let temperature = if temperature == 0.0 {
        json!(0)
    } else {
        json!(temperature)
    };
    json!({
        "model": model_id,
        "messages": messages,
        "temperature": temperature,
        "max_tokens": max_tokens,
        "stream": false,
    })
}

pub(super) fn set_json_object(body: &mut Value) {
    body["response_format"] = json!({ "type": "json_object" });
}

pub(super) fn set_json_object_schema(body: &mut Value, schema: Value) {
    body["response_format"] = json!({
        "type": "json_object",
        "schema": schema,
    });
}

pub(super) fn set_json_schema(body: &mut Value, name: &str, schema: Value) {
    body["response_format"] = json!({
        "type": "json_schema",
        "json_schema": {
            "name": name,
            "strict": true,
            "schema": schema,
        }
    });
}

pub(super) fn parse_completion(value: &Value) -> Result<CompletionOutcome, AiGatewayError> {
    let choice = value
        .get("choices")
        .and_then(Value::as_array)
        .and_then(|choices| choices.first())
        .ok_or_else(|| {
            error(
                "ai_empty_response",
                "The provider returned no completion choice.",
                false,
                None,
            )
        })?;
    let content = choice
        .get("message")
        .and_then(|message| message.get("content"))
        .and_then(Value::as_str)
        .ok_or_else(|| {
            error(
                "ai_empty_response",
                "The provider returned no final text content.",
                false,
                None,
            )
        })?;
    if content.is_empty() {
        return Err(error(
            "ai_empty_response",
            "The provider returned empty final text content.",
            false,
            None,
        ));
    }
    let finish_reason = choice
        .get("finish_reason")
        .and_then(Value::as_str)
        .map(ToString::to_string);
    let usage = normalized_usage(value);
    Ok(CompletionOutcome {
        content: content.to_string(),
        finish_reason,
        prompt_tokens: usage
            .as_ref()
            .and_then(|usage| usage.get("prompt_tokens"))
            .and_then(nonnegative_i64),
        completion_tokens: usage
            .as_ref()
            .and_then(|usage| usage.get("completion_tokens"))
            .and_then(nonnegative_i64),
    })
}

pub(super) fn completion_finish_reason(value: &Value) -> Option<String> {
    value
        .pointer("/choices/0/finish_reason")
        .and_then(Value::as_str)
        .map(ToString::to_string)
}

pub(super) fn completion_content(value: &Value) -> Option<&str> {
    value
        .pointer("/choices/0/message/content")
        .and_then(Value::as_str)
        .or_else(|| value.pointer("/choices/0/text").and_then(Value::as_str))
        .filter(|content| !content.trim().is_empty())
}

pub(super) fn strict_tool_arguments<'a>(
    value: &'a Value,
    expected_name: &str,
) -> Option<&'a str> {
    let function = value.pointer("/choices/0/message/tool_calls/0/function")?;
    if function.get("name").and_then(Value::as_str) != Some(expected_name) {
        return None;
    }
    function
        .get("arguments")
        .and_then(Value::as_str)
        .filter(|arguments| !arguments.trim().is_empty())
}

pub(super) fn normalize_json_content(value: &str) -> &str {
    let trimmed = value.trim();
    let without_open = trimmed
        .strip_prefix("```json")
        .or_else(|| trimmed.strip_prefix("```JSON"))
        .or_else(|| trimmed.strip_prefix("```"))
        .unwrap_or(trimmed)
        .trim();
    without_open
        .strip_suffix("```")
        .unwrap_or(without_open)
        .trim()
}

fn nonnegative_i64(value: &Value) -> Option<i64> {
    value
        .as_i64()
        .filter(|value| *value >= 0)
        .or_else(|| {
            value
                .as_u64()
                .map(|value| i64::try_from(value).unwrap_or(i64::MAX))
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn common_completion_parser_accepts_openai_compatible_shape() {
        let outcome = parse_completion(&json!({
            "choices": [{
                "message": {"content": "OSTUDY_OK"},
                "finish_reason": "length"
            }],
            "usage": {"input_tokens": 5, "output_tokens": 3}
        }))
        .expect("completion should parse");
        assert_eq!(outcome.content, "OSTUDY_OK");
        assert_eq!(outcome.finish_reason.as_deref(), Some("length"));
        assert_eq!(outcome.prompt_tokens, Some(5));
        assert_eq!(outcome.completion_tokens, Some(3));
    }

    #[test]
    fn markdown_json_fences_are_normalized_once_for_all_content_providers() {
        assert_eq!(
            normalize_json_content("```json\n{\"ok\":true}\n```"),
            "{\"ok\":true}"
        );
    }
}
