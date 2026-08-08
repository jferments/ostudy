use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use url::Url;

use super::{
    catalog::ProviderKind,
    endpoint_policy::join_endpoint,
    gateway::{error, AiGateway, AiGatewayError},
    profiles::ProfileSnapshot,
};

const MAX_MODEL_LIST_BYTES: usize = 512 * 1024;
const MAX_COMPLETION_BYTES: usize = 256 * 1024;
const MAX_MODELS: usize = 500;

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderModel {
    pub id: String,
    pub display_name: String,
    pub owned_by: Option<String>,
}

#[derive(Clone, Debug)]
pub struct CompletionOutcome {
    pub content: String,
    pub finish_reason: Option<String>,
    pub prompt_tokens: Option<i64>,
    pub completion_tokens: Option<i64>,
}

#[derive(Clone, Debug, Default)]
pub struct ProviderProperties {
    pub server_chat_template_present: Option<bool>,
    pub server_chat_template_hash: Option<String>,
    pub image_input_declared: Option<bool>,
    pub context_window_tokens: Option<i64>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct StructuredProbePayload {
    ok: bool,
    echo: String,
}

pub async fn list_models(
    gateway: &AiGateway,
    profile: &ProfileSnapshot,
    bearer: Option<&str>,
) -> Result<Vec<ProviderModel>, AiGatewayError> {
    let url = match profile.provider_kind {
        ProviderKind::LlamaCpp => llama_openai_endpoint(&profile.base_url, "models")?,
        ProviderKind::Deepseek => {
            join_endpoint(&profile.base_url, "models").map_err(command_to_gateway)?
        }
        _ => {
            return Err(error(
                "ai_provider_not_implemented",
                "This AI provider adapter is not implemented.",
                false,
                None,
            ));
        }
    };
    let value = gateway
        .get_json(
            url,
            bearer,
            profile.request_timeout_ms,
            MAX_MODEL_LIST_BYTES,
        )
        .await?;
    parse_model_list(&value)
}

pub async fn load_properties(
    gateway: &AiGateway,
    profile: &ProfileSnapshot,
    bearer: Option<&str>,
) -> Result<ProviderProperties, AiGatewayError> {
    if profile.provider_kind != ProviderKind::LlamaCpp {
        return Ok(ProviderProperties::default());
    }
    let mut url = llama_root_endpoint(&profile.base_url, "props")?;
    url.query_pairs_mut()
        .append_pair("model", &profile.model_id);
    match gateway
        .get_json(
            url,
            bearer,
            profile.request_timeout_ms,
            MAX_MODEL_LIST_BYTES,
        )
        .await
    {
        Ok(value) => Ok(parse_llama_properties(&value)),
        Err(error_value)
            if matches!(
                error_value.safe.code.as_str(),
                "ai_provider_protocol_error" | "ai_http_not_found"
            ) =>
        {
            Ok(ProviderProperties::default())
        }
        Err(error_value) => Err(error_value),
    }
}

pub async fn run_text_probe(
    gateway: &AiGateway,
    profile: &ProfileSnapshot,
    bearer: Option<&str>,
) -> Result<CompletionOutcome, AiGatewayError> {
    let messages = json!([
        {
            "role": "system",
            "content": "You are performing a connectivity test. Follow the user's exact output instruction."
        },
        {
            "role": "user",
            "content": "Reply with exactly OSTUDY_OK and no other text."
        }
    ]);
    let body = match profile.provider_kind {
        ProviderKind::LlamaCpp => json!({
            "model": profile.model_id,
            "messages": messages,
            "temperature": 0,
            "max_tokens": 64,
            "stream": false,
            "reasoning_effort": "none",
            "chat_template_kwargs": { "enable_thinking": false }
        }),
        ProviderKind::Deepseek => json!({
            "model": profile.model_id,
            "messages": messages,
            "temperature": 0,
            "max_tokens": 64,
            "stream": false,
            "thinking": { "type": "disabled" }
        }),
        _ => {
            return Err(error(
                "ai_provider_not_implemented",
                "This AI provider adapter is not implemented.",
                false,
                None,
            ));
        }
    };
    let outcome = send_completion(gateway, profile, bearer, body).await?;
    if outcome.content.trim() != "OSTUDY_OK" {
        let finish = outcome.finish_reason.as_deref().unwrap_or("unreported");
        let message = format!(
            "The model responded, but it did not follow the minimal text probe (finish: {finish})."
        );
        return Err(error("ai_text_probe_invalid", &message, false, None));
    }
    Ok(outcome)
}

pub async fn run_structured_probe(
    gateway: &AiGateway,
    profile: &ProfileSnapshot,
    bearer: Option<&str>,
    echo: &str,
) -> Result<CompletionOutcome, AiGatewayError> {
    let schema = json!({
        "type": "object",
        "additionalProperties": false,
        "required": ["ok", "echo"],
        "properties": {
            "ok": { "const": true },
            "echo": { "const": echo }
        }
    });
    let body = match profile.provider_kind {
        ProviderKind::LlamaCpp => json!({
            "model": profile.model_id,
            "messages": [
                {
                    "role": "system",
                    "content": "Return only the JSON object required by the response schema."
                },
                {
                    "role": "user",
                    "content": format!("Return ok=true and echo exactly {echo}.")
                }
            ],
            "temperature": 0,
            "max_tokens": 128,
            "stream": false,
            "reasoning_effort": "none",
            "chat_template_kwargs": { "enable_thinking": false },
            "response_format": {
                "type": "json_schema",
                "json_schema": {
                    "name": "ostudy_connection_probe",
                    "strict": true,
                    "schema": schema
                }
            }
        }),
        ProviderKind::Deepseek => json!({
            "model": profile.model_id,
            "messages": [
                {
                    "role": "system",
                    "content": concat!(
                        "Return only valid JSON. The object must contain exactly two fields: ",
                        "ok (the boolean true) and echo (the exact string requested by the user)."
                    )
                },
                {
                    "role": "user",
                    "content": format!(
                        "Return this JSON object with no Markdown: {{\"ok\":true,\"echo\":\"{echo}\"}}"
                    )
                }
            ],
            "temperature": 0,
            "max_tokens": 128,
            "stream": false,
            "thinking": { "type": "disabled" },
            "response_format": { "type": "json_object" }
        }),
        _ => {
            return Err(error(
                "ai_provider_not_implemented",
                "This AI provider adapter is not implemented.",
                false,
                None,
            ));
        }
    };

    let outcome = send_completion(gateway, profile, bearer, body).await?;
    let parsed: StructuredProbePayload =
        serde_json::from_str(outcome.content.trim()).map_err(|_| {
            error(
                "ai_invalid_json",
                "The provider did not return a valid JSON object for the structured-output probe.",
                false,
                None,
            )
        })?;
    if !parsed.ok || parsed.echo != echo {
        return Err(error(
            "ai_structured_output_invalid",
            "The provider returned JSON that did not satisfy the structured-output probe.",
            false,
            None,
        ));
    }
    Ok(outcome)
}

async fn send_completion(
    gateway: &AiGateway,
    profile: &ProfileSnapshot,
    bearer: Option<&str>,
    body: Value,
) -> Result<CompletionOutcome, AiGatewayError> {
    let url = match profile.provider_kind {
        ProviderKind::LlamaCpp => llama_openai_endpoint(&profile.base_url, "chat/completions")?,
        ProviderKind::Deepseek => {
            join_endpoint(&profile.base_url, "chat/completions").map_err(command_to_gateway)?
        }
        _ => {
            return Err(error(
                "ai_provider_not_implemented",
                "This AI provider adapter is not implemented.",
                false,
                None,
            ));
        }
    };
    let value = gateway
        .post_json(
            url,
            bearer,
            &body,
            profile.request_timeout_ms,
            MAX_COMPLETION_BYTES,
        )
        .await?;
    parse_completion(&value)
}

fn llama_openai_endpoint(base_url: &str, suffix: &str) -> Result<Url, AiGatewayError> {
    let relative = if llama_base_includes_v1(base_url) {
        suffix.to_string()
    } else {
        format!("v1/{suffix}")
    };
    join_endpoint(base_url, &relative).map_err(command_to_gateway)
}

fn llama_root_endpoint(base_url: &str, suffix: &str) -> Result<Url, AiGatewayError> {
    let relative = if llama_base_includes_v1(base_url) {
        format!("../{suffix}")
    } else {
        suffix.to_string()
    };
    join_endpoint(base_url, &relative).map_err(command_to_gateway)
}

fn llama_base_includes_v1(base_url: &str) -> bool {
    Url::parse(base_url)
        .ok()
        .is_some_and(|url| url.path().trim_end_matches('/').ends_with("/v1"))
}

fn parse_model_list(value: &Value) -> Result<Vec<ProviderModel>, AiGatewayError> {
    let data = value.get("data").and_then(Value::as_array).ok_or_else(|| {
        error(
            "ai_provider_protocol_error",
            "The provider model list did not use the expected response shape.",
            false,
            None,
        )
    })?;
    if data.len() > MAX_MODELS {
        return Err(error(
            "ai_model_list_too_large",
            "The provider returned more models than ostudy accepts in one response.",
            false,
            None,
        ));
    }

    let mut models = Vec::with_capacity(data.len());
    for item in data {
        let id = item.get("id").and_then(Value::as_str).ok_or_else(|| {
            error(
                "ai_provider_protocol_error",
                "A provider model entry did not include an ID.",
                false,
                None,
            )
        })?;
        if id.trim().is_empty() || id.len() > 300 || id.chars().any(char::is_control) {
            return Err(error(
                "ai_provider_protocol_error",
                "A provider model ID exceeded ostudy's accepted bounds.",
                false,
                None,
            ));
        }
        let owned_by = item
            .get("owned_by")
            .and_then(Value::as_str)
            .filter(|value| value.len() <= 200 && !value.chars().any(char::is_control))
            .map(ToString::to_string);
        models.push(ProviderModel {
            id: id.to_string(),
            display_name: id.to_string(),
            owned_by,
        });
    }
    models.sort_by(|left, right| left.id.cmp(&right.id));
    models.dedup_by(|left, right| left.id == right.id);
    Ok(models)
}

fn parse_completion(value: &Value) -> Result<CompletionOutcome, AiGatewayError> {
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
    let usage = value.get("usage");
    Ok(CompletionOutcome {
        content: content.to_string(),
        finish_reason,
        prompt_tokens: usage
            .and_then(|value| value.get("prompt_tokens"))
            .and_then(Value::as_i64),
        completion_tokens: usage
            .and_then(|value| value.get("completion_tokens"))
            .and_then(Value::as_i64),
    })
}

fn parse_llama_properties(value: &Value) -> ProviderProperties {
    let chat_template = value
        .get("chat_template")
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty());
    let server_chat_template_hash = chat_template.map(|template| {
        let digest = Sha256::digest(template.as_bytes());
        format!("{digest:x}")
    });
    let image_input_declared = value
        .get("modalities")
        .and_then(Value::as_array)
        .map(|modalities| {
            modalities.iter().any(|item| {
                item.as_str()
                    .is_some_and(|value| matches!(value, "image" | "vision"))
            })
        })
        .or_else(|| value.get("has_vision_encoder").and_then(Value::as_bool));
    let context_window_tokens = value.get("n_ctx").and_then(Value::as_i64).or_else(|| {
        value
            .pointer("/default_generation_settings/n_ctx")
            .and_then(Value::as_i64)
    });
    ProviderProperties {
        server_chat_template_present: Some(chat_template.is_some()),
        server_chat_template_hash,
        image_input_declared,
        context_window_tokens,
    }
}

