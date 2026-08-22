# ADR 0042: AnyDoc-backed non-PDF document adapters

- **Status:** Accepted for implementation
- **Date:** 2026-08-22
- **Base:** `fd15f28cb53608e84d652db375253c8e45ee37ec` (PR #64 merged)

## Context

PR #63 established the app-owned `CanonicalDocument`, source-unit, block, asset,
semantic-chunk, and provenance contracts. PR #64 then split document intelligence
into format-neutral orchestration/reasoning modules and isolated provider
transport. PDF ingestion remains mature and must continue through the native
oxidize-pdf/PDFium/OCR adapter.

The remaining user-facing document import flow must accept well-supported
non-PDF formats without creating parallel AI pipelines or format-selection UI.
AnyDoc 0.2.3 is a pure-Rust MIT parser that exposes a structured document model
for Word, PowerPoint, OpenDocument, EPUB, RTF, spreadsheet, and CSV families.
Its PDF path is intentionally not used.

## Decision

### Backend isolation

`anydoc` is pinned as a private parsing dependency below the existing
`DocumentAdapter` boundary. No `anydoc::model` type may cross into persistence,
semantic planning, document intelligence, IPC, or React. The permanent domain
boundary remains `CanonicalDocument`.

### Enabled source families

The AnyDoc adapter family supports:

- Word: `.doc`, `.docx`, `.docm`;
- PowerPoint: `.ppt`, `.pps`, `.pot`, `.pptx`, `.pptm`, `.ppsx`, `.ppsm`;
- OpenDocument: `.odt`, `.odp`, `.ods`;
- EPUB: `.epub`;
- Rich Text Format: `.rtf`;
- spreadsheets: `.xls`, `.xlsx`, `.xlsm`, `.xlsb`;
- delimited tables: `.csv`.

PDF continues through `PdfDocumentAdapter`. Unsupported formats fail before AI
provider traffic.

### Canonical source units

- presentation formats produce `Slide` units;
- EPUB produces ordered `Chapter` units where package provenance permits it;
- word-processing/RTF formats produce logical `Section` units;
- spreadsheets produce `Section` units per worksheet/table region;
- CSV produces one or more bounded `Section` units.

AnyDoc's public model is a structured block stream rather than a complete source
unit graph. The oStudy adapter therefore owns deterministic unit segmentation,
heading hierarchy, list/table serialization, asset assignment, and stable hashes.
It records degraded provenance when the source format cannot expose an exact
native chapter/slide boundary through AnyDoc's public API.

### Shared normalization

Related formats share private normalizers:

- `FlowDocumentNormalizer` for DOC/DOCX/ODT/RTF/EPUB;
- `PresentationNormalizer` for PPT/PPTX/ODP;
- `TabularDocumentNormalizer` for XLS/XLSX/ODS/CSV.

All produce canonical blocks, source-unit text projections, assets, and semantic
chunks. Downstream outline, flashcard, curation, checkpoint, retry, and provider
routing code is unchanged and consumes PR #64's format-neutral interfaces.

### UI and acquisition

The Documents screen exposes one **Upload document** action. File filters list
all supported extensions but users never choose a parser. Content detection is
authoritative; extension and picker MIME values are hints. Document cards show a
format label and natural source-unit count such as pages, slides, sections, or
chapters while retaining common Retry, Generate More, archive, open, and delete
actions.

### Security and limits

- macros, scripts, embedded executables, and external relationships are never
  executed or fetched;
- parser input, uncompressed package size, block count, text size, table size,
  asset count, asset bytes, and source-unit count are bounded;
- source bytes are immutable managed files; extracted assets are rebuildable;
- malformed, encrypted, protected, or unsupported documents fail locally with
  safe error codes;
- diagnostics record format, counts, durations, and backend identity but never
  document text or asset bytes.

## Consequences

- New formats reuse the canonical semantic/document-intelligence pipeline from
  PR #63/#64 rather than adding format-specific prompts or provider requests.
- AnyDoc may be upgraded or replaced without a schema or AI-layer rewrite.
- Whole-slide visual rendering is not fabricated. Embedded assets are retained;
  a later optional rendering backend may add full-slide `UnitRender` evidence.
- AnyDoc's format limitations are represented as extraction diagnostics rather
  than silently invented provenance.
- HTML, arbitrary plaintext, photographed notes/OCR, and audio/video remain
  separate later adapters because AnyDoc does not provide the desired source
  model for those inputs.

## Qualification requirements

Before review readiness:

1. exact dependency and license verification;
2. adapter contract tests and generated fixtures for every enabled format family;
3. content-detection and disguised-extension tests;
4. malformed/package-limit tests;
5. migration/backup/delete/reconciliation regressions;
6. UI upload/filter/format/unit-label tests;
7. exact-head static, fast, full Linux/Tauri, Android package, and emulator gates;
8. human import/generation smoke for representative Word, presentation, EPUB,
   OpenDocument, RTF, spreadsheet, and CSV documents.
