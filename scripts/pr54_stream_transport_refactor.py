#!/usr/bin/env python3
"""Apply the PR #54 provider-neutral document streaming/liveness refactor.

The script is a one-shot delivery mechanism. Product code remains ordinary Rust
and TypeScript; this helper is removed after exact-head qualification.
"""
from __future__ import annotations

import re
from pathlib import Path


def replace_once(path: str, old: str, new: str, label: str) -> None:
    p = Path(path)
    value = p.read_text()
    if new in value:
        return
    count = value.count(old)
    if count != 1:
        raise SystemExit(f"{label}: expected one anchor in {path}, found {count}")
    p.write_text(value.replace(old, new, 1))


def remove_once(path: str, old: str, label: str) -> None:
    p = Path(path)
    value = p.read_text()
    count = value.count(old)
    if count == 0:
        return
    if count != 1:
        raise SystemExit(f"{label}: expected at most one anchor in {path}, found {count}")
    p.write_text(value.replace(old, "", 1))


def regex_once(path: str, pattern: str, replacement: str, label: str) -> None:
    p = Path(path)
    value = p.read_text()
    updated, count = re.subn(pattern, replacement, value, count=1, flags=re.DOTALL)
    if count != 1:
        if replacement in value:
            return
        raise SystemExit(f"{label}: regex anchor not found in {path}")
    p.write_text(updated)


