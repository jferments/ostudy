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
            })
        }
        ProviderDialect::Deepseek => {
            let mut body = base;
            deepseek::disable_thinking(&mut body);
            deepseek::apply_strict_document_tool(&mut body, schema);
            Ok(PreparedDocumentRequest {
                url: deepseek::completion_endpoint(profile, true)?,
                body,
                compatibility_body: None,
                decoder: DocumentOutputDecoder::StrictTool(deepseek::DOCUMENT_TOOL_NAME),
            })
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
            })
        }
    }
}

pub(crate) fn decode_document_output(
    request: &PreparedDocumentRequest,
    response: &Value,
) -> Result<Value, ProviderOutputError> {
    let serialized = match request.decoder {
        DocumentOutputDecoder::ContentJson => openai_compatible::completion_content(response)
            .map(openai_compatible::normalize_json_content)
            .ok_or(ProviderOutputError {
                message: "The AI provider returned no structured document content.",
            })?,
        DocumentOutputDecoder::StrictTool(name) => {
            openai_compatible::strict_tool_arguments(response, name).ok_or(ProviderOutputError {
                message:
                    "The AI provider did not return the required schema-constrained tool result.",
            })?
        }
    };
    serde_json::from_str(serialized).map_err(|_| ProviderOutputError {
        message: "The AI provider returned structured document content that was not valid JSON.",
    })
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
                Some(&json!("system"))
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
}
