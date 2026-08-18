#!/usr/bin/env python3
"""One-shot PR #54 real-provider acceptance fixes.

This script converts the real 55-page OpenRouter failure into durable source,
telemetry, routing, and regression changes. It is intentionally removed after
its product changes are committed.
"""
from pathlib import Path


def replace_once(path: str, old: str, new: str, label: str) -> None:
    p = Path(path)
    value = p.read_text()
    if new in value:
        return
    if old not in value:
        raise SystemExit(f"{label}: anchor not found in {path}")
    p.write_text(value.replace(old, new, 1))


# ---------------------------------------------------------------------------
# Live progress: represent a concurrent source-window batch honestly.
# ---------------------------------------------------------------------------
path = "src-tauri/src/task_cancellation.rs"
replace_once(
    path,
    "    pub current_window: Option<usize>,\n    pub completed_windows: usize,",
    "    pub current_window: Option<usize>,\n    pub current_window_end: Option<usize>,\n    pub completed_windows: usize,",
    "task progress window end field",
)
replace_once(
    path,
    "            current_window: None,\n            completed_windows: 0,",
    "            current_window: None,\n            current_window_end: None,\n            completed_windows: 0,",
    "task progress window end default",
)
old = '''pub(crate) fn set_current_task_phase(
    phase: &str,
    current_window: Option<usize>,
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
            progress.completed_windows = completed_windows;
            progress.total_windows = total_windows;
            progress.current_page_start = current_page_start;
            progress.current_page_end = current_page_end;
            progress.current_partition = current_partition;
        }
    });
}'''
new = '''pub(crate) fn set_current_task_phase(
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
}'''
replace_once(path, old, new, "task progress range setter")

# ---------------------------------------------------------------------------
# OpenRouter routing: preserve price/uptime load balancing across independent
# windows, strengthen batch throughput preference, and remove document-wide
# sticky routing that concentrated concurrent windows on one endpoint.
# ---------------------------------------------------------------------------
path = "src-tauri/src/ai/providers/openrouter.rs"
replace_once(
    path,
    'const PREFERRED_MIN_THROUGHPUT_TPS: u64 = 40;\nconst PREFERRED_MAX_P90_LATENCY_SECONDS: u64 = 20;',
    'const PREFERRED_MIN_P90_THROUGHPUT_TPS: u64 = 50;\nconst PREFERRED_MAX_P90_LATENCY_SECONDS: u64 = 20;',
    "OpenRouter performance constants",
)
old = '''/// Endpoint precision is a hard quality boundary: INT4/FP4/FP6 and endpoints
/// whose quantization is unknown are never selected. Within the eligible set,
/// price is the primary sort while throughput/latency are soft preferences so
/// an outage can still fall back to another high-precision endpoint.'''
new = '''/// Endpoint precision is a hard quality boundary: INT4/FP4/FP6 and endpoints
/// whose quantization is unknown are never selected. Within the eligible set,
/// ostudy leaves OpenRouter's price/uptime load balancing enabled so concurrent
/// independent windows can spread across healthy endpoints. A p90 throughput
/// preference keeps consistently slow endpoints behind cheaper fast candidates;
/// performance preferences remain soft so healthy high-precision fallbacks are
/// still available during constrained capacity.'''
replace_once(path, old, new, "OpenRouter routing comment")
old = '''        "quantizations": HIGH_PRECISION_QUANTIZATIONS,
        "sort": "price",
        "preferred_min_throughput": {
            "p50": PREFERRED_MIN_THROUGHPUT_TPS
        },
        "preferred_max_latency": {
            "p90": PREFERRED_MAX_P90_LATENCY_SECONDS
        }'''
new = '''        "quantizations": HIGH_PRECISION_QUANTIZATIONS,
        "preferred_min_throughput": {
            "p90": PREFERRED_MIN_P90_THROUGHPUT_TPS
        },
        "preferred_max_latency": {
            "p90": PREFERRED_MAX_P90_LATENCY_SECONDS
        }'''
replace_once(path, old, new, "OpenRouter routing body")
replace_once(
    path,
    '        assert_eq!(strict.pointer("/sort"), Some(&json!("price")));\n        assert_eq!(\n            strict.pointer("/preferred_min_throughput/p50"),\n            Some(&json!(40))\n        );',
    '        assert!(strict.pointer("/sort").is_none());\n        assert_eq!(\n            strict.pointer("/preferred_min_throughput/p90"),\n            Some(&json!(50))\n        );',
    "OpenRouter routing regression",
)

