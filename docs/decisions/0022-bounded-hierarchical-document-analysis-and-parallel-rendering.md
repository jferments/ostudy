# ADR 0022: Bounded hierarchical document analysis and parallel page rendering

- **Status:** Accepted
- **Date:** 2026-08-09
- **Scope:** PR #38 large-document generation reliability and local PDF performance

## Context

Real-provider acceptance established an important asymmetry:

- an ~11-page document with a previously persisted outline could successfully run **Generate more cards**;
- a ~138-page document could complete local extraction/rendering but fail while building the initial document outline with both DeepSeek and llama.cpp / Unsloth.

The successful second pass was not evidence that first-pass outline construction worked: Generate-more intentionally reuses the previously persisted outline.

The earlier first-pass architecture did split a PDF into semantic windows, but several large-document-only failure surfaces remained. Outline source windows were still large, a pathological extractor chunk could bypass the nominal limit, full outline fragments were recursively sent back to an LLM for a global merge, and the resulting global outline could inflate every later prompt. Provider `finish_reason` was also not preserved, so an output/context truncation could look like generic schema drift.

Real testing then exposed two provider-specific symptoms:

- llama.cpp / Unsloth could accept a streamed outline request but yield no final content visible through the narrow OpenAI delta-content parser;
- DeepSeek could reach its structured-output completion limit while producing an outline fragment.

Separately, the PDF rasterizer had originally been serial. Multicore rendering solved most of the local preprocessing delay, but the document projection still did not expose the known page total and per-page progress until processing completed.

Document deletion also exposed a lifecycle bug: a soft-deleted generated deck still had a `document_decks` provenance row and was incorrectly counted as a surviving citation destination.

## Decision

### Hard-bounded semantic provider windows

Document intelligence uses conservative source limits:

- outline windows: approximately **3,200 estimated source tokens**, at most **8 physical pages**, with about **350 tokens** of semantic overlap;
- initial flashcard-generation windows: approximately **5,000 estimated source tokens**, at most **10 physical pages**, with about **700 tokens** of overlap;
- Generate-more windows: the same 5,000-token / 10-page generation boundary.

These are source-content limits, not exact provider-tokenizer counts. They intentionally leave room for task instructions, schema/tool definitions, compact global context, rendered images, and completion tokens.

If a persisted semantic chunk is individually larger than a provider window, ostudy splits only that pathological chunk into bounded continuation source units. Continuations retain the original chunk/page provenance and are marked as model-facing segments. Normal semantic chunks remain intact.

Large documents therefore scale by creating **more bounded requests**, not by increasing individual prompt size.

### Concise outline fragments

Each outline window asks for a deliberately small structured fragment rather than allowing the provider to fill the application-wide outline maximums. The fragment contract caps the number and size of objectives, concepts, sections, section summaries, and section-local objectives/concepts.

DeepSeek outline requests receive a larger completion allowance than llama.cpp while keeping the source and requested fragment shape smaller. This directly addresses the real `finish_reason=length` failure without increasing the source window.

### Deterministic hierarchy plus bounded whole-document LLM synthesis

Each outline source window produces one provider-enforced structured fragment. Native Rust then assembles the authoritative document hierarchy:

- sanitize and bound fragment text/items;
- clamp provider page ranges to the source window/document;
- repair reversed page ranges;
- deduplicate objectives and concepts;
- sort sections by physical page range;
- stitch adjacent/overlapping same-title sections;
- synthesize a bounded fallback section when a provider returns no section list.

ostudy no longer asks a model to recursively merge multiple **full** outline objects. The section hierarchy and page spans therefore do not depend on one fragile large model call.

However, the configured model still receives a global semantic view. After native hierarchy assembly, ostudy builds a bounded whole-document digest that reserves coverage for sections from the beginning through the end of the document. One optional schema-constrained **global synthesis** request converts that digest into a concise whole-document summary, major learning objectives, and key concepts. It cannot rewrite section/page hierarchy.

The synthesis is non-fatal. If it is rejected, truncated, or fails local validation, the already-valid native aggregate remains the persisted global context and generation continues. This preserves the value of a model-level whole-document view without recreating the old context-sensitive outline-of-outlines bottleneck.

### Compact relevant context during generation

A flashcard-generation request receives:

- the bounded whole-document summary;
- a bounded subset of global objectives/concepts;
- only outline sections whose page ranges overlap the current source window;
- the bounded source window and selected page renders.

The full hierarchy is not copied into every provider request. Final curation also receives a compact global learning summary.

### Provider-enforced structure with compatibility parsing

llama.cpp / Unsloth continues to use llama-server schema-constrained JSON generation. Current llama-server accepts JSON Schema in the response-format path and also supports a top-level `json_schema` compatibility field. ostudy retries the exact same server/profile/model once with that compatibility representation when the primary request shape is rejected.

