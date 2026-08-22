# AnyDoc document-adapter implementation contract

Status: implemented architecture contract for PR #66

## Scope

This stage adds production adapters for the non-PDF document formats handled by
the pinned AnyDoc 0.2.3 Rust backend while preserving oStudy's existing native
PDF path. PDF continues through oxidize-pdf/PDFium/selective OCR; AnyDoc's PDF
support is never selected by the adapter registry.

The enabled AnyDoc-backed source families are:

- Word: DOC and DOCX-family inputs;
- PowerPoint: PPT and PPTX-family inputs;
- OpenDocument: ODT, ODP, and ODS;
- EPUB;
- RTF;
- spreadsheets: legacy/OOXML Excel inputs;
- CSV.

HTML, arbitrary plaintext, photographed-note/OCR, audio, and video remain outside
this stage.

## Permanent boundary

AnyDoc is a replaceable in-process parser backend. Its `Document`, block, asset,
warning, and format types stay private to `anydoc_document_adapter`. No AnyDoc
type enters database records, Tauri IPC, semantic planning, provider prompts,
card authoring, backup manifests, or React state.

Every adapter returns oStudy-owned canonical data before document intelligence
runs:

```
managed immutable source
        |
        v
native PDF adapter OR private AnyDoc adapter
        |
        v
CanonicalDocument
  source files
  source units
  semantic blocks
  visual assets
  extraction report
        |
        v
CanonicalSemanticChunks
        |
        v
PR #64 document-intelligence pipeline
```

PR #64's orchestration/reasoning modules remain format-neutral and do not import
AnyDoc.

## Source-unit policy

PDF preserves authored pages as `Page` units. The current AnyDoc public model is
a structured block stream but does not provide a sufficiently general,
app-verifiable authored slide/chapter/worksheet boundary graph for every enabled
family. oStudy therefore uses deterministic `Section` units for every current
AnyDoc-backed format instead of fabricating exact provenance.

Sections are split at top-level heading boundaries, bounded capacity boundaries,
and—for tabular formats—successive table boundaries. Presentation and EPUB
sources record degraded/inferred segmentation diagnostics because an oStudy
section must not be presented as an exact authored slide or chapter when that
provenance is unavailable through the backend contract.

Source units, semantic chunks, and provider windows remain separate. The adapter
does not size source units according to the selected AI model.

## Structure mapping

The shared app-owned mapper preserves the structure exposed by AnyDoc, including
titles/headings, paragraphs, nested lists, tables, quotations, code blocks,
equations, footnotes/endnotes, links, checkboxes, and embedded-image references.
A generic block quote remains a `Quote`; it is not reclassified as a speaker note
without explicit note provenance.

Family-aware behavior is limited to deterministic segmentation rules such as
successive table boundaries. Word, PowerPoint, OpenDocument, EPUB, RTF, and
spreadsheet names do not leak into document intelligence or provider transport.

## Assets and multimodal evidence

Embedded image assets exposed by AnyDoc become canonical source assets with
stable IDs, hashes, bounded byte sizes, and managed paths. Non-image embedded
assets are not executed or surfaced as model inputs; their omission is recorded
as degraded extraction metadata. External image references are counted but never
fetched.

AnyDoc image files use immutable, versioned managed paths. A retry writes new
content to a new path rather than deleting the last committed visual evidence;
identical content can reuse the same immutable file.

Whole-slide rendering is not fabricated. PDF page renders remain PDF-only
`UnitRender` assets. A later rendering backend may add faithful presentation
renders without changing the canonical or document-intelligence boundary.

## Detection and acquisition

The Documents workspace exposes one **Upload document** action and no parser
selector. The platform copies a selected file into app-private staging first.
Shared Rust then content-detects the format before durable import or AI traffic.

Signature-bearing formats are authoritative by content, not filename or
operating-system MIME. CSV is the deliberate exception because it has no unique
binary signature; a `.csv` filename hint is required before the bounded CSV
parser is selected. Picker aliases such as macro-enabled OOXML extensions are
canonicalized to the corresponding app-owned source format and managed
extension.

The original display filename is retained in document metadata. The immutable
managed source uses a safe canonical extension owned by the detected format.

## Security and resource limits

Adapters never execute macros, scripts, embedded executables, OLE payloads, or
linked templates; never launch embedded files; and never fetch remote resources.
Encrypted/protected, malformed, unsupported, or parser-resource-limited inputs
fail locally with safe errors before provider traffic.

The app bounds the managed source to 512 MB, each retained embedded image to
32 MB, retained embedded images to 128 MB total, source-unit and block text,
semantic-chunk text, headings, and attribute values. AnyDoc's own package parser
also enforces its decompression/nesting/node resource limits and reports those as
`ResourceLimit`; oStudy maps that to a content-free safe error.

Diagnostics contain format/backend identity, counts, durations, and bounded
error metadata—not document text, image bytes, raw prompts, credentials, or
provider responses.

## Persistence and lifecycle

The database persists the detected source format, canonical source files,
sections, blocks, visual assets, semantic chunks, extraction diagnostics, and
stable hashes. PDF compatibility projections remain PDF-only; non-PDF adapters
do not fabricate legacy page rows.

Import, retry, cancellation, Generate More, open-original, archive, delete,
startup reconciliation, progress, checkpointing, complete-library backup, and
restore operate on generic document identities. Complete-library backup stores
generic source files and canonical source images while retaining compatibility
with older `source_pdf` backup rows.

## Document intelligence

Outline generation, flashcard generation, curation, checkpoint reuse, semantic
windowing, and provider transport consume only canonical semantic chunks and
source locators. There is no AnyDoc import or format-specific prompt branch in
those modules.

Canonical chunk/source identities are also the future attachment point for local
embedding derivatives; embeddings and vector indexes are not part of this PR.

## Qualification gates

Before review readiness, PR #66 must prove:

- pinned AnyDoc dependency/license and committed dependency graph;
- content-authoritative detection plus the explicit CSV exception;
- deterministic canonical ordering/hashes and truthful section provenance;
- adapter fixtures covering each enabled source family and malformed inputs;
- PDF regression behavior;
- generic import/lifecycle/UI/backup/restore behavior;
- static and fast qualification on the exact final source head;
- full Linux/Tauri qualification;
- Android compile/package qualification;
- Android emulator smoke qualification.

Representative live imports through the graphical workflow remain a human
pre-merge acceptance step.