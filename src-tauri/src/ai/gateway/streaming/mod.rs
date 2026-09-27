pub(super) mod sse;
pub(super) mod usage;

use std::time::Duration;

use serde_json::{json, Value};
use tokio::time::{sleep, timeout, Instant};
use url::Url;

use crate::{
    ai::generation::{reserve_current_recovery, RecoveryReason},
    task_cancellation::{
        current_signal, record_current_provider_activity, record_current_provider_request,
        record_current_usage, CancellationSignal,
    },
};

use super::{
    client::AiGateway,
    contracts::AiGatewayError,
    errors::{
        canceled_error, classify_provider_error_response, classify_reqwest_error,
        empty_stream_failure, error, initial_timeout_error, stream_inactivity_error,
    },
    metadata::{is_openrouter_url, safe_metadata_string, MAX_SAFE_PROVIDER_METADATA_CHARS},
    request::{prepare_streaming_body, streaming_request},
    response::{generation_id, read_bounded_body, retry_after_seconds, MAX_ERROR_BODY_BYTES},
    retry::{document_retry_delay, should_retry_document_stream_error},
};
use sse::SseChatAccumulator;
use usage::usage_delta;

const STREAM_WIRE_MULTIPLIER: usize = 16;

/// Owned by the physical attempt, not its successful decode path. A failure,
/// cancellation or empty reasoning-only completion must not erase reported cost.
struct StreamCapture {
    maximum_content_bytes: usize,
    stream: Option<SseChatAccumulator>,
}

impl StreamCapture {
    fn usage(&self) -> Option<&Value> {
        self.stream.as_ref().and_then(|stream| stream.usage.as_ref())
    }

    fn observation(&self) -> Value {
        let mut observation = json!({});
        if let Some(stream) = &self.stream {
            if let Some(usage) = &stream.usage {
                observation["usage"] = usage.clone();
            }
            observation["id"] = json!(stream.generation_id);
            observation["choices"] = json!([{"finish_reason": stream.finish_reason}]);
        }
        observation
    }
}

impl AiGateway {
    /// Interactive tasks supply their own cancellation lifetime rather than
    /// impersonating a document job. The same SSE framing, inactivity limits,
    /// response bounds and redaction remain authoritative for both task types.
    pub(crate) async fn post_task_json(
        &self,
        url: Url,
        bearer: Option<&str>,
        body: &Value,
        inactivity_timeout_ms: i64,
        maximum_content_bytes: usize,
        cancellation: &CancellationSignal,
    ) -> Result<Value, AiGatewayError> {
        self.post_bounded_streaming_json(
            url,
            bearer,
            body,
            inactivity_timeout_ms,
            maximum_content_bytes,
            cancellation,
        )
        .await
    }

    pub(crate) async fn post_document_json(
        &self,
        url: Url,
        bearer: Option<&str>,
        body: &Value,
        inactivity_timeout_ms: i64,
        maximum_content_bytes: usize,
    ) -> Result<Value, AiGatewayError> {
        let Some(cancellation) = current_signal() else {
            return Err(error(
                "ai_document_task_scope_missing",
                "A document AI request was started outside its cancellable document task scope.",
                false,
                None,
            ));
        };
        self.post_bounded_streaming_json(
            url,
            bearer,
            body,
            inactivity_timeout_ms,
            maximum_content_bytes,
            &cancellation,
        )
        .await
    }

