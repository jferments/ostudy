# ADR 0024: Generalized document platform, provider capabilities, and calibrated AI planning

- **Status:** Accepted for PR #39 stabilization
- **Date:** 2026-08-09
- **Scope:** Stage 4 document-platform stabilization before additional providers or source formats

## Context

PR #38 established managed PDF ingestion, structured extraction, bounded document AI, canonical card authoring, cancellation, backup/restore, citations, and second-pass generation. PR #39 added large-document partitions, resumable validated checkpoints, and metered-provider whole-job budget guards after a 138-page DeepSeek run consumed roughly 1.45 million aggregate tokens before a late truncated completion.

Real acceptance testing exposed a cross-layer defect: partitioned semantic windows were locally numbered inside one partition, so an 11-page llama.cpp source could produce multiple windows with the same `window_index`. SQLite correctly rejected the duplicate `(job_id, window_index)` primary key even though component-level tests and a clean CI database passed. After that defect was fixed, a second acceptance finding showed that applying a 1.2M aggregate-token ceiling to local llama.cpp was conceptually wrong: a 138-page source was already decomposed into safe bounded requests, but the planner rejected the job only because the sum of those requests was large.

Stage 4 must be stabilized before adding OpenAI, Claude, Gemini, Z.ai, DOCX, PPTX, or Google export acquisition.

## Decision

### Documents are a domain; PDF is an ingestion adapter

The document AI pipeline consumes an ostudy-owned `DocumentSourceDescriptor`, semantic chunks/windows, and source locators. Current PDF acquisition maps physical pages to `SourceUnitKind::Page`. Future adapters can map PPTX slides to `Slide` and DOCX structural blocks to `Block` without changing AI planning or canonical authoring.

PDF parsing/rendering lives in `document_ingestion.rs` and the existing PDF backend/render boundaries. The active PDF adapter emits an `IngestedDocument` containing:

- a format-neutral source descriptor;
- normalized text/semantic structure;
- optional rendered source-unit derivatives;
- extraction/render backend identities.

The durable `documents` row now records `source_format` and `source_unit_kind` explicitly. Current public commands remain PDF-specific because no additional source adapter is included in this stage.

### Source locators are the generic provenance boundary

`SemanticWindow` retains current PDF page projections for compatibility with the existing extraction/provider tests, but format-neutral consumers use `SemanticWindow::source_locator()` for:

- validation;
- checkpoint hashes;
- durable generation-window persistence;
- safe diagnostics;
- planner/job identity.

Before any generation job is inserted, the planner validates that:

- windows are non-empty;
- `window_index` values are unique, contiguous, and globally ordered;
- chunk ranges are valid;
- source content/token estimates are non-empty;
- locators use the document's source-unit kind;
- locators stay within the source-unit count.

The 11-page single-partition/two-window regression is protected at both semantic and SQLite integration boundaries.

### Document orchestration is decomposed into services

The Stage 4 native layer is split into:

- `document_domain.rs` — format/source-unit/locator contracts;
- `document_ingestion.rs` — current PDF ingestion adapter and derivative persistence;
- `document_semantics.rs` — semantic partitions/windows and invariant validation;
- `document_scale.rs` — context/budget-aware planning;
- `document_cost.rs` — empirical cost calibration model and benchmark contract;
- `ai/document_provider.rs` — provider-neutral document capability snapshot;
- `document_checkpoints.rs` — rebuildable validated checkpoint cache;
- `document_store.rs` — generation state/job/window persistence and affected-row invariants;
- `document_generation.rs` — shared bounded generation-window executor;
- `documents.rs` — lifecycle plus high-level initial-generation composition;
- `document_more.rs` — second-pass context plus the same shared generation executor.

Initial generation and Generate More no longer maintain separate window execution/retry/image/progress implementations.

### Provider capabilities constrain planning

Document planning consumes normalized capability facts rather than provider wire formats:

- model identity;
- reported context window when available;
- image-input support;
- usage-reporting support;
- structured-output strategy.

The current transport implementation remains limited to the already-supported llama.cpp and DeepSeek adapters. This ADR does not add another provider.

A provider-reported context size is a hard upper bound. It may shrink ostudy's conservative source windows but never increases them automatically. If the reported context cannot fit ostudy's task/schema/output reserve, the job fails locally before provider traffic with `document_ai_context_too_small`.

Provider keys in durable document/profile/job/outline tables are bounded strings rather than repeated closed SQLite enums. Application provider registries still reject unsupported adapters at runtime, but adding a future adapter does not require rebuilding every historical document table just to admit a new string literal.

### Large local jobs are window-bounded, not job-bounded

A whole-document token estimate is useful for diagnostics, but it is not a safety boundary for local llama.cpp/Unsloth execution. Local inference has no ostudy-managed per-token billing exposure, and the document pipeline already provides the correct safety mechanism: every outline and generation call is a bounded semantic window constrained by the model's reported context when available.

Therefore llama.cpp uses `AggregateTokenCeiling::Windowed`:

