# ADR 0023: Budgeted, resumable large-document generation

- **Status:** Accepted
- **Date:** 2026-08-09
- **Scope:** Stage 4.7 / PR #39

## Context

Real-provider acceptance after PR #38 established three useful reference points:

- an 11-page PDF completed with DeepSeek in about 45,000 aggregate tokens;
- a 48-page PDF completed with Qwen 3.6 27B through llama.cpp / Unsloth;
- a 138-page PDF reached roughly 1.45 million aggregate DeepSeek tokens before a late structured completion hit its output limit.

The failure was not evidence that one request exceeded DeepSeek's advertised context window. A document job is a sequence of outline, generation, repair, global-synthesis, and curation calls. Repeating the fixed system prompt and strict schema many times can make the aggregate job expensive even when each request is individually small. A late failure is especially wasteful if validated work from earlier windows is discarded.

The product also needs large sources to be useful: a 100–150 page chapter or packet can reasonably justify hundreds of flashcards. A document-wide 120-card limit or an arbitrary 50-page upload limit would solve the wrong problem.

## Decision

### Semantic partitions are internal processing boundaries, not new Documents

ostudy keeps one immutable managed PDF and one reusable document-intelligence model. oxidize-pdf structure, headings, page provenance, and semantic chunks are grouped locally into heading-aware partitions.

For large DeepSeek jobs, partitions target roughly 48 pages and are hard-bounded at 60 pages. This is intentionally close to processing a 138-page source as about three chapter-scale regions, but it does **not** create three separate managed PDFs or lose global document identity.

Each partition is still divided into bounded provider windows. The partition supplies semantic/chapter context and a checkpoint namespace; it is not sent to a provider as one unbounded prompt.

### Provider-aware request policy

Documents of 50 pages or fewer retain the conservative PR #38 request policy.

For larger DeepSeek documents:

- partition target: about 32,000 source tokens / 48 pages;
- partition hard maximum: about 48,000 source tokens / 60 pages;
- outline window: up to about 10,000 source tokens / 20 pages;
- generation window: up to about 6,000 source tokens / 8 pages;
- at most 16 generated cards in one generation response;
- no page images are sent to DeepSeek.

For larger llama.cpp / Unsloth documents, ostudy retains conservative local-model windows because usable context/KV-cache capacity depends on the server, model, and accelerator:

- outline window: about 3,200 source tokens / 8 pages;
- generation window: about 5,000 source tokens / 10 pages;
- at most 16 cards per generation response;
- up to three selected visual-evidence pages when the configured model has current image-input capability evidence.

### No small document-wide card cap

The 120-card document ceiling is removed. Card capacity scales with the number of generation windows:

`generation window count × maximum cards per window`

subject to the existing absolute defensive bound of 10,000 generated candidates.

For example, a 138-page DeepSeek source that produces approximately 18 generation windows has a first-pass capacity of roughly 288 cards. A learner can subsequently run **Generate more cards**, which reuses the retained document intelligence and tests uncovered material against the existing deck.

The per-window card cap is a structured-output reliability bound, not a learning-content limit.

### Persist validated intermediate work

The durable document remains the source of truth for expensive preprocessing:

- immutable source PDF;
- extracted per-page text;
- rendered page derivatives;
- oxidize-pdf semantic elements;
- semantic chunks and heading/page relationships;
- the final reusable global `document_outlines` row.

Stage 4.7 additionally stores validated provider checkpoints:

- `document_outline_checkpoints`: one validated outline fragment per source window;
- `document_generation_checkpoints`: validated card candidates per first-pass generation window while the canonical deck has not yet committed.

Checkpoint contract keys include the provider/profile/model revision, credential epoch, extraction backend, scale/window policy, source-window hash, and relevant document contract. A changed source/provider/model/policy therefore does not accidentally reuse stale output.

An interrupted retry reuses matching completed outline and generation windows. After the final deck commits successfully, first-pass generation checkpoints may be removed because the canonical cards are now durable; semantic chunks and the final global outline remain attached to the document. **Generate more cards** reuses those retained semantic chunks and global outline, so it does not repeat PDF extraction/rendering or the initial outline pass.

Raw provider responses, credentials, prompts, and hidden reasoning are never checkpointed.

### One compact recovery attempt, never recursive retries

A source-window request is intentionally bounded. ostudy must not keep paying to repeat a request until it happens to fit.

For card generation:

1. make the normal bounded structured request;
2. if it is specifically truncated/oversized, make **one** reduced request with approximately half the card allocation;
3. return that result or fail the window.

There is no recursive halving loop.

Separately, the structured-output adapter may make one contract-repair request after an invalid/empty result. That is also bounded to one attempt. A truncation is not itself routed through the generic contract-repair path.

For outline fragments, a recoverable truncated/invalid fragment in a multi-window document may use the local heading/page/source excerpt as a conservative native fallback instead of repeatedly asking the provider. The full native hierarchy and optional bounded global synthesis still provide document-wide context.

Final LLM curation is quality improvement rather than an atomicity boundary. If a curation batch fails, already validated cards are retained and native exact-duplicate filtering still applies.

### Whole-job preflight budget

Before the first outline provider call, ostudy calculates a conservative aggregate-token estimate covering:

- source windows and overlap;
- repeated system/schema overhead;
- structured completion allowances;
- a bounded compact-repair reserve;
- global synthesis;
- final curation.

Initial automatic ceilings:

- DeepSeek/cloud: approximately 650,000 estimated aggregate tokens;
- local llama.cpp: approximately 1.2 million estimated aggregate tokens;
- maximum automatic source: 300 pages and 250,000 estimated extracted source tokens.

These estimates are safety heuristics, not provider billing predictions. If the estimate exceeds the ceiling, ostudy stops **before** the expensive provider sequence with `document_ai_budget_exceeded` and asks the learner to split the source at a chapter/section boundary.

The safe diagnostic event `document_ai_scale_plan` records provider/model, page/source size, window counts, first-pass card capacity, estimated aggregate tokens, and the selected ceiling without recording source text or credentials.

### Reduced rendered-page derivative

The AI raster derivative is reduced from 144 DPI to 96 DPI and receives a new renderer-backend identity so stale derivatives are rebuilt. This reduces raster work, managed image size, and multimodal image context for compatible local llama.cpp models while leaving the original PDF unchanged.

DeepSeek receives extracted text rather than rendered page images, so the DPI change does not reduce DeepSeek text-token usage.

## Consequences

- Large documents can produce hundreds of cards without one enormous response.
- A 138-page source can be represented internally as several chapter-scale semantic partitions while retaining one global document hierarchy.
- A late interruption or provider error no longer requires repeating every validated source window.
- Generate-more reuses durable preprocessing/outline work.
- Automatic retries are explicitly bounded; ostudy prefers a visible failure or local native fallback to unbounded token spending.
- Very large/expensive jobs may be rejected before provider traffic and require chapter-level source splitting.
- Provider token usage remains an empirical acceptance metric; the conservative preflight estimate should be calibrated against real DeepSeek usage after representative runs.