# ---------------------------------------------------------------------------
# Gateway: use useful-output idle time rather than a blunt whole-request wall
# clock. OpenRouter keepalive comments do not count as model progress.
# ---------------------------------------------------------------------------
path = "src-tauri/src/ai/gateway.rs"
replace_once(
    path,
    '    current_document_id, current_signal, record_current_provider_request,\n    record_current_provider_retry, record_current_usage, set_current_reasoning_effort,',
    '    current_signal, record_current_provider_request, record_current_provider_retry,\n    record_current_usage, set_current_reasoning_effort,',
    "gateway task-cancellation imports",
)
replace_once(
    path,
    'use tokio::time::{sleep, timeout};',
    'use tokio::time::{sleep, timeout, Instant};',
    "gateway Instant import",
)
replace_once(
    path,
    'const DOCUMENT_RETRY_DELAY_MILLIS: u64 = 750;\nconst MAX_SAFE_PROVIDER_METADATA_CHARS: usize = 300;',
    'const DOCUMENT_RETRY_DELAY_MILLIS: u64 = 750;\nconst DOCUMENT_STREAM_PROGRESS_TIMEOUT_SECONDS: u64 = 75;\nconst MAX_SAFE_PROVIDER_METADATA_CHARS: usize = 300;',
    "stream progress timeout constant",
)
replace_once(
    path,
    '''        let initial_timeout = Duration::from_millis(initial_response_timeout_ms.max(1) as u64);
        let mut streaming_body = body.clone();''',
    '''        let initial_timeout = Duration::from_millis(initial_response_timeout_ms.max(1) as u64);
        let progress_timeout = Duration::from_secs(DOCUMENT_STREAM_PROGRESS_TIMEOUT_SECONDS);
        let request_started = Instant::now();
        let mut streaming_body = body.clone();''',
    "stream timing setup",
)
old = '''        let first_chunk = tokio::select! {
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
        let mut stream = SseChatAccumulator::for_request(
            maximum_content_bytes,
            maximum_wire_bytes,
            openrouter,
            generation_id,
            retry_after_seconds,
        );
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
        stream.finish()?;'''
new = '''        let first_chunk = tokio::select! {
            _ = cancellation.canceled() => return Err(canceled_error()),
            result = timeout(progress_timeout, response.chunk()) => match result {
                Ok(result) => result.map_err(classify_reqwest_error)?,
                Err(_) => return Err(stream_progress_timeout_error()),
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
        let mut first_useful_output_ms = (useful_progress_bytes > 0).then(|| {
            u64::try_from(request_started.elapsed().as_millis()).unwrap_or(u64::MAX)
        });
        let mut last_useful_progress = Instant::now();

        while !stream.done {
            let remaining = progress_timeout.saturating_sub(last_useful_progress.elapsed());
            if remaining.is_zero() {
                return Err(stream_progress_timeout_error());
            }
            let next = tokio::select! {
                _ = cancellation.canceled() => return Err(canceled_error()),
                result = timeout(remaining, response.chunk()) => match result {
                    Ok(result) => result.map_err(classify_reqwest_error)?,
                    Err(_) => return Err(stream_progress_timeout_error()),
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
        stream.finish()?;'''
replace_once(path, old, new, "SSE useful-progress timeout loop")
old = '''        let usage = stream.usage.clone();
        if let Some(usage) = usage.as_ref() {
            record_current_usage(usage_delta(usage));
        }
        let message = if stream.tool_arguments.trim().is_empty() {'''
new = '''        let usage = stream.usage.clone();
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
        let message = if stream.tool_arguments.trim().is_empty() {'''
replace_once(path, old, new, "OpenRouter successful stream diagnostics")
replace_once(
    path,
    '''    retry_after_seconds: Option<u64>,
}''',
    '''    retry_after_seconds: Option<u64>,
    upstream_provider: Option<String>,
    model: Option<String>,
}''',
    "SSE routing metadata fields",
)
replace_once(
    path,
    '''            retry_after_seconds: None,
        }''',
    '''            retry_after_seconds: None,
            upstream_provider: None,
            model: None,
        }''',
    "SSE routing metadata defaults",
)
old = '''        if let Some(usage) = safe_usage_envelope(&value) {
            self.usage = Some(usage);
        }
        if value.get("error").is_some_and(|value| !value.is_null()) {'''
new = '''        if let Some(usage) = safe_usage_envelope(&value) {
            self.usage = Some(usage);
        }
        if self.openrouter {
            if self.upstream_provider.is_none() {
                self.upstream_provider = safe_json_string(
                    value.get("provider"),
                    MAX_SAFE_PROVIDER_METADATA_CHARS,
                );
            }
            if self.model.is_none() {
                self.model = safe_json_string(value.get("model"), MAX_SAFE_PROVIDER_METADATA_CHARS);
            }
        }
        if value.get("error").is_some_and(|value| !value.is_null()) {'''
