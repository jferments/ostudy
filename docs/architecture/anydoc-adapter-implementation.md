# AnyDoc document-adapter implementation contract

Status: accepted implementation contract for the post-PR-64 document adapter stage

## Scope

This stage adds production adapters for the non-PDF document formats that the
pinned AnyDoc Rust backend can parse with adequate structure and provenance.
oStudy's existing native-first PDF adapter remains authoritative for PDF; AnyDoc's
PDF support is not compiled into or selected by the adapter registry.

The enabled AnyDoc-backed formats are:

- DOCX and legacy DOC;
- PPTX and legacy PPT;
- ODT and ODP;
- EPUB;
- RTF.

Spreadsheet formats are deliberately excluded from this stage. HTML, plaintext,
and photographed-note ingestion use separate app-owned adapters and are not
silently routed through AnyDoc merely because a backend may expose partial text
support.

## Permanent boundary

AnyDoc is a replaceable parser backend. Its public `Document`, block, asset,
warning, and format types must remain private to `anydoc_backend` and the
format-specific adapters. No AnyDoc type may enter database records, Tauri IPC,
semantic planning, provider prompts, card authoring, backup manifests, or UI
state.

Every adapter returns an oStudy-owned `CanonicalIngestion`:

```
managed immutable source
        |
        v
AnyDoc parser backend
        |
        v
format normalizer
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

## Source-unit policy

- DOCX, DOC, ODT, and RTF use authored sections when available and otherwise
  deterministic logical sections built from heading boundaries. Oversized
  heading-less content is split into bounded `segment` units at block
  boundaries.
- PPTX, PPT, and ODP use one `slide` source unit per authored slide in
  presentation order. Speaker notes remain tagged blocks belonging to their
  slide.
- EPUB uses ordered spine items/chapters. Navigation resources, package order,
  and archive entry order never override the publication spine.

Source units, semantic chunks, and provider windows remain separate. Adapters do
not size source units according to the selected AI model.

## Structure mapping

The shared AnyDoc block mapper preserves titles, headings, paragraphs, nested
lists, tables, quotations, code blocks, equations when provided, captions,
footnotes/endnotes, key/value structures, speaker notes, and image descriptions.
Unknown non-empty blocks are retained as bounded paragraphs with a diagnostic
attribute rather than discarded.

Flowing-document and presentation normalization are separate app-owned modules.
This avoids teaching the AI pipeline about Word, PowerPoint, OpenDocument, or
EPUB internals while still allowing format-specific reading-order rules.

## Assets and multimodal evidence

Embedded images and other binary visual resources become canonical assets with
stable content hashes. Presentation adapters retain slide-level visual evidence
when the parser backend exposes it; absence of a faithful whole-slide renderer
is recorded as a bounded extraction warning rather than hidden.

Model-generated visual interpretation is a rebuildable semantic derivative. It
never overwrites native text, source assets, or parser output.

## Detection and security

The file picker exposes one supported-document filter and no format selector.
The native importer stages bytes first, then detects the format from bounded
magic/package inspection. Extensions and operating-system MIME claims are hints,
not authorities.

Adapters never execute macros, scripts, embedded objects, or linked templates;
never fetch remote resources; never launch embedded files; and fail closed on
encrypted/protected sources. Package parsing must retain AnyDoc's decompression
and resource bounds and oStudy's managed-source byte limits.

## Persistence and lifecycle

Original filenames and extensions are preserved in the immutable managed source
bundle. Existing retry, cancellation, archive, delete, open-original, backup,
restore, and crash-reconciliation behavior applies to every enabled format.

PDF compatibility projections remain PDF-only. Non-PDF adapters write canonical
source records directly and must not fabricate legacy page rows.

## Document intelligence

PR #64's document-intelligence system consumes only canonical semantic chunks
and source locators. There must be no format switch in outline generation,
flashcard generation, study-guide generation, curation, or provider transport.

Chunk identities include normalized content and canonical source provenance.
This is also the future attachment point for local embedding models; vectors are
not part of this stage.

## Qualification gates

Before a format is advertised, the implementation must prove:

- deterministic source-unit and block ordering;
- stable canonical hashes;
- actual fixtures produced by at least two relevant producers where practical;
- malformed, encrypted, path-traversal, external-resource, and oversized-input
  behavior;
- Linux/Tauri and Android compilation;
- preservation of existing PDF behavior;
- one automatic upload workflow with natural page/slide/chapter/section labels;
- full exact-head repository qualification.
