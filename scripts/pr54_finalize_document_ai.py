#!/usr/bin/env python3
from pathlib import Path

intelligence = Path('src-tauri/src/ai/document_intelligence.rs')
value = intelligence.read_text()
old = '''fn outline_completion_tokens_for_provider(_provider_kind: ProviderKind) -> usize {
    1_536
}'''
new = '''#[cfg(test)]
fn outline_completion_tokens_for_provider(_provider_kind: ProviderKind) -> usize {
    1_536
}'''
if old not in value:
    raise SystemExit('outline helper anchor not found')
intelligence.write_text(value.replace(old, new, 1))

openrouter = Path('src-tauri/src/ai/providers/openrouter.rs')
value = openrouter.read_text()
old = 'const HIGH_PRECISION_QUANTIZATIONS: [&str; 5] = ["int8", "fp8", "fp16", "bf16", "fp32"];'
new = 'const HIGH_PRECISION_QUANTIZATIONS: [&str; 6] = ["int8", "fp8", "mxfp8", "fp16", "bf16", "fp32"];'
if old not in value:
    raise SystemExit('quantization allowlist anchor not found')
value = value.replace(old, new, 1)
old = 'Some(&json!(["int8", "fp8", "fp16", "bf16", "fp32"]))'
new = 'Some(&json!(["int8", "fp8", "mxfp8", "fp16", "bf16", "fp32"]))'
if old not in value:
    raise SystemExit('quantization regression anchor not found')
openrouter.write_text(value.replace(old, new, 1))

docs = Path('docs/architecture/ai-and-document-processing.md')
value = docs.read_text()
old = '''Provider source windows are deliberately conservative. Large documents scale through **more bounded requests**, not larger individual prompts. A pathological semantic chunk larger than the provider boundary is split into bounded continuation units while retaining original source/page provenance.

Local llama.cpp/Unsloth jobs are **window-bounded, not aggregate-job-bounded**. The former whole-job 1.2M-token ceiling and cloud-only source-token guard do not apply to local execution. Individual requests remain constrained by the configured/reported model context. Metered/cloud providers retain conservative aggregate runaway-cost protection.'''
new = '''Provider source windows are deliberately conservative and provider-neutral. Heading changes remain semantic hierarchy metadata but do **not** force request boundaries. Large-context models can pack more adjacent source units into each bounded request; smaller-context models automatically receive smaller windows and therefore more requests. A pathological semantic chunk larger than the provider boundary is split into bounded continuation units while retaining original source/page provenance.

All providers are **window-bounded, not aggregate-job-bounded**. Whole-document token estimates are diagnostic/cost information rather than a provider-specific rejection rule. Every individual request must still fit the configured/reported model context after prompt, schema, response, image, and calibrated safety reserves.'''
if old not in value:
    raise SystemExit('windowing architecture anchor not found')
value = value.replace(old, new, 1)
old = 'Before card generation, ostudy requests one concise schema-constrained outline fragment per bounded source window. Fragments contain a summary, learning objectives, concepts, and section/chapter-like source spans.'
new = 'Before card generation, ostudy requests one concise schema-constrained outline fragment per bounded source window. Request packing is capacity-driven rather than heading-driven, so a heading-dense textbook does not degenerate into one provider call per section or page. Fragments still carry the headings, summary, learning objectives, concepts, and source spans needed to reconstruct the hierarchy.'
if old not in value:
    raise SystemExit('outline architecture anchor not found')
value = value.replace(old, new, 1)
old = "DeepSeek document tasks use strict Function Calling. The native adapter converts internal schemas to DeepSeek's supported strict subset and Rust enforces tighter length/count/semantic rules afterward. Metered-provider aggregate protection remains active."
new = "DeepSeek document tasks use strict Function Calling. The native adapter converts internal schemas to DeepSeek's supported strict subset and Rust enforces tighter length/count/semantic rules afterward. Large jobs use the same per-request context/window bounds as other providers."
if old not in value:
    raise SystemExit('DeepSeek architecture anchor not found')
value = value.replace(old, new, 1)
old = 'A running document task owns one process-local cancellation signal keyed by document ID. The same registry spans extraction, platform rendering, OCR, and provider execution.'
new = 'A running document task owns one process-local cancellation signal keyed by document ID. The same registry spans extraction, platform rendering, OCR, and provider execution. Independent remote-provider outline/generation windows execute with bounded concurrency (currently OpenRouter 4, direct DeepSeek 2, local llama.cpp 1); network work may finish out of order, but checkpoints and SQLite state are committed serially in source-window order. Each remote window also has a bounded work deadline so one accepted but stalled stream cannot pin an entire document indefinitely.'
if old not in value:
    raise SystemExit('long-running architecture anchor not found')
docs.write_text(value.replace(old, new, 1))
