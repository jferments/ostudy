#!/usr/bin/env python3
"""Apply the PR #54 document pipeline efficiency and retry-policy refactor."""
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
# Concurrency follows execution topology rather than provider brand. Cloud API
# windows are independent and can overlap; local/LAN inference remains serial to
# avoid contending for one explicitly configured model server.
# ---------------------------------------------------------------------------
path = Path("src-tauri/src/ai/concurrency.rs")
new_concurrency = '''//! Bounded concurrency policy for independent document-provider windows.
//!
//! Concurrency follows the execution topology, not a list of provider brands.
//! Cloud requests can overlap because every request owns a disjoint bounded
//! source window and durable commits remain source-ordered. Local and LAN model
//! servers remain serial so ostudy does not assume that one user-managed
//! inference process was started with multiple decoding slots.

pub(crate) const CLOUD_DOCUMENT_PARALLELISM: usize = 4;

pub(crate) fn document_request_parallelism(network_scope: &str) -> usize {
    match network_scope {
        "cloud" => CLOUD_DOCUMENT_PARALLELISM,
        _ => 1,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cloud_windows_overlap_while_user_managed_servers_stay_serial() {
        assert_eq!(document_request_parallelism("cloud"), 4);
        assert_eq!(document_request_parallelism("local"), 1);
        assert_eq!(document_request_parallelism("lan"), 1);
    }
}
'''
if "Concurrency follows the execution topology" not in path.read_text():
    path.write_text(new_concurrency)

# ---------------------------------------------------------------------------
# Outline execution: no extra timeout-specific retry/fallback layer. The shared
# gateway owns transport retry once; native fallback is reserved for invalid or
# exhausted outline output, not a silent connection failure.
# ---------------------------------------------------------------------------
path = "src-tauri/src/ai/document_intelligence_adapter.rs"
replace_once(
    path,
    '''    task_cancellation::{record_current_provider_retry, set_current_task_phase_range},''',
    '''    task_cancellation::set_current_task_phase_range,''',
    "remove outline transport retry ownership",
)
replace_once(
    path,
    '''    let parallelism = super::concurrency::document_request_parallelism(&task.provider_kind)
        .clamp(1, windows.len());''',
    '''    let parallelism = super::concurrency::document_request_parallelism(&task.network_scope)
        .clamp(1, windows.len());''',
    "topology-based outline concurrency",
)
replace_once(
    path,
    '''        set_current_task_phase_range(
            "outline",
            Some(batch[0] + 1),
            batch.last().map(|index| *index + 1),
            completed,''',
    '''        set_current_task_phase_range(
            "outline",
            (Some(batch[0] + 1), batch.last().map(|index| *index + 1)),
            completed,''',
    "outline active-window tuple",
)
replace_once(
    path,
    '''    let result = match request_outline_with_resilience(
        runtime,
        task,
        document_title,
        page_count,
        window,
    )
    .await''',
    '''    let result = match implementation::request_outline_fragment(
        runtime,
        task,
        document_title,
        page_count,
        window,
        &[],
    )
    .await''',
    "shared-gateway outline request",
)
regex_once(
    path,
    r"\nasync fn request_outline_with_resilience\(.*?\n\}\n\n#\[cfg\(test\)\]",
    "\n#[cfg(test)]",
    "remove duplicate outline transport retry",
)

path = "src-tauri/src/ai/document_intelligence.rs"
remove_once(
    path,
    '''            | "ai_stream_progress_timeout"
''',
    "remove transport inactivity from native outline fallback",
)

