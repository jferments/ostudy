# AI architecture

ostudy keeps AI integration native-only. React requests narrow domain tasks; it never receives provider credentials, raw provider responses, PDF page bytes, or arbitrary model completions. Provider profiles, credential isolation, endpoint trust policy, model discovery, capability evidence, bounded HTTP/SSE transport, and task defaults are implemented under `src-tauri/src/ai/`.

There is no generic prompt IPC command and no silent provider fallback.

## Document intelligence

Managed documents are durable local sources. The active PDF adapter is deliberately split into independent native capabilities:

- `pdf_backend.rs`: native text and structure extraction through oxidize-pdf;
- `pdf_render_backend.rs`: visible-page rasterization through PDFium on desktop;
- `pdf_ocr_backend.rs`: platform-aware OCR provider contract, with Tesseract as the current desktop implementation;
- `document_ingestion.rs`: extraction-quality validation, selective OCR recovery, derivative persistence, and progress;
- `document_semantics.rs`: ostudy-owned semantic chunks and hard-bounded provider windows;
- `ai/document_intelligence.rs`: provider-enforced outline/generation/curation tasks plus deterministic native hierarchy assembly.

No parser, renderer, or OCR library type crosses the document boundary. Those adapters return ostudy-owned text pages, semantic elements/chunks, PNG bytes, OCR text/confidence, and safe provenance. They have no authority to contact an AI provider or author cards.

Derived text, semantic chunks, outline data, OCR recovery, and rendered pages are rebuildable from the immutable managed source. SQLite records extraction/render backend identities so Retry rebuilds stale derivatives after an implementation upgrade.

### Why extraction and rendering use different engines

A PDF may render perfectly while its embedded font-to-Unicode map is broken or deliberately obfuscated. Text extraction therefore asks a different question from visual rendering.

The desktop pipeline uses:

```text
immutable PDF
    │
    ├── oxidize-pdf ──> text + semantic structure
    │                       │
    │                       └── extraction-quality gate
    │                                   │
    └── PDFium ───────> faithful page raster
                                    │
                                    └── Tesseract OCR when native text is unreliable
```

This separation is intentional. oxidize-pdf remains useful for RAG-oriented semantic structure even when one font mapping is corrupt, while PDFium provides a Chromium-grade raster source for visual grounding and OCR.

## PDF extraction and malformed-text recovery

Raw per-page text extraction runs through oxidize-pdf. Independent physical pages may be extracted in parallel with up to 16 workers; each worker opens its own parser document rather than sharing one parser handle. The canonical structure-aware `ExtractionProfile::Rag` partition/chunk pass remains sequential so concurrency cannot reorder headings or semantic relationships.

Extracted text is untrusted until it passes quality validation. The validator detects at least:

- Unicode Private Use Area mappings;
- Unicode replacement characters;
- invalid embedded control characters;
- pathological high-Latin substitution patterns observed in malformed textbook fonts;
- native page extraction exceptions;
- sparse/missing page text;
- corrupt structured semantics even when raw page text remains readable.

Good raw page text with bad semantic structure uses a page-text semantic fallback instead of OCR. Corrupt non-empty raw text requires trustworthy OCR before any provider request. Blank/image-only pages may remain visual evidence when no text is recoverable.

Recovered text passes the same quality boundary again. ostudy does not hard-code a substitution alphabet for a particular PDF/font.

## PDFium rendering

Desktop rendering uses `pdfium-bundled 0.1.1+pdfium-7881`, which resolves `pdfium-render 0.9.3` and PDFium build `7881`.

The `bundled` build path provisions the target-specific PDFium shared library during Cargo compilation, embeds it, and binds the extracted local copy at runtime. A normal desktop ostudy install therefore does not require a separately installed system PDFium library or runtime PDFium download. Controlled/offline builds can provide the exact library through `PDFIUM_BUNDLE_LIB`.

Two raster classes exist:

- **120 DPI durable pages** for visual grounding, backup, and provider image context;
- **300 DPI temporary OCR pages** used only while recognition is running.

PDFium rendering is kept serialized within the process. Upstream PDFium does not guarantee concurrent use; pdfium-render's thread-safe mode serializes calls behind a mutex, so wrapping one Pdfium instance in many page threads would not provide meaningful renderer parallelism.

The render backend identity is versioned as a PDFium identity. Retry automatically rebuilds derivatives that were created by the former justpdf renderer.

