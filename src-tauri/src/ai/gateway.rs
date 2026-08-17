use std::time::Duration;

use reqwest::{redirect::Policy, Client, Method, Response};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tokio::time::timeout;
use url::Url;

use crate::task_cancellation::{current_signal, CancellationSignal};

const CONNECT_TIMEOUT_SECONDS: u64 = 10;
const MAX_ERROR_BODY_BYTES: usize = 32 * 1024;
const STREAM_WIRE_MULTIPLIER: usize = 16;
const OPENROUTER_HOST: &str = "openrouter.ai";

#[derive(Clone)]
pub struct AiGateway {
    client: Client,
    streaming_client: Client,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SafeAiError {
    pub code: String,
    pub message: String,
    pub retryable: bool,
    pub retry_after_seconds: Option<u64>,
}

#[derive(Clone, Debug)]
pub struct AiGatewayError {
    pub safe: SafeAiError,
}

impl AiGateway {
    pub fn new() -> Result<Self, String> {
        let client = Client::builder()
            .redirect(Policy::none())
            .connect_timeout(Duration::from_secs(CONNECT_TIMEOUT_SECONDS))
            .user_agent(concat!("ostudy/", env!("CARGO_PKG_VERSION")))
            .build()
            .map_err(|error| format!("Could not initialize the AI HTTP client: {error}"))?;
        // Long document-generation requests deliberately have no reqwest-wide
        // timeout. Their configured timeout covers connection establishment and
        // waiting for provider response headers only. Once the provider accepts
        // the request, inference may continue until completion or user cancel.
        let streaming_client = Client::builder()
            .redirect(Policy::none())
            .connect_timeout(Duration::from_secs(CONNECT_TIMEOUT_SECONDS))
            .user_agent(concat!("ostudy/", env!("CARGO_PKG_VERSION")))
            .build()
            .map_err(|error| format!("Could not initialize the AI streaming client: {error}"))?;
        Ok(Self {
            client,
            streaming_client,
        })
    }

    pub async fn get_json(
        &self,
        url: Url,
        bearer: Option<&str>,
        timeout_ms: i64,
        maximum_bytes: usize,
    ) -> Result<Value, AiGatewayError> {
        self.request_json(Method::GET, url, bearer, None, timeout_ms, maximum_bytes)
            .await
    }

    pub async fn post_json(
        &self,
        url: Url,
        bearer: Option<&str>,
        body: &Value,
        timeout_ms: i64,
        maximum_bytes: usize,
    ) -> Result<Value, AiGatewayError> {
        // Document AI runs inside a task-local cancellation scope. Only those
        // long-running calls are transparently upgraded to SSE; provider probes
        // and model discovery retain their ordinary bounded request behavior.
        if let Some(cancellation) = current_signal() {
            return self
                .post_sse_chat_json(url, bearer, body, timeout_ms, maximum_bytes, &cancellation)
                .await;
        }
        self.request_json(
            Method::POST,
            url,
            bearer,
            Some(body),
            timeout_ms,
            maximum_bytes,
        )
        .await
    }

