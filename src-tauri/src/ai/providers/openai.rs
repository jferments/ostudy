//! OpenAI's own Chat Completions API. It is the reference shape of the shared
//! protocol family, so this dialect only renames the output cap, leaves out
//! sampling parameters reasoning models reject, maps the resolved effort to
//! `reasoning_effort`, and supplies model facts the OpenAI models list does
//! not carry.

use serde_json::{json, Value};
use url::Url;

use crate::ai::{
    endpoint_policy::join_endpoint, gateway::AiGatewayError, profiles::ProfileSnapshot,
};

use super::{dialect::command_to_gateway, ProviderModelReasoning, ProviderProperties};

/// The OpenAI models list returns ids only, so chat models are recognised by
/// family. Ids naming another modality or a Responses-only model are left out.
const CHAT_MODEL_PREFIXES: [&str; 5] = ["gpt-", "chatgpt-", "o1", "o3", "o4"];
const NON_CHAT_MARKERS: [&str; 15] = [
    "audio",
    "realtime",
    "search",
    "transcribe",
    "tts",
    "image",
    "embedding",
    "moderation",
    "instruct",
    "codex",
    "-pro",
    "deep-research",
    "computer-use",
    "-oss",
    "whisper",
];

const REASONING_PROBE_MAX_TOKENS: usize = 2_048;

pub(super) fn completion_endpoint(profile: &ProfileSnapshot) -> Result<Url, AiGatewayError> {
    join_endpoint(&profile.base_url, "chat/completions").map_err(command_to_gateway)
}

/// Lower the shared chat body for OpenAI. `effort` is already resolved against
/// the model's declared efforts; "automatic" leaves the model's default.
pub(super) fn lower_chat_body(chat: &Value, effort: &str) -> Value {
    let mut body = chat.clone();
    let Some(object) = body.as_object_mut() else {
        return body;
    };
    if let Some(max_tokens) = object.remove("max_tokens") {
        object.insert("max_completion_tokens".to_string(), max_tokens);
    }
    let model = object
        .get("model")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    let Some(efforts) = reasoning_efforts(&model) else {
        return body;
    };
    // Reasoning models accept only their default sampling.
    object.remove("temperature");
    object.remove("top_p");
    if effort != "automatic" && efforts.contains(&effort) {
        object.insert("reasoning_effort".to_string(), json!(effort));
    }
    body
}

/// Probes ask for a few visible tokens; a reasoning model spends hidden
/// tokens first, so its cap leaves room for them.
pub(super) fn probe_max_tokens(model: &str, effort: &str, visible_tokens: usize) -> usize {
    match reasoning_efforts(model) {
        Some(efforts) if !(effort == "none" && efforts.contains(&"none")) => {
            visible_tokens.max(REASONING_PROBE_MAX_TOKENS)
        }
        _ => visible_tokens,
    }
}

pub(super) fn is_chat_model(id: &str) -> bool {
    let id = id.to_ascii_lowercase();
    CHAT_MODEL_PREFIXES
        .iter()
        .any(|prefix| id.starts_with(prefix))
        && !NON_CHAT_MARKERS.iter().any(|marker| id.contains(marker))
}

/// Reasoning efforts by model family, from OpenAI's model documentation.
/// Newer families not listed get the conservative set every current
/// reasoning model accepts; a live probe confirms the profile.
fn reasoning_efforts(model: &str) -> Option<&'static [&'static str]> {
    let model = model.to_ascii_lowercase();
    if model.contains("chat-latest") || model.starts_with("chatgpt-") {
        return None;
    }
    if model.starts_with("o1") || model.starts_with("o3") || model.starts_with("o4") {
        return Some(&["low", "medium", "high"]);
    }
    if model == "gpt-5" || model.starts_with("gpt-5-") {
        return Some(&["minimal", "low", "medium", "high"]);
    }
    if model.starts_with("gpt-5.") || model.starts_with("gpt-6") {
        return Some(&["none", "low", "medium", "high"]);
    }
    None
}

pub(super) fn model_reasoning(id: &str) -> Option<ProviderModelReasoning> {
    let efforts = reasoning_efforts(id)?;
    let mandatory = !efforts.contains(&"none");
    Some(ProviderModelReasoning {
        supported_efforts: Some(efforts.iter().map(ToString::to_string).collect()),
        default_effort: None,
        default_enabled: Some(mandatory),
        supports_max_tokens: false,
        mandatory,
    })
}

