use std::time::Duration;

use reqwest::{redirect::Policy, Client, Method, Response};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tokio::time::{sleep, timeout, Instant};
use url::Url;

use crate::task_cancellation::{
    current_signal, record_current_provider_request, record_current_provider_retry,
    record_current_usage, set_current_reasoning_effort, CancellationSignal, DocumentTaskUsageDelta,
};

const CONNECT_TIMEOUT_SECONDS: u64 = 10;
const MAX_ERROR_BODY_BYTES: usize = 32 * 1024;
const STREAM_WIRE_MULTIPLIER: usize = 16;
const OPENROUTER_HOST: &str = "openrouter.ai";
const DOCUMENT_RETRY_DELAY_MILLIS: u64 = 750;
const OPENROUTER_STREAM_PROGRESS_TIMEOUT_SECONDS: u64 = 75;
const DEFAULT_STREAM_PROGRESS_TIMEOUT_SECONDS: u64 = 600;
const MAX_SAFE_PROVIDER_METADATA_CHARS: usize = 300;

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
        // wall-clock deadline. Connection/header waiting remains bounded, and an
        // accepted stream is governed by useful-output progress plus user cancel.
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
        let prepared_body = prepare_provider_body(&url, body);
        let body = prepared_body.as_ref().unwrap_or(body);

        // Document AI runs inside a task-local cancellation scope. Only those
        // long-running calls are transparently upgraded to SSE; provider probes
        // and model discovery retain their ordinary bounded request behavior.
        if let Some(cancellation) = current_signal() {
            record_current_provider_request();
            let first = self
                .post_sse_chat_json(
                    url.clone(),
                    bearer,
                    body,
                    timeout_ms,
                    maximum_bytes,
                    &cancellation,
                )
                .await;
            return match first {
                Err(first_error) if should_retry_document_stream_error(&first_error) => {
                    let delay = document_retry_delay(&first_error);
                    let delay_ms = u64::try_from(delay.as_millis()).unwrap_or(u64::MAX);
                    let model = body.get("model").and_then(Value::as_str).and_then(|value| {
                        safe_metadata_string(value, MAX_SAFE_PROVIDER_METADATA_CHARS)
                    });
                    record_current_provider_retry();
                    crate::logging::warn_target(
                        "ai.gateway.retry",
                        "provider_request_retry",
                        "A transient AI provider failure caused one bounded document request to be retried from scratch.",
                        json!({
                            "endpointHost": url.host_str(),
                            "model": model.as_deref(),
                            "firstErrorCode": first_error.safe.code.as_str(),
                            "retryDelayMs": delay_ms,
                        }),
                    );
                    tokio::select! {
                        _ = cancellation.canceled() => return Err(canceled_error()),
                        _ = sleep(delay) => {}
                    }
                    record_current_provider_request();
                    match self
                        .post_sse_chat_json(
                            url.clone(),
                            bearer,
                            body,
                            timeout_ms,
                            maximum_bytes,
                            &cancellation,
                        )
                        .await
                    {
                        Ok(value) => {
                            crate::logging::info_target(
                                "ai.gateway.retry",
                                "provider_request_retry_succeeded",
                                "The bounded document request succeeded on its one transient retry.",
                                json!({
                                    "endpointHost": url.host_str(),
                                    "model": model.as_deref(),
                                    "firstErrorCode": first_error.safe.code.as_str(),
                                }),
                            );
                            Ok(value)
                        }
                        Err(mut second_error) => {
                            crate::logging::warn_target(
                                "ai.gateway.retry",
                                "provider_request_retry_exhausted",
                                "The bounded document request failed again after its one transient retry.",
                                json!({
                                    "endpointHost": url.host_str(),
                                    "model": model.as_deref(),
                                    "firstErrorCode": first_error.safe.code.as_str(),
                                    "secondErrorCode": second_error.safe.code.as_str(),
                                }),
                            );
                            second_error.safe.message = format!(
                                "{} ostudy retried this bounded source-window request once from scratch, but the retry also failed.",
                                second_error.safe.message
                            );
                            Err(second_error)
                        }
                    }
                }
                other => other,
            };
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
    /// ostudy does not impose a blunt total inference deadline: actively producing
    /// streams may run as needed, while a provider that stops producing usable
    /// model output is canceled by a provider-aware progress timeout. Response-size
    /// bounds and exact-task cancellation remain authoritative.
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
        let request_started = Instant::now();
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
        let progress_timeout_seconds = if openrouter {
            OPENROUTER_STREAM_PROGRESS_TIMEOUT_SECONDS
        } else {
            DEFAULT_STREAM_PROGRESS_TIMEOUT_SECONDS
        };
        let progress_timeout = Duration::from_secs(progress_timeout_seconds);
        let mut request = self
            .streaming_client
            .post(url)
            .header(reqwest::header::ACCEPT, "text/event-stream")
            .header(reqwest::header::CACHE_CONTROL, "no-cache")
            .json(&streaming_body);
        if openrouter {
            request = request
                .header("X-OpenRouter-Title", "ostudy")
                .header("X-OpenRouter-Metadata", "enabled");
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
        let generation_id = response
            .headers()
            .get("x-generation-id")
            .and_then(|value| value.to_str().ok())
            .and_then(|value| safe_metadata_string(value, 200));

        if status.is_redirection() {
            return Err(error(
                "ai_redirect_rejected",
                "The AI provider attempted to redirect the request. Save the final trusted endpoint explicitly.",
                false,
                None,
            ));
        }
        if !status.is_success() {
            let bytes =
                read_bounded_body(&mut response, MAX_ERROR_BODY_BYTES, cancellation).await?;
            return Err(classify_provider_error_response(
                status.as_u16(),
                retry_after_seconds,
                &bytes,
                openrouter,
                generation_id.as_deref(),
                true,
            ));
        }

        let first_chunk = tokio::select! {
            _ = cancellation.canceled() => return Err(canceled_error()),
            result = timeout(progress_timeout, response.chunk()) => match result {
                Ok(result) => result.map_err(classify_reqwest_error)?,
                Err(_) => return Err(stream_progress_timeout_error(progress_timeout_seconds)),
            },
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
        let mut stream = SseChatAccumulator::for_request(
            maximum_content_bytes,
            maximum_wire_bytes,
            openrouter,
            generation_id,
            retry_after_seconds,
        );
        stream.push_bytes(&first_chunk)?;
        let mut useful_progress_bytes = stream.useful_progress_bytes();
        let mut first_useful_output_ms = (useful_progress_bytes > 0)
            .then(|| u64::try_from(request_started.elapsed().as_millis()).unwrap_or(u64::MAX));
        let mut last_useful_progress = Instant::now();

        while !stream.done {
            let remaining = progress_timeout.saturating_sub(last_useful_progress.elapsed());
            if remaining.is_zero() {
                return Err(stream_progress_timeout_error(progress_timeout_seconds));
            }
            let next = tokio::select! {
                _ = cancellation.canceled() => return Err(canceled_error()),
                result = timeout(remaining, response.chunk()) => match result {
                    Ok(result) => result.map_err(classify_reqwest_error)?,
                    Err(_) => return Err(stream_progress_timeout_error(progress_timeout_seconds)),
                },
            };
            let Some(chunk) = next else {
                break;
            };
            stream.push_bytes(&chunk)?;
            let next_progress_bytes = stream.useful_progress_bytes();
            if next_progress_bytes > useful_progress_bytes {
                useful_progress_bytes = next_progress_bytes;
                last_useful_progress = Instant::now();
                if first_useful_output_ms.is_none() {
                    first_useful_output_ms = Some(
                        u64::try_from(request_started.elapsed().as_millis()).unwrap_or(u64::MAX),
                    );
                }
            }
        }
        stream.finish()?;

        if stream.content.trim().is_empty() && stream.tool_arguments.trim().is_empty() {
            let finish = stream.finish_reason.as_deref().unwrap_or("not provided");
            return Err(empty_stream_failure(finish));
        }
        let usage = stream.usage.clone();
        if let Some(usage) = usage.as_ref() {
            record_current_usage(usage_delta(usage));
        }
        if openrouter {
            crate::logging::debug_target(
                "ai.gateway.openrouter",
                "openrouter_stream_completed",
                "An OpenRouter document request completed without logging provider content.",
                json!({
                    "generationId": stream.generation_id.as_deref(),
                    "upstreamProvider": stream.upstream_provider.as_deref(),
                    "model": stream.model.as_deref(),
                    "finishReason": stream.finish_reason.as_deref(),
                    "durationMs": u64::try_from(request_started.elapsed().as_millis()).unwrap_or(u64::MAX),
                    "firstUsefulOutputMs": first_useful_output_ms,
                    "contentBytes": stream.content.len(),
                    "toolArgumentBytes": stream.tool_arguments.len(),
                }),
            );
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
        let mut result = json!({
            "choices": [{
                "message": message,
                "finish_reason": stream.finish_reason
            }]
        });
        if let Some(usage) = usage {
            result["usage"] = usage;
        }
        Ok(result)
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
        let progress_timeout_seconds = if openrouter {
            OPENROUTER_STREAM_PROGRESS_TIMEOUT_SECONDS
        } else {
            DEFAULT_STREAM_PROGRESS_TIMEOUT_SECONDS
        };
        let progress_timeout = Duration::from_secs(progress_timeout_seconds);
        let mut request = self
            .client
            .request(method, url)
            .timeout(Duration::from_millis(timeout_ms as u64))
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

        let mut response = request.send().await.map_err(classify_reqwest_error)?;
        let status = response.status();
        let retry_after_seconds = response
            .headers()
            .get(reqwest::header::RETRY_AFTER)
            .and_then(|value| value.to_str().ok())
            .and_then(|value| value.parse::<u64>().ok());
        let generation_id = response
            .headers()
            .get("x-generation-id")
            .and_then(|value| value.to_str().ok())
            .and_then(|value| safe_metadata_string(value, 200));

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
            return Err(classify_provider_error_response(
                status.as_u16(),
                retry_after_seconds,
                &bytes,
                openrouter,
                generation_id.as_deref(),
                false,
            ));
        }

        let value: Value = serde_json::from_slice(&bytes).map_err(|_| {
            error(
                "ai_provider_protocol_error",
                "The AI provider returned a response that was not valid JSON.",
                false,
                None,
            )
        })?;
        if openrouter && provider_error_object(&value).is_some() {
            return Err(classify_provider_error_value(
                &value,
                None,
                retry_after_seconds,
                true,
                generation_id.as_deref(),
                false,
            ));
        }
        Ok(value)
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
    usage: Option<Value>,
    wire_bytes: usize,
    maximum_content_bytes: usize,
    maximum_wire_bytes: usize,
    done: bool,
    openrouter: bool,
    generation_id: Option<String>,
    retry_after_seconds: Option<u64>,
    upstream_provider: Option<String>,
    model: Option<String>,
}

impl SseChatAccumulator {
    fn new(maximum_content_bytes: usize, maximum_wire_bytes: usize) -> Self {
        Self {
            pending: Vec::new(),
            content: String::new(),
            tool_name: None,
            tool_arguments: String::new(),
            finish_reason: None,
            usage: None,
            wire_bytes: 0,
            maximum_content_bytes,
            maximum_wire_bytes,
            done: false,
            openrouter: false,
            generation_id: None,
            retry_after_seconds: None,
            upstream_provider: None,
            model: None,
        }
    }

    fn for_request(
        maximum_content_bytes: usize,
        maximum_wire_bytes: usize,
        openrouter: bool,
        generation_id: Option<String>,
        retry_after_seconds: Option<u64>,
    ) -> Self {
        let mut stream = Self::new(maximum_content_bytes, maximum_wire_bytes);
        stream.openrouter = openrouter;
        stream.generation_id = generation_id;
        stream.retry_after_seconds = retry_after_seconds;
        stream
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
        if let Some(usage) = safe_usage_envelope(&value) {
            self.usage = Some(usage);
        }
        if self.openrouter {
            if self.upstream_provider.is_none() {
                self.upstream_provider =
                    safe_json_string(value.get("provider"), MAX_SAFE_PROVIDER_METADATA_CHARS);
            }
            if self.model.is_none() {
                self.model = safe_json_string(value.get("model"), MAX_SAFE_PROVIDER_METADATA_CHARS);
            }
        }
        if value.get("error").is_some_and(|value| !value.is_null()) {
            return Err(classify_stream_event_error(
                &value,
                self.openrouter,
                self.generation_id.as_deref(),
                self.retry_after_seconds,
            ));
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

    fn useful_progress_bytes(&self) -> usize {
        self.content.len().saturating_add(self.tool_arguments.len())
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
    url.scheme() == "https"
        && url.host_str() == Some(OPENROUTER_HOST)
        && url.path().starts_with("/api/v1/")
}

fn prepare_provider_body(url: &Url, body: &Value) -> Option<Value> {
    let document_task = current_signal().is_some();
    let reasoning_effort = document_task.then(crate::settings::current_ai_reasoning_effort);
    if let Some(reasoning_effort) = reasoning_effort.as_deref() {
        set_current_reasoning_effort(reasoning_effort);
    }
    prepare_provider_body_for_document(url, body, reasoning_effort.as_deref())
}

fn prepare_provider_body_for_document(
    url: &Url,
    body: &Value,
    reasoning_effort: Option<&str>,
) -> Option<Value> {
    if !is_openrouter_url(url) {
        return None;
    }
    let mut prepared = body.clone();
    let object = prepared.as_object_mut()?;
    object.insert(
        "plugins".to_string(),
        json!([{
            "id": "context-compression",
            "enabled": false
        }]),
    );
    object.insert("usage".to_string(), json!({ "include": true }));
    // Document windows are independent batch work, not conversational turns.
    // A shared session_id pins every concurrent request to one provider endpoint,
    // concentrating a batch on the same slow/stalled route for little cache gain.
    // Leave session stickiness unset so OpenRouter can spread independent windows.
    if let Some(reasoning_effort) = reasoning_effort.filter(|value| *value != "automatic") {
        object.insert(
            "reasoning".to_string(),
            json!({ "effort": reasoning_effort, "exclude": true }),
        );
    }
    Some(prepared)
}

fn safe_usage_envelope(value: &Value) -> Option<Value> {
    let usage = value.get("usage")?.as_object()?;
    let prompt_tokens = nonnegative_integer(usage.get("prompt_tokens"));
    let completion_tokens = nonnegative_integer(usage.get("completion_tokens"));
    let total_tokens = nonnegative_integer(usage.get("total_tokens"));
    let cached_tokens = nonnegative_integer(
        usage
            .get("prompt_tokens_details")
            .and_then(Value::as_object)
            .and_then(|details| details.get("cached_tokens")),
    );
    let cache_write_tokens = nonnegative_integer(
        usage
            .get("prompt_tokens_details")
            .and_then(Value::as_object)
            .and_then(|details| details.get("cache_write_tokens")),
    );
    let reasoning_tokens = nonnegative_integer(
        usage
            .get("completion_tokens_details")
            .and_then(Value::as_object)
            .and_then(|details| details.get("reasoning_tokens")),
    );
    let cost = usage
        .get("cost")
        .and_then(Value::as_f64)
        .filter(|value| value.is_finite() && *value >= 0.0)
        .map(Value::from);
    if prompt_tokens.is_none()
        && completion_tokens.is_none()
        && total_tokens.is_none()
        && cached_tokens.is_none()
        && cache_write_tokens.is_none()
        && reasoning_tokens.is_none()
        && cost.is_none()
    {
        return None;
    }
    Some(json!({
        "prompt_tokens": prompt_tokens,
        "completion_tokens": completion_tokens,
        "total_tokens": total_tokens,
        "prompt_tokens_details": {
            "cached_tokens": cached_tokens,
            "cache_write_tokens": cache_write_tokens,
        },
        "completion_tokens_details": {
            "reasoning_tokens": reasoning_tokens,
        },
        "cost": cost,
    }))
}

fn usage_delta(usage: &Value) -> DocumentTaskUsageDelta {
    DocumentTaskUsageDelta {
        prompt_tokens: nonnegative_integer(usage.get("prompt_tokens")),
        completion_tokens: nonnegative_integer(usage.get("completion_tokens")),
        reasoning_tokens: nonnegative_integer(
            usage.pointer("/completion_tokens_details/reasoning_tokens"),
        ),
        total_tokens: nonnegative_integer(usage.get("total_tokens")),
        cached_prompt_tokens: nonnegative_integer(
            usage.pointer("/prompt_tokens_details/cached_tokens"),
        ),
        cache_write_tokens: nonnegative_integer(
            usage.pointer("/prompt_tokens_details/cache_write_tokens"),
        ),
    }
}

fn nonnegative_integer(value: Option<&Value>) -> Option<u64> {
    value.and_then(|value| {
        value
            .as_u64()
            .or_else(|| value.as_i64().and_then(|value| u64::try_from(value).ok()))
    })
}

fn provider_error_object(value: &Value) -> Option<&Value> {
    value
        .get("error")
        .filter(|error| !error.is_null())
        .or_else(|| {
            value
                .pointer("/choices/0/error")
                .filter(|error| !error.is_null())
        })
}

fn provider_error_code(error_value: &Value) -> String {
    error_value
        .get("code")
        .and_then(Value::as_str)
        .and_then(|value| safe_metadata_string(value, 120))
        .or_else(|| {
            error_value
                .get("code")
                .and_then(Value::as_i64)
                .map(|value| value.to_string())
        })
        .unwrap_or_else(|| "unknown".to_string())
}

fn provider_error_status(error_value: &Value) -> Option<u16> {
    error_value
        .get("code")
        .and_then(Value::as_u64)
        .and_then(|value| u16::try_from(value).ok())
        .or_else(|| {
            error_value
                .get("code")
                .and_then(Value::as_i64)
                .and_then(|value| u16::try_from(value).ok())
        })
        .or_else(|| {
            error_value
                .get("code")
                .and_then(Value::as_str)
                .and_then(|value| value.parse::<u16>().ok())
        })
}

fn safe_json_string(value: Option<&Value>, maximum_chars: usize) -> Option<String> {
    value
        .and_then(Value::as_str)
        .and_then(|value| safe_metadata_string(value, maximum_chars))
}

fn safe_metadata_string(value: &str, maximum_chars: usize) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() || trimmed.chars().any(char::is_control) {
        return None;
    }
    Some(trimmed.chars().take(maximum_chars).collect())
}

fn selected_openrouter_provider(value: &Value) -> Option<String> {
    safe_json_string(value.get("provider"), MAX_SAFE_PROVIDER_METADATA_CHARS).or_else(|| {
        value
            .pointer("/openrouter_metadata/endpoints/available")
            .and_then(Value::as_array)
            .and_then(|endpoints| {
                endpoints.iter().find(|endpoint| {
                    endpoint.get("selected").and_then(Value::as_bool) == Some(true)
                })
            })
            .and_then(|endpoint| {
                safe_json_string(endpoint.get("provider"), MAX_SAFE_PROVIDER_METADATA_CHARS)
            })
    })
}

fn safe_openrouter_attempts(value: &Value) -> Value {
    let attempts = value
        .pointer("/openrouter_metadata/attempts")
        .and_then(Value::as_array)
        .map(|attempts| {
            attempts
                .iter()
                .take(8)
                .map(|attempt| {
                    json!({
                        "provider": safe_json_string(
                            attempt.get("provider"),
                            MAX_SAFE_PROVIDER_METADATA_CHARS,
                        ),
                        "model": safe_json_string(
                            attempt.get("model"),
                            MAX_SAFE_PROVIDER_METADATA_CHARS,
                        ),
                        "status": attempt.get("status").and_then(Value::as_u64),
                    })
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    Value::Array(attempts)
}

fn classify_provider_error_response(
    status: u16,
    retry_after_seconds: Option<u64>,
    body: &[u8],
    openrouter: bool,
    generation_id: Option<&str>,
    streaming: bool,
) -> AiGatewayError {
    if openrouter {
        if let Ok(value) = serde_json::from_slice::<Value>(body) {
            if provider_error_object(&value).is_some() {
                return classify_provider_error_value(
                    &value,
                    Some(status),
                    retry_after_seconds,
                    true,
                    generation_id,
                    streaming,
                );
            }
        }
    }
    classify_http_status(status, retry_after_seconds)
}

fn classify_stream_event_error(
    value: &Value,
    openrouter: bool,
    generation_id: Option<&str>,
    retry_after_seconds: Option<u64>,
) -> AiGatewayError {
    if provider_error_object(value).is_some() {
        return classify_provider_error_value(
            value,
            None,
            retry_after_seconds,
            openrouter,
            generation_id,
            true,
        );
    }
    error(
        "ai_stream_interrupted",
        "The AI provider ended the streaming response with an unrecognized error event. Partial output was discarded.",
        true,
        retry_after_seconds,
    )
}

fn classify_provider_error_value(
    value: &Value,
    http_status: Option<u16>,
    retry_after_seconds: Option<u64>,
    openrouter: bool,
    generation_id: Option<&str>,
    streaming: bool,
) -> AiGatewayError {
    let null_error = Value::Null;
    let error_value = provider_error_object(value).unwrap_or(&null_error);
    let code = provider_error_code(error_value);
    let provider_status =
        provider_error_status(error_value).or_else(|| http_status.filter(|status| *status >= 400));
    let error_type = safe_json_string(
        error_value
            .pointer("/metadata/error_type")
            .or_else(|| error_value.get("error_type")),
        120,
    );
    let provider_code = safe_json_string(
        error_value.pointer("/metadata/provider_code"),
        MAX_SAFE_PROVIDER_METADATA_CHARS,
    );
    let upstream_provider = selected_openrouter_provider(value);
    let model =
        safe_json_string(value.get("model"), MAX_SAFE_PROVIDER_METADATA_CHARS).or_else(|| {
            safe_json_string(
                value.pointer("/openrouter_metadata/requested"),
                MAX_SAFE_PROVIDER_METADATA_CHARS,
            )
        });
    let finish_reason = safe_json_string(value.pointer("/choices/0/finish_reason"), 120);
    let native_finish_reason =
        safe_json_string(value.pointer("/choices/0/native_finish_reason"), 120);
    let routing_strategy = safe_json_string(value.pointer("/openrouter_metadata/strategy"), 120);
    let routing_summary = safe_json_string(
        value.pointer("/openrouter_metadata/summary"),
        MAX_SAFE_PROVIDER_METADATA_CHARS,
    );
    let routing_attempt = value
        .pointer("/openrouter_metadata/attempt")
        .and_then(Value::as_u64);
    let normalized_code = code.to_ascii_lowercase();
    let normalized_type = error_type
        .as_deref()
        .unwrap_or(normalized_code.as_str())
        .to_ascii_lowercase();
    let status_label = provider_status
        .map(|status| format!(" (HTTP {status})"))
        .unwrap_or_default();
    let route_label = upstream_provider
        .as_deref()
        .map(|provider| format!(" through {provider}"))
        .unwrap_or_default();
    let partial_label = if streaming {
        " Partial output was discarded."
    } else {
        ""
    };
    let (safe_code, message, retryable) = match normalized_type.as_str() {
        "authentication" | "unauthorized" | "401" => (
            "ai_authentication_failed",
            format!("The AI provider rejected the saved credential{status_label}."),
            false,
        ),
        "insufficient_credits" | "402" => (
            "ai_insufficient_credits",
            format!("The AI provider account does not have enough credits for this request{status_label}."),
            false,
        ),
        "rate_limit_exceeded" | "429" => (
            "ai_rate_limited",
            format!("The AI provider rate-limited the request{status_label}."),
            true,
        ),
        "provider_unavailable" | "502" => (
            "ai_provider_unavailable",
            if openrouter {
                format!(
                    "OpenRouter reported provider_unavailable{status_label}{route_label}: the upstream model endpoint became unavailable or returned an invalid or empty response.{partial_label}"
                )
            } else {
                format!(
                    "The upstream model endpoint became unavailable or returned an invalid response{status_label}.{partial_label}"
                )
            },
            true,
        ),
        "provider_overloaded" | "529" => (
            "ai_provider_overloaded",
            if openrouter {
                format!(
                    "OpenRouter reported provider_overloaded{status_label}{route_label}: the upstream model endpoint is temporarily overloaded.{partial_label}"
                )
            } else {
                format!("The AI provider is temporarily overloaded{status_label}.{partial_label}")
            },
            true,
        ),
        "no_available_providers" => (
            "ai_no_compatible_endpoint",
            format!(
                "No compatible model endpoint was available for the requested parameters and privacy policy{status_label}."
            ),
            true,
        ),
        "timeout" | "504" | "408" => (
            "ai_provider_timeout",
            if openrouter {
                format!(
                    "OpenRouter reported a provider timeout{status_label}{route_label}.{partial_label}"
                )
            } else {
                format!("The AI provider timed out{status_label}.{partial_label}")
            },
            true,
        ),
        "context_length_exceeded" => (
            "ai_context_length_exceeded",
            format!(
                "The provider rejected the request because it exceeded the model context window{status_label}."
            ),
            false,
        ),
        "max_tokens_exceeded" | "token_limit_exceeded" | "string_too_long" => (
            "ai_output_limit_reached",
            format!(
                "The provider reached a model/output token limit before the structured result completed{status_label}.{partial_label}"
            ),
            false,
        ),
        "content_policy_violation" | "refusal" => (
            "ai_content_policy_rejected",
            format!("The provider stopped this request because of its content policy{status_label}."),
            false,
        ),
        "invalid_request" | "invalid_prompt" | "unprocessable" | "400" | "422" => (
            "ai_http_bad_request",
            format!(
                "The AI provider rejected this request as unsupported or invalid{status_label}. Verify the selected model and provider capabilities, then retry."
            ),
            false,
        ),
        "server" | "server_error" | "500" => (
            "ai_server_unavailable",
            format!("The AI provider reported an internal server error{status_label}.{partial_label}"),
            true,
        ),
        "503" => (
            "ai_server_unavailable",
            format!(
                "The AI provider reported HTTP 503: routing capacity or a compatible upstream endpoint was unavailable.{partial_label}"
            ),
            true,
        ),
        _ => (
            if streaming {
                "ai_stream_interrupted"
            } else {
                "ai_provider_error"
            },
            if openrouter {
                format!(
                    "OpenRouter reported provider error type {}{status_label}{route_label}.{partial_label}",
                    error_type.as_deref().unwrap_or(code.as_str())
                )
            } else {
                format!("The AI provider reported an error{status_label}.{partial_label}")
            },
            true,
        ),
    };

    crate::logging::warn_target(
        if openrouter {
            "ai.gateway.openrouter"
        } else {
            "ai.gateway.streaming"
        },
        if streaming {
            "provider_stream_error"
        } else {
            "provider_response_error"
        },
        &message,
        json!({
            "transport": if streaming { "sse" } else { "json" },
            "providerStatus": provider_status,
            "errorType": error_type.as_deref(),
            "providerCode": provider_code.as_deref(),
            "upstreamProvider": upstream_provider.as_deref(),
            "model": model.as_deref(),
            "finishReason": finish_reason.as_deref(),
            "nativeFinishReason": native_finish_reason.as_deref(),
            "routingStrategy": routing_strategy.as_deref(),
            "routingAttempt": routing_attempt,
            "routingSummary": routing_summary.as_deref(),
            "routingAttempts": safe_openrouter_attempts(value),
            "generationId": generation_id,
        }),
    );
    error(safe_code, &message, retryable, retry_after_seconds)
}

fn stream_progress_timeout_error(timeout_seconds: u64) -> AiGatewayError {
    error(
        "ai_stream_progress_timeout",
        &format!(
            "The AI provider produced no usable streamed output for {timeout_seconds} seconds. ostudy canceled this stalled request so the document can retry or resume without waiting indefinitely."
        ),
        true,
        None,
    )
}

fn empty_stream_failure(finish_reason: &str) -> AiGatewayError {
    let code = if finish_reason == "length" {
        "ai_document_output_truncated"
    } else {
        "ai_empty_response"
    };
    error(
        code,
        &format!(
            "The provider ended the streamed document request with finish_reason={finish_reason}, but returned no final content or structured tool arguments."
        ),
        true,
        None,
    )
}

fn should_retry_document_stream_error(error_value: &AiGatewayError) -> bool {
    matches!(
        error_value.safe.code.as_str(),
        "ai_provider_unavailable"
            | "ai_provider_overloaded"
            | "ai_provider_timeout"
            | "ai_stream_interrupted"
            | "ai_server_unavailable"
            | "ai_network_error"
            | "ai_connection_failed"
            | "ai_request_timeout"
            | "ai_initial_response_timeout"
            | "ai_rate_limited"
            | "ai_http_rate_limited"
    )
}

fn document_retry_delay(error_value: &AiGatewayError) -> Duration {
    match error_value.safe.retry_after_seconds {
        Some(seconds @ 1..=5) => Duration::from_secs(seconds),
        _ => Duration::from_millis(DOCUMENT_RETRY_DELAY_MILLIS),
    }
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
        assert_eq!(failure.safe.code, "ai_server_unavailable");
        assert!(!failure.safe.message.contains("private upstream detail"));
    }

    #[test]
    fn openrouter_midstream_502_preserves_typed_safe_failure_context() {
        let mut stream = SseChatAccumulator::for_request(
            1_024,
            16_384,
            true,
            Some("gen-test-123".to_string()),
            None,
        );
        stream
            .push_bytes(b"data: {\"choices\":[{\"delta\":{\"content\":\"partial\"}}]}\n")
            .expect("first delta should parse");
        let failure = stream
            .push_bytes(
                br#"data: {"model":"deepseek/deepseek-v4-flash-0731","provider":"DeepSeek","error":{"code":502,"message":"private upstream detail","metadata":{"error_type":"provider_unavailable","provider_code":"upstream_bad_gateway"}},"choices":[{"delta":{"content":""},"finish_reason":"error","native_finish_reason":"provider_error"}],"openrouter_metadata":{"strategy":"fallback","attempt":2,"summary":"available=3, selected=DeepSeek","attempts":[{"provider":"ExampleOne","model":"deepseek/deepseek-v4-flash-0731","status":502},{"provider":"DeepSeek","model":"deepseek/deepseek-v4-flash-0731","status":502}]}}
"#,
            )
            .expect_err("OpenRouter 502 should abort the partial response");
        assert_eq!(failure.safe.code, "ai_provider_unavailable");
        assert!(failure.safe.retryable);
        assert!(failure.safe.message.contains("provider_unavailable"));
        assert!(failure.safe.message.contains("HTTP 502"));
        assert!(failure.safe.message.contains("DeepSeek"));
        assert!(failure
            .safe
            .message
            .contains("Partial output was discarded"));
        assert!(!failure.safe.message.contains("private upstream detail"));
    }

    #[test]
    fn openrouter_http_errors_use_typed_envelope_without_raw_provider_message() {
        let failure = classify_provider_error_response(
            503,
            Some(2),
            br#"{"error":{"code":503,"message":"private capacity detail","metadata":{"error_type":"provider_overloaded","provider_code":"capacity"}},"openrouter_metadata":{"strategy":"fallback","attempt":3,"summary":"available=2"}}"#,
            true,
            Some("gen-http-test"),
            false,
        );
        assert_eq!(failure.safe.code, "ai_provider_overloaded");
        assert_eq!(failure.safe.retry_after_seconds, Some(2));
        assert!(failure.safe.message.contains("provider_overloaded"));
        assert!(!failure.safe.message.contains("private capacity detail"));
    }

    #[test]
    fn empty_length_finished_stream_is_output_truncation() {
        let failure = empty_stream_failure("length");
        assert_eq!(failure.safe.code, "ai_document_output_truncated");
        assert!(!should_retry_document_stream_error(&failure));

        let ordinary_empty = empty_stream_failure("stop");
        assert_eq!(ordinary_empty.safe.code, "ai_empty_response");
    }

    #[test]
    fn keepalive_comments_do_not_count_as_useful_model_progress() {
        let mut stream = SseChatAccumulator::new(1_024, 16_384);
        stream
            .push_bytes(b": OPENROUTER PROCESSING\n")
            .expect("SSE keepalive should be accepted");
        assert_eq!(stream.useful_progress_bytes(), 0);
        stream
            .push_bytes(b"data: {\"choices\":[{\"delta\":{\"content\":\"{\"}}]}\n")
            .expect("model content should be accepted");
        assert!(stream.useful_progress_bytes() > 0);
    }

    #[test]
    fn progress_timeout_is_not_retried_identically_by_the_gateway() {
        let failure = stream_progress_timeout_error(OPENROUTER_STREAM_PROGRESS_TIMEOUT_SECONDS);
        assert_eq!(failure.safe.code, "ai_stream_progress_timeout");
        assert!(!should_retry_document_stream_error(&failure));
    }

    #[test]
    fn document_stream_retry_is_limited_to_transient_transport_failures() {
        assert!(should_retry_document_stream_error(&error(
            "ai_provider_unavailable",
            "unavailable",
            true,
            None,
        )));
        assert!(should_retry_document_stream_error(&error(
            "ai_network_error",
            "network",
            true,
            None,
        )));
        assert!(!should_retry_document_stream_error(&error(
            "ai_authentication_failed",
            "credential",
            false,
            None,
        )));
        assert!(!should_retry_document_stream_error(&error(
            "ai_context_length_exceeded",
            "context",
            false,
            None,
        )));
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
    fn openrouter_posts_disable_provider_side_context_compression_and_apply_document_policy() {
        let url = Url::parse("https://openrouter.ai/api/v1/chat/completions").unwrap();
        let prepared = prepare_provider_body_for_document(
            &url,
            &json!({ "model": "example/model" }),
            Some("low"),
        )
        .expect("OpenRouter requests should be normalized");
        assert_eq!(prepared["plugins"][0]["id"], "context-compression");
        assert_eq!(prepared["plugins"][0]["enabled"], false);
        assert_eq!(prepared["usage"]["include"], true);
        assert!(prepared.get("session_id").is_none());
        assert_eq!(prepared["reasoning"]["effort"], "low");
        assert_eq!(prepared["reasoning"]["exclude"], true);

        let automatic = prepare_provider_body_for_document(
            &url,
            &json!({ "model": "example/model" }),
            Some("automatic"),
        )
        .unwrap();
        assert!(automatic.get("reasoning").is_none());
        assert!(prepare_provider_body(
            &Url::parse("https://api.deepseek.com/chat/completions").unwrap(),
            &json!({})
        )
        .is_none());
    }

    #[test]
    fn streaming_usage_preserves_reasoning_and_cache_counters_without_content() {
        let mut stream = SseChatAccumulator::new(2_048, 8_192);
        stream
            .push_bytes(
                br#"data: {"choices":[{"delta":{"content":"{}"},"finish_reason":"stop"}]}
data: {"usage":{"prompt_tokens":1000,"completion_tokens":250,"total_tokens":1250,"prompt_tokens_details":{"cached_tokens":700,"cache_write_tokens":120},"completion_tokens_details":{"reasoning_tokens":80},"cost":0.0003}}
data: [DONE]
"#,
            )
            .unwrap();
        stream.finish().unwrap();
        let usage = stream.usage.expect("usage should be retained");
        assert_eq!(usage["prompt_tokens"], 1_000);
        assert_eq!(usage["completion_tokens_details"]["reasoning_tokens"], 80);
        assert_eq!(usage["prompt_tokens_details"]["cached_tokens"], 700);
        assert_eq!(usage["prompt_tokens_details"]["cache_write_tokens"], 120);
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
