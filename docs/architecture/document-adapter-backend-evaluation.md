# Document adapter backend evaluation

Status: accepted research note for the canonical document foundation

## Decision context

The canonical document boundary is deliberately owned by ostudy. Parser crates,
platform APIs, OCR engines, and office-rendering tools may be replaced without
changing semantic planning, multimodal evidence selection, outline generation,
flashcard generation, or future study-guide and tutoring features.

This note records the evaluation criteria for the later format-adapter PRs. It
does not enable another format and does not add a parser dependency.

## Non-negotiable backend requirements

A backend considered for production must:

1. compile for every target on which the adapter is advertised, including
   Android rather than only Linux desktop;
2. keep macros, scripts, external relationships, remote images, and linked
   templates inert;
3. enforce archive-entry, uncompressed-byte, compression-ratio, XML-depth,
   image-dimension, and total-memory bounds;
4. preserve deterministic source-unit order: slides for presentations,
   chapters/spine items for EPUB, and logical sections for flowing documents;
5. retain tables, lists, notes, captions, embedded images, and source
   provenance rather than flattening directly to Markdown;
6. return app-independent data that can be translated into
   `CanonicalDocument`; and
7. fail closed on encrypted, protected, malformed, or unsupported content.

A backend failing any of these requirements remains a research dependency even
if it extracts happy-path text successfully.

## Rust `anydoc`

`anydoc` is a useful candidate because it currently advertises support for
DOC/DOCX, PPT/PPTX, ODT/ODP, EPUB, and several other document formats under an
MIT license. It could reduce format-specific code and provide one parser-facing
block/asset vocabulary.

It must not become ostudy's domain model. Its current public model is primarily
a flattened stream of blocks, notes, and assets. ostudy additionally needs
stable source-unit boundaries, immutable source-file provenance, explicit
visual-evidence identities, deterministic hashes, and format-neutral locators.
The crate is also still early in its version history. Before adoption, a later
spike must pin an exact revision and test ordering, hostile packages, table and
list fidelity, speaker notes, embedded assets, binary size, and all supported
platform builds.

If accepted, it will sit behind an `OfficePackageBackend` implementation and be
translated immediately into ostudy-owned canonical types. Its Markdown output
will not be stored as the canonical representation. The existing PDF pipeline
will not be replaced by its PDF support.

## Direct package parsing

DOCX and PPTX use Open Packaging Conventions; ODT and ODP use OpenDocument
packages; EPUB uses a ZIP container with a package document, manifest, spine,
and navigation resources. A shared bounded package reader is therefore a
viable fallback when a general parser does not pass the production gates.

The shared layer should own ZIP-bomb protection, path normalization, duplicate
entry rejection, XML parser hardening, cancellation, temporary-storage limits,
and network denial. Thin format parsers should only interpret relationships and
translate source structure into canonical units and blocks.

Direct parsing costs more engineering time, but it gives ostudy precise control
over source-unit boundaries and mobile compatibility. The canonical adapter
contract allows this choice to be changed later without rewriting document
intelligence.

## Rendering policy

Faithful Office rendering is not a baseline requirement for the first adapters.
Depending on Microsoft Office or LibreOffice would add a desktop-only external
runtime and would not solve Android. The baseline should extract native
structure and embedded visual assets. Optional renderers may later produce
additional `UnitRender` assets when available.

PowerPoint and ODP slides remain inherently visual. Text, tables, chart data,
notes, images, and alt text should be extracted deterministically, while slide
renders or embedded assets are retained as multimodal evidence. A capable
vision-language model can then interpret arrows, spatial relationships,
animations represented by selected states, diagrams, and other meaning that is
not reducible to text alone. Those model descriptions are rebuildable semantic
derivatives and never overwrite the source slide or canonical native text.

## Legacy binary Office formats

`.doc` and `.ppt` are a compatibility tier, not aliases for DOCX and PPTX.
Support is advertised only after fixtures demonstrate stable ordering, tables,
lists, images, corruption handling, resource bounds, and Android compatibility.
If no safe cross-platform backend meets those gates, modern OOXML and
OpenDocument formats ship first and legacy binary formats remain explicitly
unsupported until a dedicated adapter is ready.

## EPUB and HTML reuse

EPUB XHTML and standalone HTML should use one non-executing HTML content
extractor. EPUB-specific code validates the container and follows package/spine
reading order; HTML-specific code identifies the primary article structure.
Neither path executes scripts or fetches remote resources.

## OCR and handwritten notes

The current PDF OCR orchestration should become a general image OCR service in
a later PR. Tesseract and platform text-recognition APIs remain useful for
printed text. Handwriting requires a separately declared capability and a
measured backend such as PaddleOCR's handwriting-oriented models or a layout
pipeline feeding a line recognizer such as TrOCR.

Original note images, geometry, confidence, and preprocessing versions must be
retained. Low-confidence recognition is reported as degraded extraction rather
than silently treated as authoritative text. Multimodal model recovery is a
separate, privacy-aware derivative pass.

## Embedding readiness

No embedding runtime or vector index is added in the foundation PR. Canonical
semantic chunks instead receive stable content hashes and source locators.
A later local embedding backend, potentially a small Qwen embedding model served
through Candle, can key vectors by:

- document ID;
- canonical schema version;
- chunk index and content SHA-256;
- source-locator stable identity;
- embedding model/revision;
- tokenizer and pooling identity; and
- vector-normalization/version identity.

Changing a parser or OCR result invalidates only derivatives whose canonical
content identity changed. This supports semantic search, topic maps, study-gap
analysis, related-content discovery, and retrieval for interactive tutoring
without making vectors part of the immutable source representation.

## Required later spike output

Before enabling Office/OpenDocument/EPUB adapters, the implementation PR must
publish:

- exact dependency revisions and licenses;
- a supported-target build matrix;
- extraction-quality results for the shared golden corpus;
- hostile-package and resource-limit results;
- binary-size and peak-memory measurements;
- known fidelity losses by format; and
- a clear fallback or direct-parser decision.

Until that evidence exists, the adapter registry continues to enable only the
mature PDF reference adapter.