# ---------------------------------------------------------------------------
# Task-local provider activity: report transport responsiveness separately from
# model-output progress without storing provider content or reasoning.
# ---------------------------------------------------------------------------
path = "src-tauri/src/task_cancellation.rs"
replace_once(
    path,
    "    pub usage_reported_requests: u64,\n    pub usage_unreported_requests: u64,",
    "    pub usage_reported_requests: u64,\n    pub usage_unreported_requests: u64,\n    pub provider_activity: String,\n    pub last_provider_activity_at: Option<i64>,\n    pub last_model_activity_at: Option<i64>,",
    "provider activity progress fields",
)
replace_once(
    path,
    "            usage_reported_requests: 0,\n            usage_unreported_requests: 0,",
    "            usage_reported_requests: 0,\n            usage_unreported_requests: 0,\n            provider_activity: \"preparing\".to_string(),\n            last_provider_activity_at: None,\n            last_model_activity_at: None,",
    "provider activity progress defaults",
)
replace_once(
    path,
    '''struct ActiveDocumentTask {
    document_id: String,
    signal: CancellationSignal,
    progress: Arc<Mutex<DocumentTaskProgress>>,
}

impl ActiveDocumentTask {
    fn new(document_id: &str, started_at: i64) -> Self {
        Self {
            document_id: document_id.to_string(),
            signal: CancellationSignal::new(),
            progress: Arc::new(Mutex::new(DocumentTaskProgress::new(started_at))),
        }
    }
}''',
    '''struct ActiveDocumentTask {
    signal: CancellationSignal,
    progress: Arc<Mutex<DocumentTaskProgress>>,
}

impl ActiveDocumentTask {
    fn new(started_at: i64) -> Self {
        Self {
            signal: CancellationSignal::new(),
            progress: Arc::new(Mutex::new(DocumentTaskProgress::new(started_at))),
        }
    }
}''',
    "remove obsolete task-local document id",
)
replace_once(
    path,
    "        let task = ActiveDocumentTask::new(document_id, unix_time_millis()?);",
    "        let task = ActiveDocumentTask::new(unix_time_millis()?);",
    "active task constructor",
)
remove_once(
    path,
    '''pub(crate) fn current_document_id() -> Option<String> {
    CURRENT_DOCUMENT_TASK
        .try_with(|task| task.document_id.clone())
        .ok()
}

''',
    "obsolete task-local document-id accessor",
)
old_phase = '''pub(crate) fn set_current_task_phase(
    phase: &str,
    current_window: Option<usize>,
    completed_windows: usize,
    total_windows: usize,
    current_page_start: Option<i64>,
    current_page_end: Option<i64>,
    current_partition: Option<usize>,
) {
    set_current_task_phase_range(
        phase,
        current_window,
        current_window,
        completed_windows,
        total_windows,
        current_page_start,
        current_page_end,
        current_partition,
    );
}

pub(crate) fn set_current_task_phase_range(
    phase: &str,
    current_window: Option<usize>,
    current_window_end: Option<usize>,
    completed_windows: usize,
    total_windows: usize,
    current_page_start: Option<i64>,
    current_page_end: Option<i64>,
    current_partition: Option<usize>,
) {
    let _ = CURRENT_DOCUMENT_TASK.try_with(|task| {
        if let Ok(mut progress) = task.progress.lock() {
            progress.phase = phase.chars().take(80).collect();
            progress.current_window = current_window;
            progress.current_window_end = current_window_end;
            progress.completed_windows = completed_windows;
            progress.total_windows = total_windows;
            progress.current_page_start = current_page_start;
            progress.current_page_end = current_page_end;
            progress.current_partition = current_partition;
        }
    });
}
'''
new_phase = '''pub(crate) fn set_current_task_phase(
    phase: &str,
    current_window: Option<usize>,
    completed_windows: usize,
    total_windows: usize,
    current_page_start: Option<i64>,
    current_page_end: Option<i64>,
    current_partition: Option<usize>,
) {
    set_current_task_phase_range(
        phase,
        (current_window, current_window),
        completed_windows,
        total_windows,
        current_page_start,
        current_page_end,
        current_partition,
    );
}

pub(crate) fn set_current_task_phase_range(
    phase: &str,
    current_windows: (Option<usize>, Option<usize>),
    completed_windows: usize,
    total_windows: usize,
    current_page_start: Option<i64>,
    current_page_end: Option<i64>,
    current_partition: Option<usize>,
) {
    let (current_window, current_window_end) = current_windows;
    let _ = CURRENT_DOCUMENT_TASK.try_with(|task| {
        if let Ok(mut progress) = task.progress.lock() {
            progress.phase = phase.chars().take(80).collect();
            progress.current_window = current_window;
            progress.current_window_end = current_window_end;
            progress.completed_windows = completed_windows;
            progress.total_windows = total_windows;
            progress.current_page_start = current_page_start;
            progress.current_page_end = current_page_end;
            progress.current_partition = current_partition;
            if matches!(phase, "outline" | "generating" | "curating") {
                progress.provider_activity = "connecting".to_string();
                progress.last_provider_activity_at = None;
                progress.last_model_activity_at = None;
            }
        }
    });
}

pub(crate) fn record_current_provider_activity(model_progress: bool) {
    let now = unix_time_millis().unwrap_or_default();
    let _ = CURRENT_DOCUMENT_TASK.try_with(|task| {
        if let Ok(mut progress) = task.progress.lock() {
            progress.last_provider_activity_at = Some(now);
            if model_progress {
                progress.provider_activity = "streaming".to_string();
                progress.last_model_activity_at = Some(now);
            } else if progress.provider_activity != "streaming" {
                progress.provider_activity = "responsive".to_string();
            }
        }
    });
}
'''
replace_once(path, old_phase, new_phase, "provider activity phase contract")

