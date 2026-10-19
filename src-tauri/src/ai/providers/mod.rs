//! Provider protocol family and narrow provider dialects.
//!
//! Implemented providers share one internal protocol family: OpenAI-style
//! model catalogs, Chat Completions messages/responses, SSE, and usage.
//! Dialect modules contain only the differences that cannot be expressed by
//! the shared protocol helpers. OpenAI is that family's reference API. Claude speaks Anthropic's Messages API on the
//! wire; its dialect lowers requests and normalizes responses at the edge.

mod claude;
mod deepseek;
mod dialect;
mod document;
pub(crate) mod embeddings;
mod llama_cpp;
mod models;
mod openai;
mod openai_compatible;
mod openrouter;
mod probes;
mod schema;

use serde::{Deserialize, Serialize};

use crate::ai::{
    catalog::ProviderKind,
    gateway::{ProviderAccess, ProviderWire},
    profiles::ProfileSnapshot,
};

pub(crate) use document::{decode_document_output, prepare_document_request};
pub(crate) use models::{list_models, load_properties};
pub(crate) use probes::{run_document_schema_probe, run_structured_probe, run_text_probe};

/// The credential and wire family for a request to this profile's provider.
pub(crate) fn provider_access<'a>(
    profile: &ProfileSnapshot,
    secret: Option<&'a str>,
) -> ProviderAccess<'a> {
    let wire = if profile.provider_kind == ProviderKind::Claude {
        ProviderWire::AnthropicMessages
    } else {
        ProviderWire::OpenAiCompatible
    };
    ProviderAccess::new(wire, secret)
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderModelReasoning {
    pub supported_efforts: Option<Vec<String>>,
    pub default_effort: Option<String>,
    pub default_enabled: Option<bool>,
    pub supports_max_tokens: bool,
    pub mandatory: bool,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderModel {
    pub id: String,
    pub display_name: String,
    pub owned_by: Option<String>,
    pub context_window_tokens: Option<i64>,
    pub reasoning: Option<ProviderModelReasoning>,
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
