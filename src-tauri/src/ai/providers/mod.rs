//! OpenAI-compatible provider protocol family and narrow provider dialects.
//!
//! All currently implemented providers share one internal protocol family:
//! OpenAI-style model catalogs, Chat Completions messages/responses, SSE, and
//! usage. Dialect modules contain only the differences that cannot be expressed
//! by the shared protocol helpers.

mod deepseek;
mod dialect;
mod document;
mod llama_cpp;
mod models;
mod openai_compatible;
mod openrouter;
mod probes;
mod schema;

use serde::{Deserialize, Serialize};

pub(crate) use document::{decode_document_output, prepare_document_request};
pub(crate) use models::{list_models, load_properties};
pub(crate) use probes::{run_document_schema_probe, run_structured_probe, run_text_probe};

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