# ---------------------------------------------------------------------------
# Generation: compact-output retry is only for output exhaustion/shape pressure.
# Transport inactivity is retried once by the gateway with the same logical
# request; changing card budget would diagnose the wrong failure mode.
# ---------------------------------------------------------------------------
path = "src-tauri/src/document_generation.rs"
replace_once(
    path,
    '''    let parallelism =
        crate::ai::concurrency::document_request_parallelism(&input.task.provider_kind)
            .clamp(1, input.windows.len().max(1));''',
    '''    let parallelism =
        crate::ai::concurrency::document_request_parallelism(&input.task.network_scope)
            .clamp(1, input.windows.len().max(1));''',
    "topology-based generation concurrency",
)
replace_once(
    path,
    '''        set_current_task_phase_range(
            "generating",
            Some(batch[0] + 1),
            batch.last().map(|position| *position + 1),
            completed,''',
    '''        set_current_task_phase_range(
            "generating",
            (
                Some(batch[0] + 1),
                batch.last().map(|position| *position + 1),
            ),
            completed,''',
    "generation active-window tuple",
)
replace_once(
    path,
    '''        Err(error)
            if input.maximum_cards > 2
                && (matches!(
                    error.code.as_str(),
                    "ai_document_output_truncated"
                        | "ai_stream_progress_timeout"
                        | "ai_empty_response"
                ) || (error.code == "ai_document_output_invalid"
                    && error.message.contains("more than"))) =>''',
    '''        Err(error) if should_retry_with_smaller_output(&error, input.maximum_cards) =>''',
    "output-specific compact retry",
)
helper_anchor = '''#[cfg(test)]
mod tests {'''
compact_helper = '''fn should_retry_with_smaller_output(error: &CommandError, maximum_cards: usize) -> bool {
    maximum_cards > 2
        && (matches!(
            error.code.as_str(),
            "ai_document_output_truncated" | "ai_empty_response"
        ) || (error.code == "ai_document_output_invalid"
            && error.message.contains("more than")))
}

'''
replace_once(path, helper_anchor, compact_helper + helper_anchor, "compact retry predicate")
replace_once(
    path,
    '''    #[test]
    fn provider_parallelism_is_bounded_before_window_execution() {
        assert_eq!(
            crate::ai::concurrency::document_request_parallelism("openrouter"),
            4
        );
        assert_eq!(
            crate::ai::concurrency::document_request_parallelism("deepseek"),
            2
        );
        assert_eq!(
            crate::ai::concurrency::document_request_parallelism("llama_cpp"),
            1
        );
    }''',
    '''    #[test]
    fn provider_parallelism_is_bounded_by_execution_topology() {
        assert_eq!(
            crate::ai::concurrency::document_request_parallelism("cloud"),
            4
        );
        assert_eq!(
            crate::ai::concurrency::document_request_parallelism("local"),
            1
        );
        assert_eq!(
            crate::ai::concurrency::document_request_parallelism("lan"),
            1
        );
    }

    #[test]
    fn transport_inactivity_never_changes_the_card_budget() {
        let inactivity = CommandError::new("ai_stream_inactive", "silent transport");
        assert!(!should_retry_with_smaller_output(&inactivity, 10));

        let truncation = CommandError::new("ai_document_output_truncated", "length");
        assert!(should_retry_with_smaller_output(&truncation, 10));
    }''',
    "generation retry/concurrency regressions",
)

