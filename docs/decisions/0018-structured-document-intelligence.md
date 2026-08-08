# ADR 0018: Structured document intelligence and split PDF backends

- **Status:** Accepted
- **Date:** 2026-08-08
- **Scope:** AI Stages 4.3–4.6 follow-up in PR #38

## Context

The first Stage 4.3–4.6 implementation extracted a page-level string and rendered each page through one `justpdf` backend, then generated flashcards one physical PDF page at a time. Real-provider testing showed that card-contract reliability could be fixed at the AI boundary, but the page-level document representation still discarded useful layout/semantic information and treated an arbitrary print-layout boundary as a reasoning boundary.

The document layer is intended to support more than flashcards. Study-guide generation, quizzes, document search, and interactive tutoring all need the same reusable understanding of sections, headings, paragraphs, tables, lists, geometry, reading order, and cross-page concepts.

## Decision

### Extraction and rendering are separate capabilities

ostudy no longer has one concrete PDF engine embedded in document lifecycle code.

- `src-tauri/src/pdf_backend.rs` is the structured extraction boundary.
- `src-tauri/src/pdf_render_backend.rs` is the raster page-rendering boundary.
- `src-tauri/src/documents.rs` owns limits, managed paths, hashing, SQLite state, retries, AI orchestration, and canonical authoring.

The active structured extractor is **oxidize-pdf 4.2.2** with the `Rag` extraction profile. OCR features are explicitly disabled. The active renderer is **Pdfium build 7881** through `pdfium-render 0.9.4`.

This split permits future replacement of either capability independently. For example, a platform-native renderer can replace Pdfium without changing extraction or document intelligence, and another structured extractor can replace oxidize-pdf without changing rendering, persistence, provider adapters, or card authoring.

### Semantic intermediate representation

The immutable source PDF remains the durable source of truth. Derived semantic data is stored as ostudy-owned records rather than oxidize-pdf Rust types:

- per-page extracted text;
- typed semantic elements such as title, paragraph, table, list item, image, code block, key/value, header, and footer;
- element page number and bounding box;
- heading path and parent heading;
- font/format signals where available;
- structure-aware semantic chunks with page mappings, element types, heading context, token estimates, and bounding boxes.

The backend identity is recorded with the derived data. Retry rebuilds semantics when the active extraction/render backend changes.

### Semantic windows, not page prompts

Physical pages are provenance/layout units, not AI reasoning units. Flashcard generation therefore operates on overlapping contiguous **semantic windows** built from structure-aware chunks.

Windows are bounded by both estimated tokens and distinct page count. The current generation defaults permit up to 20,000 source tokens and 40 physical pages per window with semantic-chunk overlap; outline construction permits larger 32,000-token / 48-page windows. A single semantic chunk is never split merely to hit a limit.

This preserves paragraphs, tables, and concepts crossing page boundaries while still bounding requests for smaller local models.

### Reusable hierarchical document outline

Before flashcard generation, the configured `flashcard_generation` provider creates a hierarchical outline from the semantic windows. For documents larger than one outline window, validated outline fragments are recursively merged in bounded groups.

The final durable outline contains:

- document summary;
- major learning objectives;
- key concepts;
- section hierarchy with page spans;
- per-section summary, objectives, and concepts.

The outline is stored independently of the generated cards so later study-guide, quiz, search, and tutoring tasks can reuse it instead of reinterpreting flattened PDF text.

### Outline-guided flashcards and global curation

Every flashcard-generation window receives both its local structured source and the validated global document outline. The prompt instructs the model to judge local details against global learning objectives and to create cross-page/integrative questions when supported.

After all windows return locally valid cards, a final bounded relevance pass selects which cards remain. It removes globally minor/hyperspecific material, bibliography/citation artifacts, redundant coverage, and near-duplicates while preserving broad learning-objective coverage and pedagogical variety. The curation model may select or reject a card but cannot rewrite it.

The resulting cards still pass the canonical generated-import validators and are committed directly into an ordinary deck; no review queue is introduced.

### Pdfium runtime provenance

`pdfium-render` is only a binding layer and does not contain Pdfium. Build/test/release scripts therefore provision the exact **chromium/7881** Pdfium release from `bblanchon/pdfium-binaries`. The archive is cached outside Git and accepted only when GitHub's immutable release metadata supplies a matching SHA-256 digest.

The verified platform library is copied into `src-tauri/resources/pdfium/` for Tauri bundling. That generated directory is Git-ignored. Packaged applications resolve the bundled resource; development runs also expose its exact path through `OSTUDY_PDFIUM_LIBRARY`.

### OCR remains out of scope

oxidize-pdf is built with default features disabled and only `compression` plus `semantic` enabled. `rusty-tesseract` is prohibited by dependency verification. Image-only pages can still be rendered and may later participate in an image-first analysis path, but ostudy does not silently store model transcription as extracted source text.

## Consequences

- Document understanding now survives page boundaries and retains substantially more structure than the previous page-string model.
- AI generation requires an outline pass plus a global curation pass, increasing model calls but improving global relevance and creating a reusable artifact for later features.
- Pdfium adds a target-specific native runtime, but the runtime is deterministic, checksummed, bundled, and isolated behind a renderer interface.
- Existing managed PDFs created by the earlier PR #38 implementation are automatically re-extracted on retry because their recorded backend identity is `legacy`/`justpdf` rather than the current backend IDs.
- Full-library backup continues to require only the database plus source/render managed files; semantic elements, chunks, outlines, and generation-window state are already in SQLite.
