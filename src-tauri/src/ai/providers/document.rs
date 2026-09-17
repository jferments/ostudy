mod json_output;

use serde_json::{json, Value};
use url::Url;

use crate::ai::{
    endpoint_policy::join_endpoint, gateway::AiGatewayError, profiles::ProfileSnapshot,
};

use super::{
    deepseek,
    dialect::{command_to_gateway, ProviderDialect},
    llama_cpp, openai_compatible, openrouter, schema as schema_support,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum DocumentOutputDecoder {
    ContentJson,
    StrictTool(&'static str),
}

pub(crate) struct PreparedDocumentRequest {
    pub url: Url,
    pub body: Value,
    pub compatibility_body: Option<Value>,
    decoder: DocumentOutputDecoder,
    schema: Value,
}

#[derive(Debug)]
pub(crate) struct ProviderOutputError {
    pub message: &'static str,
}

/// Build the common document Chat Completions request once and lower only the
/// structured-output/routing details that differ between provider dialects.
pub(crate) fn prepare_document_request(
    profile: &ProfileSnapshot,
    system_prompt: &str,
    user_content: Value,
    schema: &Value,
    max_tokens: usize,
    reasoning_effort: &str,
) -> Result<PreparedDocumentRequest, AiGatewayError> {
    let dialect = ProviderDialect::for_kind(profile.provider_kind)?;
    let system_prompt = crate::ai::structured_output::system_with_schema(system_prompt, schema);
    let messages = json!([
        {"role": "system", "content": system_prompt},
        {"role": "user", "content": user_content}
    ]);
    let base = openai_compatible::chat_body(&profile.model_id, messages, 0.12, max_tokens);

    match dialect {
        ProviderDialect::LlamaCpp => {
            let (body, compatibility_body) = llama_cpp::document_schema_bodies(&base, schema);
            Ok(PreparedDocumentRequest {
                url: llama_cpp::completion_endpoint(profile)?,
                body,
                compatibility_body: Some(compatibility_body),
                decoder: DocumentOutputDecoder::ContentJson,
                schema: schema.clone(),
            })
        }
        ProviderDialect::Deepseek => {
            let mut body = base;
            deepseek::disable_thinking(&mut body);
            if schema_support::deepseek_strict_schema_compatible(schema) {
                deepseek::apply_strict_document_tool(&mut body, schema);
                Ok(PreparedDocumentRequest {
                    url: deepseek::completion_endpoint(profile, true)?,
                    body,
                    compatibility_body: None,
                    decoder: DocumentOutputDecoder::StrictTool(deepseek::DOCUMENT_TOOL_NAME),
                    schema: schema.clone(),
                })
            } else {
                // DeepSeek strict tools reject schemas containing nullable union
                // types and other unsupported JSON-Schema constructs before the
                // model runs. JSON-object mode is a supported provider contract;
                // the full schema remains in the prompt and native admission is
                // still authoritative after the response is parsed.
                openai_compatible::set_json_object(&mut body);
                crate::logging::debug_target(
                    "ai.contract",
                    "deepseek_structured_output_json_fallback",
                    "Used DeepSeek JSON-object mode because the task schema is outside the provider's strict-tool subset.",
                    json!({
                        "model": profile.model_id,
                        "schemaFingerprint": crate::ai::structured_output::fingerprint(schema),
                    }),
                );
                Ok(PreparedDocumentRequest {
                    url: deepseek::completion_endpoint(profile, false)?,
                    body,
                    compatibility_body: None,
                    decoder: DocumentOutputDecoder::ContentJson,
                    schema: schema.clone(),
                })
            }
        }
        ProviderDialect::Openrouter => {
            let mut body = base;
            openrouter::apply_document_policy(
                &mut body,
                reasoning_effort,
                profile.require_zero_data_retention,
            );
            openai_compatible::set_json_schema(
                &mut body,
                "ostudy_document_task",
                schema_support::portable_strict_schema(schema),
            );
            Ok(PreparedDocumentRequest {
                url: join_endpoint(&profile.base_url, "chat/completions")
                    .map_err(command_to_gateway)?,
                body,
                compatibility_body: None,
                decoder: DocumentOutputDecoder::ContentJson,
                schema: schema.clone(),
            })
        }
    }
}

pub(crate) fn decode_document_output(
    request: &PreparedDocumentRequest,
    response: &Value,
) -> Result<Value, ProviderOutputError> {
    let value = match request.decoder {
        DocumentOutputDecoder::ContentJson => {
            let content =
                openai_compatible::completion_content(response).ok_or(ProviderOutputError {
                    message: "The AI provider returned no structured document content.",
                })?;
            json_output::parse(content)?
        }
        DocumentOutputDecoder::StrictTool(name) => {
            let calls = response
                .pointer("/choices/0/message/tool_calls")
                .and_then(Value::as_array)
                .filter(|calls| !calls.is_empty() && calls.len() <= 128)
                .ok_or(ProviderOutputError {
                    message: "The AI provider did not return bounded structured tool results.",
                })?;
            let mut values = Vec::with_capacity(calls.len());
            for call in calls {
                let function = call.get("function").ok_or(ProviderOutputError {
                    message: "The AI provider returned an invalid structured tool result.",
                })?;
                if function.get("name").and_then(Value::as_str) != Some(name) {
                    return Err(ProviderOutputError {
                        message: "The AI provider returned an unexpected structured-output tool.",
                    });
                }
                let arguments = function.get("arguments").and_then(Value::as_str).ok_or(
                    ProviderOutputError {
                        message: "The structured tool result did not contain JSON arguments.",
                    },
                )?;
                values.push(json_output::parse(arguments)?);
            }
            if values.len() == 1 {
                values.remove(0)
            } else {
                // Preserve independent invocations, never concatenate their JSON
                // bytes or pick one silently. Only a domain with additive batch
                // semantics may admit this collection; scalar tasks reject it.
                crate::logging::debug_target(
                    "ai.contract",
                    "structured_tool_results_separated",
                    "Preserved independent tool results for domain-level admission.",
                    json!({"toolCalls":values.len()}),
                );
                Value::Array(values)
            }
        }
    };
    // A shape mismatch is diagnostic here, not a whole-batch veto. Domain
    // decoders admit independent units and preserve partial-coverage semantics.
    crate::ai::structured_output::log_shape(&value, &request.schema, "document_response");
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::catalog::{AuthMode, ProviderKind};

    fn profile(kind: ProviderKind, base_url: &str, model_id: &str) -> ProfileSnapshot {
        ProfileSnapshot {
            id: "a".repeat(32),
            provider_kind: kind,
            base_url: base_url.to_string(),
            model_id: model_id.to_string(),
            reasoning_effort: "low".to_string(),
            auth_mode: if kind == ProviderKind::LlamaCpp {
                AuthMode::None
            } else {
                AuthMode::Bearer
            },
            enabled: true,
            allow_insecure_http: false,
            require_zero_data_retention: false,
            request_timeout_ms: 60_000,
            credential_epoch: 0,
            revision: 1,
        }
    }

    #[test]
    fn common_document_fields_are_identical_before_dialect_lowering() {
        let schema = json!({"type":"object","properties":{"name":{"type":"string"}}});
        let llama = prepare_document_request(
            &profile(ProviderKind::LlamaCpp, "http://127.0.0.1:8080/", "local"),
            "system",
            Value::String("user".to_string()),
            &schema,
            100,
            "none",
        )
        .unwrap();
        let deepseek = prepare_document_request(
            &profile(
                ProviderKind::Deepseek,
                "https://api.deepseek.com/",
                "deepseek-v4-flash",
            ),
            "system",
            Value::String("user".to_string()),
            &schema,
            100,
            "none",
        )
        .unwrap();
        let openrouter = prepare_document_request(
            &profile(
                ProviderKind::Openrouter,
                "https://openrouter.ai/api/v1/",
                "openai/example",
            ),
            "system",
            Value::String("user".to_string()),
            &schema,
            100,
            "low",
        )
        .unwrap();

        for request in [&llama, &deepseek, &openrouter] {
            assert_eq!(
                request.body.pointer("/messages/0/content"),
                Some(&json!(crate::ai::structured_output::system_with_schema(
                    "system", &schema
                )))
            );
            assert_eq!(
                request.body.pointer("/messages/1/content"),
                Some(&json!("user"))
            );
            assert_eq!(request.body.pointer("/max_tokens"), Some(&json!(100)));
        }
        assert_eq!(
            llama.body.pointer("/response_format/type"),
            Some(&json!("json_object"))
        );
        assert_eq!(
            llama
                .compatibility_body
                .as_ref()
                .and_then(|body| body.pointer("/json_schema/type")),
            Some(&json!("object"))
        );
        assert_eq!(
            deepseek.body.pointer("/tools/0/function/strict"),
            Some(&json!(true))
        );
        assert_eq!(
            openrouter.body.pointer("/provider/require_parameters"),
            Some(&json!(true))
        );
    }

    #[test]
    fn deepseek_nullable_contract_uses_supported_json_object_mode() {
        let schema = json!({
            "type":"object",
            "properties":{
                "prompt":{"type":"string"},
                "explanation":{"type":["string","null"]}
            },
            "required":["prompt","explanation"]
        });
        let request = prepare_document_request(
            &profile(
                ProviderKind::Deepseek,
                "https://api.deepseek.com/",
                "deepseek-flash",
            ),
            "system",
            Value::String("user".to_string()),
            &schema,
            100,
            "none",
        )
        .unwrap();
        assert_eq!(request.url.as_str(), "https://api.deepseek.com/chat/completions");
        assert_eq!(
            request.body.pointer("/response_format/type"),
            Some(&json!("json_object"))
        );
        assert!(request.body.get("tools").is_none());
        assert_eq!(request.decoder, DocumentOutputDecoder::ContentJson);
    }
}

#[cfg(test)]
mod decoding_regressions {
    use super::*;

    fn request(decoder: DocumentOutputDecoder) -> PreparedDocumentRequest {
        PreparedDocumentRequest {
            url: Url::parse("https://example.org/").unwrap(),
            body: json!({}),
            compatibility_body: None,
            decoder,
            schema: json!({"type":"object","properties":{"units":{"type":"array"}}}),
        }
    }

    #[test]
    fn tool_arguments_receive_the_same_bounded_json_normalization_as_content() {
        let text = "```json\n{\"units\":[],}\n```";
        let content = decode_document_output(
            &request(DocumentOutputDecoder::ContentJson),
            &json!({"choices":[{"message":{"content":text}}]}),
        )
        .unwrap();
        let tool = decode_document_output(&request(DocumentOutputDecoder::StrictTool("submit")),
            &json!({"choices":[{"message":{"tool_calls":[{"function":{"name":"submit","arguments":text}}]}}]})).unwrap();
        assert_eq!(content, json!({"units":[]}));
        assert_eq!(tool, content);
    }

    #[test]
    fn independent_tool_results_stay_separate_instead_of_concatenating_or_selecting() {
        let response = json!({"choices":[{"message":{"tool_calls":[
            {"function":{"name":"submit","arguments":"{\"units\":[1]}"}},
            {"function":{"name":"submit","arguments":"{\"units\":[2]}"}}
        ]}}]});
        let request = request(DocumentOutputDecoder::StrictTool("submit"));
        let decoded = decode_document_output(&request, &response).unwrap();
        assert_eq!(decoded, json!([{"units":[1]},{"units":[2]}]));
        let mut malformed = response;
        malformed["choices"][0]["message"]["tool_calls"][1]["function"]["arguments"] =
            json!("{unfinished");
        assert!(decode_document_output(&request, &malformed).is_err());
        malformed["choices"][0]["message"]["tool_calls"][1]["function"]["name"] =
            json!("unexpected");
        assert!(decode_document_output(&request, &malformed).is_err());
    }
}
