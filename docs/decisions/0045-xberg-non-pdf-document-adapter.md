# ADR 0045: Xberg-backed non-PDF document adapter

- **Status:** Accepted for implementation in stacked PR #72
- **Date:** 2026-08-24
- **Base:** PR #71 (`agent/document-ingestion-modularization`)
- **Supersedes:** ADR 0042 for active backend selection

## Context

ADR 0042 introduced AnyDoc for Word, PowerPoint, OpenDocument, EPUB, RTF,
spreadsheet, and CSV sources. PR #71 subsequently established an app-owned,
object-safe `DocumentAdapter` port, explicit per-format registry ownership,
canonical persistence outside parser implementations, and isolated native PDF
processing packages.

Xberg provides a broader, actively maintained Rust document-intelligence stack
with richer structured output, page/slide/sheet content, speaker notes, tables,
images, metadata, cooperative cancellation, and future OCR, URL, embedding, and
transcription capabilities. Upstream PR xberg-io/xberg#1469 made the existing
cancellation token usable by downstream Rust applications. The follow-up commit
`276cbba4b7ae6a91352fc6da1358688cab72b637` also rejects pre-cancelled work before
extractor dispatch.

The current PDF implementation remains mature and includes oStudy-specific
PDFium rendering and platform OCR behavior. This decision does not replace that
path.

## Decision

### Backend ownership

The explicit production registry assigns:

- PDF to the existing native PDF/PDFium/OCR adapter;
- DOC, DOCX, PPT, PPTX, ODT, ODP, RTF, EPUB, Excel, ODS, and CSV to Xberg; and
- unsupported formats to no adapter.

Selection remains an explicit `DocumentFormat` match. Registration order and
"first supporting backend" selection are forbidden. There is no parser selector
and no silent fallback to AnyDoc.

### Dependency profile

Until a crates.io release contains the required cancellation behavior, oStudy
pins Xberg to commit
`276cbba4b7ae6a91352fc6da1358688cab72b637` with `default-features = false` and
only:

```text
tokio-runtime
office
excel
```

The non-PDF cutover does not enable Xberg PDF, OCR, layout detection, URL
acquisition, embeddings, transcription, API, MCP, or aggregate `formats`/`full`
profiles. Repository policy verifies the exact revision and feature list.

### Isolation

`xberg::` types terminate inside `src-tauri/src/xberg_document_adapter/`.
The adapter returns only oStudy-owned `CanonicalIngestion` through the PR #71
port. Persistence, document lifecycle, learning plans, outlines, study guides,
flashcards, checkpoints, backup, and IPC remain parser-neutral.

The following transformations remain oStudy-owned:

- canonical source-unit, block, asset, and semantic-chunk construction;
- deterministic section segmentation for continuous-flow formats;
- immutable managed-asset storage and byte limits;
- canonical text bounding and validation;
- stable error mapping;
- backend identity and derivative invalidation; and
- cancellation and user-facing progress semantics.

Xberg chunking, embeddings, caching, OCR, and model-backed analysis are disabled.

### Source units

This backend-replacement PR preserves the durable semantics accepted by ADR 0042:
all current non-PDF formats remain canonical `Section` sources.

Xberg's exact `PageContent` boundaries are used internally to keep presentation
slides and spreadsheet sheets separate, including speaker notes, presentation
section names, and sheet names. They are deliberately persisted as sections in
this PR so parser replacement is not coupled to a migration of source-locator,
prompt, backup, and checkpoint semantics.

A later focused ADR may promote verified presentation units to `Slide`, add a
`Sheet` source-unit kind, or expose EPUB spine items as `Chapter` units.

### Cancellation

The adapter creates one Xberg `CancellationToken`, stores a clone in
`ExtractionConfig`, and retains another clone while extraction runs. It polls the
existing oStudy document-task cancellation signal and calls `cancel()` when the
user stops the job. Cancellation remains cooperative and returns the existing
`ai_request_canceled` application error. The adapter checks cancellation before
source reading, before dispatch, while Xberg runs, after Xberg returns, while
canonical units are built, and immediately before the canonical result boundary.
Partial canonical results are never committed.

### Progress

Xberg does not currently expose detailed page-by-page progress for these
extractors. oStudy therefore reports native extraction as indeterminate while
Xberg runs, followed by exact progress while the known canonical source units are
assembled. The UI must not invent percentages or source-unit totals before the
backend returns them. Existing PDF rendering and OCR counters remain unchanged.

### Content detection

Generic staged-file detection remains content-authoritative. Xberg MIME detection
is used for ZIP-based Office/OpenDocument/EPUB packages and ordinary structured
formats. PDF signature detection remains app-owned so PDFs cannot route through
the non-PDF adapter. Legacy OLE formats require both the OLE signature and an
authorized DOC/PPT/XLS filename hint. CSV remains the only plain-text extension
fallback.

### Assets and structure

Xberg document nodes map to canonical titles, headings, paragraphs, lists,
tables, images, code blocks, quotations, equations, footnotes, headers, footers,
key-value blocks, and speaker notes. Typed Xberg tables are rendered
deterministically into the current canonical table-text representation. A future
canonical schema may preserve full structured cell payloads without changing the
adapter port.

Embedded images use parser-neutral `source-asset:*` identities and immutable
versioned paths. Individual assets remain bounded to 32 MiB and aggregate retained
assets to 128 MiB. External image references are recorded but never fetched.
Whole-slide renders are not fabricated.

### Existing documents

Existing canonical documents produced by AnyDoc remain readable. No database or
backup migration is required because AnyDoc objects were never persisted. Stored
AnyDoc backend identities are historical facts. When existing workflow policy
requires re-ingestion, an identity mismatch selects Xberg and atomically replaces
only rebuildable canonical derivatives; learner cards, schedules, reviews,
annotations, and completed artifacts remain governed by their existing lifecycle
contracts.

## Consequences

- AnyDoc is removed from the dependency graph and production source.
- Non-PDF extraction gains richer slide, sheet, table, image, note, and metadata
  information without changing downstream learning-artifact contracts.
- The exact git revision is a temporary reproducibility requirement until a
  suitable Xberg release is published.
- PDFium, oxidize-pdf, and current platform OCR remain intentionally present.
- URL/web-page acquisition, Xberg OCR, Xberg PDF extraction, efficient Xberg page
  rendering, embeddings, and transcription remain separate follow-up stages.

## Qualification requirements

1. exact Xberg revision and feature-graph verification;
2. no AnyDoc dependency or production symbol;
3. Xberg type-isolation and explicit-routing static checks;
4. deterministic fixtures for every enabled source family;
5. content-authoritative detection and disguised-extension tests;
6. encrypted, malformed, resource-limit, and hostile-input failures before AI;
7. cancellation before dispatch, during extraction, and at the completion race;
8. unchanged PDF extraction, PDFium rendering, and OCR tests;
9. unchanged canonical persistence, backup, lifecycle, card, and study-guide tests;
10. exact-head static, fast, Linux/Tauri, Android package, and emulator gates; and
11. human source-quality review using representative private documents.
