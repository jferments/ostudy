# 0041 — Modular document-AI architecture

Status: accepted

Date: 2026-08-21

## Context

PR #63 made `CanonicalDocument`, canonical semantic chunks, source locators, and
visual-evidence identities the authoritative input to document planning. The
large-document AI implementation predates that boundary. Its public service was
already partly separated: the execution facade performed bounded concurrent
outline work, checkpoint reuse, deterministic persistence, and final assembly.
The underlying `document_intelligence.rs`, however, combined approximately
113 KiB of unrelated responsibilities:

- document-learning contracts and local validation;
- outline normalization, deterministic assembly, and semantic synthesis;
- flashcard authoring and final curation;
- provider profile loading and task context;
- provider request bodies, endpoint selection, schema lowering, response
  decoding, compatibility retries, and token accounting; and
- PDF-specific language that no longer represented the canonical input model.

That shape made provider changes risky because they shared a file with learning
logic, made format-neutral reasoning difficult to verify, and gave future
study-guide, practice-test, tutoring, semantic-search, and embedding features no
stable domain layer to reuse.

## Decision

`crate::ai::document_intelligence` remains the stable application-facing service
boundary. It is a normal Rust module directory with the following owners:

- `mod.rs` — format-neutral orchestration. It owns bounded concurrency, cached
  fragment reuse, source-order persistence, deterministic collection, and
  coordination between domain tasks.
- `contracts.rs` — provider-neutral outlines, learning objectives, generated
  study-card candidates, visual evidence, and local validation contracts.
- `context.rs` — the immutable provider/profile snapshot used by a document job,
  effective capability negotiation, token-budget policy, and aggregate usage.
- `outline.rs` — local outline reasoning, normalization, deterministic hierarchy
  assembly, whole-document synthesis, and compact outline projections for later
  learning tasks.
- `flashcards.rs` — source-grounded card-authoring prompts, bounded retries, and
  conversion into validated ostudy card values.
- `curation.rs` — selection-only global quality curation, bounded batching, and
  deterministic de-duplication.
- `schemas.rs` — provider-neutral JSON task schemas. Providers may lower these
  schemas for their wire format but may not redefine the domain contract.
- `transport.rs` — credential resolution, endpoints, provider request bodies,
  provider-specific schema lowering, response decoding, compatibility retries,
  content-stop handling, and token-envelope parsing.
- `support.rs` — small shared error and bounded-diagnostic helpers.

The dependency direction is:

```text
application document workflow
            |
            v
format-neutral document-intelligence orchestrator
            |
            +--> outline / flashcards / curation
                    |          |
                    v          v
                contracts <- schemas
                    |
                    v
                  transport --> AI gateway/provider adapters
```

Transport must not depend on `SemanticWindow`, canonical parser structures, or a
specific source format. Domain tasks translate canonical windows and selected
visual evidence into bounded text/image requests before calling transport.
Provider wire objects must not escape transport.

## Canonical source-unit semantics

PR #63 intentionally preserves `pageStart`, `pageEnd`, `page_count`, and
`SourcePageImage` as compatibility projections while PDF remains the only
shipping ingestion adapter. This refactor does not change persisted outline or
checkpoint JSON and therefore does not require a database migration. Within new
reasoning code, those numeric values are interpreted as one-based canonical
source-unit bounds. New prompts, errors, and diagnostics use “source unit”,
“source window”, or “visual evidence” unless a message is specifically
describing the PDF adapter.

The outline, flashcard, and curation prompts changed materially from PDF-oriented
to canonical source-unit language, so their prompt versions advance. Existing
outline and generation checkpoints are rebuildable acceleration data, not
learner data. Outline checkpoint identity already includes outline prompt/schema
versions; generation checkpoint identity now includes flashcard prompt/schema
versions as well. Old rows remain harmless and simply stop matching the new
contract. Serialized card, outline, and checkpoint row shapes remain unchanged.

A later versioned contract may replace the compatibility names with explicit
`SourceLocator` values. That change must update persisted checkpoints, repair or
migrate existing rows where necessary, and revise prompt/schema versions
atomically. It is not part of this architecture refactor.

## Semantic reasoning and future embeddings

The outline is a reusable learning model, not a flashcard-only intermediate.
Deterministic Rust assembly remains authoritative for hierarchy and provenance;
an optional provider synthesis may improve the concise whole-document summary,
major objectives, and concepts but cannot invent source structure.

Future embedding work, including a small local model such as Qwen Embedding
0.6B, will consume canonical semantic chunks and produce versioned derived
artifacts keyed by source hashes. Embeddings may improve retrieval, topic maps,
coverage analysis, and window selection. They do not replace canonical content,
source locators, deterministic outline assembly, or local validation. The module
split leaves that future service adjacent to document reasoning rather than
inside provider transport.

## Multimodal reasoning

Visual evidence is selected by the canonical document workflow and passed to a
domain task together with its source locator. Transport only encodes bounded
bytes in the provider’s supported wire representation. This allows diagrams,
tables, equations, slide layouts, and image-only source units to inform outlines,
flashcards, study guides, tests, and tutoring without making those features PDF
specific.

## Invariants

1. Provider credentials and raw provider responses remain native-only.
2. Source and model content are never written to routine diagnostics.
3. Independent provider calls may run concurrently; durable writes and returned
   artifact order remain deterministic by canonical source-window order.
4. Persisted outline/card JSON shapes remain compatible. Prompt/schema version
   changes invalidate only rebuildable checkpoints through explicit contract
   identity; they do not require learner-data migration.
5. A failed optional global synthesis cannot discard deterministic local outline
   assembly.
6. A failed curation batch retains already validated candidates rather than
   silently deleting them.
7. Domain schemas are authoritative. Provider adapters may only perform explicit,
   tested lowering required by a provider.
8. No document-intelligence source file may become a replacement monolith; a
   static architecture check enforces the module set, dependency direction, and
   bounded file size.

## Workflow and qualification

The repository static gate verifies the document-AI boundary before compiling.
The verifier rejects path-attributed module aliases, reintroduction of the old
monolith, provider-wire vocabulary outside transport, transport dependencies on
semantic windows, missing required modules, and oversized replacement files.
Regression tests exercise both accepted and rejected fixture layouts.

Because this is a shared Rust-domain refactor, every source-tree change must pass
static qualification and a test-compiling Rust gate before handoff. Exact-head
qualification evidence is invalidated by every later source change, as required
by the PR #60 execution contract and its PR #63 hardening.

## Consequences

Provider changes become localized and independently testable. Learning logic can
be reused by later artifact types, and non-PDF adapters can feed the same
orchestrator without inheriting PDF wording or wire details. More files and
explicit visibility boundaries are introduced, and some compatibility names
remain until a future versioned persistence change. The prompt-version increase
causes existing document-AI checkpoints to be recomputed on the next run, but no
canonical document, deck, card, review history, or user setting is lost. The
refactor deliberately prefers those explicit seams over a smaller file count
with hidden coupling.

## Alternatives rejected

- **Keep one file and add comment sections.** This does not enforce ownership or
  dependency direction and does not prevent continued growth.
- **Put each provider’s complete document pipeline in its provider adapter.**
  That would duplicate prompts, schemas, validation, retries, and learning logic.
- **Make LLM output the canonical semantic representation.** Provider output is
  nondeterministic and cannot replace source-grounded canonical content or
  stable provenance.
- **Rename compatibility fields during this refactor.** That would mix a module
  extraction with a persisted-contract migration and make failures much harder
  to isolate.