# ---------------------------------------------------------------------------
# Global curation: four cloud batches can run in one wave while local/LAN
# inference remains serial. Results are consumed in original batch/index order,
# so concurrency does not change final deterministic card ordering.
# ---------------------------------------------------------------------------
path = "src-tauri/src/ai/document_intelligence.rs"
curation_block = r'''pub(crate) async fn curate_document_cards(
    runtime: &AiRuntimeState,
    task: &DocumentTaskContext,
    outline: &DocumentOutline,
    cards: Vec<Value>,
) -> Result<Vec<Value>, CommandError> {
    if cards.is_empty() {
        return Ok(cards);
    }

    let outline_json = outline_context_for_curation(outline)?;
    let batches = into_curation_batches(cards);
    let parallelism = super::concurrency::document_request_parallelism(&task.network_scope)
        .clamp(1, batches.len().max(1));
    crate::logging::debug_target(
        "ai.document.curation",
        "document_curation_execution_plan",
        "Prepared bounded card-curation batches without logging generated card content.",
        json!({
            "provider": task.provider_kind.as_str(),
            "model": task.model_id.as_str(),
            "batches": batches.len(),
            "parallelism": parallelism,
            "candidates": batches.iter().map(Vec::len).sum::<usize>(),
        }),
    );

    let mut retained = Vec::new();
    let mut cursor = 0_usize;
    while cursor < batches.len() {
        let end = (cursor + parallelism).min(batches.len());
        let indices = (cursor..end).collect::<Vec<_>>();
        let p0 = indices.first().copied();
        let p1 = indices.get(1).copied();
        let p2 = indices.get(2).copied();
        let p3 = indices.get(3).copied();
        let (r0, r1, r2, r3) = tokio::join!(
            curation_slot(runtime, task, &outline_json, &batches, p0),
            curation_slot(runtime, task, &outline_json, &batches, p1),
            curation_slot(runtime, task, &outline_json, &batches, p2),
            curation_slot(runtime, task, &outline_json, &batches, p3),
        );

        for (batch_index, result) in [r0, r1, r2, r3].into_iter().flatten() {
            let batch = &batches[batch_index];
            match result {
                Ok(keep) => retained.extend(
                    keep.into_iter()
                        .filter_map(|index| batch.get(index).cloned()),
                ),
                Err(error) => {
                    crate::logging::warn_target(
                        "ai.document.curation",
                        "document_curation_batch_skipped",
                        "A bounded curation batch failed; validated candidates from that batch were retained rather than discarded.",
                        json!({
                            "batch": batch_index + 1,
                            "candidates": batch.len(),
                            "code": error.code.as_str(),
                            "detail": bounded_diagnostic(&error.message, 400),
                        }),
                    );
                    retained.extend(batch.iter().cloned());
                }
            }
        }
        cursor = end;
    }

    let mut seen = HashSet::new();
    retained.retain(|card| seen.insert(card_identity(card)));
    Ok(retained)
}

fn into_curation_batches(cards: Vec<Value>) -> Vec<Vec<Value>> {
    let mut cards = cards.into_iter();
    let mut batches = Vec::new();
    loop {
        let batch = cards
            .by_ref()
            .take(MAX_CURATE_CARDS_PER_BATCH)
            .collect::<Vec<_>>();
        if batch.is_empty() {
            break;
        }
        batches.push(batch);
    }
    batches
}

async fn curation_slot(
    runtime: &AiRuntimeState,
    task: &DocumentTaskContext,
    outline_json: &str,
    batches: &[Vec<Value>],
    batch_index: Option<usize>,
) -> Option<(usize, Result<Vec<usize>, CommandError>)> {
    let batch_index = batch_index?;
    let batch = batches.get(batch_index)?;
    let summaries = batch
        .iter()
        .enumerate()
        .map(|(index, card)| compact_card_summary(index, card))
        .collect::<Vec<_>>();
    let user = format!(
        "Global document outline:
<ostudy-global-outline>
{outline_json}
</ostudy-global-outline>

         Candidate flashcards:
<ostudy-candidates>
{}
</ostudy-candidates>

         Return the zero-based indices of candidates that should remain. Keep strong coverage of major learning objectives and a useful mix of recall and application.          QUALITY GATE: reject a candidate unless its answer directly and completely answers its question and is supported by the document context. Reject question/answer mismatches, incomplete answers to multi-part questions, answers that are only headings/topic labels, unsupported numerical claims, reversed question/answer direction, vague prompts whose answer is not uniquely determined, and cards that require seeing the PDF to make sense. For definitions, keep only genuine term/definition pairs.          Also remove globally minor or hyperspecific trivia, bibliography/citation artifacts, near-duplicates, and redundant cards that test the same knowledge in essentially the same way. When uncertain whether a candidate is semantically correct, drop it. Do not rewrite cards and do not invent replacements.",
        summaries.join("\n")
    );
    Some((
        batch_index,
        request_curation_with_retry(runtime, task, &user, batch.len()).await,
    ))
}

pub(super) async fn request_outline_fragment'''
regex_once(
    path,
    r"pub\(crate\) async fn curate_document_cards\(.*?\npub\(super\) async fn request_outline_fragment",
    curation_block,
    "parallel deterministic curation",
)
replace_once(
    path,
    '''    crate::logging::info(
        "document_ai_usage",
        format!(
            "provider={} model={} reported={} prompt_tokens={} completion_tokens={} total_tokens={}",
            task.provider_kind,
            task.model_id,
            reported,
            prompt_tokens.map_or_else(|| "unknown".to_string(), |value| value.to_string()),
            completion_tokens.map_or_else(|| "unknown".to_string(), |value| value.to_string()),
            total_tokens.map_or_else(|| "unknown".to_string(), |value| value.to_string()),
        ),
    );''',
    '''    crate::logging::debug_target(
        "ai.document.usage",
        "document_ai_usage",
        "Recorded provider token usage without logging prompt or model content.",
        json!({
            "provider": task.provider_kind.as_str(),
            "model": task.model_id.as_str(),
            "reported": reported,
            "promptTokens": prompt_tokens,
            "completionTokens": completion_tokens,
            "totalTokens": total_tokens,
        }),
    );''',
    "AI-targeted usage diagnostics",
)
# Add a pure batching regression beside the existing curation parser test.
replace_once(
    path,
    '''    #[test]
    fn curation_rejects_duplicate_or_out_of_range_indices() {''',
    '''    #[test]
    fn curation_batches_preserve_order_and_never_exceed_the_contract() {
        let cards = (0..97).map(|index| json!({ "index": index })).collect();
        let batches = into_curation_batches(cards);
        assert_eq!(batches.iter().map(Vec::len).collect::<Vec<_>>(), vec![32, 32, 32, 1]);
        assert_eq!(batches[0][0]["index"], 0);
        assert_eq!(batches[3][0]["index"], 96);
    }

    #[test]
    fn curation_rejects_duplicate_or_out_of_range_indices() {''',
    "curation batching regression",
)