# ---------------------------------------------------------------------------
# Gateway: one provider-neutral SSE inactivity contract. There is no total
# inference wall clock. Any received SSE bytes, including standards-compliant
# comments/heartbeats, prove that the transport is responsive and reset the
# configurable inactivity timer. Model deltas are tracked separately.
# ---------------------------------------------------------------------------
path = "src-tauri/src/ai/gateway.rs"
replace_once(
    path,
    '''use crate::task_cancellation::{
    current_signal, record_current_provider_request, record_current_provider_retry,
    record_current_usage, set_current_reasoning_effort, CancellationSignal, DocumentTaskUsageDelta,
};''',
    '''use crate::task_cancellation::{
    current_signal, record_current_provider_activity, record_current_provider_request,
    record_current_provider_retry, record_current_usage, set_current_reasoning_effort,
    CancellationSignal, DocumentTaskUsageDelta,
};''',
    "gateway provider-activity import",
)
remove_once(
    path,
    "const OPENROUTER_STREAM_PROGRESS_TIMEOUT_SECONDS: u64 = 75;\n",
    "OpenRouter-only progress timeout",
)
remove_once(
    path,
    "const DEFAULT_STREAM_PROGRESS_TIMEOUT_SECONDS: u64 = 600;\n",
    "provider-specific progress timeout",
)
replace_once(
    path,
    '''        // Long document-generation requests deliberately have no reqwest-wide
        // wall-clock deadline. Connection/header waiting remains bounded, and an
        // accepted stream is governed by useful-output progress plus user cancel.''',
    '''        // Long document-generation requests deliberately have no reqwest-wide
        // wall-clock deadline. Connection/header waiting and transport silence are
        // bounded by the profile's inactivity setting; active streams may run as
        // long as needed and remain exactly cancelable by the document task.''',
    "streaming client contract comment",
)
post_sse = r'''    async fn post_sse_chat_json(
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
            inactivity_timeout,
            cancellation,
            request.send(),
            "The AI provider sent no response headers before the configured inactivity timeout.",
        )
        .await?;
        record_current_provider_activity(false);
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
        record_current_provider_activity(false);
        let before_first = stream.model_progress_bytes();
        stream.push_bytes(&first_chunk)?;
        let mut model_progress_bytes = stream.model_progress_bytes();
        let mut first_model_output_ms = None;
        if model_progress_bytes > before_first {
            record_current_provider_activity(true);
            first_model_output_ms = Some(
                u64::try_from(request_started.elapsed().as_millis()).unwrap_or(u64::MAX),
            );
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

        if stream.content.trim().is_empty() && stream.tool_arguments.trim().is_empty() {
            let finish = stream.finish_reason.as_deref().unwrap_or("not provided");
            return Err(empty_stream_failure(finish));
        }
        let usage = stream.usage.clone();
        if let Some(usage) = usage.as_ref() {
            record_current_usage(usage_delta(usage));
        }
        crate::logging::debug_target(
            "ai.gateway.streaming",
            "provider_stream_completed",
            "A document-provider stream completed without logging source, model output, or reasoning content.",
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
                "toolArgumentBytes": stream.tool_arguments.len(),
                "reasoningBytesObserved": stream.reasoning_bytes,
                "usageReported": usage.is_some(),
            }),
        );
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

'''
regex_once(
    path,
    r"    async fn post_sse_chat_json\(.*?\n    async fn request_json\(",
    post_sse + "    async fn request_json(",
    "provider-neutral SSE implementation",
)
# The old cleanup accidentally inserted streaming-only timeout variables in the
# ordinary bounded JSON path. Remove them if still present.
replace_once(
    path,
    '''        let openrouter = is_openrouter_url(&url);
        let progress_timeout_seconds = if openrouter {
            OPENROUTER_STREAM_PROGRESS_TIMEOUT_SECONDS
        } else {
            DEFAULT_STREAM_PROGRESS_TIMEOUT_SECONDS
        };
        let progress_timeout = Duration::from_secs(progress_timeout_seconds);
        let mut request = self
            .client''',
    '''        let openrouter = is_openrouter_url(&url);
        let mut request = self
            .client''',
    "non-streaming timeout residue",
)
replace_once(
    path,
    '''struct SseChatAccumulator {
    pending: Vec<u8>,
    content: String,
    tool_name: Option<String>,
    tool_arguments: String,
    finish_reason: Option<String>,
    usage: Option<Value>,
    wire_bytes: usize,''',
    '''struct SseChatAccumulator {
    pending: Vec<u8>,
    content: String,
    tool_name: Option<String>,
    tool_arguments: String,
    finish_reason: Option<String>,
    usage: Option<Value>,
    reasoning_bytes: usize,
    comment_events: u64,
    wire_bytes: usize,''',
    "SSE activity counters",
)
replace_once(
    path,
    '''            finish_reason: None,
            usage: None,
            wire_bytes: 0,''',
    '''            finish_reason: None,
            usage: None,
            reasoning_bytes: 0,
            comment_events: 0,
            wire_bytes: 0,''',
    "SSE activity counter defaults",
)
replace_once(
    path,
    '''    fn process_line(&mut self, line: &[u8]) -> Result<(), AiGatewayError> {
        if line.is_empty() || line.starts_with(b":") {
            return Ok(());
        }''',
    '''    fn process_line(&mut self, line: &[u8]) -> Result<(), AiGatewayError> {
        if line.is_empty() {
            return Ok(());
        }
        if line.starts_with(b":") {
            self.comment_events = self.comment_events.saturating_add(1);
            return Ok(());
        }''',
    "SSE keepalive accounting",
)
replace_once(
    path,
    '''        let delta = choice.and_then(|choice| choice.get("delta"));
        if let Some(content) = delta''',
    '''        let delta = choice.and_then(|choice| choice.get("delta"));
        if let Some(reasoning) = delta
            .and_then(|delta| delta.get("reasoning_content"))
            .and_then(Value::as_str)
        {
            self.reasoning_bytes = self.reasoning_bytes.saturating_add(reasoning.len());
        }
        if let Some(content) = delta''',
    "reasoning progress accounting",
)
replace_once(
    path,
    '''    fn useful_progress_bytes(&self) -> usize {
        self.content.len().saturating_add(self.tool_arguments.len())
    }''',
    '''    fn model_progress_bytes(&self) -> usize {
        self.content
            .len()
            .saturating_add(self.tool_arguments.len())
            .saturating_add(self.reasoning_bytes)
    }''',
    "model progress counter",
)
# A generic transport helper requests the standard final usage chunk from every
# implemented OpenAI-compatible provider. OpenRouter accepts but now ignores the
# deprecated option because it reports usage automatically.
insert_anchor = '''fn trim_ascii(mut bytes: &[u8]) -> &[u8] {'''
stream_helper = '''fn prepare_streaming_body(body: &Value) -> Result<Value, AiGatewayError> {
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

'''
replace_once(path, insert_anchor, stream_helper + insert_anchor, "stream request normalization")
remove_once(
    path,
    '''    object.insert("usage".to_string(), json!({ "include": true }));
''',
    "deprecated OpenRouter usage request",
)
regex_once(
    path,
    r"fn safe_usage_envelope\(value: &Value\) -> Option<Value> \{.*?\n\}\n\nfn usage_delta",
    '''fn safe_usage_envelope(value: &Value) -> Option<Value> {
    let read = |pointers: &[&str]| {
        pointers
            .iter()
            .find_map(|pointer| value.pointer(pointer).and_then(|item| nonnegative_integer(Some(item))))
    };
    let prompt_tokens = read(&["/usage/prompt_tokens", "/usage/input_tokens"]);
    let completion_tokens = read(&["/usage/completion_tokens", "/usage/output_tokens"]);
    let total_tokens = read(&["/usage/total_tokens"]).or_else(|| {
        prompt_tokens
            .zip(completion_tokens)
            .map(|(prompt, completion)| prompt.saturating_add(completion))
    });
    let cached_tokens = read(&[
        "/usage/prompt_tokens_details/cached_tokens",
        "/usage/prompt_cache_hit_tokens",
    ]);
    let cache_write_tokens = read(&["/usage/prompt_tokens_details/cache_write_tokens"]);
    let reasoning_tokens = read(&["/usage/completion_tokens_details/reasoning_tokens"]);
    let cost = value
        .pointer("/usage/cost")
        .and_then(Value::as_f64)
        .filter(|value| value.is_finite() && *value >= 0.0)
        .map(Value::from);

    if prompt_tokens.is_some()
        || completion_tokens.is_some()
        || total_tokens.is_some()
        || cached_tokens.is_some()
        || cache_write_tokens.is_some()
        || reasoning_tokens.is_some()
        || cost.is_some()
    {
        return Some(json!({
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
        }));
    }

    // llama.cpp versions differ on whether their final stream event includes a
    // standard usage object, native timings, or both. Normalize timings into the
    // same usage envelope rather than making document processing provider-aware.
    let prompt_processed = read(&[
        "/timings/prompt_n",
        "/__verbose/timings/prompt_n",
        "/tokens_evaluated",
        "/__verbose/tokens_evaluated",
    ]);
    let cached = read(&[
        "/timings/cache_n",
        "/__verbose/timings/cache_n",
        "/tokens_cached",
        "/__verbose/tokens_cached",
    ])
    .unwrap_or(0);
    let completion = read(&[
        "/timings/predicted_n",
        "/__verbose/timings/predicted_n",
        "/tokens_predicted",
        "/__verbose/tokens_predicted",
    ]);
    let prompt = prompt_processed.map(|value| value.saturating_add(cached));
    let total = prompt
        .zip(completion)
        .map(|(prompt, completion)| prompt.saturating_add(completion));
    if prompt.is_none() && completion.is_none() && total.is_none() {
        return None;
    }
    Some(json!({
        "prompt_tokens": prompt,
        "completion_tokens": completion,
        "total_tokens": total,
        "prompt_tokens_details": {
            "cached_tokens": cached,
            "cache_write_tokens": Value::Null,
        },
        "completion_tokens_details": {
            "reasoning_tokens": Value::Null,
        },
        "cost": Value::Null,
    }))
}

fn usage_delta''',
    "provider-neutral usage normalization",
)
replace_once(
    path,
    '''fn stream_progress_timeout_error(timeout_seconds: u64) -> AiGatewayError {
    error(
        "ai_stream_progress_timeout",
        &format!(
            "The AI provider produced no usable streamed output for {timeout_seconds} seconds. ostudy canceled this stalled request so the document can retry or resume without waiting indefinitely."
        ),
        true,
        None,
    )
}''',
    '''fn stream_inactivity_error(timeout_ms: i64) -> AiGatewayError {
    let timeout_seconds = (timeout_ms.max(1) as f64 / 1_000.0).max(0.001);
    error(
        "ai_stream_inactive",
        &format!(
            "The AI provider sent no response data for {timeout_seconds:.1} seconds. ostudy canceled this inactive connection; active generation is never stopped by a total wall-clock deadline."
        ),
        true,
        None,
    )
}''',
    "stream inactivity error",
)
replace_once(
    path,
    '''            | "ai_initial_response_timeout"
            | "ai_rate_limited"''',
    '''            | "ai_initial_response_timeout"
            | "ai_stream_inactive"
            | "ai_rate_limited"''',
    "stream inactivity retry classification",
)

