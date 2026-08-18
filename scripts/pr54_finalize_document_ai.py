#!/usr/bin/env python3
# One-shot PR #54 cleanup; removed after the finalizer commits the product changes.
from pathlib import Path
import re


def replace_once(value: str, old: str, new: str, label: str) -> str:
    if new in value:
        return value
    if old not in value:
        raise SystemExit(f'{label} anchor not found')
    return value.replace(old, new, 1)


# Keep the provider-specific completion helper available only to its regression tests.
intelligence = Path('src-tauri/src/ai/document_intelligence.rs')
value = intelligence.read_text()
old = '''fn outline_completion_tokens_for_provider(_provider_kind: ProviderKind) -> usize {
    1_536
}'''
new = '''#[cfg(test)]
fn outline_completion_tokens_for_provider(_provider_kind: ProviderKind) -> usize {
    1_536
}'''
intelligence.write_text(replace_once(value, old, new, 'outline helper'))

# OpenRouter: explicitly accept every 8-bit-or-better quantization label we support.
openrouter = Path('src-tauri/src/ai/providers/openrouter.rs')
value = openrouter.read_text()
if '"mxfp8"' not in value.split('pub(super) fn text_probe_body', 1)[0]:
    pattern = re.compile(
        r'const HIGH_PRECISION_QUANTIZATIONS: \[&str; \d+\]\s*=\s*\[[^\]]+\];',
        re.MULTILINE,
    )
    replacement = (
        'const HIGH_PRECISION_QUANTIZATIONS: [&str; 6] = '
        '["int8", "fp8", "mxfp8", "fp16", "bf16", "fp32"];'
    )
    value, count = pattern.subn(replacement, value, count=1)
    if count != 1:
        raise SystemExit('quantization allowlist anchor not found')
if 'json!(["int8", "fp8", "mxfp8", "fp16", "bf16", "fp32"])' not in value:
    old_sequence = '["int8", "fp8", "fp16", "bf16", "fp32"]'
    if old_sequence not in value:
        raise SystemExit('quantization regression anchor not found')
    value = value.replace(
        old_sequence,
        '["int8", "fp8", "mxfp8", "fp16", "bf16", "fp32"]',
        1,
    )
openrouter.write_text(value)

# Give the normal card request and its smaller truncation-recovery request independent
# provider deadlines. A slow first response must not consume the compact retry's budget.
generation = Path('src-tauri/src/document_generation.rs')
value = generation.read_text()
start = value.find('async fn generation_slot(')
end = value.find('\n#[cfg(test)]', start)
if start < 0 or end < 0:
    raise SystemExit('generation recovery block not found')
replacement = r'''async fn generation_slot(
    input: &GenerationExecution<'_>,
    position: Option<usize>,
    images: &[document_intelligence::SourcePageImage],
) -> Option<(usize, Result<Vec<Value>, CommandError>)> {
    let position = position?;
    let window = &input.windows[position];
    Some((
        position,
        generate_window_with_recovery(WindowGenerationRequest {
            runtime: input.runtime,
            task: input.task,
            document_title: input.document_title,
            outline: input.outline,
            window,
            images,
            maximum_cards: input.plan.max_cards_per_window,
            diagnostic_prefix: input.diagnostic_prefix,
        })
        .await,
    ))
}

async fn generate_provider_call_with_deadline(
    input: &WindowGenerationRequest<'_>,
    maximum_cards: usize,
) -> Result<Vec<Value>, CommandError> {
    for attempt in 1..=2 {
        let request = document_intelligence::generate_window_flashcards(
            input.runtime,
            input.task,
            input.document_title,
            input.outline,
            input.window,
            input.images,
            maximum_cards.max(1),
        );
        match timeout(
            Duration::from_secs(GENERATION_PROVIDER_ATTEMPT_SECONDS),
            request,
        )
        .await
        {
            Ok(result) => return result,
            Err(_) if attempt == 1 => {
                crate::logging::warn_target(
                    "ai.document.generation",
                    "document_generation_window_deadline_retry",
                    "A provider took too long on one bounded card-generation request; ostudy canceled that request and is retrying the same bounded request once.",
                    serde_json::json!({
                        "provider": input.task.provider_kind.as_str(),
                        "model": input.task.model_id.as_str(),
                        "window": input.window.window_index + 1,
                        "pageStart": input.window.page_start,
                        "pageEnd": input.window.page_end,
                        "cardBudget": maximum_cards,
                        "attemptSeconds": GENERATION_PROVIDER_ATTEMPT_SECONDS,
                    }),
                );
            }
            Err(_) => {
                return Err(CommandError::new(
                    "ai_provider_deadline",
                    format!(
                        "The AI provider exceeded the {GENERATION_PROVIDER_ATTEMPT_SECONDS}-second card-generation request deadline twice. ostudy canceled both incomplete requests instead of allowing this document window to stall indefinitely."
                    ),
                ));
            }
        }
    }
    unreachable!("the bounded two-attempt loop always returns")
}

async fn generate_window_with_recovery(
    input: WindowGenerationRequest<'_>,
) -> Result<Vec<Value>, CommandError> {
    let first_budget = input.maximum_cards.max(1);
    let first = generate_provider_call_with_deadline(&input, first_budget).await;
    match first {
        Err(error)
            if input.maximum_cards > 2
                && (error.code == "ai_document_output_truncated"
                    || (error.code == "ai_document_output_invalid"
                        && error.message.contains("more than"))) =>
        {
            let reduced_budget = (input.maximum_cards / 2).max(2);
            crate::logging::info(
                "document_generation_window_compact_retry",
                format!(
                    "path={} window={} partition={} locator={} old_budget={} new_budget={} code={}",
                    input.diagnostic_prefix,
                    input.window.window_index + 1,
                    input.window.partition_index + 1,
                    input.window.source_locator().stable_identity(),
                    input.maximum_cards,
                    reduced_budget,
                    error.code,
                ),
            );
            generate_provider_call_with_deadline(&input, reduced_budget).await
        }
        other => other,
    }
}
'''
value = value[:start] + replacement + value[end:]
generation.write_text(value)