replace_once(path, old, new, "SSE successful routing metadata capture")
old = '''    fn append_compat_content(&mut self, content: &str) -> Result<(), AiGatewayError> {'''
new = '''    fn useful_progress_bytes(&self) -> usize {
        self.content.len().saturating_add(self.tool_arguments.len())
    }

    fn append_compat_content(&mut self, content: &str) -> Result<(), AiGatewayError> {'''
replace_once(path, old, new, "SSE useful progress helper")
old = '''fn prepare_provider_body(url: &Url, body: &Value) -> Option<Value> {
    let session = current_document_id();
    let reasoning_effort = session
        .as_ref()
        .map(|_| crate::settings::current_ai_reasoning_effort());
    if let Some(reasoning_effort) = reasoning_effort.as_deref() {
        set_current_reasoning_effort(reasoning_effort);
    }
    prepare_provider_body_for_document(url, body, session.as_deref(), reasoning_effort.as_deref())
}

fn prepare_provider_body_for_document(
    url: &Url,
    body: &Value,
    document_id: Option<&str>,
    reasoning_effort: Option<&str>,
) -> Option<Value> {'''
new = '''fn prepare_provider_body(url: &Url, body: &Value) -> Option<Value> {
    let document_task = current_signal().is_some();
    let reasoning_effort =
        document_task.then(crate::settings::current_ai_reasoning_effort);
    if let Some(reasoning_effort) = reasoning_effort.as_deref() {
        set_current_reasoning_effort(reasoning_effort);
    }
    prepare_provider_body_for_document(url, body, reasoning_effort.as_deref())
}

fn prepare_provider_body_for_document(
    url: &Url,
    body: &Value,
    reasoning_effort: Option<&str>,
) -> Option<Value> {'''
replace_once(path, old, new, "OpenRouter document request preparation")
old = '''    if let Some(document_id) = document_id.filter(|value| !value.is_empty()) {
        object.insert(
            "session_id".to_string(),
            Value::String(format!("ostudy-document-{document_id}")),
        );
    }
    if let Some(reasoning_effort) = reasoning_effort.filter(|value| *value != "automatic") {'''
new = '''    // Document windows are independent batch work, not conversational turns.
    // A shared session_id pins every concurrent request to one provider endpoint,
    // concentrating a batch on the same slow/stalled route for little cache gain.
    // Leave session stickiness unset so OpenRouter can spread independent windows.
    if let Some(reasoning_effort) = reasoning_effort.filter(|value| *value != "automatic") {'''
replace_once(path, old, new, "remove document-wide OpenRouter session stickiness")
old = '''fn empty_stream_failure(finish_reason: &str) -> AiGatewayError {'''
new = '''fn stream_progress_timeout_error() -> AiGatewayError {
    error(
        "ai_stream_progress_timeout",
        &format!(
            "The AI provider produced no usable streamed output for {DOCUMENT_STREAM_PROGRESS_TIMEOUT_SECONDS} seconds. ostudy canceled this stalled request so the document can retry or resume without waiting indefinitely."
        ),
        true,
        None,
    )
}

fn empty_stream_failure(finish_reason: &str) -> AiGatewayError {'''
replace_once(path, old, new, "stream progress timeout error")
old = '''        let prepared = prepare_provider_body_for_document(
            &url,
            &json!({ "model": "example/model" }),
            Some("0123456789abcdef0123456789abcdef"),
            Some("low"),
        )'''
new = '''        let prepared = prepare_provider_body_for_document(
            &url,
            &json!({ "model": "example/model" }),
            Some("low"),
        )'''
replace_once(path, old, new, "gateway OpenRouter test call")
replace_once(
    path,
    '''        assert_eq!(
            prepared["session_id"],
            "ostudy-document-0123456789abcdef0123456789abcdef"
        );''',
    '''        assert!(prepared.get("session_id").is_none());''',
    "gateway session-id regression",
)
replace_once(
    path,
    '''        let automatic = prepare_provider_body_for_document(
            &url,
            &json!({ "model": "example/model" }),
            Some("document"),
            Some("automatic"),
        )''',
    '''        let automatic = prepare_provider_body_for_document(
            &url,
            &json!({ "model": "example/model" }),
            Some("automatic"),
        )''',
    "gateway automatic reasoning test call",
)
old = '''    #[test]
    fn document_stream_retry_is_limited_to_transient_transport_failures() {'''
new = '''    #[test]
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
        let failure = stream_progress_timeout_error();
        assert_eq!(failure.safe.code, "ai_stream_progress_timeout");
        assert!(!should_retry_document_stream_error(&failure));
    }

    #[test]
    fn document_stream_retry_is_limited_to_transient_transport_failures() {'''
replace_once(path, old, new, "gateway progress timeout regressions")