    /// POST one Chat Completions request as an SSE stream.
    ///
    /// `initial_response_timeout_ms` is intentionally *not* a job deadline. It
    /// covers connection establishment and receipt of provider response headers.
    /// Successful headers are the provider acknowledgment boundary. After that,
    /// ostudy waits for streamed completion content without a wall-clock limit,
    /// while still enforcing response-size bounds and exact-task cancellation.
    /// This deliberately avoids optional llama.cpp progress/ping request fields
    /// so older compatible llama-server/Unsloth builds are not rejected merely
    /// because they do not recognize newer transport extensions.
    async fn post_sse_chat_json(
        &self,
        url: Url,
        bearer: Option<&str>,
        body: &Value,
        initial_response_timeout_ms: i64,
        maximum_content_bytes: usize,
        cancellation: &CancellationSignal,
    ) -> Result<Value, AiGatewayError> {
        if cancellation.is_canceled() {
            return Err(canceled_error());
        }
        let initial_timeout = Duration::from_millis(initial_response_timeout_ms.max(1) as u64);
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

        let openrouter = is_openrouter_url(&url);
        let mut request = self
            .streaming_client
            .post(url)
            .header(reqwest::header::ACCEPT, "text/event-stream")
            .header(reqwest::header::CACHE_CONTROL, "no-cache")
            .json(&streaming_body);
        if openrouter {
            request = request.header("X-OpenRouter-Title", "ostudy");
        }
        if let Some(secret) = bearer {
            request = request.bearer_auth(secret);
        }

        let mut response = wait_initial(
            initial_timeout,
            cancellation,
            request.send(),
            "The AI provider did not accept the request before the configured initial-response timeout.",
        )
        .await?;
        let status = response.status();
        let retry_after_seconds = response
            .headers()
            .get(reqwest::header::RETRY_AFTER)
            .and_then(|value| value.to_str().ok())
            .and_then(|value| value.parse::<u64>().ok());

        if status.is_redirection() {
            return Err(error(
                "ai_redirect_rejected",
                "The AI provider attempted to redirect the request. Save the final trusted endpoint explicitly.",
                false,
                None,
            ));
        }
        if !status.is_success() {
            let _ = read_bounded_body(&mut response, MAX_ERROR_BODY_BYTES, cancellation).await?;
            return Err(classify_http_status(status.as_u16(), retry_after_seconds));
        }

        let first_chunk = tokio::select! {
            _ = cancellation.canceled() => return Err(canceled_error()),
            result = response.chunk() => result.map_err(classify_reqwest_error)?,
        };
        let first_chunk = first_chunk.ok_or_else(|| {
            error(
                "ai_provider_protocol_error",
                "The AI provider accepted the request but closed the streaming response without content.",
                true,
                None,
            )
        })?;
        if first_chunk.is_empty() {
            return Err(error(
                "ai_provider_protocol_error",
                "The AI provider returned an empty streaming response.",
                true,
                None,
            ));
        }

        let maximum_wire_bytes = maximum_content_bytes
            .saturating_mul(STREAM_WIRE_MULTIPLIER)
            .max(maximum_content_bytes);
        let mut stream = SseChatAccumulator::new(maximum_content_bytes, maximum_wire_bytes);
        stream.push_bytes(&first_chunk)?;

        while !stream.done {
            let next = tokio::select! {
                _ = cancellation.canceled() => return Err(canceled_error()),
                result = response.chunk() => result.map_err(classify_reqwest_error)?,
            };
            let Some(chunk) = next else {
                break;
            };
            stream.push_bytes(&chunk)?;
        }
        stream.finish()?;

        if stream.content.trim().is_empty() && stream.tool_arguments.trim().is_empty() {
            let finish = stream.finish_reason.as_deref().unwrap_or("not provided");
            return Err(error(
                "ai_empty_response",
                &format!(
                    "The provider ended the streamed document request with finish_reason={finish}, but returned no final content or structured tool arguments."
                ),
                true,
                None,
            ));
        }
        let message = if stream.tool_arguments.trim().is_empty() {
            json!({ "content": stream.content })
        } else {
            json!({
                "content": if stream.content.trim().is_empty() { Value::Null } else { Value::String(stream.content) },
                "tool_calls": [{
                    "type": "function",
                    "function": {
                        "name": stream.tool_name.unwrap_or_default(),
                        "arguments": stream.tool_arguments
                    }
                }]
            })
        };
        Ok(json!({
            "choices": [{
                "message": message,
                "finish_reason": stream.finish_reason
            }]
        }))
    }

