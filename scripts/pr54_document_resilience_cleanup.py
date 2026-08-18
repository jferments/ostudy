#!/usr/bin/env python3
from pathlib import Path


def remove_once(path: str, text: str, label: str) -> None:
    p = Path(path)
    value = p.read_text()
    if text not in value:
        return
    p.write_text(value.replace(text, "", 1))


def replace_once(path: str, old: str, new: str, label: str) -> None:
    p = Path(path)
    value = p.read_text()
    if old not in value:
        if new in value:
            return
        raise SystemExit(f"{label}: anchor not found in {path}")
    p.write_text(value.replace(old, new, 1))


# Remove remnants that the first one-shot helper intentionally did not delete
# because its idempotency guard treated an empty replacement as already present.
remove_once(
    "src-tauri/src/document_generation.rs",
    "use tokio::time::timeout;\n",
    "stale generation timeout import",
)
remove_once(
    "src-tauri/src/document_generation.rs",
    "const GENERATION_PROVIDER_ATTEMPT_SECONDS: u64 = 180;\n",
    "stale generation deadline constant",
)
remove_once(
    "src-tauri/src/ai/document_intelligence_adapter.rs",
    "const OUTLINE_PROVIDER_ATTEMPT_SECONDS: u64 = 120;\n",
    "stale outline deadline constant",
)
replace_once(
    "src-tauri/src/document_scale.rs",
    "        max_cards_per_window: 16,",
    "        max_cards_per_window: 10,",
    "large-document card budget",
)

# A 75-second useful-output timeout is appropriate for remote OpenRouter routes,
# but local inference may legitimately have a long prefill. Keep a much larger
# no-progress safety bound for non-OpenRouter document providers.
replace_once(
    "src-tauri/src/ai/gateway.rs",
    "const DOCUMENT_STREAM_PROGRESS_TIMEOUT_SECONDS: u64 = 75;\n",
    "const OPENROUTER_STREAM_PROGRESS_TIMEOUT_SECONDS: u64 = 75;\nconst DEFAULT_STREAM_PROGRESS_TIMEOUT_SECONDS: u64 = 600;\n",
    "provider-aware stream progress constants",
)
replace_once(
    "src-tauri/src/ai/gateway.rs",
    "        let initial_timeout = Duration::from_millis(initial_response_timeout_ms.max(1) as u64);\n        let progress_timeout = Duration::from_secs(DOCUMENT_STREAM_PROGRESS_TIMEOUT_SECONDS);\n        let request_started = Instant::now();",
    "        let initial_timeout = Duration::from_millis(initial_response_timeout_ms.max(1) as u64);\n        let request_started = Instant::now();",
    "defer progress timeout selection until provider is known",
)
replace_once(
    "src-tauri/src/ai/gateway.rs",
    "        let openrouter = is_openrouter_url(&url);\n        let mut request = self",
    "        let openrouter = is_openrouter_url(&url);\n        let progress_timeout_seconds = if openrouter {\n            OPENROUTER_STREAM_PROGRESS_TIMEOUT_SECONDS\n        } else {\n            DEFAULT_STREAM_PROGRESS_TIMEOUT_SECONDS\n        };\n        let progress_timeout = Duration::from_secs(progress_timeout_seconds);\n        let mut request = self",
    "provider-aware progress timeout selection",
)
p = Path("src-tauri/src/ai/gateway.rs")
value = p.read_text()
value = value.replace(
    "return Err(stream_progress_timeout_error()),",
    "return Err(stream_progress_timeout_error(progress_timeout_seconds)),",
)
value = value.replace(
    "let failure = stream_progress_timeout_error();",
    "let failure = stream_progress_timeout_error(OPENROUTER_STREAM_PROGRESS_TIMEOUT_SECONDS);",
)
old = '''fn stream_progress_timeout_error() -> AiGatewayError {
    error(
        "ai_stream_progress_timeout",
        &format!(
            "The AI provider produced no usable streamed output for {DOCUMENT_STREAM_PROGRESS_TIMEOUT_SECONDS} seconds. ostudy canceled this stalled request so the document can retry or resume without waiting indefinitely."
        ),
        true,
        None,
    )
}'''
new = '''fn stream_progress_timeout_error(timeout_seconds: u64) -> AiGatewayError {
    error(
        "ai_stream_progress_timeout",
        &format!(
            "The AI provider produced no usable streamed output for {timeout_seconds} seconds. ostudy canceled this stalled request so the document can retry or resume without waiting indefinitely."
        ),
        true,
        None,
    )
}'''
if old in value:
    value = value.replace(old, new, 1)
elif new not in value:
    raise SystemExit("stream progress error helper anchor not found")
p.write_text(value)

replace_once(
    "src-tauri/src/ai/gateway.rs",
    "    /// Successful headers are the provider acknowledgment boundary. After that,\n    /// ostudy waits for streamed completion content without a wall-clock limit,\n    /// while still enforcing response-size bounds and exact-task cancellation.",
    "    /// Successful headers are the provider acknowledgment boundary. After that,\n    /// ostudy does not impose a blunt total inference deadline: actively producing\n    /// streams may run as needed, while a provider that stops producing usable\n    /// model output is canceled by a provider-aware progress timeout. Response-size\n    /// bounds and exact-task cancellation remain authoritative.",
    "gateway stream timeout documentation",
)

# Make per-window outline timing visible under --debug=ai. The prior generic
# app-target log was intentionally invisible to an AI-only diagnostic filter.
replace_once(
    "src-tauri/src/ai/document_intelligence_adapter.rs",
    '''    crate::logging::info(
        "document_outline_window_started",
        format!(
            "provider={} model={} window={}/{} partition={} pages={}-{} source_tokens={}",
            task.provider_kind,
            task.model_id,
            index + 1,
            windows.len(),
            window.partition_index + 1,
            window.page_start,
            window.page_end,
            window.token_estimate,
        ),
    );''',
    '''    crate::logging::debug_target(
        "ai.document.outline",
        "document_outline_window_started",
        "A bounded outline fragment started without logging document content.",
        serde_json::json!({
            "provider": task.provider_kind.as_str(),
            "model": task.model_id.as_str(),
            "window": index + 1,
            "totalWindows": windows.len(),
            "partition": window.partition_index + 1,
            "pageStart": window.page_start,
            "pageEnd": window.page_end,
            "sourceTokens": window.token_estimate,
        }),
    );''',
    "AI-targeted outline start diagnostic",
)

replace_once(
    "docs/architecture/ai-and-document-processing.md",
    "A running document task owns one process-local cancellation signal keyed by document ID. The same registry spans extraction, platform rendering, OCR, and provider execution. Independent remote-provider outline/generation windows execute with bounded concurrency (currently OpenRouter 4, direct DeepSeek 2, local llama.cpp 1); network work may finish out of order, but checkpoints and SQLite state are committed serially in source-window order. Each remote request also has a bounded work deadline so one accepted but stalled stream cannot pin an entire document indefinitely.",
    "A running document task owns one process-local cancellation signal keyed by document ID. The same registry spans extraction, platform rendering, OCR, and provider execution. Independent remote-provider outline/generation windows execute with bounded concurrency (currently OpenRouter 4, direct DeepSeek 2, local llama.cpp 1); network work may finish out of order, but checkpoints and SQLite state are committed serially in source-window order. Provider resilience is based on useful stream progress rather than a blunt whole-request wall clock: OpenRouter requests that produce no usable model output for 75 seconds are canceled, while actively producing streams may run longer; non-OpenRouter/local inference receives a substantially larger no-progress allowance.",
    "long-running document resilience architecture",
)

print("PR #54 resilience cleanup applied.")