# ---------------------------------------------------------------------------
# Outline execution: fan out immediately, accurately describe concurrent page
# spans, make cache reuse visible in AI diagnostics, and recover from a stalled
# provider with one fresh route before falling back natively.
# ---------------------------------------------------------------------------
path = "src-tauri/src/ai/document_intelligence_adapter.rs"
replace_once(path, 'use std::time::Duration;\n\nuse tokio::time::timeout;\n', 'use std::time::Instant;\n', "outline timing imports")
replace_once(
    path,
    '    task_cancellation::set_current_task_phase,',
    '    task_cancellation::{\n        record_current_provider_retry, set_current_task_phase_range,\n    },',
    "outline task progress imports",
)
replace_once(path, 'const OUTLINE_PROVIDER_ATTEMPT_SECONDS: u64 = 120;\n', '', "remove outline hard wall clock")
old = '''/// Text-bearing PDFs request independent local outline fragments with bounded
/// provider concurrency. The first OpenRouter request is deliberately warmed on
/// its own so the stable document `session_id` can establish sticky routing
/// before later windows fan out. Completed fragments are persisted serially,
/// then the existing deterministic Rust assembly/global-synthesis path consumes
/// them as cached fragments so request completion order cannot change hierarchy.'''
new = '''/// Text-bearing PDFs request independent local outline fragments with bounded
/// provider concurrency. Remote windows fan out immediately; they deliberately
/// do not share an OpenRouter sticky session because these are independent batch
/// requests with different source text. Completed fragments are persisted
/// serially, then the deterministic Rust assembly/global-synthesis path consumes
/// them as cached fragments so request completion order cannot change hierarchy.'''
replace_once(path, old, new, "outline concurrency documentation")
old = '''    crate::logging::info(
        "document_outline_execution_plan",
        format!(
            "provider={} model={} windows={} cached={} parallelism={} sticky_warmup={}",
            task.provider_kind,
            task.model_id,
            windows.len(),
            windows.len().saturating_sub(missing.len()),
            parallelism,
            task.provider_kind == "openrouter" && missing.len() > 1,
        ),
    );

    let mut completed = windows.len().saturating_sub(missing.len());
    let mut cursor = 0_usize;
    let warm_first = task.provider_kind == "openrouter" && missing.len() > 1;
    while cursor < missing.len() {
        let batch_size = if warm_first && cursor == 0 {
            1
        } else {
            parallelism
        };'''
new = '''    crate::logging::debug_target(
        "ai.document.outline",
        "document_outline_execution_plan",
        "Prepared bounded outline-provider work without logging document content.",
        serde_json::json!({
            "provider": task.provider_kind.as_str(),
            "model": task.model_id.as_str(),
            "windows": windows.len(),
            "cachedWindows": windows.len().saturating_sub(missing.len()),
            "providerWindows": missing.len(),
            "parallelism": parallelism,
        }),
    );

    let mut completed = windows.len().saturating_sub(missing.len());
    let mut cursor = 0_usize;
    while cursor < missing.len() {
        let batch_size = parallelism;'''
replace_once(path, old, new, "outline execution plan and warmup removal")
old = '''        set_current_task_phase(
            "outline",
            Some(batch[0] + 1),
            completed,
            windows.len(),
            Some(first.page_start),
            Some(last.page_end),
            Some(first.partition_index + 1),
        );'''
new = '''        set_current_task_phase_range(
            "outline",
            Some(batch[0] + 1),
            batch.last().map(|index| index + 1),
            completed,
            windows.len(),
            Some(first.page_start),
            Some(last.page_end),
            Some(first.partition_index + 1),
        );'''
