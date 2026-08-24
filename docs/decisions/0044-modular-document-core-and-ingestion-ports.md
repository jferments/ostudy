# ADR 0044: Modular document core and replaceable ingestion ports

- **Status:** Accepted for implementation
- **Date:** 2026-08-23
- **Base:** `a08afbd723e30b04fd7ac0734c60ca92b688efcc` (PR #70 merged)

## Context

PR #63 established ostudy-owned canonical document records. PR #64 separated
provider-neutral document intelligence from provider transport. PR #66 added
AnyDoc-backed non-PDF adapters. PR #70 introduced shared learning plans,
study-guide artifacts, and topic-linked cards, and moved initial flashcard
generation out of `documents.rs`.

Those changes established the correct durable boundaries, but the source layout
around them still reflects the sequence in which the PDF and AnyDoc features were
added:

- `document_adapters.rs` combines adapter selection, progress persistence,
  canonical persistence, PDF canonicalization, and the temporary PDF
  compatibility projection;
- `document_ingestion.rs` is physically named as the generic ingestion module but
  actually implements the native PDF extraction/render/OCR pipeline;
- `anydoc_document_adapter.rs` combines format probing, parser invocation,
  normalization, segmentation, chunking, asset persistence, and canonical
  assembly;
- `pdf_backend.rs` combines native extraction, semantic conversion, text-quality
  policy, and recovery planning; and
- `documents.rs` still combines IPC contracts, catalog queries, source
  resolution, lifecycle policy, failure projection, and startup reconciliation.

`lib.rs` currently uses path-attributed aliases to make the physical
`document_adapters.rs` file appear as `document_ingestion` and the physical
`document_ingestion.rs` file appear as `pdf_document_ingestion`. This obscures
ownership and makes backend replacement harder to review.

The planned Xberg evaluation may replace AnyDoc and may eventually supply PDF
extraction, OCR, layout understanding, or rendering. ostudy must be able to make
that change by implementing and explicitly assigning a document adapter, not by
rewriting learning plans, study guides, flashcard generation, persistence, or
IPC.

## Decision

### 1. The document core becomes a package

`crate::documents` remains the stable document-library boundary, but its
implementation becomes a normal module directory:

```text
documents/
├── mod.rs
├── commands.rs
├── contracts.rs
├── repository.rs
├── source.rs
├── lifecycle.rs
├── generation.rs
└── reconciliation.rs
```

The package root contains declarations and narrow re-exports. Core document
modules do not depend on Tauri-managed state, AI provider code, a parser crate,
PDF rendering, OCR, or artifact-specific generation implementations. Only
`commands.rs` owns Tauri command wrappers.

- `contracts.rs` owns existing serialized request/result DTOs without changing
  command payloads or field names.
- `repository.rs` owns ordinary `documents` table projections and artifact
  presence queries.
- `source.rs` validates and resolves managed source paths.
- `lifecycle.rs` owns the settled document-level `processing`, `ready`, `failed`,
  and `archived` policy. An unarchived document is ready when at least one usable
  learning artifact remains, even when the latest generation attempt failed.
- `generation.rs` adapts the existing initial-flashcard use case to ordinary Rust
  dependencies and maps its result back to the stable IPC contract.
- `reconciliation.rs` coordinates interrupted operations, transient staging
  cleanup, and missing-source detection.
- `commands.rs` contains thin Tauri wrappers for catalog and archive operations.

Artifact modules remain responsible for their own job state and atomic artifact
commits. They call the document lifecycle service rather than reimplementing
settled document readiness after failure, cancellation, or interruption.

### 2. Generic ingestion is an application-owned port

`crate::document_ingestion` becomes a physical module directory. It owns the
contracts used to select, execute, validate, and persist document adapters:

```text
document_ingestion/
├── mod.rs
├── contracts.rs
├── registry.rs
├── progress.rs
├── persistence.rs
├── anydoc/
└── pdf/
```

The stable port is an object-safe application-owned `DocumentAdapter` contract.
It accepts an ostudy-owned extraction request and progress sink and returns an
ostudy-owned extraction output. Parser, OCR, renderer, subprocess, or RPC types
may not cross the port.

The request contains only managed source identity, the recorded app-owned
`DocumentFormat`, bounded source-unit limits, and an exact-document cancellation
check. Runtime-specific dependencies are injected when the registry constructs
the selected adapter implementation, rather than leaking into the generic
request. The output contains `CanonicalIngestion`, versioned backend identities,
and an optional app-owned compatibility projection.

The registry owns an explicit `DocumentFormat` to production-backend assignment.
Registration order never decides ownership and “first adapter that reports
support” selection is forbidden. The current assignment is:

- PDF is owned by the native PDF/PDFium/OCR adapter;
- the currently enabled office, OpenDocument, EPUB, RTF, spreadsheet, and CSV
  families are owned by the AnyDoc adapter; and
- a future Xberg adapter must explicitly take ownership of reviewed formats in
  the registry match after its provenance, cancellation, and parity contracts
  are qualified.

There is no runtime parser-selection UI, no ambiguous overlapping ownership, and
no silent fallback from one backend to another after extraction begins.

### 3. Backend identity is part of the port

Every adapter exposes a versioned `DocumentAdapterIdentity` with:

- adapter identity;
- extraction identity; and
- optional render identity.

Persistence compares the output identity with the currently assigned adapter
before commit. A backend upgrade invalidates rebuildable derivatives while
preserving immutable sources, completed learner artifacts, annotations, cards,
schedules, and review history.

Identity values remain compatible with the existing database fields during this
refactor. No migration is introduced.

### 4. Persistence is separate from adapter execution

Adapters do not write the document catalog, progress projection, canonical
ledger, outline, learning plan, study guide, cards, or provider state.

`document_ingestion::persistence` validates the selected adapter identity,
validates `CanonicalIngestion`, verifies immutable source metadata, and commits
canonical records and any temporary compatibility projection atomically.

`document_ingestion::progress` translates app-owned progress events into bounded
document projections. Adapter implementations report progress; they do not know
the `documents` table schema.

### 5. PDF remains one adapter implementation, not the generic model

The current native PDF behavior remains the regression oracle but is split across
three explicit ownership layers:

```text
document_ingestion/pdf/
├── adapter.rs
├── canonicalize.rs
└── compatibility.rs

pdf_document_ingestion/
├── contracts.rs
├── pipeline.rs
├── rendering.rs
├── ocr.rs
├── recovery.rs
├── validation.rs
├── errors.rs
├── files.rs
└── progress.rs

pdf_backend/
├── contracts.rs
├── extraction.rs
├── quality.rs
└── recovery.rs
```

The native PDF adapter coordinates the PDF processing service, then converts its
output into canonical records. `pdf_document_ingestion` owns rendering, OCR,
recovery sequencing, durable render writes, and progress. `pdf_backend` owns
low-level extraction, semantic conversion, text-quality assessment, and recovery
planning.

This decomposition is not imposed on future adapters. An Xberg adapter may
produce canonical blocks and assets directly through a subprocess or library
while still satisfying the top-level `DocumentAdapter` port.

The temporary PDF compatibility projection remains isolated in
`document_ingestion/pdf/compatibility.rs`. Non-PDF adapters never fabricate
legacy page rows.

### 6. AnyDoc remains isolated behind its adapter

The generic registry wrapper is intentionally small:

```text
document_ingestion/anydoc/
└── adapter.rs
```

All AnyDoc-specific parsing and transformation stages live in the backend package:

```text
anydoc_document_adapter/
├── adapter.rs
├── detection.rs
├── model.rs
├── normalization.rs
├── segmentation.rs
├── chunking.rs
├── assets.rs
├── errors.rs
└── support.rs
```

`anydoc::model` values terminate inside this package. The backend first builds an
app-owned normalized representation, then creates canonical source units, blocks,
assets, and chunks. This creates a direct replacement seam for Xberg without
leaking either backend into canonical persistence or document intelligence.

Current source-unit truthfulness remains unchanged: AnyDoc-backed inputs produce
bounded inferred sections where exact authored-unit provenance is unavailable.

### 7. Cancellation is explicit at the adapter boundary

The adapter request supplies an exact-document cancellation check. Current
implementations bridge that check to the existing task-cancellation registry.
Future Xberg integration may forward the same cancellation signal to an upstream
cancellation hook or terminate a scoped worker process. Cancellation remains
cooperative, document-scoped, and incapable of committing partial canonical
output.

### 8. Compatibility is preserved

This refactor does not change:

- database schema or migration checksums;
- canonical document, semantic chunk, outline, learning-plan, study-guide,
  checkpoint, backup, or generated-card serialized shapes;
- Tauri command names or frontend IPC fields;
- current backend identity strings;
- AnyDoc format detection, segmentation, or asset limits;
- native PDF extraction, rendering, OCR, quality, or recovery behavior;
- provider prompts, schemas, capability routing, or concurrency; or
- document lifecycle user-visible behavior except where a regression proves
  existing duplicated policy is inconsistent with the accepted guide/deck
  readiness rule.

## Dependency direction

```text
commands / artifact workflows
            |
            v
       documents core
            |
            v
 document_ingestion registry
            |
      +-----+-----+
      |           |
      v           v
 native PDF     AnyDoc       future Xberg
 adapter        adapter       adapter
      |           |              |
      +-----------+--------------+
                  |
                  v
       CanonicalIngestion
                  |
                  v
     canonical persistence
                  |
                  v
 semantic planning / outline / DocumentLearningPlan
                  |
          +-------+-------+
          |               |
          v               v
      flashcards      study guides
```

Dependencies may point downward through this diagram. Backend implementations do
not import artifact generation, provider transport, or IPC command modules.

## Enforcement and qualification

The static architecture verifier requires the final module topology, rejects the
path-attributed ingestion aliases, rejects third-party parser types outside
backend implementation directories, rejects Tauri state/commands outside the
core command module, forbids first-supporting-backend routing, and prevents
replacement monoliths from reappearing.

Regression coverage must include:

1. explicit registry ownership and content-authoritative probing;
2. backend identity validation before persistence;
3. unchanged PDF canonical and compatibility projections;
4. unchanged AnyDoc family fixtures, segmentation, and asset identity;
5. cancellation before and during adapter work;
6. document source resolution and lifecycle truth tables;
7. startup interruption and missing-source reconciliation;
8. exact serialized IPC compatibility; and
9. static, fast, full Linux/Tauri, Android package, and emulator qualification on
   the exact pull-request head.

Representative PDF, Word, presentation, EPUB, OpenDocument, RTF, spreadsheet,
and CSV generation remain human acceptance boundaries.

## Consequences

The source tree gains more files and explicit interfaces, but each module has a
single durable owner. Current behavior remains available while parser
implementation decisions become local to the explicit adapter ownership map.
Xberg can be introduced incrementally and compared against the existing adapters
without creating a parallel document-learning pipeline or changing authoritative
learner data.

A later Xberg integration still requires its own dependency/license/runtime,
process-isolation, capability, provenance, cancellation, and migration analysis.
This ADR establishes the port; it does not select or ship Xberg.
