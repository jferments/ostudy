# ADR 0042: AnyDoc-backed non-PDF document adapters

- **Status:** Accepted and implemented in PR #66
- **Date:** 2026-08-22
- **Base:** `fd15f28cb53608e84d652db375253c8e45ee37ec` (PR #64 merged)

## Context

PR #63 established the app-owned `CanonicalDocument`, source-unit, block, asset,
semantic-chunk, and provenance contracts. PR #64 then split document intelligence
into format-neutral orchestration/reasoning modules and isolated provider
transport. PDF ingestion remains mature and continues through the native
oxidize-pdf/PDFium/OCR adapter.

The Documents workflow also needs well-supported Word, PowerPoint,
OpenDocument, EPUB, RTF, spreadsheet, and CSV inputs without creating parallel
AI pipelines or a parser-selection UI. AnyDoc 0.2.3 is a pinned MIT-licensed
in-process Rust dependency that supplies parsers for those families. Its PDF path
is intentionally not selected by oStudy.

## Decision

### Backend isolation

`anydoc` lives below the existing document-adapter boundary. No
`anydoc::model` type may cross into persistence, semantic planning, document
intelligence, IPC, backup manifests, or React. The permanent domain boundary is
oStudy's canonical document model.

PR #64's document-intelligence orchestration, reasoning, curation, checkpoint,
and provider-transport modules remain format-neutral.

### Enabled source families

The AnyDoc adapter accepts the parser families behind these picker extensions:

- Word: `.doc`, `.docx`, `.docm`;
- PowerPoint: `.ppt`, `.pps`, `.pot`, `.pptx`, `.pptm`, `.ppsx`, `.potx`;
- OpenDocument: `.odt`, `.odp`, `.ods`;
- EPUB: `.epub`;
- RTF: `.rtf`;
- Excel: `.xls`, `.xlsx`, `.xlsm`, `.xlsb`;
- CSV: `.csv`.

Aliases and macro-enabled OOXML names are normalized to the corresponding
app-owned format. PDF continues through the native PDF adapter. Unsupported or
disguised inputs fail before AI provider traffic.

### Canonical source units

The current AnyDoc public model is a structured block stream rather than a
complete, uniform authored-unit graph. oStudy therefore does not label inferred
boundaries as exact slides, chapters, or worksheets.

All current AnyDoc-backed formats produce deterministic `Section` source units.
Sections split at top-level heading boundaries, bounded capacity boundaries, and
successive table boundaries for tabular sources. Presentation and EPUB inputs
record inferred/degraded segmentation metadata because exact slide/chapter
provenance is not guaranteed by the generic backend contract.

A future adapter version may expose exact authored unit kinds only when the
backend supplies provenance that oStudy can validate without inference.

### Shared normalization

One private app-owned AnyDoc mapper translates parser blocks into canonical
blocks/assets and applies small family-aware segmentation rules. It preserves the
structure the backend exposes—headings, paragraphs, nested lists, tables,
quotations, code, equations, notes, links, checkboxes, and embedded-image
references—without teaching document intelligence about source formats.

A generic block quote remains a canonical `Quote`; it is not treated as a
speaker note unless explicit note provenance exists.

### Assets and multimodal evidence

Embedded image assets exposed by AnyDoc become canonical source assets with
bounded byte sizes, hashes, stable IDs, and immutable versioned paths. External
image references are never fetched. Non-image embedded assets are not executed
or supplied to models; skipped resources are reflected in degraded extraction
metadata.

AnyDoc retries do not delete the last committed source images before a new
canonical commit. Changed image bytes receive a new immutable path; identical
bytes may reuse an existing immutable path.

Whole-slide rendering is not fabricated. PDF page renders remain the only
current `UnitRender` assets.

### UI and acquisition

The Documents screen exposes one **Upload document** action and no parser
selector. The platform first copies bytes into app-private staging. Shared Rust
then performs content detection before durable import.

Signature-bearing formats are content-authoritative. Extension and OS MIME are
not authorities. CSV is the explicit exception because it has no unique binary
signature, so a `.csv` filename hint is required to select CSV parsing.

Document cards show the detected format and the truthful canonical unit label:
PDF pages or AnyDoc sections. Retry, Generate More, archive, open, delete,
progress, and cancellation are shared across formats; PDF-only render/OCR UI is
shown only for PDFs.

### Security and limits

- macros, scripts, embedded executables, OLE objects, and linked templates are
  never executed;
- remote relationships/images are never fetched;
- managed sources are bounded to 512 MB;
- each retained embedded image is bounded to 32 MB and retained images are
  bounded to 128 MB total;
- canonical unit/block/chunk/heading/attribute text is bounded;
- AnyDoc's decompression, nesting, and node resource limits remain active;
- malformed, encrypted/protected, unsupported, or resource-limited inputs fail
  locally with safe errors;
- diagnostics record format/backend identity and counts, not source text or
  asset bytes.

### Lifecycle and backup

Generic document identities now own import, retry, cancellation, Generate More,
open, archive, delete, reconciliation, progress, checkpoints, backup, and
restore. Complete-library backups store generic source documents and canonical
source images and retain restore compatibility with the older `source_pdf`
backup representation.

PDF compatibility projections stay PDF-only; non-PDF adapters do not fabricate
legacy page rows.

## Consequences

- New formats reuse the PR #63/#64 canonical semantic and document-intelligence
  pipeline rather than adding format-specific prompts/provider requests.
- AnyDoc can be upgraded or replaced without changing the AI-layer contract.
- oStudy prefers truthful deterministic sections over fabricated slide/chapter
  provenance.
- Multimodal evidence can use retained embedded images where available; absence
  of faithful whole-slide rendering is explicit rather than hidden.
- HTML, arbitrary plaintext, photographed notes/OCR, audio, and video remain
  separate future adapters.

## Qualification requirements

Before review readiness:

1. exact dependency and license verification;
2. adapter fixtures for every enabled family from the pinned parser version;
3. content-detection/disguised-extension and CSV-exception tests;
4. malformed/resource-limit tests;
5. migration/backup/delete/reconciliation regressions;
6. UI upload/format/unit-label tests;
7. exact-head static, fast, full Linux/Tauri, Android package, and emulator gates;
8. human import/generation smoke for representative Word, presentation, EPUB,
   OpenDocument, RTF, spreadsheet/CSV, and PDF documents.