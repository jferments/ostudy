use serde::Deserialize;
use serde_json::{json, Value};

use crate::ai::{
    gateway::{normalized_usage, AiGateway, AiGatewayError},
    profiles::ProfileSnapshot,
};

use super::{
    dialect::ProviderDialect,
    document::{decode_document_output, prepare_document_request, PreparedDocumentRequest},
    openai_compatible, CompletionOutcome,
};

const MAX_COMPLETION_BYTES: usize = 256 * 1024;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct StructuredProbePayload {
    ok: bool,
    echo: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct DocumentSchemaProbePayload {
    cards: Vec<DocumentSchemaProbeCard>,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
enum DocumentSchemaProbeCard {
    Basic {
        front: String,
        back: String,
        tags: Vec<String>,
    },
    Cloze {
        text: String,
        blanks: Vec<String>,
        mode: String,
        tags: Vec<String>,
    },
}

pub(crate) async fn run_text_probe(
    gateway: &AiGateway,
    profile: &ProfileSnapshot,
    bearer: Option<&str>,
) -> Result<CompletionOutcome, AiGatewayError> {
    let dialect = ProviderDialect::for_kind(profile.provider_kind)?;
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
    let mut body = openai_compatible::chat_body(&profile.model_id, messages, 0.0, 64);
    dialect.apply_probe_nonthinking(&mut body, profile);
    let outcome = send_completion(gateway, profile, bearer, dialect, body).await?;
    if outcome.content.trim() != "OSTUDY_OK" {
        let finish = outcome.finish_reason.as_deref().unwrap_or("unreported");
        return Err(crate::ai::gateway::error(
            "ai_text_probe_invalid",
            &format!(
                "The model responded, but it did not follow the minimal text probe (finish: {finish})."
            ),
            false,
            None,
        ));
    }
    Ok(outcome)
}

pub(crate) async fn run_structured_probe(
    gateway: &AiGateway,
    profile: &ProfileSnapshot,
    bearer: Option<&str>,
    echo: &str,
) -> Result<CompletionOutcome, AiGatewayError> {
    let dialect = ProviderDialect::for_kind(profile.provider_kind)?;
    let schema = json!({
        "type": "object",
        "additionalProperties": false,
        "required": ["ok", "echo"],
        "properties": {
            "ok": {"const": true},
            "echo": {"const": echo}
        }
    });
    let body = dialect.structured_probe_body(profile, echo, &schema);
    let outcome = send_completion(gateway, profile, bearer, dialect, body).await?;
    let parsed: StructuredProbePayload =
        serde_json::from_str(outcome.content.trim()).map_err(|_| {
            crate::ai::gateway::error(
                "ai_invalid_json",
                "The provider did not return a valid JSON object for the structured-output probe.",
                false,
                None,
            )
        })?;
    if !parsed.ok || parsed.echo != echo {
        return Err(crate::ai::gateway::error(
            "ai_structured_output_invalid",
            "The provider returned JSON that did not satisfy the structured-output probe.",
            false,
            None,
        ));
    }
    Ok(outcome)
}

/// Probe the exact structured-output path used by real document work for every
/// implemented provider, rather than giving only OpenRouter a representative
/// document-contract check.
pub(crate) async fn run_document_schema_probe(
    gateway: &AiGateway,
    profile: &ProfileSnapshot,
    bearer: Option<&str>,
) -> Result<CompletionOutcome, AiGatewayError> {
    let schema = document_schema_probe_schema();
    let prepared = prepare_document_request(
        profile,
        "You are validating a structured-output contract. Return only the structured result required by the schema.",
        Value::String(concat!(
            "Return exactly two synthetic cards. First: type basic, front Probe question, ",
            "back Probe answer, tags containing probe. Second: type cloze, text The probe value is stable., ",
            "blanks containing stable, mode one_at_a_time, tags containing probe."
        ).to_string()),
        &schema,
        768,
        "none",
    )?;
    let response = send_prepared_probe(gateway, profile, bearer, &prepared).await?;
    let decoded = decode_document_output(&prepared, &response).map_err(|failure| {
        crate::ai::gateway::error(
            "ai_document_schema_probe_invalid",
            failure.message,
            false,
            None,
        )
    })?;
    let content = decoded.to_string();
    let parsed: DocumentSchemaProbePayload = serde_json::from_value(decoded).map_err(|_| {
        crate::ai::gateway::error(
            "ai_document_schema_probe_invalid",
            "The model returned invalid JSON for ostudy's document-generation contract probe.",
            false,
            None,
        )
    })?;
    validate_document_schema_probe(&parsed)?;
    Ok(outcome_from_response(&response, content))
}

fn document_schema_probe_schema() -> Value {
    json!({
        "type": "object",
        "additionalProperties": false,
        "required": ["cards"],
        "properties": {
            "cards": {
                "type": "array",
                "minItems": 2,
                "maxItems": 2,
                "items": {
                    "oneOf": [
                        {
                            "type": "object",
                            "additionalProperties": false,
                            "required": ["type", "front", "back", "tags"],
                            "properties": {
                                "type": {"const": "basic"},
                                "front": {"const": "Probe question"},
                                "back": {"const": "Probe answer"},
                                "tags": {"type": "array", "items": {"const": "probe"}}
                            }
                        },
                        {
                            "type": "object",
                            "additionalProperties": false,
                            "required": ["type", "text", "blanks", "mode", "tags"],
                            "properties": {
                                "type": {"const": "cloze"},
                                "text": {"const": "The probe value is stable."},
                                "blanks": {"type": "array", "items": {"const": "stable"}},
                                "mode": {"const": "one_at_a_time"},
                                "tags": {"type": "array", "items": {"const": "probe"}}
                            }
                        }
                    ]
                }
            }
        }
    })
}

fn validate_document_schema_probe(
    payload: &DocumentSchemaProbePayload,
) -> Result<(), AiGatewayError> {
    if payload.cards.len() != 2 {
        return Err(crate::ai::gateway::error(
            "ai_document_schema_probe_invalid",
            "The model did not return the two required document-contract probe cards.",
            false,
            None,
        ));
    }
    let basic_ok = payload.cards.iter().any(|card| match card {
        DocumentSchemaProbeCard::Basic { front, back, tags } => {
            front == "Probe question" && back == "Probe answer" && tags == &["probe"]
        }
        _ => false,
    });
    let cloze_ok = payload.cards.iter().any(|card| match card {
        DocumentSchemaProbeCard::Cloze {
            text,
            blanks,
            mode,
            tags,
        } => {
            text == "The probe value is stable."
                && blanks == &["stable"]
                && mode == "one_at_a_time"
                && tags == &["probe"]
        }
        _ => false,
    });
    if !basic_ok || !cloze_ok {
        return Err(crate::ai::gateway::error(
            "ai_document_schema_probe_invalid",
            "The model returned structured JSON but did not satisfy ostudy's representative document-generation contract.",
            false,
            None,
        ));
    }
    Ok(())
}

async fn send_completion(
    gateway: &AiGateway,
    profile: &ProfileSnapshot,
    bearer: Option<&str>,
    dialect: ProviderDialect,
    body: Value,
) -> Result<CompletionOutcome, AiGatewayError> {
    let url = dialect.completion_endpoint(profile)?;
    crate::logging::debug_target(
        "ai.provider",
        "provider_probe_request_started",
        "A bounded provider capability probe started.",
        json!({
            "providerKind": profile.provider_kind.as_str(),
            "modelId": profile.model_id.as_str(),
        }),
    );
    let value = gateway
        .post_json(
            url,
            bearer,
            &body,
            profile.request_timeout_ms,
            MAX_COMPLETION_BYTES,
        )
        .await?;
    let outcome = openai_compatible::parse_completion(&value)?;
    crate::logging::debug_target(
        "ai.provider",
        "provider_probe_request_completed",
        "The bounded provider capability probe completed.",
        json!({
            "providerKind": profile.provider_kind.as_str(),
            "finishReason": outcome.finish_reason.as_deref(),
            "promptTokens": outcome.prompt_tokens,
            "completionTokens": outcome.completion_tokens,
        }),
    );
    Ok(outcome)
}

async fn send_prepared_probe(
    gateway: &AiGateway,
    profile: &ProfileSnapshot,
    bearer: Option<&str>,
    prepared: &PreparedDocumentRequest,
) -> Result<Value, AiGatewayError> {
    let first = gateway
        .post_json(
            prepared.url.clone(),
            bearer,
            &prepared.body,
            profile.request_timeout_ms,
            MAX_COMPLETION_BYTES,
        )
        .await;
    match first {
        Err(first_error) if first_error.safe.code == "ai_http_bad_request" => {
            if let Some(compatibility) = prepared.compatibility_body.as_ref() {
                gateway
                    .post_json(
                        prepared.url.clone(),
                        bearer,
                        compatibility,
                        profile.request_timeout_ms,
                        MAX_COMPLETION_BYTES,
                    )
                    .await
            } else {
                Err(first_error)
            }
        }
        other => other,
    }
}

fn outcome_from_response(response: &Value, content: String) -> CompletionOutcome {
    let usage = normalized_usage(response);
    CompletionOutcome {
        content,
        finish_reason: openai_compatible::completion_finish_reason(response),
        prompt_tokens: usage
            .as_ref()
            .and_then(|usage| usage.get("prompt_tokens"))
            .and_then(nonnegative_i64),
        completion_tokens: usage
            .as_ref()
            .and_then(|usage| usage.get("completion_tokens"))
            .and_then(nonnegative_i64),
    }
}

fn nonnegative_i64(value: &Value) -> Option<i64> {
    value.as_i64().filter(|value| *value >= 0).or_else(|| {
        value
            .as_u64()
            .map(|value| i64::try_from(value).unwrap_or(i64::MAX))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn representative_document_contract_validation_is_provider_neutral() {
        let payload: DocumentSchemaProbePayload = serde_json::from_value(json!({
            "cards": [
                {"type":"basic","front":"Probe question","back":"Probe answer","tags":["probe"]},
                {"type":"cloze","text":"The probe value is stable.","blanks":["stable"],"mode":"one_at_a_time","tags":["probe"]}
            ]
        }))
        .unwrap();
        validate_document_schema_probe(&payload).unwrap();
    }
}