pub(super) fn model_context_window_tokens(id: &str) -> Option<i64> {
    let id = id.to_ascii_lowercase();
    if id.starts_with("gpt-4.1") {
        Some(1_047_576)
    } else if id == "gpt-5" || id.starts_with("gpt-5-") {
        Some(400_000)
    } else if id.starts_with("o3") || id.starts_with("o4") || id == "o1" || id.starts_with("o1-2") {
        Some(200_000)
    } else if id.starts_with("gpt-4o") {
        Some(128_000)
    } else {
        None
    }
}

/// Every current OpenAI chat family takes image input except the o1-mini and
/// o3-mini text models.
pub(super) fn model_image_input(id: &str) -> Option<bool> {
    let id = id.to_ascii_lowercase();
    if !is_chat_model(&id) {
        return None;
    }
    Some(!(id.starts_with("o1-mini") || id.starts_with("o3-mini") || id.starts_with("o1-preview")))
}

pub(super) fn properties(id: &str) -> ProviderProperties {
    ProviderProperties {
        image_input_declared: model_image_input(id),
        context_window_tokens: model_context_window_tokens(id),
        ..ProviderProperties::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn chat(model: &str) -> Value {
        json!({
            "model": model,
            "messages": [{"role": "user", "content": "Hi"}],
            "temperature": 0.12,
            "max_tokens": 900,
            "stream": true,
        })
    }

    #[test]
    fn reasoning_models_get_effort_and_default_sampling() {
        let body = lower_chat_body(&chat("gpt-5.4-mini"), "none");
        assert_eq!(body["max_completion_tokens"], json!(900));
        assert!(body.get("max_tokens").is_none());
        assert!(body.get("temperature").is_none());
        assert_eq!(body["reasoning_effort"], json!("none"));

        let body = lower_chat_body(&chat("gpt-5-mini"), "minimal");
        assert_eq!(body["reasoning_effort"], json!("minimal"));

        let body = lower_chat_body(&chat("o4-mini"), "automatic");
        assert!(body.get("reasoning_effort").is_none());
        assert!(body.get("temperature").is_none());
    }

    #[test]
    fn non_reasoning_models_keep_temperature_and_send_no_effort() {
        let body = lower_chat_body(&chat("gpt-4.1-mini"), "none");
        assert_eq!(body["temperature"], json!(0.12));
        assert_eq!(body["max_completion_tokens"], json!(900));
        assert!(body.get("reasoning_effort").is_none());
    }

    #[test]
    fn model_families_declare_their_efforts() {
        let gpt5 = model_reasoning("gpt-5-nano-2025-08-07").unwrap();
        assert!(gpt5.mandatory);
        assert_eq!(
            super::super::super::reasoning::resolve_declared_effort(
                "none",
                gpt5.supported_efforts.as_deref(),
                None,
                gpt5.default_enabled,
                gpt5.mandatory,
            ),
            "minimal"
        );
        let gpt54 = model_reasoning("gpt-5.4").unwrap();
        assert!(!gpt54.mandatory);
        let o3 = model_reasoning("o3").unwrap();
        assert!(o3.mandatory);
        assert!(model_reasoning("gpt-4o").is_none());
        assert!(model_reasoning("gpt-5.2-chat-latest").is_none());
    }

    #[test]
    fn only_chat_models_are_listed() {
        for id in [
            "gpt-5.4",
            "gpt-4.1-nano",
            "o4-mini",
            "gpt-6-sol",
            "chatgpt-4o-latest",
        ] {
            assert!(is_chat_model(id), "{id}");
        }
        for id in [
            "text-embedding-3-small",
            "gpt-4o-audio-preview",
            "gpt-4o-mini-search-preview",
            "gpt-realtime",
            "gpt-image-1",
            "dall-e-3",
            "whisper-1",
            "gpt-5.2-pro",
            "gpt-4o-mini-transcribe",
            "omni-moderation-latest",
            "gpt-5.1-codex",
            "babbage-002",
        ] {
            assert!(!is_chat_model(id), "{id}");
        }
    }

    #[test]
    fn properties_come_from_the_model_family() {
        let gpt41 = properties("gpt-4.1");
        assert_eq!(gpt41.context_window_tokens, Some(1_047_576));
        assert_eq!(gpt41.image_input_declared, Some(true));
        assert_eq!(properties("o3-mini").image_input_declared, Some(false));
        assert_eq!(properties("gpt-6-luna").context_window_tokens, None);
    }
}