    async fn request_json(
        &self,
        method: Method,
        url: Url,
        bearer: Option<&str>,
        body: Option<&Value>,
        timeout_ms: i64,
        maximum_bytes: usize,
    ) -> Result<Value, AiGatewayError> {
        let openrouter = is_openrouter_url(&url);
        let mut request = self
            .client
            .request(method, url)
            .timeout(Duration::from_millis(timeout_ms as u64))
            .header(reqwest::header::ACCEPT, "application/json");
        if openrouter {
            request = request.header("X-OpenRouter-Title", "ostudy");
        }
        if let Some(secret) = bearer {
            request = request.bearer_auth(secret);
        }
        if let Some(body) = body {
            request = request.json(body);
        }

        let mut response = request.send().await.map_err(classify_reqwest_error)?;
        let status = response.status();
        let retry_after_seconds = response
            .headers()
            .get(reqwest::header::RETRY_AFTER)
            .and_then(|value| value.to_str().ok())
            .and_then(|value| value.parse::<u64>().ok());

        let body_limit = if status.is_success() {
            maximum_bytes
        } else {
            maximum_bytes.min(MAX_ERROR_BODY_BYTES)
        };
        if response
            .content_length()
            .is_some_and(|length| length > body_limit as u64)
        {
            return Err(error(
                "ai_response_too_large",
                "The AI provider returned a response larger than ostudy accepts.",
                false,
                None,
            ));
        }

        let mut bytes = Vec::new();
        while let Some(chunk) = response.chunk().await.map_err(classify_reqwest_error)? {
            if bytes.len().saturating_add(chunk.len()) > body_limit {
                return Err(error(
                    "ai_response_too_large",
                    "The AI provider returned a response larger than ostudy accepts.",
                    false,
                    None,
                ));
            }
            bytes.extend_from_slice(&chunk);
        }

        if status.is_redirection() {
            return Err(error(
                "ai_redirect_rejected",
                "The AI provider attempted to redirect the request. Save the final trusted endpoint explicitly.",
                false,
                None,
            ));
        }
        if !status.is_success() {
            return Err(classify_http_status(status.as_u16(), retry_after_seconds));
        }

        serde_json::from_slice(&bytes).map_err(|_| {
            error(
                "ai_provider_protocol_error",
                "The AI provider returned a response that was not valid JSON.",
                false,
                None,
            )
        })
    }
}

async fn wait_initial<F, T>(
    timeout_duration: Duration,
    cancellation: &CancellationSignal,
    future: F,
    timeout_message: &'static str,
) -> Result<T, AiGatewayError>
where
    F: std::future::Future<Output = Result<T, reqwest::Error>>,
{
    tokio::select! {
        _ = cancellation.canceled() => Err(canceled_error()),
        result = timeout(timeout_duration, future) => match result {
            Ok(value) => value.map_err(classify_reqwest_error),
            Err(_) => Err(initial_timeout_error(timeout_message)),
        },
    }
}

async fn read_bounded_body(
    response: &mut Response,
    maximum_bytes: usize,
    cancellation: &CancellationSignal,
) -> Result<Vec<u8>, AiGatewayError> {
    let mut bytes = Vec::new();
    loop {
        let next = tokio::select! {
            _ = cancellation.canceled() => return Err(canceled_error()),
            result = response.chunk() => result.map_err(classify_reqwest_error)?,
        };
        let Some(chunk) = next else {
            break;
        };
        if bytes.len().saturating_add(chunk.len()) > maximum_bytes {
            return Err(error(
                "ai_response_too_large",
                "The AI provider returned a response larger than ostudy accepts.",
                false,
                None,
            ));
        }
        bytes.extend_from_slice(&chunk);
    }
    Ok(bytes)
}

struct SseChatAccumulator {
    pending: Vec<u8>,
    content: String,
    tool_name: Option<String>,
    tool_arguments: String,
    finish_reason: Option<String>,
    wire_bytes: usize,
    maximum_content_bytes: usize,
    maximum_wire_bytes: usize,
    done: bool,
}

impl SseChatAccumulator {
    fn new(maximum_content_bytes: usize, maximum_wire_bytes: usize) -> Self {
        Self {
            pending: Vec::new(),
            content: String::new(),
            tool_name: None,
            tool_arguments: String::new(),
            finish_reason: None,
            wire_bytes: 0,
            maximum_content_bytes,
            maximum_wire_bytes,
            done: false,
        }
    }

    fn push_bytes(&mut self, bytes: &[u8]) -> Result<(), AiGatewayError> {
        self.wire_bytes = self.wire_bytes.saturating_add(bytes.len());
        if self.wire_bytes > self.maximum_wire_bytes {
            return Err(error(
                "ai_response_too_large",
                "The AI provider returned a streaming response larger than ostudy accepts.",
                false,
                None,
            ));
        }
        self.pending.extend_from_slice(bytes);
        while let Some(position) = self.pending.iter().position(|byte| *byte == b'\n') {
            let mut line = self.pending.drain(..=position).collect::<Vec<_>>();
            line.pop();
            if line.last() == Some(&b'\r') {
                line.pop();
            }
            self.process_line(&line)?;
            if self.done {
                break;
            }
        }
        Ok(())
    }

    fn finish(&mut self) -> Result<(), AiGatewayError> {
        if !self.pending.is_empty() && !self.done {
            let line = std::mem::take(&mut self.pending);
            self.process_line(&line)?;
        }
        Ok(())
    }