## Parallel OCR

Desktop OCR uses Tesseract only after the extraction-quality gate selects affected pages. Normal PDFs never require the OCR runtime.

OCR is pipelined rather than serial:

```text
PDFium 300-DPI producer
          │
          ▼
 bounded high-resolution queue
          │
   ┌──────┼────────┐
   ▼      ▼        ▼
Tesseract Tesseract Tesseract ...
worker 1 worker 2 worker N
```

The worker count is bounded by candidate pages and available CPU parallelism, with a maximum of eight concurrent page workers. PDFium can continue rasterizing while previous pages are recognized, but the queue is deliberately small so a large document cannot accumulate hundreds of 300-DPI PNGs in memory.

Each Tesseract worker has its own provider instance and temporary workspace. Recognition uses explicit 300-DPI input and begins with page-segmentation mode 3. Weak/sparse output can retry with PSM 6 and PSM 11 inside the same bounded page timeout. The best result is selected using substantive text coverage and confidence, then revalidated before semantic reconstruction.

OCR output, PDF source text, provider prompts, credentials, and raw provider responses are not written to diagnostics. Safe metrics may include page number, selected PSM, confidence bucket, recognized-character counts, and whether a fallback layout was used.

Cancellation is exact-document scoped. Stop generation terminates active Tesseract children and prevents partial recovered text from being committed.

### Mobile OCR and rendering

The document core depends on rendering/OCR contracts rather than desktop subprocess assumptions.

- Android: planned native Tauri/Kotlin rendering/OCR adapters; the preferred OCR engine is bundled/offline ML Kit Text Recognition v2.
- iOS: planned native Tauri/Swift rendering/OCR adapters; the preferred OCR engine is Apple Vision.

The mobile projects are not initialized yet. Current Android/iOS cfg branches therefore fail locally before corrupted source reaches an AI provider rather than pretending desktop PDFium/Tesseract binaries are available.

## Hard-bounded semantic source windows

A physical PDF page is provenance/layout, not necessarily a semantic reasoning boundary. Document-aware AI consumes overlapping semantic windows derived from structure-aware chunks.

Provider source windows are deliberately conservative. Large documents scale through **more bounded requests**, not larger individual prompts. A pathological semantic chunk larger than the provider boundary is split into bounded continuation units while retaining original source/page provenance.

Local llama.cpp/Unsloth jobs are **window-bounded, not aggregate-job-bounded**. The former whole-job 1.2M-token ceiling and cloud-only source-token guard do not apply to local execution. Individual requests remain constrained by the configured/reported model context. Metered/cloud providers retain conservative aggregate runaway-cost protection.

## Hierarchical first pass and whole-document view

Before card generation, ostudy requests one concise schema-constrained outline fragment per bounded source window. Fragments contain a summary, learning objectives, concepts, and section/chapter-like source spans.

The hierarchy is assembled deterministically in Rust. Native assembly bounds/sanitizes content, clamps source ranges, repairs reversed ranges, deduplicates objectives/concepts, orders sections by physical position, stitches compatible adjacent sections, and creates a bounded fallback when a fragment omits sections.

ostudy does not recursively send complete outline objects back through an unbounded model merge. A bounded whole-document digest samples coverage from beginning through end; optional schema-constrained global synthesis can improve the summary/objectives/concepts but cannot rewrite source hierarchy. If global synthesis fails, the already-valid native aggregate remains authoritative.

The persisted outline is reusable document intelligence for flashcards today and later study-guide, quiz, search, and tutoring tasks.

## Outline-guided flashcard generation

Every generation request receives a bounded whole-document summary, bounded global objectives/concepts, only the outline sections relevant to the current source window, the bounded structured semantic source, and selected rendered pages when the configured provider has verified image capability.

The task prompt favors durable concepts, mechanisms, comparisons, applications, and supported cross-page relationships. It suppresses bibliography/reference lists, legal/publishing scaffolding, navigation/index content, citation metadata, and low-value trivia.

Generated forms include Basic conceptual questions, definitions, four-option multiple choice with explanation, critical-thinking/application/scenario questions, and Cloze. Definition/MCQ/critical-thinking forms normalize into existing safe Basic cards; Rust constructs Cloze markers from validated exact substrings.