# Gateway regressions: heartbeats prove transport liveness, reasoning deltas are
# model progress, inactivity is transient, and llama timings become usage.
replace_once(
    path,
    '''    #[test]
    fn keepalive_comments_do_not_count_as_useful_model_progress() {
        let mut stream = SseChatAccumulator::new(1_024, 16_384);
        stream
            .push_bytes(b": OPENROUTER PROCESSING\\n")
            .expect("SSE keepalive should be accepted");
        assert_eq!(stream.useful_progress_bytes(), 0);
        stream
            .push_bytes(b"data: {\\"choices\\":[{\\"delta\\":{\\"content\\":\\"{\\"}}]}\\n")
            .expect("model content should be accepted");
        assert!(stream.useful_progress_bytes() > 0);
    }

    #[test]
    fn progress_timeout_is_not_retried_identically_by_the_gateway() {
        let failure = stream_progress_timeout_error(OPENROUTER_STREAM_PROGRESS_TIMEOUT_SECONDS);
        assert_eq!(failure.safe.code, "ai_stream_progress_timeout");
        assert!(!should_retry_document_stream_error(&failure));
    }''',
    '''    #[test]
    fn keepalive_comments_are_transport_activity_but_not_model_output() {
        let mut stream = SseChatAccumulator::new(1_024, 16_384);
        stream
            .push_bytes(b": OPENROUTER PROCESSING\\n")
            .expect("SSE keepalive should be accepted");
        assert_eq!(stream.comment_events, 1);
        assert_eq!(stream.model_progress_bytes(), 0);
        stream
            .push_bytes(b"data: {\\"choices\\":[{\\"delta\\":{\\"content\\":\\"{\\"}}]}\\n")
            .expect("model content should be accepted");
        assert!(stream.model_progress_bytes() > 0);
    }

    #[test]
    fn reasoning_deltas_count_as_model_progress_without_being_retained() {
        let mut stream = SseChatAccumulator::new(1_024, 16_384);
        stream
            .push_bytes(
                b"data: {\\"choices\\":[{\\"delta\\":{\\"reasoning_content\\":\\"working\\"}}]}\\n",
            )
            .unwrap();
        assert_eq!(stream.content, "");
        assert_eq!(stream.reasoning_bytes, 7);
        assert_eq!(stream.model_progress_bytes(), 7);
    }

    #[test]
    fn stream_inactivity_is_a_transient_transport_failure() {
        let failure = stream_inactivity_error(60_000);
        assert_eq!(failure.safe.code, "ai_stream_inactive");
        assert!(should_retry_document_stream_error(&failure));
        assert!(failure.safe.message.contains("total wall-clock deadline"));
    }''',
    "gateway liveness regressions",
)
replace_once(
    path,
    '''        assert_eq!(prepared["usage"]["include"], true);
        assert!(prepared.get("session_id").is_none());''',
    '''        assert!(prepared.get("usage").is_none());
        assert!(prepared.get("session_id").is_none());''',
    "remove deprecated OpenRouter usage test",
)
openrouter_test_anchor = '''    #[test]
    fn streaming_usage_preserves_reasoning_and_cache_counters_without_content() {'''
