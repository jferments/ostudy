use serde_json::{json, Value};
use url::Url;

use crate::ai::{
    catalog::ProviderKind,
    endpoint_policy::join_endpoint,
    gateway::{error, AiGatewayError},
    profiles::ProfileSnapshot,
};

use super::{claude, deepseek, llama_cpp, openai_compatible, openrouter, schema as schema_support};

pub(super) const DEFAULT_MAX_MODEL_LIST_BYTES: usize = 512 * 1024;
pub(super) const DEFAULT_MAX_MODELS: usize = 500;

/// Implemented providers are one OpenAI-compatible protocol family with a few
/// explicit dialect choices; Claude lowers that family to the Messages API.
/// This enum is the single dispatch seam for those differences.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum ProviderDialect {
    LlamaCpp,
    Deepseek,
    Openrouter,
    Claude,
}

impl ProviderDialect {
    pub(super) fn for_kind(kind: ProviderKind) -> Result<Self, AiGatewayError> {
        match kind {
            ProviderKind::LlamaCpp => Ok(Self::LlamaCpp),
            ProviderKind::Deepseek => Ok(Self::Deepseek),
            ProviderKind::Openrouter => Ok(Self::Openrouter),
            ProviderKind::Claude => Ok(Self::Claude),
            _ => Err(error(
                "ai_provider_not_implemented",
                "This AI provider adapter is not implemented.",
                false,
                None,
            )),
        }
    }

    pub(super) fn model_endpoint(self, profile: &ProfileSnapshot) -> Result<Url, AiGatewayError> {
        match self {
            Self::LlamaCpp => llama_cpp::openai_endpoint(&profile.base_url, "models"),
            Self::Deepseek => {
                join_endpoint(&profile.base_url, "models").map_err(command_to_gateway)
            }
            Self::Openrouter => {
                join_endpoint(&profile.base_url, "models/user").map_err(command_to_gateway)
            }
            Self::Claude => claude::models_endpoint(profile),
        }
    }

    pub(super) fn completion_endpoint(
        self,
        profile: &ProfileSnapshot,
    ) -> Result<Url, AiGatewayError> {
        match self {
            Self::LlamaCpp => llama_cpp::completion_endpoint(profile),
            Self::Deepseek => deepseek::completion_endpoint(profile, false),
            Self::Openrouter => {
                join_endpoint(&profile.base_url, "chat/completions").map_err(command_to_gateway)
            }
            Self::Claude => claude::messages_endpoint(profile),
        }
    }

    pub(super) fn model_limits(self) -> (usize, usize) {
        match self {
            Self::Openrouter => (openrouter::MAX_MODEL_LIST_BYTES, openrouter::MAX_MODELS),
            Self::LlamaCpp | Self::Deepseek | Self::Claude => {
                (DEFAULT_MAX_MODEL_LIST_BYTES, DEFAULT_MAX_MODELS)
            }
        }
    }

    pub(super) fn apply_probe_nonthinking(self, body: &mut Value, profile: &ProfileSnapshot) {
        match self {
            Self::LlamaCpp => llama_cpp::apply_probe_nonthinking(body),
            Self::Deepseek => deepseek::disable_thinking(body),
            Self::Openrouter => {
                openrouter::apply_request_policy(body, true, profile.require_zero_data_retention);
                openrouter::apply_reasoning_policy(body, &profile.reasoning_effort, None);
            }
            Self::Claude => {
                let visible = body["max_tokens"].as_u64().unwrap_or(64) as usize;
                body["max_tokens"] =
                    json!(claude::probe_max_tokens(&profile.reasoning_effort, visible));
                *body = claude::lower_chat_body(body, &profile.reasoning_effort, None);
            }
        }
    }

    pub(super) fn structured_probe_body(
        self,
        profile: &ProfileSnapshot,
        echo: &str,
        schema: &Value,
    ) -> Value {
        let messages = json!([
            {
                "role": "system",
                "content": if self == Self::Deepseek {
                    "Return only valid JSON. The object must contain exactly two fields: ok (the boolean true) and echo (the exact string requested by the user)."
                } else {
                    "Return only the JSON object required by the response schema."
                }
            },
            {
                "role": "user",
                "content": if self == Self::Deepseek {
                    format!("Return this JSON object with no Markdown: {{\"ok\":true,\"echo\":\"{echo}\"}}")
                } else {
                    format!("Return ok=true and echo exactly {echo}.")
                }
            }
        ]);
        let max_tokens = match self {
            Self::Openrouter => 256,
            Self::Claude => claude::probe_max_tokens(&profile.reasoning_effort, 128),
            Self::LlamaCpp | Self::Deepseek => 128,
        };
        let mut body = openai_compatible::chat_body(&profile.model_id, messages, 0.0, max_tokens);
        match self {
            Self::LlamaCpp => {
                llama_cpp::apply_probe_nonthinking(&mut body);
                openai_compatible::set_json_schema(
                    &mut body,
                    "ostudy_connection_probe",
                    schema.clone(),
                );
            }
            Self::Deepseek => {
                deepseek::disable_thinking(&mut body);
                openai_compatible::set_json_object(&mut body);
            }
            Self::Openrouter => {
                openrouter::apply_request_policy(
                    &mut body,
                    true,
                    profile.require_zero_data_retention,
                );
                openrouter::apply_reasoning_policy(&mut body, &profile.reasoning_effort, None);
                openai_compatible::set_json_schema(
                    &mut body,
                    "ostudy_connection_probe",
                    schema_support::portable_strict_schema(schema),
                );
            }
            Self::Claude => {
                body = claude::lower_chat_body(
                    &body,
                    &profile.reasoning_effort,
                    claude::strict_schema(schema),
                );
            }
        }
        body
    }

    /// Bring a non-streaming response to the chat-completion shape the
    /// provider-neutral readers use.
    pub(super) fn normalize_response(self, value: Value) -> Result<Value, AiGatewayError> {
        match self {
            Self::Claude => claude::normalize_message(&value),
            Self::LlamaCpp | Self::Deepseek | Self::Openrouter => Ok(value),
        }
    }
}

pub(super) fn command_to_gateway(error_value: crate::error::CommandError) -> AiGatewayError {
    error(&error_value.code, &error_value.message, false, None)
}