# ---------------------------------------------------------------------------
# Architecture documentation: one generic stream-liveness contract and one
# topology-based concurrency rule. Fixed whole-request deadlines are explicitly
# rejected, while provider-specific routing remains isolated to OpenRouter.
# ---------------------------------------------------------------------------
path = "docs/architecture/document-ai-routing.md"
p = Path(path)
value = p.read_text()
value = value.replace(
    "The current provider concurrency policy is conservative: local llama.cpp runs one request at a time, direct DeepSeek uses two, and OpenRouter may use four.",
    "Concurrency follows execution topology rather than provider brand: cloud profiles may run up to four independent document windows concurrently, while local and LAN profiles remain serial unless a future explicit server-capacity capability says otherwise.",
)
value = value.replace(
    "Provider resilience is progress-based rather than a blunt whole-request wall clock. Once a stream is accepted, an actively producing response may run longer than three minutes; a stream that produces no usable model output for 75 seconds is treated as stalled. OpenRouter keepalive comments do not reset that timer. Output exhaustion (`finish_reason=length`) is classified as truncation, not malformed JSON. For card generation, either truncation or a true stream stall retries the affected source window once with a smaller card/output budget instead of repeating the identical oversized request. Completed windows remain checkpointed for Retry.",
    "Provider resilience is based on transport inactivity rather than a blunt whole-request wall clock. The profile timeout bounds silence while connecting and between received SSE events. Any provider heartbeat/comment, reasoning delta, output delta, tool-argument delta, or final usage event proves the connection is responsive and resets that timer; an active request may therefore run as long as needed. A silent transport failure is retried once with the same logical request. Output exhaustion (`finish_reason=length`) is a separate condition: only truncation/output-shape pressure receives a smaller card budget. Completed windows remain checkpointed for Retry.",
)
p.write_text(value)

path = "docs/architecture/ai-and-document-processing.md"
replace_once(
    path,
    "A running document task owns one process-local cancellation signal keyed by document ID. The same registry spans extraction, platform rendering, OCR, and provider execution. Independent remote-provider outline/generation windows execute with bounded concurrency (currently OpenRouter 4, direct DeepSeek 2, local llama.cpp 1); network work may finish out of order, but checkpoints and SQLite state are committed serially in source-window order. Provider resilience is based on useful stream progress rather than a blunt whole-request wall clock: OpenRouter requests that produce no usable model output for 75 seconds are canceled, while actively producing streams may run longer; non-OpenRouter/local inference receives a substantially larger no-progress allowance.",
    "A running document task owns one process-local cancellation signal keyed by document ID. The same registry spans extraction, platform rendering, OCR, and provider execution. Independent cloud outline/generation/curation batches execute with bounded concurrency of up to four; local and LAN model servers remain serial unless a future verified capacity capability says otherwise. Network work may finish out of order, but checkpoints, final card ordering, and SQLite writes remain deterministic. Provider resilience uses one shared inactivity contract rather than provider-name deadlines or total request timers: the profile timeout bounds silence between response headers/SSE events, while heartbeats, reasoning deltas, output deltas, tool arguments, and final usage keep an active request alive indefinitely.",
    "long-running document provider contract",
)
replace_once(
    path,
    "After source windows complete, a bounded curation task selects which cards remain.",
    "After source windows complete, bounded curation batches select which cards remain. Cloud batches may overlap under the same four-request ceiling, but their results are consumed in original batch/index order; local/LAN curation remains serial.",
    "curation concurrency documentation",
)

print("PR #54 document pipeline efficiency refactor applied.")