stream_body_test = '''    #[test]
    fn every_document_stream_requests_a_final_usage_chunk() {
        let prepared = prepare_streaming_body(&json!({
            "model": "example/model",
            "stream": false
        }))
        .unwrap();
        assert_eq!(prepared["stream"], true);
        assert_eq!(prepared["stream_options"]["include_usage"], true);
    }

    #[test]
    fn llama_timings_are_normalized_into_standard_stream_usage() {
        let usage = safe_usage_envelope(&json!({
            "timings": {"cache_n": 180, "prompt_n": 40, "predicted_n": 45}
        }))
        .expect("llama timings should be usable");
        assert_eq!(usage["prompt_tokens"], 220);
        assert_eq!(usage["completion_tokens"], 45);
        assert_eq!(usage["total_tokens"], 265);
        assert_eq!(usage["prompt_tokens_details"]["cached_tokens"], 180);
    }

'''
replace_once(path, openrouter_test_anchor, stream_body_test + openrouter_test_anchor, "stream usage regressions")

# ---------------------------------------------------------------------------
# Frontend progress exposes provider responsiveness independently from stages,
# windows, and token accounting.
# ---------------------------------------------------------------------------
path = "src/ipc/documents.ts"
replace_once(
    path,
    '''  usageReportedRequests: number;
  usageUnreportedRequests: number;
}''',
    '''  usageReportedRequests: number;
  usageUnreportedRequests: number;
  providerActivity: "preparing" | "connecting" | "responsive" | "streaming";
  lastProviderActivityAt: number | null;
  lastModelActivityAt: number | null;
}''',
    "frontend provider activity contract",
)