After source windows complete, a bounded curation task selects which cards remain. It cannot rewrite cards. Selected cards pass through canonical generated-card authoring for restricted Markdown/content validation, Cloze syntax, tags, duplicate fingerprints, fresh schedules, and transactional deck/card creation. There is no generated-card review queue.

`Generate more cards` reuses retained extraction/OCR derivatives, outline, and current deck coverage. It should not rerender or OCR an unchanged source.

## Provider-enforced structured output

Prompts describe educational intent, but prompt prose is not the structural guarantee.

### llama.cpp / Unsloth

llama-server receives task JSON Schema through schema-constrained JSON output. Compatible retry forms remain grammar/schema constrained and never silently change providers. The server owns the model chat template; ostudy does not construct tokenizer-specific chat templates in the WebView or document feature.

The document SSE gateway supports ordinary OpenAI-compatible streamed content plus the small compatibility forms required by supported llama-server versions. Output size and schema validation remain native responsibilities.

### DeepSeek

DeepSeek document tasks use strict Function Calling. The native adapter converts internal schemas to DeepSeek's supported strict subset and Rust then enforces tighter length/count/semantic rules. Metered-provider aggregate protection remains active.

### Native validation remains authoritative

Provider structure is necessary but not sufficient. Rust still validates text/tag/card bounds, card direction rules, MCQ options, Cloze exact blanks, outline source ranges, curation indices, restricted Markdown, canonical card structure, duplicates, and scheduling.

A structurally valid response can contain one semantically unusable card; that candidate may be dropped while valid cards from the same bounded window continue. A malformed outer task object fails the request.

## Long-running tasks, close protection, and cancellation

A running document task owns one process-local cancellation signal keyed by document ID. The same registry spans extraction, PDFium rendering, OCR, and provider execution.

**Stop generation** cancels only that document task and drops its exact provider stream/active OCR work. Durable completed checkpoints remain reusable; canceled documents remain retryable.

Application close is also guarded natively. If the main window receives `CloseRequested` while any document task is active, ostudy prevents the close and shows a native warning that the running request will be interrupted and uncommitted work may be lost. **Keep working** leaves the process/job running; **Close ostudy** proceeds with shutdown. No warning appears when no document job is active.

Startup reconciliation marks interrupted durable state retryable and never resumes provider transmission automatically.

## Progress and provenance

The structure extraction step determines the physical page count before durable rasterization. Documents records total page count and updates rendered progress as page PNGs are committed. OCR has separate candidate/completed counts, extraction mode, engine identity, and mean confidence.

The persistent successful-OCR warning appears only after OCR derivatives are committed: completed count must equal candidate count and committed confidence must exist. Failed/in-progress OCR never claims successful recovery.

## Managed document lifetime

Archiving a document is reversible and keeps source PDF, derivatives, and generated deck provenance. Permanent deletion is supported when no generation task is active.

Deletion removes the managed PDF, rendered pages, structured extraction, outline, generation jobs, and document-library provenance. Surviving generated decks remain ordinary study content and retain a detached human-readable source citation when appropriate. Soft-deleted decks are not citation destinations.

The delete workflow stages the managed document directory before the SQLite transaction. A database failure restores it; post-commit filesystem cleanup is best-effort and startup reconciliation removes unreachable staging residue.

## Privacy and trust

PDF import, extraction, semantic chunking, PDFium rendering, OCR, native outline assembly, and checkpoint persistence are local. Only explicit provider requests cross the configured AI endpoint boundary. There is no automatic provider fallback.

Source material is untrusted data and cannot replace system/task instructions. Credentials, Authorization headers, raw provider responses, reasoning content, and OCR/source text are not written to diagnostics.

The original PDF is not uploaded as an opaque file. Bounded source context and selected rendered pages are transmitted according to configured provider capabilities.

## Backup

Complete library backups preserve the coherent SQLite snapshot, managed card images, retained source PDFs, rendered pages, semantic/OCR provenance, outlines, and document-intelligence state. AI credentials remain excluded.

Backups are blocked while a document is actively processing so the database and managed files are captured coherently.

## Extension rule

Future study guides, quizzes, tutoring, document search, and learning-analysis features should reuse the managed document/semantic/outline platform rather than adding feature-specific PDF parsers or generic prompt IPC. New source formats supply ingestion adapters; new platforms supply renderer/OCR adapters; new providers implement the native provider boundary. Task code must never silently substitute another provider.
