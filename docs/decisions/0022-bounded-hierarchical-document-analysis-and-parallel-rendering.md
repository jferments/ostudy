# ADR 0022: Bounded hierarchical document analysis and parallel page rendering

- **Status:** Accepted
- **Date:** 2026-08-09
- **Scope:** PR #38 large-document generation reliability and local PDF performance

## Context

Real-provider acceptance established an important asymmetry:

- an ~11-page document with a previously persisted outline could successfully run **Generate more cards**;
- a ~138-page document could complete local extraction/rendering but fail while building the initial document outline with both DeepSeek and llama.cpp / Unsloth.

The old first-pass architecture did split a PDF into semantic windows, but the bounds were still too large for a portable provider contract: outline windows allowed about 32,000 estimated source tokens / 48 pages and generation windows allowed about 20,000 source tokens / 40 pages. Large documents then required additional model calls that merged up to eight complete outline fragments at a time. Finally, the entire resulting global outline was copied into every later generation request.

One additional loophole made those limits soft: a single semantic chunk larger than the configured window could remain intact. Complex PDFs can produce oversized table/text chunks, so a nominally bounded pipeline could still emit one unexpectedly large provider request.

This created several large-document-only failure surfaces:

1. a local llama-server can have a smaller configured runtime context than the model's advertised native context;
2. the model-generated outline merge could itself become a large prompt/large structured result;
3. large global outline JSON inflated every later flashcard request;
4. an individually oversized semantic chunk could bypass the nominal window cap;
5. a structurally valid provider outline could be rejected because of recoverable semantic imperfections such as a reversed or slightly out-of-window page range;
6. streaming completion `finish_reason` was discarded, making output truncation look like generic schema drift.

The PDF rasterizer also rendered pages serially, leaving multicore desktop systems underutilized during long local preprocessing.

## Decision

### Hard-bounded semantic provider windows

Document intelligence uses conservative, provider-independent source limits:

- outline windows: approximately **5,000 estimated source tokens**, at most **10 physical pages**, with a small semantic overlap;
- flashcard-generation windows: approximately **5,000 estimated source tokens**, at most **10 physical pages**, with a small semantic overlap.

These are source-content limits rather than claims about an exact tokenizer. The point is to keep provider requests comfortably below common local server context allocations after system instructions, JSON Schema, compact outline context, images, and completion allowance are included.

When a persisted semantic chunk is individually larger than a provider window, ostudy splits only that pathological chunk into bounded continuation source units. Continuations retain the original chunk identity/page provenance and are marked as segments in the model-facing source. Normal semantic chunks remain intact.

Large documents therefore scale by creating **more bounded requests**, not by increasing individual prompt size.

### Hierarchical first pass without a model-generated global merge

Each outline source window produces one provider-enforced structured outline fragment. The native application then assembles the global outline deterministically:

- sanitize and bound fragment text/items;
- clamp provider page ranges to the source window and document;
- repair reversed page ranges;
- deduplicate objectives and concepts;
- sort sections by physical page range;
- stitch adjacent/overlapping same-title sections across neighboring windows;
- synthesize a bounded fallback section when a provider returns no section list.

The application no longer asks another LLM request to merge multiple full outline objects. This removes a second-order context/output failure point while preserving chapter/section information when the provider identifies it correctly.

This is intentionally more robust than making detected chapters the only processing unit. Chapters/sections inform context and organization, but later flashcard generation still uses bounded semantic windows. Incorrect or missing chapter boundaries therefore cannot prevent the document from being processed.

### Compact relevant outline context during generation

A flashcard-generation request receives:

- a bounded global document summary;
- a bounded subset of global objectives and concepts;
- only the outline sections whose page ranges overlap the current semantic source window.

The full global outline is not copied into every provider request. Final card curation likewise receives a compact global learning summary rather than the full section hierarchy.

### Distinguish truncation from contract errors

The native SSE gateway preserves `choices[0].finish_reason` when reconstructing a streamed Chat Completions response.

Document intelligence treats at least these cases separately:

- `length` → `ai_document_output_truncated`;
- `content_filter` → `ai_document_output_filtered`;
- `insufficient_system_resource` → `ai_provider_resource_exhausted`.

A genuine outer-object decode/contract failure still uses `ai_document_output_invalid`, but its retry failure includes a bounded final validation detail.

Outline errors additionally identify the exact source-window number, total window count, PDF page range, and estimated source-token count. Safe diagnostic logging records the same bounded metadata without raw source text, raw provider response bodies, credentials, or Authorization headers.

### Normalize recoverable outline semantics locally

Provider JSON Schema owns structure and types. Rust owns semantic cleanup and safety bounds.

A valid structured outline is no longer discarded merely because the model supplies a reversed page range or a range slightly outside the current source window. Native normalization clamps/repairs those values and bounds text/list sizes before persistence and downstream use.

### Bounded parallel page rasterization

The justpdf renderer remains the pure-Rust rasterization backend, but CPU-heavy page rendering now uses a bounded worker pool:

- worker count = `min(available_parallelism, page_count, 16)`;
- workers share the immutable parsed `Document` and render different physical pages concurrently;
- page indexes are allocated atomically;
- rendered PNG results are returned through a channel;
- the coordinator reorders results by physical page number before invoking the existing persistence visitor;
- first render or persistence failure stops assignment of additional work.

This improves multicore utilization without making SQLite writes or managed-file promotion concurrent. Persistence therefore remains deterministic and retains the existing integrity/error boundaries.

The renderer backend identity is versioned as:

```text
justpdf-render-0.1.2/pure-rust-parallel-v1
```

A document generated with the previous renderer identity is treated as having stale derivatives on retry, so its page renders are rebuilt with the parallel backend.

## Consequences

- The 138-page and 11-page documents use the same bounded architecture; large files simply produce more requests.
- Local llama-server runtime context configuration matters less because ostudy no longer attempts 20k–32k source windows.
- Pathological parser chunks cannot silently bypass the provider-window cap.
- Chapter/section information improves organization without becoming a brittle hard dependency.
- There is no model-generated outline-of-outlines merge to overflow or drift.
- Later generation requests do not grow linearly with total document outline size.
- Diagnostics can distinguish truncation, provider resource failure, provider/schema decode failure, and the specific source window involved.
- Rendering uses multiple CPU cores while keeping persistence ordered and transactional boundaries unchanged.
- Provider calls remain sequential for one document in this stage; parallelizing LLM requests is deliberately separate because local inference servers commonly share one constrained accelerator/context resource and cloud providers impose rate limits.

## Validation

Automated coverage verifies hard semantic-window bounds (including oversized continuation chunks), deterministic outline-fragment normalization/assembly, relevant-section filtering, streamed finish-reason retention, separate truncation errors, provider-enforced schema paths, parallel renderer worker bounds, a real PDF extraction/render smoke, the full Rust/frontend suite, and native Tauri smoke tests.

Real-provider acceptance must still retest the representative large PDF with DeepSeek and the configured llama.cpp / Unsloth server. Performance acceptance should compare large-document render elapsed time and CPU utilization with the previous serial build; exact speedup is workload and hardware dependent.