replace_once(path, old, new, "outline concurrent progress range")
replace_once(path, 'request_outline_with_deadline(', 'request_outline_with_resilience(', "outline resilient request call")
old = '''async fn request_outline_with_deadline(
    runtime: &AiRuntimeState,
    task: &DocumentTaskContext,
    document_title: &str,
    page_count: i64,
    window: &SemanticWindow,
) -> Result<DocumentOutline, CommandError> {
    for attempt in 1..=2 {
        let request = implementation::request_outline_fragment(
            runtime,
            task,
            document_title,
            page_count,
            window,
            &[],
        );
        match timeout(
            Duration::from_secs(OUTLINE_PROVIDER_ATTEMPT_SECONDS),
            request,
        )
        .await
        {
            Ok(result) => return result,
            Err(_) if attempt == 1 => {
                crate::logging::warn_target(
                    "ai.document.outline",
                    "document_outline_window_deadline_retry",
                    "A provider took too long on one bounded outline fragment; ostudy canceled that request and is retrying the source window once.",
                    serde_json::json!({
                        "provider": task.provider_kind.as_str(),
                        "model": task.model_id.as_str(),
                        "window": window.window_index + 1,
                        "pageStart": window.page_start,
                        "pageEnd": window.page_end,
                        "attemptSeconds": OUTLINE_PROVIDER_ATTEMPT_SECONDS,
                    }),
                );
            }
            Err(_) => {
                return Err(CommandError::new(
                    "ai_provider_deadline",
                    format!(
                        "The AI provider exceeded the {OUTLINE_PROVIDER_ATTEMPT_SECONDS}-second outline-window deadline twice. ostudy canceled both incomplete requests instead of allowing this document window to stall indefinitely."
                    ),
                ));
            }
        }
    }
    unreachable!("the bounded two-attempt loop always returns")
}'''
new = '''async fn request_outline_with_resilience(
    runtime: &AiRuntimeState,
    task: &DocumentTaskContext,
    document_title: &str,
    page_count: i64,
    window: &SemanticWindow,
) -> Result<DocumentOutline, CommandError> {
    let first = implementation::request_outline_fragment(
        runtime,
        task,
        document_title,
        page_count,
        window,
        &[],
    )
    .await;
    match first {
        Err(error) if error.code == "ai_stream_progress_timeout" => {
            record_current_provider_retry();
            crate::logging::warn_target(
                "ai.document.outline",
                "document_outline_window_progress_retry",
                "One outline route stopped producing useful output; ostudy is retrying this concise source window once on a fresh route.",
                serde_json::json!({
                    "provider": task.provider_kind.as_str(),
                    "model": task.model_id.as_str(),
                    "window": window.window_index + 1,
                    "pageStart": window.page_start,
                    "pageEnd": window.page_end,
                }),
            );
            implementation::request_outline_fragment(
                runtime,
                task,
                document_title,
                page_count,
                window,
                &[],
            )
            .await
        }
        other => other,
    }
}'''
replace_once(path, old, new, "outline stall resilience")
# Add useful per-window duration diagnostics around the provider call.
replace_once(
    path,
    '''    let result = match request_outline_with_resilience(
        runtime,
        task,
        document_title,
        page_count,
        window,
    )''',
    '''    let started = Instant::now();
    let result = match request_outline_with_resilience(
        runtime,
        task,
        document_title,
        page_count,
        window,
    )''',
    "outline window timer",
)
replace_once(
    path,
    '''    Some((index, result))
}''',
    '''    if let Ok(fragment) = &result {
        crate::logging::debug_target(
            "ai.document.outline",
            "document_outline_window_completed",
            "A bounded outline fragment completed without logging source or model output.",
            serde_json::json!({
                "provider": task.provider_kind.as_str(),
                "model": task.model_id.as_str(),
                "window": index + 1,
                "pageStart": window.page_start,
                "pageEnd": window.page_end,
                "sourceTokens": window.token_estimate,
                "sections": fragment.sections.len(),
                "objectives": fragment.learning_objectives.len(),
                "concepts": fragment.key_concepts.len(),
                "durationMs": u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX),
            }),
        );
    }
    Some((index, result))
}''',
    "outline completion diagnostics",
)

# A second stalled outline attempt can use the existing conservative native
# fragment rather than aborting an otherwise healthy multi-window document.
path = "src-tauri/src/ai/document_intelligence.rs"
replace_once(
    path,
    '        "ai_document_output_truncated" | "ai_document_output_invalid" | "ai_empty_response"',
    '        "ai_document_output_truncated"\n            | "ai_document_output_invalid"\n            | "ai_empty_response"\n            | "ai_stream_progress_timeout"',
    "recoverable outline stall classification",
)
replace_once(
    path,
    '        assert_eq!(body.pointer("/provider/sort"), Some(&json!("price")));',
    '        assert!(body.pointer("/provider/sort").is_none());\n        assert_eq!(\n            body.pointer("/provider/preferred_min_throughput/p90"),\n            Some(&json!(50))\n        );',
    "document OpenRouter routing regression",
)

# ---------------------------------------------------------------------------
# Generation: no identical 180-second retries. A request can run as long as it
# is producing useful stream progress; a true stall or truncation gets one
# smaller card-budget retry. Concurrent batches are represented as ranges.
# ---------------------------------------------------------------------------
path = "src-tauri/src/document_generation.rs"
replace_once(path, 'use std::time::Duration;\n', 'use std::time::Instant;\n', "generation timing import")
replace_once(path, 'use tokio::time::timeout;\n', '', "remove generation timeout import")
replace_once(
    path,
    '    task_cancellation::set_current_task_phase,',
    '    task_cancellation::{\n        record_current_provider_retry, set_current_task_phase, set_current_task_phase_range,\n    },',
    "generation progress imports",
)
replace_once(path, 'const GENERATION_PROVIDER_ATTEMPT_SECONDS: u64 = 180;\n', '', "remove generation hard wall clock")
old = '''    crate::logging::info(
        "document_generation_execution_plan",
        format!(
            "document={} provider={} model={} windows={} parallelism={} sticky_warmup={}",
            input.document_id,
            input.task.provider_kind,
            input.task.model_id,
            input.windows.len(),
            parallelism,
            input.task.provider_kind == "openrouter",
        ),
    );'''
