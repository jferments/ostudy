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
) -> RequestBuilder {
    let openrouter = is_openrouter_url(&url);
    let mut request = client
        .request(method, url)
        .timeout(Duration::from_millis(timeout_ms.max(1) as u64))
        .header(reqwest::header::ACCEPT, "application/json");
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
        ProviderWire::GeminiGenerateContent => {
            if let Some(secret) = access.secret {
                request = api_key_header(request, "x-goog-api-key", secret);
            }
        }
        ProviderWire::OpenAiCompatible => {
            if let Some(secret) = access.secret {
                request = request.bearer_auth(secret);
            }
        }
        ProviderWire::AnthropicMessages => {
            request = request.header("anthropic-version", ANTHROPIC_VERSION);
            if let Some(secret) = access.secret {
                request = api_key_header(request, "x-api-key", secret);
            }
        }
    }
    request
}

/// Mark an API-key header sensitive like bearer_auth does, so it never shows
/// up in reqwest's Debug output.
fn api_key_header(request: RequestBuilder, name: &'static str, secret: &str) -> RequestBuilder {
    match reqwest::header::HeaderValue::from_str(secret) {
        Ok(mut value) => {
            value.set_sensitive(true);
            request.header(name, value)
        }
        // reqwest reports the invalid header when the request is sent; never
        // echo the secret itself.
        Err(_) => request.header(name, "\u{0}"),
    }
}

/// Gemini streams from a sibling method of the one-shot endpoint; the other
/// wires stream from the same URL.
pub(super) fn streaming_url(mut url: Url, wire: ProviderWire) -> Url {
    if wire == ProviderWire::GeminiGenerateContent {
        if let Some(path) = url.path().strip_suffix(":generateContent") {
            let path = format!("{path}:streamGenerateContent");
            url.set_path(&path);
            url.query_pairs_mut().append_pair("alt", "sse");
        }
    }
    url
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
    // Gemini chooses streaming by method and rejects unknown fields.
    if wire == ProviderWire::GeminiGenerateContent {
        return Ok(streaming_body);
    }
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
