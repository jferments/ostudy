# Document AI architecture, routing, and concurrency

ostudy separates four concerns that were historically mixed together in one large Rust file:

1. canonical document content and provenance;
2. format-neutral document-learning reasoning;
3. large-job orchestration and resumability; and
4. provider-specific transport.

This separation is required for more than source-code tidiness. It lets PDF, slide, word-processing, web, and future document adapters feed one reasoning system; lets flashcards, study guides, practice tests, tutoring, and semantic search reuse the same learning model; and prevents provider wire behavior from redefining ostudy's domain contracts.

The architectural decision and compatibility policy are recorded in [ADR 0041](../decisions/0041-modular-document-ai-architecture.md).

## End-to-end dependency direction

The intended flow is one-way:

```text
format adapter
    |
    v
CanonicalDocument + semantic chunks + source locators + visual evidence
    |
    v
document-generation workflow
    |
    v
ai::document_intelligence facade
    |
    +--> outline reasoning
    +--> flashcard authoring
    +--> curation
    |
    v
provider-neutral contracts and schemas
    |
    v
provider transport
    |
    v
AI gateway / provider endpoint
```

A provider response is never the canonical document representation. Canonical text, structure, source locators, and visual-evidence identities come from the document adapter and semantic planner. AI output is a derived learning artifact that must pass native decoding, validation, normalization, and persistence rules.

Provider transport must not know about `SemanticWindow`, `CanonicalDocument`, PDF parsing, slide parsing, or semantic partition construction. A domain task first selects and bounds the source text and visual evidence, then passes an ordinary structured task request to transport. Conversely, provider request bodies, endpoint paths, response envelopes, tool-call arguments, and token-field aliases must not leak into outline, flashcard, or curation logic.

## Module ownership

The stable application-facing boundary is `src-tauri/src/ai/document_intelligence/mod.rs`. The directory is deliberately divided by responsibility:

- `mod.rs` coordinates bounded work, cache reuse, deterministic source-order persistence, and final assembly.
- `context.rs` snapshots the selected profile, network scope, credential storage mode, negotiated provider capabilities, resolved reasoning effort, and accumulated usage for one job.
- `contracts.rs` owns provider-neutral outlines, source visual evidence, supported generated-card variants, and all native validation and normalization.
- `outline.rs` owns source-grounded outline prompts, local fragment normalization, deterministic hierarchy assembly, optional whole-document synthesis, and compact projections used by later tasks.
- `flashcards.rs` owns card-authoring prompts, bounded repair, card-count/output budgeting, and conversion into locally validated ostudy card values.
- `curation.rs` owns bounded selection-only quality curation and deterministic final de-duplication. It never rewrites cards.
- `schemas.rs` owns the authoritative provider-neutral JSON schemas for outline, flashcard, global synthesis, and curation tasks.
- `transport.rs` owns credential resolution, endpoint construction, provider request bodies, schema lowering, compatibility retries, response decoding, policy/resource stops, and token-envelope parsing.
- `support.rs` owns small shared retry/error and bounded-diagnostic helpers.

These are ownership boundaries, not merely file names. A new learning-artifact type should normally add domain reasoning beside `outline.rs`, `flashcards.rs`, and `curation.rs`, then reuse contracts, context, and transport. A new provider should normally extend transport or the shared provider gateway without duplicating document reasoning.

## Canonical source units and compatibility projections

PR #63 made canonical source locators authoritative. Document reasoning therefore speaks in terms of **source units**, **source windows**, **semantic partitions**, and **visual evidence** rather than assuming every input is a PDF page.

Some persisted and provider-schema fields still use PDF-era names such as `pageStart`, `pageEnd`, `page_count`, and `SourcePageImage`. They are compatibility projections: their numeric values represent one-based canonical source-unit bounds, while the canonical `SourceLocator` remains the richer provenance identity. Renaming these fields would require a separately versioned persistence and checkpoint change, so this architecture refactor documents rather than disguises them.

Prompt wording changed from PDF-specific to canonical source-unit language. Outline, flashcard, and curation prompt versions therefore advance. Persisted outline/card JSON shapes do not change. Rebuildable checkpoint identities include the relevant prompt/schema versions, so old acceleration rows stop matching and are recomputed without a database migration or loss of learner data.