    async fn post_bounded_streaming_json(
        &self,
        url: Url,
        bearer: Option<&str>,
        body: &Value,
        inactivity_timeout_ms: i64,
        maximum_content_bytes: usize,
        cancellation: &CancellationSignal,
    ) -> Result<Value, AiGatewayError> {
        let first = self
            .post_sse_chat_json(
                url.clone(),
                bearer,
                body,
                inactivity_timeout_ms,
                maximum_content_bytes,
                cancellation,
            )
            .await;
        match first {
            Err(first_error)
                if should_retry_document_stream_error(&first_error)
                    && reserve_current_recovery(
                        RecoveryReason::TransientTransport,
                        &first_error.safe.code,
                    ) =>
            {
                let delay = document_retry_delay(&first_error);
                let delay_ms = u64::try_from(delay.as_millis()).unwrap_or(u64::MAX);
                let model = body.get("model").and_then(Value::as_str).and_then(|value| {
                    safe_metadata_string(value, MAX_SAFE_PROVIDER_METADATA_CHARS)
                });
                crate::logging::warn_target(
                    "ai.gateway.retry",
                    "provider_request_retry",
                    "A transient AI provider failure caused one bounded logical task to be retried from scratch.",
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
                match self
                    .post_sse_chat_json(
                        url.clone(),
                        bearer,
                        body,
                        inactivity_timeout_ms,
                        maximum_content_bytes,
                        cancellation,
                    )
                    .await
                {
                    Ok(value) => {
                        crate::logging::info_target(
                            "ai.gateway.retry",
                            "provider_request_retry_succeeded",
                            "The bounded logical task succeeded on its one transient retry.",
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
                            "The bounded logical task failed again after its transient retry.",
                            json!({
                                "endpointHost": url.host_str(),
                                "model": model.as_deref(),
                                "firstErrorCode": first_error.safe.code.as_str(),
                                "secondErrorCode": second_error.safe.code.as_str(),
                            }),
                        );
                        second_error.safe.message = format!(
                            "{} ostudy used the logical task's bounded transient recovery once, but that retry also failed.",
                            second_error.safe.message
                        );
                        Err(second_error)
                    }
                }
            }
            other => other,
        }
    }

    async fn post_sse_chat_json(
        &self,
        url: Url,
        bearer: Option<&str>,
        body: &Value,
        inactivity_timeout_ms: i64,
        maximum_content_bytes: usize,
        cancellation: &CancellationSignal,
    ) -> Result<Value, AiGatewayError> {
        if cancellation.is_canceled() {
            return Err(canceled_error());
        }
        record_current_provider_request();
        let mut attempt = crate::ai::generation::start_current_attempt(body);
        let mut capture = StreamCapture {
            maximum_content_bytes,
            stream: None,
        };
        let result = self
            .post_sse_chat_json_inner(
                url,
                bearer,
                body,
                inactivity_timeout_ms,
                &mut capture,
                cancellation,
            )
            .await;
        // Exactly once per physical request, whether decoding succeeds or fails.
        // Missing usage stays unknown; a failed result does not imply zero cost.
        record_current_usage(capture.usage().map(usage_delta).unwrap_or_default());
        if let Some(attempt) = attempt.as_mut() {
            match &result {
                Ok(response) => attempt.response(response),
                Err(error) => attempt.failed_with_observation(&error.safe.code, &capture.observation()),
            }
        }
        result
    }

    async fn post_sse_chat_json_inner(
        &self,
        url: Url,
        bearer: Option<&str>,
        body: &Value,
        inactivity_timeout_ms: i64,
        capture: &mut StreamCapture,
        cancellation: &CancellationSignal,
    ) -> Result<Value, AiGatewayError> {
        if cancellation.is_canceled() {
            return Err(canceled_error());
        }
        let inactivity_timeout = Duration::from_millis(inactivity_timeout_ms.max(1) as u64);
        let request_started = Instant::now();
        let streaming_body = prepare_streaming_body(body)?;
        let openrouter = is_openrouter_url(&url);
        let endpoint_host = url
            .host_str()
            .and_then(|value| safe_metadata_string(value, MAX_SAFE_PROVIDER_METADATA_CHARS));
        let requested_model = body
            .get("model")
            .and_then(Value::as_str)
            .and_then(|value| safe_metadata_string(value, MAX_SAFE_PROVIDER_METADATA_CHARS));
        let request = streaming_request(&self.streaming_client, url, bearer, &streaming_body);

        let mut response = wait_initial(
            inactivity_timeout,
            cancellation,
            request.send(),
            "The AI provider sent no response headers before the configured inactivity timeout.",
        )
        .await?;
        record_current_provider_activity(false);
        let status = response.status();
        let retry_after_seconds = retry_after_seconds(&response);
        let generation_id = generation_id(&response);

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
            result = timeout(inactivity_timeout, response.chunk()) => match result {
                Ok(result) => result.map_err(classify_reqwest_error)?,
                Err(_) => return Err(stream_inactivity_error(inactivity_timeout_ms)),
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

        let maximum_content_bytes = capture.maximum_content_bytes;
        let maximum_wire_bytes = maximum_content_bytes
            .saturating_mul(STREAM_WIRE_MULTIPLIER)
            .max(maximum_content_bytes);
        let stream = capture.stream.insert(SseChatAccumulator::for_request(
            maximum_content_bytes,
            maximum_wire_bytes,
            openrouter,
            generation_id,
            retry_after_seconds,
        ));
        record_current_provider_activity(false);
        let before_first = stream.model_progress_bytes();
        stream.push_bytes(&first_chunk)?;
        let mut model_progress_bytes = stream.model_progress_bytes();
        let mut first_model_output_ms = None;
        if model_progress_bytes > before_first {
            record_current_provider_activity(true);
            first_model_output_ms =
                Some(u64::try_from(request_started.elapsed().as_millis()).unwrap_or(u64::MAX));
        }

        while !stream.done {
            let next = tokio::select! {
                _ = cancellation.canceled() => return Err(canceled_error()),
                result = timeout(inactivity_timeout, response.chunk()) => match result {
                    Ok(result) => result.map_err(classify_reqwest_error)?,
                    Err(_) => return Err(stream_inactivity_error(inactivity_timeout_ms)),
                },
            };
            let Some(chunk) = next else {
                break;
            };
            record_current_provider_activity(false);
            stream.push_bytes(&chunk)?;
            let next_progress_bytes = stream.model_progress_bytes();
            if next_progress_bytes > model_progress_bytes {
                model_progress_bytes = next_progress_bytes;
                record_current_provider_activity(true);
                if first_model_output_ms.is_none() {
                    first_model_output_ms = Some(
                        u64::try_from(request_started.elapsed().as_millis()).unwrap_or(u64::MAX),
                    );
                }
            }
        }
        stream.finish()?;

        if stream.content.trim().is_empty() && stream.tool_arguments_len() == 0 {
            let finish = stream.finish_reason.as_deref().unwrap_or("not provided");
            return Err(empty_stream_failure(finish));
        }
        let usage = stream.usage.clone();
        crate::logging::debug_target(
            "ai.gateway.streaming",
            "provider_stream_completed",
            "A provider stream completed without logging source, model output, or reasoning content.",
            json!({
                "endpointHost": endpoint_host.as_deref(),
                "generationId": stream.generation_id.as_deref(),
                "upstreamProvider": stream.upstream_provider.as_deref(),
                "model": stream.model.as_deref().or(requested_model.as_deref()),
                "finishReason": stream.finish_reason.as_deref(),
                "durationMs": u64::try_from(request_started.elapsed().as_millis()).unwrap_or(u64::MAX),
                "firstModelOutputMs": first_model_output_ms,
                "keepaliveEvents": stream.comment_events,
                "contentBytes": stream.content.len(),
                "toolArgumentBytes": stream.tool_arguments_len(),
                "toolCallCount": stream.tool_calls().len(),
                "reasoningBytesObserved": stream.reasoning_bytes,
                "usageReported": usage.is_some(),
            }),
        );
        let calls = stream.tool_calls();
        let content = std::mem::take(&mut stream.content);
        let message = if calls.is_empty() {
            json!({ "content": content })
        } else {
            json!({
                "content": if content.trim().is_empty() { Value::Null } else { Value::String(content) },
                "tool_calls": calls
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

#[cfg(test)]
mod accounting_tests;
