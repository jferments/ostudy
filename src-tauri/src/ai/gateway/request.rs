use std::time::Duration;

use reqwest::{Client, Method, RequestBuilder};
use serde_json::{json, Value};
use url::Url;

use super::{
    contracts::AiGatewayError,
    errors::error,
    metadata::is_openrouter_url,
};

pub(super) fn ordinary_request(
    client: &Client,
    method: Method,
    url: Url,
    bearer: Option<&str>,
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
    if let Some(secret) = bearer {
        request = request.bearer_auth(secret);
    }
    if let Some(body) = body {
        request = request.json(body);
    }
    request
}

pub(super) fn streaming_request(
    client: &Client,
    url: Url,
    bearer: Option<&str>,
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
    if let Some(secret) = bearer {
        request = request.bearer_auth(secret);
    }
    request
}

pub(super) fn prepare_streaming_body(body: &Value) -> Result<Value, AiGatewayError> {
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
    object.insert(
        "stream_options".to_string(),
        json!({ "include_usage": true }),
    );
    Ok(streaming_body)
}