- no 1.2M aggregate-job rejection;
- no 250k total extracted-source-token rejection;
- the existing 300-source-unit ingestion limit remains a separate document-processing bound;
- large sources simply produce more heading-aware partitions and bounded semantic windows;
- outline windows execute sequentially and are checkpointed;
- generation windows execute sequentially through the shared executor and are checkpointed for initial generation;
- whole-document outline synthesis consumes only a compact bounded digest;
- final curation already operates in bounded batches.

This means a 138-page local source with an estimated aggregate cost above the former 1.2M ceiling is expected to continue through its bounded windows rather than asking the user to split the file manually.

### Metered providers retain aggregate cost circuit breakers

Remote/metered providers retain aggregate safeguards because a long request sequence can create real external cost even when every individual request is context-safe. Current policy keeps:

- approximately 250k estimated extracted source tokens per automatic metered job;
- approximately 650k conservative aggregate tokens for DeepSeek/current default cloud adapters.

If a metered provider exceeds those bounds, ostudy may fail before provider traffic with a safe split/use-local message. This policy is independent from per-request context enforcement.

### Empirical usage complements declared limits

A committed benchmark corpus records only real acceptance observations and a required scenario matrix. Diagnostic-log analysis reduces real runs to safe aggregates such as:

- source-unit count;
- planner aggregate estimate;
- aggregate budget mode (`windowed` for local or a numeric metered ceiling);
- provider/model;
- prompt/completion/reasoning tokens;
- cache hit/miss tokens;
- image-input class.

No raw prompts, document text, provider response bodies, credentials, or hidden reasoning are part of the benchmark.

Empirical calibration is deliberately conservative:

1. provider-declared hard context remains authoritative;
2. the local character/token heuristic remains a fallback;
3. curated empirical observations may tighten prompt/image overhead coefficients;
4. a few noisy runs never automatically widen source windows or context assumptions.

This lets Stage 4 build concrete cost curves for small/medium/large text and multimodal sources without teaching the application unsafe limits from sparse data.

### Checkpoints are acceleration data, not canonical learner data

Validated outline/generation checkpoints are rebuildable caches. Their semantic contract identity includes:

- provider adapter key;
- provider profile identity;
- model identity;
- extraction backend;
- provider capability fingerprint;
- window/partition policy fingerprint;
- prompt/schema/task contract;
- source-window hash;
- final-outline hash for generation checkpoints.

Credential rotation and unrelated profile revision numbers are intentionally excluded because they do not change study semantics.

Checkpoint reads/writes/cleanup are best-effort. Cache failure is logged and degrades to recomputation; it does not invalidate an otherwise valid provider result. Canonical source, final outline, deck/card authoring, and provenance writes remain strict/fatal.

### SQLite lifecycle transitions are explicit state-machine boundaries

Generation state writes now verify expected affected-row counts. A zero-row update caused by cancellation, deletion, stale lifecycle state, or an unexpected race becomes a domain error instead of silently succeeding.

Database diagnostics use operation-specific event names and safe identifiers (document/job/window/partition/source locator) while keeping the user-facing database error generic. This makes failures such as duplicate generation-window identities diagnosable without exposing source text or credentials.

### CI is read-only by default

Ordinary PR qualification no longer auto-commits rustfmt changes to the branch. Draft PRs use the fast gate; ready PRs use the full gate. Generated rustfmt/lockfile application remains an explicit manual workflow action.

The fast gate includes focused document contract tests that compile the complete Rust lib test target and execute high-value document/migration invariants. `check-changed.sh` routes document-platform-only diffs to the narrowest useful document/Rust path rather than always paying for the complete native smoke suite.

This reduces branch races/noise while catching planner-to-SQLite defects earlier.

## Consequences

- Adding DOCX/PPTX later requires a new ingestion adapter and source locator mapping, not a forked AI pipeline.
- Adding a provider later requires capability/transport integration, not provider-specific branching throughout document orchestration.
- Existing PDF IPC/UI remains stable during this architecture change.
- The planner becomes safer on local models whose actual context is smaller than a generic default.
- Large local documents scale by number of bounded requests instead of being rejected by an arbitrary aggregate token total.
- Metered providers still have an early runaway-cost circuit breaker.
- Empirical cost data becomes reproducible and reviewable rather than anecdotal.
- Resume caches can become unavailable without destroying completed inference.
- SQLite failures carry enough safe operation context to diagnose cross-layer bugs.
- Draft development iterations become cheaper and less mutation-prone.

## Non-goals

This decision does **not** add OpenAI, Claude, Gemini, Z.ai, DOCX, PPTX, Google Docs, Google Slides, OCR, embeddings, or a generic arbitrary-prompt API. It does not automatically train/update cost coefficients from every user inference call.

## Validation

The architecture is protected by:

- source descriptor/locator unit tests;
- semantic-window identity/bounds tests;
- an 11-page one-partition multi-window SQLite integration regression;
- a 138-page-style local budget regression that must continue when its aggregate estimate exceeds the former 1.2M ceiling;
- a local-source regression above the cloud-only 250k source-token guard;
- metered-provider aggregate-budget rejection tests;
- provider-capability/context planning tests;
- checkpoint semantic-identity tests;
- empirical usage baseline corpus parsing;
- latest-schema and forward-upgrade migration tests;
- canonical fast/full gates;
- required real-provider acceptance on the representative small, medium, and large sources before PR #39 is merged.
