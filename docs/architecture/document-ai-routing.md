# Document AI architecture, routing, and concurrency

ostudy separates canonical evidence, shared source understanding, artifact authoring/orchestration, and provider-specific transport. This guide describes the knowledge-first implementation after merged PR #106. [ADR 0041](../decisions/0041-modular-document-ai-architecture.md) preserves the earlier modularization rationale; its original outline-first stages are not the current pipeline.

Implemented document adapters, cards, guides, Tutor and semantic retrieval share native-owned source knowledge. Future lessons or practice-test features must reuse those boundaries, but their presence in a roadmap is not implementation evidence. See [processing architecture](ai-and-document-processing.md), [knowledge representation](knowledge-representation.md), and [the hardening register](../development/post-106-hardening.md) for current limitations and separately planned changes.

## End-to-end dependency direction

```text
format adapter / retained images
    -> canonical source evidence, locators and bounded chunks
    -> shared knowledge preparation and exact evidence admission
    -> retained units, analysis receipts and source dispositions
    -> native document view / learning plan / collection synthesis
    -> bounded card or guide authoring, or scoped Tutor retrieval
    -> native validation and revision-checked publication

provider work at the relevant stages
    -> bounded provider-neutral task request
    -> provider dialect + shared native gateway
    -> untrusted result returned to the owning task validator
```

A provider response is never the canonical document representation. Text, structure, source locators and visual-evidence identities originate in native source preparation. Extracted knowledge is an admitted interpretation with provenance, not a guarantee of exhaustive or correct understanding. Artifact output must pass native decoding, validation, normalization and persistence rules.

Domain owners choose relevant evidence and budget each request. Provider adapters encode bounded text, schema and attached images; gateway transport does not choose source windows, learning topics, library writes or retrieval permissions. Provider endpoint paths, response envelopes, strict-tool conventions and token aliases stay inside their wire/transport owners.

## Module ownership

| Owner under `src-tauri/src/` | Responsibility |
| --- | --- |
| `document_outline_service.rs` | Coordinates shared preparation, prepared-source snapshots, derived document view, and atomic outline/display-metadata publication; later enrichment/indexing stays separate |
| `knowledge/` | Source projections, admitted knowledge/evidence, partial-analysis receipts, snapshots, authoring catalogs, collection views, scoped retrieval and derived indexing |
| `document_generation.rs` and related document services | Native artifact jobs, bounded work, checkpoints, progress, cancellation and publication through existing authoring owners |
| `study_materials/` | Document/collection StudyTarget orchestration, source-note reuse and collection guide publication |
| `ai/document_intelligence/mod.rs` | Provider-neutral artifact-authoring facade, exports and optional metadata entry point; not the whole document scheduler or a second source-analysis pipeline |
| `ai/document_intelligence/context.rs` | Freezes task/profile/model/capability/credential/reasoning context and validates freshness |
| `ai/document_intelligence/flashcards.rs` | Recall/typed-question candidate authoring, content/evidence admission and task bounds |
| `ai/document_intelligence/study_guide.rs` | Bounded document/collection semantic-section authoring contracts |
| `ai/document_intelligence/curation.rs` | Selection-only curation of already validated candidates |
| `ai/document_intelligence/transport.rs` | Domain-facing structured-request coordination over native provider/gateway boundaries |
| `ai/providers/`, `ai/gateway/` | Provider wire dialects, endpoint/HTTP policy, streaming, safe transport retries, usage normalization and redaction |
| `ai/profiles/` | Provider policy, credentials/metadata coordination and authoritative task model/reasoning assignments |

Historical `outline.rs`, outline DTOs and checkpoint names remain where consumed. Their existence does not prove a second raw-source outline traversal. Add a new provider through its dialect/gateway contract; add an artifact through bounded native task and publication owners rather than duplicating parsing, knowledge or transport.

## Canonical source units and compatibility projections

Reasoning uses source units, windows, semantic partitions and visual evidence rather than assuming every source is a PDF page. Current non-PDF input uses Xberg; PDF and retained image sets have their own native adapters.

Some persisted/provider fields retain names such as `pageStart`, `pageEnd`, `page_count` and `SourcePageImage`. They project one-based canonical source-unit bounds; the canonical `SourceLocator` carries the richer provenance. Rename serialized fields only with coordinated reader/checkpoint compatibility, not cosmetic substitution.