# Align architecture docs with the implemented provider-neutral window model.
docs = Path('docs/architecture/ai-and-document-processing.md')
value = docs.read_text()
value = replace_once(
    value,
    '''Provider source windows are deliberately conservative. Large documents scale through **more bounded requests**, not larger individual prompts. A pathological semantic chunk larger than the provider boundary is split into bounded continuation units while retaining original source/page provenance.

Local llama.cpp/Unsloth jobs are **window-bounded, not aggregate-job-bounded**. The former whole-job 1.2M-token ceiling and cloud-only source-token guard do not apply to local execution. Individual requests remain constrained by the configured/reported model context. Metered/cloud providers retain conservative aggregate runaway-cost protection.''',
    '''Provider source windows are deliberately conservative and provider-neutral. Heading changes remain semantic hierarchy metadata but do **not** force request boundaries. Large-context models can pack more adjacent source units into each bounded request; smaller-context models automatically receive smaller windows and therefore more requests. A pathological semantic chunk larger than the provider boundary is split into bounded continuation units while retaining original source/page provenance.

All providers are **window-bounded, not aggregate-job-bounded**. Whole-document token estimates are diagnostic/cost information rather than a provider-specific rejection rule. Every individual request must still fit the configured/reported model context after prompt, schema, response, image, and calibrated safety reserves.''',
    'windowing architecture',
)
value = replace_once(
    value,
    'Before card generation, ostudy requests one concise schema-constrained outline fragment per bounded source window. Fragments contain a summary, learning objectives, concepts, and section/chapter-like source spans.',
    'Before card generation, ostudy requests one concise schema-constrained outline fragment per bounded source window. Request packing is capacity-driven rather than heading-driven, so a heading-dense textbook does not degenerate into one provider call per section or page. Fragments still carry the headings, summary, learning objectives, concepts, and source spans needed to reconstruct the hierarchy.',
    'outline architecture',
)
value = replace_once(
    value,
    "DeepSeek document tasks use strict Function Calling. The native adapter converts internal schemas to DeepSeek's supported strict subset and Rust enforces tighter length/count/semantic rules afterward. Metered-provider aggregate protection remains active.",
    "DeepSeek document tasks use strict Function Calling. The native adapter converts internal schemas to DeepSeek's supported strict subset and Rust enforces tighter length/count/semantic rules afterward. Large jobs use the same per-request context/window bounds as other providers.",
    'DeepSeek architecture',
)
value = replace_once(
    value,
    'A running document task owns one process-local cancellation signal keyed by document ID. The same registry spans extraction, platform rendering, OCR, and provider execution.',
    'A running document task owns one process-local cancellation signal keyed by document ID. The same registry spans extraction, platform rendering, OCR, and provider execution. Independent remote-provider outline/generation windows execute with bounded concurrency (currently OpenRouter 4, direct DeepSeek 2, local llama.cpp 1); network work may finish out of order, but checkpoints and SQLite state are committed serially in source-window order. Each remote request also has a bounded work deadline so one accepted but stalled stream cannot pin an entire document indefinitely.',
    'long-running architecture',
)
docs.write_text(value)
