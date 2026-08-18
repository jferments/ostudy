#!/usr/bin/env python3
"""One-shot compile and observability follow-up for the PR #54 provider refactor."""
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


replace_once(
    "src-tauri/src/ai/document_intelligence.rs",
    "super::concurrency::document_request_parallelism(&task.network_scope)",
    "crate::ai::concurrency::document_request_parallelism(&task.network_scope)",
    "curation concurrency module path",
)
replace_once(
    "src-tauri/src/ai/gateway.rs",
    '''    /// `initial_response_timeout_ms` is intentionally *not* a job deadline. It
    /// covers connection establishment and receipt of provider response headers.
    /// Successful headers are the provider acknowledgment boundary. After that,
    /// ostudy does not impose a blunt total inference deadline: actively producing
    /// streams may run as needed, while a provider that stops producing usable
    /// model output is canceled by a provider-aware progress timeout. Response-size
    /// bounds and exact-task cancellation remain authoritative.''',
    '''    /// `inactivity_timeout_ms` is intentionally *not* a job deadline. It
    /// bounds silence while waiting for response headers and between received SSE
    /// chunks. Heartbeats, reasoning deltas, output deltas, tool arguments, and
    /// final usage events all prove transport activity and reset the timer. There
    /// is no total inference wall clock; response-size bounds and exact-task
    /// cancellation remain authoritative.''',
    "stream inactivity API documentation",
)
replace_once(
    "src-tauri/src/ai/gateway.rs",
    '''        let usage = stream.usage.clone();
        if let Some(usage) = usage.as_ref() {
            record_current_usage(usage_delta(usage));
        }''',
    '''        let usage = stream.usage.clone();
        record_current_usage(
            usage
                .as_ref()
                .map(usage_delta)
                .unwrap_or_default(),
        );''',
    "record usage-missing stream completions",
)
print("PR #54 provider refactor follow-up applied.")