Prompt/schema/backend/assembly versions identify producer contracts. An old migration or parser name can remain legitimate evidence even when its runtime adapter is retired. A version change does not authorize deleting learner edits, schedules, reviews, or supported historical input.

## Shared extraction and request boundaries

Source preparation preserves headings, relationships, qualifiers, exact native spans, locators and eligible images. Bounded interpretation requests select native evidence IDs rather than rewriting quotations as provenance. Valid siblings and per-source dispositions survive a targeted repair; partial analysis and explicit no-learning results remain distinguishable.

Request packing accounts for evidence, instructions/schema, output and reasoning reserves, optional images and safety margin. Headings are metadata, not a mandate for one request per heading/page. Aggregate material larger than one model context is handled through bounded work, not automatically rejected as one oversized request.

Saved compatible knowledge is reusable by cards, guides and Tutor. Changing a consuming model does not automatically re-extract it. Embeddings already exist as a separately configured derived retrieval index; changing embedding space rebuilds that index, not authoritative source interpretation. Indexing failure must not erase successfully retained knowledge. Ordinary navigation or opening a source selector does not authorize provider transmission.

## Derived outline and learning context

`ensure_document_outline` first prepares shared knowledge, obtains a prepared DocumentSnapshot and bounded catalogs, then requests the document view from `knowledge/document_view`. Native assembly owns topic grouping, ranges, stable identities and coverage. Optional balanced knowledge-only reduction may improve a global summary or advisory core selection; invalid or incomplete reduction leaves the native aggregate authoritative.

The snapshot includes expected source projections and receipts even when some sources produced no units. It is checked around provider work and inside the publication transaction. A changed or removed source, changed interpretation, or new source cannot silently enter a stale outline publication. Deterministic outline/display metadata and ready-state changes publish together. Optional provider metadata and derived indexing are separate postprocessing, not another required raw-source traversal.

A collection's LearningContext describes its exact authorized document/range snapshot. CollectionSynthesis is a derived cross-source map; it neither clones source interpretations nor substitutes relevance-ranked Tutor retrieval for complete generation coverage. Large collection catalogs are grouped into bounded authoring work while retaining their source-owned knowledge.

## Flashcard and guide authoring

Authoring receives bounded admitted knowledge, deduplicated evidence, compact learning context and permitted visual inputs rather than a repeated copy of the whole raw document. Card support selects native knowledge identities; guide sections identify the units they explain. Native publication verifies source/unit freshness and attaches support in the same transaction as the created artifacts.

Recall candidates use Basic, genuine reversible-definition and cloze forms. Interactive candidates use typed `single_choice`, `select_all`, `true_false`, `matching`, `grouping` and `ordering` questions. Automatic image occlusion is not generated. Legacy text-only `multiple_choice` output is rejected rather than flattened into a Basic card. Native validation checks field sizes, choices/items, cloze substrings, tags, evidence bindings, policy and output bounds.

Malformed candidates do not automatically discard independently valid siblings. Repairs are bounded and classified by the owning task; provider truncation, unsupported policy, invalid candidate content, transport failure and ambiguous publication are not interchangeable reasons to replay a whole operation. A complete valid result is not rejected merely because the provider reports a length finish marker.

Curation remains selection-only and cannot rewrite the library. A failed optional curation batch does not silently delete its already validated candidates. Final duplicate handling and import use native authoring rules, including stable presentation identities and local schedules. No generated-card approval queue is introduced.

A collection deck copies eligible unedited, range-authorized document-generated notes through StudyTarget orchestration. CollectionSynthesis does not author replacement flashcards through a parallel quality shortcut. Collection guides author semantic sections from retained authorized knowledge using the shared guide contract. Source membership changes do not erase existing learner cards/reviews.

## Multimodal evidence

Domain preparation chooses visual evidence by native identity and verified model capability. Only image references whose bytes were actually attached are selectable as image evidence for that request. Merely listing an image ID does not establish visual grounding.

PDF pages, retained images and extracted non-PDF assets can contribute evidence according to their actual adapters. Do not claim rendered slides or a platform text layer that the adapter did not produce. Image-only material needs suitable verified visual capability or an explicit actionable failure; empty text is not fabricated evidence.

## Bounded concurrency and deterministic publication

Independent cloud-backed work can overlap within the native queue/resource policy. Local/LAN inference remains serial under current policy. Bounds come from the owning queue and current resource/task contracts, not a permanently copied number in prose or a provider-brand assumption.

