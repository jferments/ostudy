# ADR 0018: Structured document intelligence and split PDF backends

- **Status:** Accepted
- **Date:** 2026-08-08
- **Scope:** AI Stages 4.3–4.6 follow-up in PR #38

## Context

The first Stage 4.3–4.6 implementation extracted a page-level string and rendered each page through one `justpdf` backend, then generated flashcards one physical PDF page at a time. Real-provider testing showed that card-contract reliability could be fixed at the AI boundary, but the page-level document representation still discarded useful layout/semantic information and treated an arbitrary print-layout boundary as a reasoning boundary.

The document layer is intended to support more than flashcards. Study-guide generation, quizzes, document search, and interactive tutoring all need the same reusable understanding of sections, headings, paragraphs, tables, lists, geometry, reading order, and cross-page concepts.

ostudy also targets Linux, Windows, macOS, Android, and iOS. A mandatory renderer that requires a separately packaged native library per operating system and CPU architecture creates disproportionate release and mobile-porting cost for a capability that can be isolated behind a narrow page-to-PNG boundary.

## Decision

### Extraction and rendering are separate capabilities

ostudy no longer has one concrete PDF engine embedded in document lifecycle code.

- `src-tauri/src/pdf_backend.rs` is the structured extraction boundary.
- `src-tauri/src/pdf_render_backend.rs` is the raster page-rendering boundary.
- `src-tauri/src/documents.rs` owns limits, managed paths, hashing, SQLite state, retries, AI orchestration, and canonical authoring.

The active structured extractor is **oxidize-pdf 4.2.2** with the `Rag` extraction profile. OCR features are explicitly disabled. The active renderer is the **pure-Rust justpdf-render 0.1.2 renderer**, accessed through the pinned justpdf 0.1.2 high-level page API.

This split permits future replacement of either capability independently. If compatibility testing discovers PDFs that the pure-Rust renderer cannot faithfully rasterize, a Pdfium-backed compatibility renderer can be added behind the same interface without changing extraction, document intelligence, persistence, provider adapters, backups, or card authoring.

### Semantic intermediate representation

The immutable source PDF remains the durable source of truth. Derived semantic data is stored as ostudy-owned records rather than oxidize-pdf Rust types:

- per-page extracted text;
- typed semantic elements such as title, paragraph, table, list item, image, code block, key/value, header, and footer;
- element page number and bounding box;
- heading path and parent heading;
- font/format signals where available;
- structure-aware semantic chunks with page mappings, element types, heading context, token estimates, and bounding boxes.

Pages for which oxidize-pdf yields no structured text receive a bounded page-evidence chunk. This keeps image-only pages represented in semantic windows without pretending that OCR has occurred. The backend identity is recorded with the derived data. Retry rebuilds semantics when the active extraction/render backend changes.

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

For a fully image-only PDF, a text-only model is not asked to invent semantic structure from placeholder text. Generation requires a previously verified image-capable llama.cpp profile and uses a conservative visual-source outline; rendered page images remain the authoritative evidence for card generation.

The outline is stored independently of the generated cards so later study-guide, quiz, search, and tutoring tasks can reuse it instead of reinterpreting flattened PDF text.

### Outline-guided flashcards and global curation

Every flashcard-generation window receives both its local structured source and the validated global document outline. When the selected llama.cpp profile has verified image input, bounded rendered-page samples are attached so diagrams, tables, equations, and image-only content can contribute evidence. The prompt instructs the model to judge local details against global learning objectives and to create cross-page/integrative questions when supported.

After all windows return locally valid cards, a final bounded relevance pass selects which cards remain. It removes globally minor/hyperspecific material, bibliography/citation artifacts, redundant coverage, and near-duplicates while preserving broad learning-objective coverage and pedagogical variety. The curation model may select or reject a card but cannot rewrite it.

The resulting cards still pass the canonical generated-import validators and are committed directly into an ordinary deck; no review queue is introduced.

### Pure-Rust rendering is the portable default

The renderer rasterizes source pages to PNG at a bounded 144 DPI. It runs in-process and requires no Pdfium, Poppler, headless browser, or platform-specific dynamic library resource.

This is the default because it keeps the five-platform architecture uniform and avoids maintaining a matrix of native renderer binaries for desktop and mobile CPU targets. Renderer output is still bounded by ostudy's per-page byte limit and is persisted with size and SHA-256 metadata before it may be sent to an image-capable provider.

Pdfium remains a plausible optional compatibility backend for difficult desktop PDFs because the renderer boundary is intentionally engine-neutral. It is not a mandatory runtime dependency.

### OCR remains out of scope

oxidize-pdf is built with default features disabled and only `compression` plus `semantic` enabled. `rusty-tesseract` is prohibited by dependency verification. Image-only pages are represented by their rendered page evidence instead of silently storing model transcription as extracted source text.

## Consequences

- Document understanding survives page boundaries and retains substantially more structure than the previous page-string model.
- AI generation requires an outline pass plus a global curation pass, increasing model calls but improving global relevance and creating a reusable artifact for later features.
- The default PDF path has no platform-specific native rendering runtime, reducing Linux/Windows/macOS/Android/iOS packaging complexity and making resource-constrained mobile deployment more practical.
- Difficult PDFs may expose rendering gaps that a mature native engine would handle better; the renderer interface preserves the option of an explicit future Pdfium compatibility backend.
- Existing managed PDFs created by the earlier PR #38 implementation are automatically re-extracted on retry because their recorded backend identities differ from the active oxidize-pdf / justpdf-render IDs.
- Full-library backup continues to require only the database plus source/render managed files; semantic elements, chunks, outlines, and generation-window state are already in SQLite.
