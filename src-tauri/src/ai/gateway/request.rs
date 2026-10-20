use std::time::Duration;

use reqwest::{Client, Method, RequestBuilder};
use serde_json::{json, Value};
use url::Url;

use super::{
    contracts::{AiGatewayError, ProviderAccess, ProviderWire},
    errors::error,
    metadata::is_openrouter_url,
};

/// Anthropic's stable Messages API version header.
pub(super) const ANTHROPIC_VERSION: &str = "2023-06-01";

pub(super) fn ordinary_request(
    client: &Client,
    method: Method,
    url: Url,
    access: ProviderAccess<'_>,
    body: Option<&Value>,
    timeout_ms: i64,
    accept: &str,
) -> RequestBuilder {
    let openrouter = is_openrouter_url(&url);
    let mut request = client
        .request(method, url)
        .timeout(Duration::from_millis(timeout_ms.max(1) as u64))
        .header(reqwest::header::ACCEPT, accept);
    if openrouter {
        request = request
            .header("X-OpenRouter-Title", "ostudy")
            .header("X-OpenRouter-Metadata", "enabled");
    }
    request = authenticate(request, access);
    if let Some(body) = body {
        request = request.json(body);
    }
    request
}

pub(super) fn streaming_request(
    client: &Client,
    url: Url,
    access: ProviderAccess<'_>,
    body: &Value,
) -> RequestBuilder {
    let openrouter = is_openrouter_url(&url);
    let mut request = client
        .post(url)
        .header(reqwest::header::ACCEPT, "text/event-stream")
        .header(reqwest::header::CACHE_CONTROL, "no-cache")
        .json(body);
    if openrouter {
        request = request
            .header("X-OpenRouter-Title", "ostudy")
            .header("X-OpenRouter-Metadata", "enabled");
    }
    authenticate(request, access)
}

fn authenticate(mut request: RequestBuilder, access: ProviderAccess<'_>) -> RequestBuilder {
    match access.wire {
        ProviderWire::OpenAiCompatible => {
            if let Some(secret) = access.secret {
                request = request.bearer_auth(secret);
            }
        }
        ProviderWire::AnthropicMessages => {
            request = request.header("anthropic-version", ANTHROPIC_VERSION);
            if let Some(secret) = access.secret {
                // Mark it sensitive like bearer_auth does, so it never shows
                // up in reqwest's Debug output.
                match reqwest::header::HeaderValue::from_str(secret) {
                    Ok(mut value) => {
                        value.set_sensitive(true);
                        request = request.header("x-api-key", value);
                    }
                    // reqwest reports the invalid header when the request is
                    // sent; never echo the secret itself.
                    Err(_) => request = request.header("x-api-key", "\u{0}"),
                }
            }
        }
    }
    request
}

pub(super) fn prepare_streaming_body(
    body: &Value,
    wire: ProviderWire,
) -> Result<Value, AiGatewayError> {
    let mut streaming_body = body.clone();
    let object = streaming_body.as_object_mut().ok_or_else(|| {
        error(
            "ai_provider_protocol_error",
            "The AI chat request was not a JSON object.",
            false,
            None,
        )
    })?;
    object.insert("stream".to_string(), Value::Bool(true));
    // Anthropic always reports usage in message_start/message_delta and
    // rejects unknown request fields.
    if wire == ProviderWire::OpenAiCompatible {
        object.insert(
            "stream_options".to_string(),
            json!({ "include_usage": true }),
        );
    }
    Ok(streaming_body)
}