The SSE gateway primarily consumes OpenAI-compatible `choices[].delta.content`, but now also accepts compatible llama-server variants seen across versions/wrappers: a full `choices[].message.content`, completion `text`, or native top-level `content`. A final full-message snapshot is not appended twice after content deltas. If the provider closes the stream with no final content/tool arguments, the safe error includes its `finish_reason` when present.

DeepSeek document tasks use Beta strict Function Calling. The result function is forced, `strict: true` is enabled, and a DeepSeek-compatible schema is supplied. Native validation remains authoritative after provider schema enforcement.

### Distinguish truncation from contract errors

The SSE gateway preserves `choices[0].finish_reason`.

Document intelligence distinguishes at least:

- `length` → `ai_document_output_truncated`;
- `content_filter` → `ai_document_output_filtered`;
- `insufficient_system_resource` → `ai_provider_resource_exhausted`;
- malformed/undecodable outer task objects → `ai_document_output_invalid`.

Outline errors additionally identify exact source-window number, total window count, PDF page range, estimated source-token count, and bounded final validation detail. Safe diagnostic logging records the same metadata without raw source text, provider bodies, credentials, Authorization headers, or hidden reasoning.

### Normalize recoverable outline semantics locally

Provider JSON Schema owns structure and types. Rust owns semantic cleanup and safety bounds.

A valid structured outline is not discarded merely for a reversed page range, a range slightly outside the current source window, or an oversized recoverable string/list. Native normalization repairs/clamps ranges and bounds/deduplicates content before persistence.

### Bounded parallel page rasterization with live progress

The justpdf renderer remains the pure-Rust rasterization backend, with worker count:

```text
min(available_parallelism, page_count, 16)
```

Workers render distinct physical pages concurrently. Results return through a channel and the coordinator preserves physical-page order before invoking the existing managed-file/SQLite persistence visitor.

Structured extraction determines the physical page count before rasterization begins. ostudy immediately records that total in the document projection, reports `0/total`, and increments `rendered_page_count` as ordered page renders are durably written. The existing Documents polling therefore exposes live `x/x rendered` progress while rasterization continues.

The renderer backend identity remains:

```text
justpdf-render-0.1.2/pure-rust-parallel-v1
```

Documents with older renderer identities rebuild their page derivatives on retry.

### Citation preservation only targets surviving decks

`document_decks` is durable provenance and can temporarily outlive a deck's visible lifetime because deck deletion is soft-delete based. Permanent document deletion must therefore not equate provenance existence with a surviving user-visible deck.

Citation preservation now selects only provenance destinations whose deck row has `deleted_at IS NULL`. A previously deleted generated deck is neither updated nor counted in `preservedDeckCitationCount`. If no active generated deck remains, deleting the document preserves zero citations and the UI does not claim otherwise.

## Consequences

- Small and large PDFs use the same architecture; large files simply produce more outline requests.
- Initial outline calls are smaller than card-generation calls because they also need structured hierarchy output.
- DeepSeek has additional structured-output headroom without increasing source-context pressure.
- llama.cpp compatibility is broader without accepting unconstrained or unvalidated model output.
- The model has a bounded global semantic view of the complete document, while Rust remains authoritative for hierarchy/page provenance.
- A failure of global synthesis does not invalidate successful local outline analysis.
- Pathological parser chunks cannot bypass request bounds.
- Chapter/section information improves organization without becoming a brittle hard dependency.
- Later generation requests do not grow linearly with total document outline size.
- Diagnostics can distinguish truncation/resource/schema/empty-stream failures and identify the precise source window.
- Rendering uses multiple CPU cores and exposes incremental progress while keeping persistence ordered.
- Soft-deleted decks are not treated as surviving citation destinations.
- Provider calls remain sequential for one document. Parallelizing LLM requests is deliberately separate because local inference servers commonly share constrained accelerator/context resources and cloud providers impose rate limits.

## Validation

Automated coverage verifies hard semantic-window bounds, oversized continuation chunks, concise outline schema bounds, deterministic native outline assembly, a whole-document digest that reaches the end of the source, relevant-section filtering, streamed finish-reason retention, llama-compatible streamed content forms, provider-enforced schema paths, soft-deleted citation filtering, live rendered-page UI state, parallel renderer worker bounds, a real PDF extraction/render smoke, the full Rust/frontend suite, and native Tauri smoke tests.

Real-provider acceptance must still retest the representative large PDF with DeepSeek and the configured llama.cpp / Unsloth server. Performance acceptance should verify incremental `x/x rendered` updates and compare elapsed render time/CPU utilization with the previous serial implementation.