new = '''    crate::logging::debug_target(
        "ai.document.generation",
        "document_generation_execution_plan",
        "Prepared independent bounded card-generation windows without logging document content.",
        serde_json::json!({
            "document": input.document_id,
            "provider": input.task.provider_kind.as_str(),
            "model": input.task.model_id.as_str(),
            "windows": input.windows.len(),
            "parallelism": parallelism,
            "maxCardsPerWindow": input.plan.max_cards_per_window,
        }),
    );'''
replace_once(path, old, new, "generation execution plan")
old = '''    let warm_first = input.task.provider_kind == "openrouter" && missing.len() > 1;
    let mut cursor = 0_usize;
    while cursor < missing.len() {
        let batch_size = if warm_first && cursor == 0 {
            1
        } else {
            parallelism
        };'''
new = '''    let mut cursor = 0_usize;
    while cursor < missing.len() {
        let batch_size = parallelism;'''
replace_once(path, old, new, "generation warmup removal")
old = '''        set_current_task_phase(
            "generating",
            Some(batch[0] + 1),
            completed,
            input.windows.len(),
            Some(first.page_start),
            Some(last.page_end),
            Some(first.partition_index + 1),
        );'''
new = '''        set_current_task_phase_range(
            "generating",
            Some(batch[0] + 1),
            batch.last().map(|position| position + 1),
            completed,
            input.windows.len(),
            Some(first.page_start),
            Some(last.page_end),
            Some(first.partition_index + 1),
        );'''
replace_once(path, old, new, "generation concurrent progress range")
start = Path(path).read_text().find('async fn generation_slot(')
end = Path(path).read_text().find('\n#[cfg(test)]', start)
if start < 0 or end < 0:
    raise SystemExit('generation resilience block not found')
value = Path(path).read_text()
new_block = r'''async fn generation_slot(
    input: &GenerationExecution<'_>,
    position: Option<usize>,
    images: &[document_intelligence::SourcePageImage],
) -> Option<(usize, Result<Vec<Value>, CommandError>)> {
    let position = position?;
    let window = &input.windows[position];
    let started = Instant::now();
    crate::logging::debug_target(
        "ai.document.generation",
        "document_generation_window_started",
        "A bounded card-generation window started without logging document content.",
        serde_json::json!({
            "provider": input.task.provider_kind.as_str(),
            "model": input.task.model_id.as_str(),
            "window": position + 1,
            "pageStart": window.page_start,
            "pageEnd": window.page_end,
            "sourceTokens": window.token_estimate,
            "cardBudget": input.plan.max_cards_per_window,
        }),
    );
    let result = generate_window_with_recovery(WindowGenerationRequest {
        runtime: input.runtime,
        task: input.task,
        document_title: input.document_title,
        outline: input.outline,
        window,
        images,
        maximum_cards: input.plan.max_cards_per_window,
        diagnostic_prefix: input.diagnostic_prefix,
    })
    .await;

    match &result {
        Ok(cards) => crate::logging::debug_target(
            "ai.document.generation",
            "document_generation_window_completed",
            "A bounded card-generation window completed without logging generated content.",
            serde_json::json!({
                "provider": input.task.provider_kind.as_str(),
                "model": input.task.model_id.as_str(),
                "window": position + 1,
                "pageStart": window.page_start,
                "pageEnd": window.page_end,
                "cards": cards.len(),
                "durationMs": u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX),
            }),
        ),
        Err(error) => crate::logging::warn_target(
            "ai.document.generation",
            "document_generation_window_failed",
            "A bounded card-generation window exhausted its safe recovery path.",
            serde_json::json!({
                "provider": input.task.provider_kind.as_str(),
                "model": input.task.model_id.as_str(),
                "window": position + 1,
                "pageStart": window.page_start,
                "pageEnd": window.page_end,
                "code": error.code.as_str(),
                "durationMs": u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX),
            }),
        ),
    }

    Some((
        position,
        result.map_err(|error| {
            CommandError::new(
                error.code,
                format!(
                    "Card generation failed for source window {}/{} (PDF pages {}–{}): {} Completed windows are checkpointed; Retry generation resumes the remaining source windows instead of starting over.",
                    position + 1,
                    input.windows.len(),
                    window.page_start,
                    window.page_end,
                    error.message,
                ),
            )
        }),
    ))
}

async fn generate_window_with_recovery(
    input: WindowGenerationRequest<'_>,
) -> Result<Vec<Value>, CommandError> {
    let first_budget = input.maximum_cards.max(1);
    let first = document_intelligence::generate_window_flashcards(
        input.runtime,
        input.task,
        input.document_title,
        input.outline,
        input.window,
        input.images,
        first_budget,
    )
    .await;
    match first {
        Err(error)
            if input.maximum_cards > 2
                && (matches!(
                    error.code.as_str(),
                    "ai_document_output_truncated"
                        | "ai_stream_progress_timeout"
                        | "ai_empty_response"
                ) || (error.code == "ai_document_output_invalid"
                    && error.message.contains("more than"))) =>
        {
            let reduced_budget = (input.maximum_cards / 2).max(2);
            record_current_provider_retry();
            crate::logging::info_target(
                "ai.document.generation",
                "document_generation_window_compact_retry",
                "The source window is being retried once with a smaller structured-output budget.",
                serde_json::json!({
                    "path": input.diagnostic_prefix,
                    "window": input.window.window_index + 1,
                    "partition": input.window.partition_index + 1,
                    "locator": input.window.source_locator().stable_identity(),
                    "oldBudget": input.maximum_cards,
                    "newBudget": reduced_budget,
                    "code": error.code.as_str(),
                }),
            );
            document_intelligence::generate_window_flashcards(
                input.runtime,
                input.task,
                input.document_title,
                input.outline,
                input.window,
                input.images,
                reduced_budget,
            )
            .await
        }
        other => other,
    }
}
'''
Path(path).write_text(value[:start] + new_block + value[end:])