## Semantic extraction and provider request boundaries

Semantic extraction preserves headings, source provenance, element types, visual-evidence references, and section relationships. Provider requests are then packed by bounded source-token and source-unit capacity rather than split every time a heading changes. A highly structured textbook or lecture deck must not degenerate into one provider request per heading, page, or slide.

The semantic planner determines what belongs together. The document-intelligence layer determines what learning task to perform. Transport determines only how that already-bounded request is represented on the wire.

This distinction is important for future embeddings. A small local embedding model may create versioned vectors over canonical chunks to improve retrieval, topic maps, coverage analysis, and window selection. Those vectors are derived indexes keyed by source hashes. They do not replace canonical content, source locators, deterministic outline assembly, or native validation, and they do not belong in provider transport.

## Outline reasoning

The outline is a reusable document-learning model rather than a flashcard-only intermediate.

Each canonical source window produces a small, validated outline fragment. Independent fragment requests may finish out of order, but validated fragments are persisted and assembled in canonical window order. Rust remains authoritative for hierarchy, section bounds, de-duplication, and provenance.

After deterministic assembly, an optional bounded whole-document synthesis may improve the concise global summary, major objectives, and key concepts. It cannot recreate or reorder section hierarchy. If synthesis fails, the deterministic assembled outline remains valid and the job continues.

For multi-window documents, a provider fragment that remains malformed after bounded repair may use a deterministic source-derived local fallback. The fallback is deliberately narrow and retains source provenance. Single-window jobs fail explicitly rather than concealing a complete provider failure behind a weak fallback.

## Flashcard authoring and curation

Flashcard generation receives only the current semantic window plus a compact outline projection relevant to that window. This avoids repeatedly sending the complete document while still letting the model judge local material against global learning objectives.

Provider output is decoded into a small provider-neutral candidate set: basic, genuine reversible definition, multiple choice, critical thinking, and cloze. Native Rust then validates field sizes, option cardinality, duplicate options, cloze substrings, tag rules, supported modes, and card-count limits. Definition candidates alone become reversible basic cards. Cloze markers are constructed locally from exact source-grounded substrings.

Malformed candidates do not automatically discard valid siblings from the same response. When every candidate is invalid, the task receives one bounded repair attempt with the validation detail. A response that reaches a completion limit is classified separately from an ordinary schema mismatch.

Curation is selection-only. It receives bounded compact card summaries and global outline context, then returns indices to retain. A failed curation batch keeps its already validated candidates rather than silently deleting them. Final native de-duplication is deterministic.

## Multimodal evidence

Visual evidence is selected by the canonical document workflow and passed to a domain task with its source identity. Transport only encodes bounded image bytes in the provider's supported request format.

This permits diagrams, tables, equations, slide layout, and image-only source units to inform outlines and learning artifacts without making those tasks PDF-specific. Text remains the primary textual evidence when extraction is available. An image-only document requires a verified image-capable profile; otherwise the workflow fails with a stable actionable error rather than pretending that an empty text extraction is sufficient.

The current shipping adapter still renders PDF pages, which explains remaining compatibility names. Future adapters can render slides, document pages, or other source units into the same visual-evidence contract.

## Bounded concurrency and deterministic persistence

For large documents, outline, generation, and curation batches are independent units of provider work. Cloud-backed windows may execute with bounded concurrency while checkpoint and database writes remain serialized in source-window order. This preserves deterministic durable state while allowing network and inference latency to overlap.

Concurrency follows execution topology rather than provider brand: cloud profiles may run up to four independent document windows concurrently, while local and LAN profiles remain serial unless a future explicit server-capacity capability says otherwise. Independent windows deliberately do not share a sticky provider session because they contain different source material; concentrating concurrent work on one endpoint can reduce throughput without meaningful prompt-cache reuse.

No document task may introduce unbounded fan-out. A new artifact generator must reuse the shared document concurrency policy and must preserve source-order persistence and result assembly.

## Checkpoints and resumability

Outline fragments and generated-window cards are rebuildable acceleration data. A cache read, decode, or write failure is logged and degrades to recomputation; it must not discard an otherwise valid provider result.

Checkpoint identity includes inputs that change semantics:

- provider kind, profile, and model;
- extraction backend;
- normalized provider-capability fingerprint;
- resolved reasoning effort;
- semantic planning fingerprint;
- relevant prompt/schema versions;
- source-window hash and canonical locator;
- card-count budget; and
- final outline identity for generation windows.

Credential rotation and unrelated profile revision counters do not invalidate reusable learning results. Canonical learner data—documents, decks, cards, schedules, and review history—does not depend on checkpoint survival.

## Provider routing

OpenRouter routing treats compatibility, privacy, and model precision as hard eligibility constraints. Document requests require parameter-compatible endpoints, deny provider data collection by default, honor the profile's optional zero-data-retention requirement, and restrict eligible endpoint quantization to INT8, FP8/MXFP8, FP16, BF16, or FP32 execution.

Long document processing is throughput-sensitive rather than price-sensitive. A single job may issue many outline, generation, repair, and curation completions; repeatedly selecting a slow endpoint can multiply a modest per-request delay into tens of minutes. OpenRouter document traffic is sorted by provider throughput while preserving fallbacks across every otherwise eligible endpoint. Existing p90 throughput and latency preferences remain secondary signals. Connection probes use the same route policy so a successful profile test exercises the same provider class used by document work.

DeepSeek document requests use the provider's strict function/tool contract after explicit schema lowering. OpenRouter uses its strict JSON-schema contract after explicit portable lowering. llama.cpp uses its OpenAI-compatible chat endpoint with image parts when enabled and a bounded compatibility retry for servers that expose the alternate `json_schema` request shape. These wire differences live only in transport.

## Timeouts, retries, and structured-output exhaustion

Provider resilience is based on transport inactivity rather than a blunt whole-request wall clock. The profile timeout bounds silence while connecting and between received stream events. A provider heartbeat, reasoning delta, output delta, tool-argument delta, or final usage event proves the connection is responsive and resets that timer. A silent transport failure is retried once with the same logical request. Completed windows remain checkpointed for Retry.

Structured-output exhaustion is separate from transport failure. The selected model's declared reasoning capabilities are normalized before planning, and the task's requested reasoning effort is resolved once for the job. Optional reasoning is never silently promoted from a requested lower effort to a more expensive unsupported effort. If the requested effort is unavailable and reasoning is not mandatory, reasoning is disabled for that task. Mandatory-reasoning models use their lowest declared supported effort. The resolved effort is part of checkpoint identity.

Completion budgets reserve visible structured output separately from hidden reasoning. For reasoning models, the wire-level ceiling includes reasoning headroom while the domain schema retains its provider-neutral visible-output budget. Large-document generation keeps the bounded 16-candidate window capacity. If a generation result is genuinely truncated, ostudy may retry with a smaller card count without reducing the original completion-token floor. A `finish_reason=length` marker alone is not failure: a complete schema-valid result is accepted; only an incomplete result is truncation.

## Usage and diagnostics

Operational token counts and request timing are safe numeric metadata. Transport normalizes OpenAI-style usage envelopes, nested response envelopes, and llama.cpp timing/token fields into one job accumulator. Usage reported before a later document failure is retained with the failure so the UI does not replace a real count with “not reported.”

Routine diagnostics never contain source text, prompts, provider completions, credentials, or inline images. They may contain provider/model identifiers, source-window indices, canonical locator identities, token counts, duration, retry counts, and stable error codes. Any new provider envelope must extend transport parsing and tests rather than leaking provider-specific fields into domain logs.

## Enforcement

`./scripts/check-static.sh` runs `verify-document-ai-boundaries.mjs`. The verifier and its fixture tests enforce:

- the required module set and normal Rust module topology;
- removal of the old monolith and path-attributed aliases;
- a bounded per-module size ceiling;
- provider-wire vocabulary confined to transport;
- no semantic-window or canonical-document dependency in transport; and
- the expected transport ownership points.

The size ceiling is an architectural tripwire, not a target. Approaching it should trigger another responsibility review before the file becomes a new monolith.

These rules apply to initial flashcard generation and future document-aware tasks. New features should reuse canonical chunks, source locators, visual evidence, capacity-bounded windows, deterministic native assembly, normalized provider capabilities, checkpoint identity, and shared routing/concurrency rather than introducing feature-specific page loops or provider pipelines.
