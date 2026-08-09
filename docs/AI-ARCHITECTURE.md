# AI architecture

ostudy keeps AI integration native-only. React requests narrow domain tasks; it never receives provider credentials or arbitrary model completions. Provider profiles, credential isolation, endpoint trust policy, model discovery, capability evidence, bounded HTTP, and task defaults are implemented under `src-tauri/src/ai/`.

## Document intelligence

Managed PDFs are local durable sources. PDF interpretation is split into two independent native capabilities:

- `pdf_backend.rs`: structure-aware extraction through oxidize-pdf;
- `pdf_render_backend.rs`: bounded raster page rendering through pure-Rust justpdf-render.

The extraction boundary maps third-party parser output into an ostudy-owned semantic intermediate representation containing page text, typed semantic elements, geometry, heading relationships, and structure-aware chunks. The renderer only returns PNG page bytes. Neither backend can mutate cards, contact an AI provider, or own database lifecycle state.

Structured derived data is persisted in SQLite and is rebuildable from the immutable managed source PDF. The database records the active extraction and render backend IDs so retry can rebuild documents after a backend upgrade.

### Hierarchical analysis

A physical PDF page is provenance/layout, not a semantic reasoning boundary. Document-aware AI therefore consumes overlapping semantic windows constructed from oxidize-pdf chunks. Windows are bounded by estimated tokens and distinct pages, preserve whole chunks, and deliberately overlap so concepts are not severed at a window edge.

Before card generation, ostudy creates a durable hierarchical document outline. Large documents are analyzed in bounded semantic windows and their validated outline fragments are recursively merged. The final outline contains a global summary, major learning objectives, key concepts, and sections with page spans and per-section learning objectives.

That outline is a reusable document artifact. Flashcard generation consumes it today; future study-guide, quiz, search, and tutoring tasks should consume the same stored outline and semantic chunks instead of independently flattening/reinterpreting the source PDF.

Pages with no recoverable text/structure remain represented by explicit page-evidence chunks. A text-only profile cannot treat those markers as source facts. A fully image-only PDF requires a verified image-capable llama.cpp task profile; rendered pages are then the authoritative evidence and no OCR transcript is inserted into the extracted-text store.

### Outline-guided flashcard generation

Every generation window receives:

- the validated global document outline;
- a contiguous structured semantic source window that can span many pages;
- selected rendered page images when the configured llama.cpp profile has current image-input capability evidence.

The task prompt ranks local details against global learning objectives and explicitly prefers cross-page/integrative cards when a concept spans page boundaries. It suppresses references, publishing/legal material, navigation/index content, citation metadata, and hyperspecific trivia that does not contribute to understanding.

Generated forms include Basic conceptual questions, definitions, four-option multiple choice with explanation, critical-thinking/application/scenario questions, and Cloze. Definition/MCQ/critical-thinking forms normalize into existing safe Basic cards; Cloze blank markers are constructed by Rust from validated exact substrings.

After all semantic windows complete, a bounded global curation task selects which cards remain. It cannot rewrite cards. Its job is to remove globally minor material, redundant coverage, near-duplicates, and document artifacts while preserving broad learning-objective coverage and useful pedagogical variety.

Selected cards then pass through the canonical generated-card authoring path for Markdown/content validation, Cloze syntax, tags, duplicate fingerprints, fresh schedules, and transactional deck creation. There is no generated-card review queue.

### Long-running requests, activity timeout, and cancellation

Document AI uses streamed Chat Completions rather than treating the profile timeout as a total generation deadline. Model discovery and explicit connection probes keep their ordinary bounded request behavior.

For a document task, the selected profile's timeout is an **initial-response/activity timeout**. It covers connection establishment, response headers, and the wait for the first streamed response bytes. llama.cpp requests enable prompt-progress/SSE ping events where supported; DeepSeek streamed requests can supply provider keep-alive comments. Once any provider stream activity is received, ostudy does not impose a fixed wall-clock completion deadline. A slow local model may therefore continue generating as long as the stream remains open.

The running document task owns a process-local cancellation signal keyed by document ID. **Stop generation** cancels only that task and drops its exact HTTP response stream. This is the common cancellation boundary for the implemented Chat Completions providers; ostudy does not invent a provider-side job ID, status-poll API, or undocumented cancellation endpoint. A stopped document remains durable and retryable, and the durable job is recorded as canceled rather than succeeded.

Cancellation state is intentionally not restored after application restart. Startup reconciliation marks interrupted work retryable and never resumes provider transmission automatically.

## Managed document lifetime

Archiving a document is reversible and keeps the source PDF, derived data, and generated deck provenance. Permanent deletion is also supported when no generation task is active.

Deletion removes the managed PDF, rendered pages, structured extraction, outline, generation jobs, and document-library provenance. Generated decks/cards remain ordinary study content. Before the document row is removed, ostudy preserves the source as a human-readable text citation in every surviving generated deck description. The file-dependent provenance rows can then cascade safely without leaving surviving study content dependent on a path that no longer exists.

The delete workflow stages the managed document directory into an application-controlled trash path before the SQLite transaction. A database failure restores the directory. After a successful transaction, filesystem cleanup is best-effort and startup reconciliation removes unreachable delete-staging residue.

## Privacy and trust

PDF import, extraction, semantic chunking, outline persistence, and page rendering are local. Provider networking begins only when a configured AI task runs. No provider fallback is automatic. The source material is always treated as untrusted data and cannot replace system/task instructions.

Credentials, Authorization headers, raw provider responses, and reasoning content are not persisted. Provider requests run outside SQLite transactions. The original PDF is not uploaded as an opaque file; bounded source text/structure and selected page images are sent according to the configured provider capability and network scope.

The WebView never receives a managed PDF filesystem path merely to open it. `open_document_source` validates the document identity/path in Rust and invokes the operating-system opener from the native side, so the frontend does not require a generic `opener:allow-open-path` capability.

## OCR policy

ostudy does not enable oxidize-pdf's OCR/Tesseract integration. The direct dependency uses `default-features = false` with only `compression` and `semantic`. Dependency qualification fails if `rusty-tesseract` enters the graph. Image-only pages remain renderable and available to verified multimodal providers, but model-derived transcription is not silently substituted for extracted source text.

## Portable page rendering

The active raster backend uses justpdf 0.1.2 / justpdf-render 0.1.2 at a bounded 144 DPI. It is an in-process Rust dependency rather than a separately distributed native PDF runtime. Builds therefore do not need to provision or bundle Pdfium/Poppler/browser binaries for each desktop and mobile CPU target.

This portable renderer is the default for Linux, Windows, macOS, Android, and iOS. The boundary remains swappable: if a compatibility corpus demonstrates rendering gaps on difficult PDFs, a future optional Pdfium desktop backend can be introduced without changing semantic extraction, durable document storage, provider contracts, backups, or card authoring.