# Start with a smaller per-window card contract. Comprehensive coverage comes
# from all source windows plus Generate More, not by asking one response for 16
# structured cards and risking a long/truncated result.
path = "src-tauri/src/document_scale.rs"
replace_once(path, '            max_cards_per_window: 12,', '            max_cards_per_window: 10,', "standard card budget")
replace_once(path, '        max_cards_per_window: 16,', '        max_cards_per_window: 10,', "large card budget")

# ---------------------------------------------------------------------------
# Frontend: distinguish a concurrent batch from a single window and stop showing
# completed-window counts twice.
# ---------------------------------------------------------------------------
path = "src/ipc/documents.ts"
replace_once(
    path,
    '  currentWindow: number | null;\n  completedWindows: number;',
    '  currentWindow: number | null;\n  currentWindowEnd?: number | null;\n  completedWindows: number;',
    "frontend progress window end",
)

path = "src/screens/DocumentsScreen.tsx"
old = '''  if (document.outlineWindowCount > 0 && document.outlineCompletedWindowCount < document.outlineWindowCount) {
    const approximatePages = Math.min(
      document.pageCount,
      Math.round((document.outlineCompletedWindowCount / document.outlineWindowCount) * document.pageCount),
    );
    return {
      title: `Building outline ${document.outlineCompletedWindowCount}/${document.outlineWindowCount}`,
      detail: `About ${approximatePages}/${document.pageCount} pages covered by completed outline windows.`,
    };
  }
  if (
    document.generationWindowCount > 0 &&
    document.generationCompletedWindowCount < document.generationWindowCount
  ) {
    const pageRange =
      progress?.currentPageStart !== null && progress?.currentPageStart !== undefined
        ? ` · pages ${progress.currentPageStart}–${progress.currentPageEnd ?? progress.currentPageStart}`
        : "";
    const window = progress?.currentWindow ?? document.generationCompletedWindowCount + 1;
    return {
      title: `Generating cards ${document.generationCompletedWindowCount}/${document.generationWindowCount}`,
      detail: `Current window ${window}/${document.generationWindowCount}${pageRange}.`,
    };
  }'''
new = '''  if (document.outlineWindowCount > 0 && document.outlineCompletedWindowCount < document.outlineWindowCount) {
    return {
      title: "Building outline",
      detail:
        activeWindowDetail(progress, document.outlineWindowCount) ??
        "Analyzing bounded source windows for document structure.",
    };
  }
  if (
    document.generationWindowCount > 0 &&
    document.generationCompletedWindowCount < document.generationWindowCount
  ) {
    return {
      title: "Generating cards",
      detail:
        activeWindowDetail(progress, document.generationWindowCount) ??
        "Generating study cards from the current bounded source window.",
    };
  }'''
replace_once(path, old, new, "document generation stage copy")
old = '''function generationPercent(
  document: DocumentSummary,
  progress: DocumentGenerationProgress | null,
): number | null {'''
new = '''function activeWindowDetail(
  progress: DocumentGenerationProgress | null,
  totalWindows: number,
): string | null {
  const start = progress?.currentWindow;
  if (start === null || start === undefined) return null;
  const end = progress?.currentWindowEnd ?? start;
  const pages =
    progress?.currentPageStart !== null && progress?.currentPageStart !== undefined
      ? ` · pages ${progress.currentPageStart}–${progress.currentPageEnd ?? progress.currentPageStart}`
      : "";
  if (end > start) {
    return `Active windows ${start}–${end} of ${totalWindows}${pages}.`;
  }
  return `Window ${start} of ${totalWindows}${pages}.`;
}

function generationPercent(
  document: DocumentSummary,
  progress: DocumentGenerationProgress | null,
): number | null {'''
replace_once(path, old, new, "active window detail helper")