path = "src/screens/DocumentsScreen.tsx"
replace_once(
    path,
    '''        <div>
          <dt>Requests</dt>
          <dd>{progress ? progress.providerRequestCount.toLocaleString() : "—"}</dd>
        </div>
        <div>
          <dt>Retries</dt>''',
    '''        <div>
          <dt>Requests</dt>
          <dd>{progress ? progress.providerRequestCount.toLocaleString() : "—"}</dd>
        </div>
        <div>
          <dt>Provider</dt>
          <dd>{formatProviderActivity(progress, now)}</dd>
        </div>
        <div>
          <dt>Retries</dt>''',
    "provider activity progress metric",
)
helper_anchor = '''function generationStage(
  document: DocumentSummary,'''
provider_helper = '''function formatProviderActivity(
  progress: DocumentGenerationProgress | null,
  now: number,
): string {
  if (!progress) return "Preparing";
  const age = progress.lastProviderActivityAt === null
    ? null
    : Math.max(0, Math.floor((now - progress.lastProviderActivityAt) / 1_000));
  const suffix = age === null ? "" : age < 2 ? " · now" : ` · ${age}s ago`;
  switch (progress.providerActivity) {
    case "streaming":
      return `Receiving output${suffix}`;
    case "responsive":
      return `Responsive, waiting${suffix}`;
    case "connecting":
      return "Connecting";
    default:
      return "Preparing";
  }
}

'''
replace_once(path, helper_anchor, provider_helper + helper_anchor, "provider activity formatter")