fn command_to_gateway(error_value: crate::error::CommandError) -> AiGatewayError {
    error(&error_value.code, &error_value.message, false, None)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_and_deduplicates_bounded_model_lists() {
        let models = parse_model_list(&json!({
            "data": [
                { "id": "model-b", "owned_by": "local" },
                { "id": "model-a" },
                { "id": "model-a" }
            ]
        }))
        .expect("model list should parse");
        assert_eq!(
            models
                .iter()
                .map(|model| model.id.as_str())
                .collect::<Vec<_>>(),
            vec!["model-a", "model-b"]
        );
    }

    #[test]
    fn llama_openai_routes_accept_server_root_or_v1_base() {
        assert_eq!(
            llama_openai_endpoint("http://127.0.0.1:8080/", "models")
                .unwrap()
                .as_str(),
            "http://127.0.0.1:8080/v1/models"
        );
        assert_eq!(
            llama_openai_endpoint("http://127.0.0.1:8080/v1/", "models")
                .unwrap()
                .as_str(),
            "http://127.0.0.1:8080/v1/models"
        );
        assert_eq!(
            llama_root_endpoint("http://127.0.0.1:8080/v1/", "props")
                .unwrap()
                .as_str(),
            "http://127.0.0.1:8080/props"
        );
    }

    #[test]
    fn rejects_structurally_empty_completion_responses() {
        assert!(parse_completion(&json!({ "choices": [] })).is_err());
    }

    #[test]
    fn accepts_valid_probe_content_even_when_provider_reports_length_finish() {
        let outcome = parse_completion(&json!({
            "choices": [{
                "message": { "content": "OSTUDY_OK" },
                "finish_reason": "length"
            }]
        }))
        .expect("valid final content should remain available to semantic validation");
        assert_eq!(outcome.content, "OSTUDY_OK");
        assert_eq!(outcome.finish_reason.as_deref(), Some("length"));
    }

    #[test]
    fn hashes_but_does_not_retain_llama_chat_templates() {
        let properties = parse_llama_properties(&json!({
            "chat_template": "{% for message in messages %}{{ message.content }}{% endfor %}",
            "modalities": ["text", "image"],
            "n_ctx": 8192
        }));
        assert_eq!(properties.server_chat_template_present, Some(true));
        assert_eq!(properties.image_input_declared, Some(true));
        assert_eq!(properties.context_window_tokens, Some(8192));
        assert_eq!(
            properties
                .server_chat_template_hash
                .as_deref()
                .map(str::len),
            Some(64)
        );
    }
}
