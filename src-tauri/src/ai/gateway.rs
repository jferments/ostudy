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

        let mut request = self
            .streaming_client
            .post(url)
            .header(reqwest::header::ACCEPT, "text/event-stream")
            .header(reqwest::header::CACHE_CONTROL, "no-cache")
            .json(&streaming_body);
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

        if stream.content.trim().is_empty() {
            return Err(error(
                "ai_empty_response",
                "The model returned no final document-task content.",
                true,
                None,
            ));
        }
        Ok(json!({
            "choices": [{
                "message": { "content": stream.content }
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
        let mut request = self
            .client
            .request(method, url)
            .timeout(Duration::from_millis(timeout_ms as u64))
            .header(reqwest::header::ACCEPT, "application/json");
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
        if let Some(content) = value
            .get("choices")
            .and_then(Value::as_array)
            .and_then(|choices| choices.first())
            .and_then(|choice| choice.get("delta"))
            .and_then(|delta| delta.get("content"))
            .and_then(Value::as_str)
        {
            if self.content.len().saturating_add(content.len()) > self.maximum_content_bytes {
                return Err(error(
                    "ai_response_too_large",
                    "The AI provider returned more generated content than ostudy accepts.",
                    false,
                    None,
                ));
            }
            self.content.push_str(content);
        }
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
    fn sse_accumulator_bounds_generated_content() {
        let mut stream = SseChatAccumulator::new(3, 1024);
        let failure = stream
            .push_bytes(b"data: {\"choices\":[{\"delta\":{\"content\":\"four\"}}]}\n")
            .expect_err("oversized streamed content should fail");
        assert_eq!(failure.safe.code, "ai_response_too_large");
    }
}