    fn process_line(&mut self, line: &[u8]) -> Result<(), AiGatewayError> {
        if line.is_empty() || line.starts_with(b":") {
            return Ok(());
        }
        let Some(data) = line.strip_prefix(b"data:") else {
            return Ok(());
        };
        let data = trim_ascii(data);
        if data == b"[DONE]" {
            self.done = true;
            return Ok(());
        }
        if data.is_empty() {
            return Ok(());
        }
        let value: Value = serde_json::from_slice(data).map_err(|_| {
            error(
                "ai_provider_protocol_error",
                "The AI provider returned a malformed streaming event.",
                false,
                None,
            )
        })?;
        if value.get("error").is_some() {
            return Err(classify_stream_event_error(&value));
        }
        let choice = value
            .get("choices")
            .and_then(Value::as_array)
            .and_then(|choices| choices.first());
        if let Some(reason) = choice
            .and_then(|choice| choice.get("finish_reason"))
            .and_then(Value::as_str)
            .filter(|reason| !reason.is_empty())
        {
            self.finish_reason = Some(reason.to_string());
        }
        let delta = choice.and_then(|choice| choice.get("delta"));
        if let Some(content) = delta
            .and_then(|delta| delta.get("content"))
            .and_then(Value::as_str)
        {
            self.append_compat_content(content)?;
        } else if let Some(content) = choice
            .and_then(|choice| choice.get("message"))
            .and_then(|message| message.get("content"))
            .and_then(Value::as_str)
            .or_else(|| {
                choice
                    .and_then(|choice| choice.get("text"))
                    .and_then(Value::as_str)
            })
            .or_else(|| value.get("content").and_then(Value::as_str))
        {
            // Some llama.cpp/Unsloth-compatible streaming builds emit a final
            // full message, completion text, or native top-level content field
            // instead of OpenAI delta.content chunks. Accept those safe text
            // variants without duplicating a final full message after deltas.
            self.append_compat_content(content)?;
        }
        if let Some(tool_call) = delta
            .and_then(|delta| delta.get("tool_calls"))
            .and_then(Value::as_array)
            .and_then(|calls| calls.first())
        {
            if let Some(name) = tool_call
                .get("function")
                .and_then(|function| function.get("name"))
                .and_then(Value::as_str)
                .filter(|name| !name.is_empty())
            {
                match self.tool_name.as_deref() {
                    Some(existing) if existing != name => {
                        return Err(error(
                            "ai_provider_protocol_error",
                            "The AI provider changed structured-output tool names mid-stream.",
                            false,
                            None,
                        ));
                    }
                    None => self.tool_name = Some(name.to_string()),
                    _ => {}
                }
            }
            if let Some(arguments) = tool_call
                .get("function")
                .and_then(|function| function.get("arguments"))
                .and_then(Value::as_str)
            {
                if self
                    .content
                    .len()
                    .saturating_add(self.tool_arguments.len())
                    .saturating_add(arguments.len())
                    > self.maximum_content_bytes
                {
                    return Err(error(
                        "ai_response_too_large",
                        "The AI provider returned more structured output than ostudy accepts.",
                        false,
                        None,
                    ));
                }
                self.tool_arguments.push_str(arguments);
            }
        }
        Ok(())
    }