path = "src/screens/DocumentsProgress.test.tsx"
replace_once(
    path,
    '''  usageReportedRequests: 6,
  usageUnreportedRequests: 1,
};''',
    '''  usageReportedRequests: 6,
  usageUnreportedRequests: 1,
  providerActivity: "streaming",
  lastProviderActivityAt: Date.now() - 1_000,
  lastModelActivityAt: Date.now() - 1_000,
};''',
    "provider progress test fixture",
)
replace_once(
    path,
    '''    expect(screen.getByText("7")).toBeTruthy();
    expect(screen.getByText("1")).toBeTruthy();''',
    '''    expect(screen.getByText("7")).toBeTruthy();
    expect(screen.getByText(/Receiving output/u)).toBeTruthy();
    expect(screen.getByText("1")).toBeTruthy();''',
    "provider progress UI assertion",
)

# Explain the existing profile timeout according to its new document-stream
# semantics. It remains the ordinary total timeout for non-streaming probes.
path = "src/screens/settings/AiLlmSettings.tsx"
replace_once(
    path,
    '''        <label className={styles.stackedField}>
          <span>Request timeout</span>
          <select''',
    '''        <label className={styles.stackedField}>
          <span>Provider inactivity timeout</span>
          <select''',
    "inactivity timeout label",
)
replace_once(
    path,
    '''            <option value={300_000}>5 minutes</option>
          </select>
        </label>''',
    '''            <option value={300_000}>5 minutes</option>
          </select>
          <small className={styles.fieldHelp}>
            During streamed document work this limits silence between provider events, not total generation time. Heartbeats, reasoning progress, output tokens, and final usage all keep an active request alive.
          </small>
        </label>''',
    "inactivity timeout help",
)

path = "docs/user-guide/ai-providers.md"
replace_once(
    path,
    "- request timeout and enabled state;",
    "- provider inactivity timeout and enabled state;",
    "AI profile timeout terminology",
)
replace_once(
    path,
    '''The application records safe progress and usage measurements needed to resume or diagnose a job, not raw provider messages or document text in diagnostic exports.''',
    '''The application records safe progress and usage measurements needed to resume or diagnose a job, not raw provider messages or document text in diagnostic exports. During streamed document work the profile timeout is an inactivity limit: received SSE heartbeats, reasoning deltas, output deltas, and final usage events prove that the connection is responsive and reset the timer. It is not a maximum duration for a complete generation request.''',
    "AI provider liveness documentation",
)

print("PR #54 provider-neutral streaming refactor applied.")