# UI regression that reproduces the user's exact confusing page span: window 2
# was really a concurrent batch of windows 2-5, not a single 9-35 page window.
path = "src/screens/DocumentsProgress.test.tsx"
p = Path(path)
value = p.read_text()
insert_anchor = '''  it("shows stage, window progress, elapsed runtime, requests, retries, reasoning and cache usage", async () => {'''
if 'describes concurrent generation as an active window range' not in value:
    regression = '''  it("describes concurrent generation as an active window range without duplicating completed counts", async () => {
    documentIpcMocks.listDocuments.mockResolvedValue([
      {
        ...document,
        outlineWindowCount: 4,
        outlineCompletedWindowCount: 4,
        generationWindowCount: 9,
        generationCompletedWindowCount: 1,
      },
    ]);
    documentIpcMocks.getDocumentGenerationProgress.mockResolvedValue({
      ...progress,
      phase: "generating",
      currentWindow: 2,
      currentWindowEnd: 5,
      completedWindows: 1,
      totalWindows: 9,
      currentPageStart: 9,
      currentPageEnd: 35,
    });

    render(
      <DocumentsScreen
        refreshDecks={vi.fn().mockResolvedValue(undefined)}
        showToast={vi.fn()}
      />,
    );

    expect((await screen.findAllByText("Generating cards")).length).toBeGreaterThan(0);
    expect(screen.getByText("Active windows 2–5 of 9 · pages 9–35.")).toBeTruthy();
    expect(screen.queryByText("Generating cards 1/9")).toBeNull();
    expect(screen.queryByText(/Current window 2\/9/u)).toBeNull();
  });

'''
    if insert_anchor not in value:
        raise SystemExit('DocumentsProgress regression anchor not found')
    value = value.replace(insert_anchor, regression + insert_anchor, 1)
    p.write_text(value)

# ---------------------------------------------------------------------------
# Architecture documentation: align it with what the real-provider test taught
# us instead of retaining the now-rejected sticky/hard-deadline assumptions.
# ---------------------------------------------------------------------------
path = "docs/architecture/document-ai-routing.md"
p = Path(path)
value = p.read_text()
value = value.replace(
    'OpenRouter routing treats compatibility, privacy, and model precision as hard eligibility constraints. Document requests require parameter-compatible endpoints, deny provider data collection by default, honor the profile\'s optional ZDR requirement, and restrict eligible endpoint quantization to INT8, FP8, FP16, BF16, or FP32 execution. Among eligible endpoints, routing prefers low price while applying throughput and latency preferences; low-grade INT4, FP4, FP6, and unknown-precision endpoints are not acceptable fallbacks.\n\nA document-scoped `session_id` keeps related OpenRouter requests on a sticky provider after the first successful request, improving cache locality while still permitting provider fallback when that route becomes unavailable.\n\nProvider attempts are also bounded at the document-task layer. A provider that accepts a stream but fails to complete useful work cannot stall the entire document indefinitely. Output exhaustion (`finish_reason=length`) is classified as truncation, not malformed JSON: flashcard generation responds by retrying the affected source window with a smaller card/output budget rather than repeating the identical oversized request twice.',
    'OpenRouter routing treats compatibility, privacy, and model precision as hard eligibility constraints. Document requests require parameter-compatible endpoints, deny provider data collection by default, honor the profile\'s optional ZDR requirement, and restrict eligible endpoint quantization to INT8, FP8/MXFP8, FP16, BF16, or FP32 execution. Independent document windows deliberately do not share a sticky `session_id`: they contain different source text, and real-provider acceptance showed negligible prompt-cache reuse while one sticky route could concentrate all concurrent work on the same slow endpoint. OpenRouter therefore retains its normal price/uptime load balancing across eligible routes, with a p90 throughput preference of at least 50 tokens/sec and a p90 latency preference of at most 20 seconds.\n\nProvider resilience is progress-based rather than a blunt whole-request wall clock. Once a stream is accepted, an actively producing response may run longer than three minutes; a stream that produces no usable model output for 75 seconds is treated as stalled. OpenRouter keepalive comments do not reset that timer. Output exhaustion (`finish_reason=length`) is classified as truncation, not malformed JSON. For card generation, either truncation or a true stream stall retries the affected source window once with a smaller card/output budget instead of repeating the identical oversized request. Completed windows remain checkpointed for Retry.'
)
p.write_text(value)

print('PR #54 document resilience patch applied.')