Completion order must not define artifact ordering, checkpoint identity or SQL publication semantics. No new artifact generator may introduce unbounded fan-out or hold the database lock across provider work. Cancellation belongs to the exact task/subscription; losing one consumer must not discard another consumer's useful shared result.

Request leases and activity admission have separate safety responsibilities. The collection ActivityGuard/close-warning integration gap is recorded in H-02; this documentation does not claim that a private cancellation registry already supplies that missing integration.

## Checkpoints and resumability

Distinguish reusable knowledge from artifact-authoring checkpoints. Compatible knowledge is source/evidence-bound and consumer-model independent. Artifact checkpoints also carry relevant authoring prompt/schema, model/capability, policy, plan and support-catalog identities. A consumer-model change can require new artifact work without requiring new source interpretation.

Guide checkpoints include content and exact support/catalog fingerprints. Old content-only or stale-source checkpoints cannot be relabelled as current. Per-unit/window/section receipts preserve accepted work where the owning operation permits reuse. A missing optional cache can degrade to recomputation; a corrupt authoritative receipt, stale source or uncertain committed operation requires its explicit reconciliation path instead of blind replay.

Credential changes do not by themselves rewrite learner content, but a live request must still satisfy its frozen credential/profile trust boundary. Documents, cards, schedules, reviews and learner edits are not disposable acceleration data. New immutable managed paths prevent a failed rebuild from destroying the last committed evidence.

## Provider routing and reasoning

OpenRouter requests express explicit endpoint eligibility, privacy, structured-parameter and throughput preferences through the native provider adapter. Optional zero-data-retention settings can reduce eligibility; request flags are not proof of a provider's external retention behavior. Fallback among explicitly eligible endpoints for the selected provider/model is distinct from silently switching a local task to cloud or choosing another model, which is not permitted.

DeepSeek strict tools, OpenRouter strict JSON schema and llama.cpp-compatible structured-request variants use their native dialects. Compatibility lowering and bounded dialect negotiation remain wire concerns, not source-selection logic.

Task assignments own the active model and requested reasoning; profile-level legacy last-selected fields are not a second authority for the task-request path. Effective reasoning follows declared/probed capability and the native resolution policy, with requested/effective values retained as provenance. Do not silently increase optional reasoning to obtain a valid response. The OpenRouter document probe exercises that same effective reasoning configuration rather than disabling optional reasoning merely to make a small probe pass. Visible structured-output reserves and hidden reasoning headroom are budgeted separately; where the model declares an explicit reasoning-token cap, the adapter enforces it so the structured result retains its reserved visible budget.

## Timeouts, retries and usage

Transport resilience uses inactivity rather than treating every slow but responsive stream as failed. Relevant received stream events reset the inactivity timer; this is not an unlimited execution guarantee or permission to ignore cancellation. Task-level content repair, output compaction, provider-dialect compatibility and transient transport retries remain distinct budgets. In shared knowledge extraction, a provider `length` finish is output compaction, not a response-contract repair: the one bounded retry receives additional visible structured-output headroom instead of replaying the same ceiling. A retry must not regenerate already durable successful siblings or repeat a committed mutation after an ambiguous acknowledgement.

Gateway/provider adapters normalize reported usage into application-owned observations. Usage received before a later failure remains useful evidence. Missing usage is unknown, not an invented exact estimate or zero. Elapsed time, input/output/reasoning/cache tokens and retry counts are useful learner cost controls; expanded implementation detail can remain contextual without hiding those observations.

Routine diagnostics exclude raw source text, prompts, provider responses, credentials, reasoning bodies and inline images. Bounded identifiers, counts, phases, timings and safe error codes are allowed. New wire envelopes need parsing/redaction fixtures rather than provider-specific payloads leaking into domain logs.

## Enforcement and extension

`./scripts/check-static.sh` includes `verify-document-ai-boundaries.mjs` and the adjacent provider/processing/IPC policy checks. They protect module topology, bounded owners, provider-wire separation and permitted dependencies. They do not establish factual grounding, exhaustive extraction, exact Rust/TypeScript shape equivalence, platform runtime behavior or live-provider reliability by themselves.

Structural size bounds are tripwires for responsibility review, not reasons to create arbitrary numbered include fragments. New artifacts reuse canonical evidence, retained knowledge, exact scopes, bounded authoring, deterministic publication and explicit lifecycle/compatibility rules. Focused native/component regressions and exact-SHA platform qualification remain separate from static policy checks and from manual educational/visual acceptance.