    fn append_compat_content(&mut self, content: &str) -> Result<(), AiGatewayError> {
        if content.is_empty() {
            return Ok(());
        }
        if !self.content.is_empty() && content.starts_with(&self.content) {
            // A provider may emit one final full-message snapshot after already
            // streaming its deltas. In that case the snapshot adds no new text.
            return Ok(());
        }
        if self.content.len().saturating_add(content.len()) > self.maximum_content_bytes {
            return Err(error(
                "ai_response_too_large",
                "The AI provider returned more generated content than ostudy accepts.",
                false,
                None,
            ));
        }
        self.content.push_str(content);
        Ok(())
    }
}

fn trim_ascii(mut bytes: &[u8]) -> &[u8] {
    while bytes.first().is_some_and(|byte| byte.is_ascii_whitespace()) {
        bytes = &bytes[1..];
    }
    while bytes.last().is_some_and(|byte| byte.is_ascii_whitespace()) {
        bytes = &bytes[..bytes.len() - 1];
    }
    bytes
}

fn is_openrouter_url(url: &Url) -> bool {
    url.scheme() == "https" && url.host_str() == Some(OPENROUTER_HOST)
}

fn classify_stream_event_error(value: &Value) -> AiGatewayError {
    let code = value
        .pointer("/error/code")
        .and_then(Value::as_str)
        .map(ToString::to_string)
        .or_else(|| {
            value
                .pointer("/error/code")
                .and_then(Value::as_i64)
                .map(|value| value.to_string())
        })
        .unwrap_or_else(|| "unknown".to_string());
    let normalized = code.to_ascii_lowercase();
    let (safe_code, message, retryable) = match normalized.as_str() {
        "401" | "unauthorized" => (
            "ai_authentication_failed",
            "The AI provider rejected the saved credential.",
            false,
        ),
        "402" | "insufficient_credits" => (
            "ai_insufficient_credits",
            "The AI provider account does not have enough credits for this request.",
            false,
        ),
        "429" | "rate_limit_exceeded" => (
            "ai_rate_limited",
            "The AI provider rate-limited the request.",
            true,
        ),
        "503" | "no_available_providers" => (
            "ai_no_compatible_endpoint",
            "No compatible model endpoint was available for the requested parameters and privacy policy.",
            true,
        ),
        "context_length_exceeded" => (
            "ai_context_length_exceeded",
            "The provider rejected the request because it exceeded the model context window.",
            false,
        ),
        _ => (
            "ai_stream_interrupted",
            "The AI provider reported an error after streaming began. Partial output was discarded.",
            true,
        ),
    };
    crate::logging::warn_target(
        "ai.gateway.streaming",
        "provider_stream_error",
        message,
        json!({ "providerCode": code }),
    );
    error(safe_code, message, retryable, None)
}

fn initial_timeout_error(message: &'static str) -> AiGatewayError {
    error("ai_initial_response_timeout", message, true, None)
}

fn canceled_error() -> AiGatewayError {
    error(
        "ai_request_canceled",
        "PDF flashcard generation was stopped. The PDF remains available to retry.",
        false,
        None,
    )
}

fn classify_reqwest_error(error_value: reqwest::Error) -> AiGatewayError {
    if error_value.is_timeout() {
        return error(
            "ai_request_timeout",
            "The AI provider did not respond before the configured timeout.",
            true,
            None,
        );
    }
    if error_value.is_connect() {
        return error(
            "ai_connection_failed",
            "ostudy could not connect to the AI provider endpoint.",
            true,
            None,
        );
    }
    error(
        "ai_network_error",
        "The AI provider request failed before a valid response was received.",
        true,
        None,
    )
}

fn classify_http_status(status: u16, retry_after_seconds: Option<u64>) -> AiGatewayError {
    match status {
        400 | 422 => error(
            "ai_http_bad_request",
            "The AI provider rejected this request as unsupported or invalid. Verify the selected model and provider/server version, then retry.",
            false,
            retry_after_seconds,
        ),
        401 => error(
            "ai_http_unauthorized",
            "The AI provider rejected this credential.",
            false,
            None,
        ),
        403 => error(
            "ai_http_forbidden",
            "The AI provider does not permit this request.",
            false,
            None,
        ),
        404 => error(
            "ai_http_not_found",
            "The configured AI endpoint or model was not found.",
            false,
            None,
        ),
        408 => error(
            "ai_request_timeout",
            "The AI provider timed out while handling the request.",
            true,
            None,
        ),
        429 => error(
            "ai_http_rate_limited",
            "The AI provider rate limit was reached.",
            true,
            retry_after_seconds,
        ),
        500..=599 => error(
            "ai_server_unavailable",
            "The AI provider is temporarily unavailable.",
            true,
            retry_after_seconds,
        ),
        _ => error(
            "ai_http_error",
            "The AI provider rejected the request.",
            false,
            retry_after_seconds,
        ),
    }
}

pub fn error(
    code: &str,
    message: &str,
    retryable: bool,
    retry_after_seconds: Option<u64>,
) -> AiGatewayError {
    AiGatewayError {
        safe: SafeAiError {
            code: code.to_string(),
            message: message.to_string(),
            retryable,
            retry_after_seconds,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_in_band_stream_errors_after_partial_output() {
        let mut stream = SseChatAccumulator::new(1_024, 16_384);
        stream
            .push_bytes(b"data: {\"choices\":[{\"delta\":{\"content\":\"partial\"}}]}\n")
            .expect("first delta should parse");
        let failure = stream
            .push_bytes(b"data: {\"error\":{\"code\":\"server_error\",\"message\":\"private upstream detail\"}}\n")
            .expect_err("embedded stream error should abort the response");
        assert_eq!(failure.safe.code, "ai_stream_interrupted");
        assert!(!failure.safe.message.contains("private upstream detail"));
    }

    #[test]
    fn recognizes_only_the_fixed_openrouter_https_host_for_attribution() {
        assert!(is_openrouter_url(
            &Url::parse("https://openrouter.ai/api/v1/models").unwrap()
        ));
        assert!(!is_openrouter_url(
            &Url::parse("https://openrouter.ai.evil.example/api/v1/models").unwrap()
        ));
        assert!(!is_openrouter_url(
            &Url::parse("http://openrouter.ai/api/v1/models").unwrap()
        ));
    }

    #[test]
    fn classifies_rate_limits_without_exposing_provider_bodies() {
        let failure = classify_http_status(429, Some(17));
        assert_eq!(failure.safe.code, "ai_http_rate_limited");
        assert_eq!(failure.safe.retry_after_seconds, Some(17));
    }

    #[test]
    fn classifies_bad_requests_as_provider_compatibility_errors() {
        let failure = classify_http_status(400, None);
        assert_eq!(failure.safe.code, "ai_http_bad_request");
        assert!(failure.safe.message.contains("server version"));
    }

    #[test]
    fn sse_accumulator_accepts_keepalive_progress_and_split_json() {
        let mut stream = SseChatAccumulator::new(1024, 4096);
        stream
            .push_bytes(b": keep-alive\n\ndata: {\"prompt_progress\":{\"processed\":2}}\n\n")
            .unwrap();
        stream
            .push_bytes(b"data: {\"choices\":[{\"delta\":{\"content\":\"hel")
            .unwrap();
        stream.push_bytes(b"lo\"}}]}\n\ndata: [DONE]\n\n").unwrap();
        stream.finish().unwrap();
        assert_eq!(stream.content, "hello");
        assert!(stream.done);
    }

    #[test]
    fn sse_accumulator_collects_streamed_tool_arguments() {
        let mut stream = SseChatAccumulator::new(2048, 8192);
        stream
            .push_bytes(
                br#"data: {"choices":[{"delta":{"tool_calls":[{"index":0,"function":{"name":"submit_ostudy_document_result","arguments":"{\"cards\":["}}]}}]}
"#,
            )
            .unwrap();
        stream
            .push_bytes(
                br#"data: {"choices":[{"delta":{"tool_calls":[{"index":0,"function":{"arguments":"]}"}}]}}]}

data: [DONE]

"#,
            )
            .unwrap();
        stream.finish().unwrap();
        assert_eq!(
            stream.tool_name.as_deref(),
            Some("submit_ostudy_document_result")
        );
        assert_eq!(stream.tool_arguments, "{\"cards\":[]}");
        assert!(stream.done);
    }

    #[test]
    fn sse_accumulator_preserves_finish_reason() {
        let mut stream = SseChatAccumulator::new(1024, 4096);
        stream
            .push_bytes(
                b"data: {\"choices\":[{\"delta\":{\"content\":\"{}\"},\"finish_reason\":\"length\"}]}\n\ndata: [DONE]\n\n",
            )
            .unwrap();
        stream.finish().unwrap();
        assert_eq!(stream.content, "{}");
        assert_eq!(stream.finish_reason.as_deref(), Some("length"));
    }

    #[test]
    fn sse_accumulator_bounds_generated_content() {
        let mut stream = SseChatAccumulator::new(3, 1024);
        let failure = stream
            .push_bytes(b"data: {\"choices\":[{\"delta\":{\"content\":\"four\"}}]}\n")
            .expect_err("oversized streamed content should fail");
        assert_eq!(failure.safe.code, "ai_response_too_large");
    }

    #[test]
    fn sse_accumulator_accepts_llama_full_message_compatibility_events() {
        let mut stream = SseChatAccumulator::new(2048, 8192);
        stream
            .push_bytes(
                br#"data: {"choices":[{"message":{"content":"{\"summary\":\"ok\"}"},"finish_reason":"stop"}]}

data: [DONE]
"#,
            )
            .unwrap();
        stream.finish().unwrap();
        assert_eq!(stream.content, "{\"summary\":\"ok\"}");
        assert_eq!(stream.finish_reason.as_deref(), Some("stop"));
    }

    #[test]
    fn sse_accumulator_accepts_llama_native_top_level_content() {
        let mut stream = SseChatAccumulator::new(2048, 8192);
        stream
            .push_bytes(
                br#"data: {"content":"{\"summary\":"}

data: {"content":"\"ok\"}"}

data: [DONE]
"#,
            )
            .unwrap();
        stream.finish().unwrap();
        assert_eq!(stream.content, "{\"summary\":\"ok\"}");
    }
}
