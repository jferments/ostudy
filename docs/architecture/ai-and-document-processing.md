# AI architecture

ostudy keeps AI integration native-only. React requests narrow domain tasks; it never receives provider credentials, raw provider responses, PDF page bytes, or arbitrary model completions. Provider profiles, credential isolation, endpoint trust policy, model discovery, capability evidence, bounded HTTP/SSE transport, and task defaults are implemented under `src-tauri/src/ai/`.

There is no generic prompt IPC command and no silent provider fallback.

## Document intelligence

Managed documents are durable local sources. The active PDF adapter is deliberately split into independent native capabilities:

- `pdf_backend.rs`: native text and structure extraction through oxidize-pdf;
- `pdf_render_backend.rs`: visible-page rasterization through externally provisioned PDFium on desktop and Android `PdfRenderer` on Android;
- `pdf_ocr_backend.rs`: platform-aware OCR provider contract, with Tesseract CLI on desktop and bundled ML Kit Text Recognition v2 on Android;
- `document_ingestion.rs`: extraction-quality validation, selective OCR recovery, derivative persistence, and progress;
- `system_resources.rs`: CPU/RAM-aware concurrency planning for independent extraction/OCR work;
- `document_semantics.rs`: ostudy-owned semantic chunks and hard-bounded provider windows;
- `ai/document_intelligence.rs`: provider-enforced outline/generation/curation tasks plus deterministic native hierarchy assembly.

No parser, renderer, or OCR library type crosses the document boundary. Those adapters return ostudy-owned text pages, semantic elements/chunks, PNG bytes, OCR text/confidence, and safe provenance. They have no authority to contact an AI provider or author cards.

Derived text, semantic chunks, outline data, OCR recovery, and rendered pages are rebuildable from the immutable managed source. SQLite records extraction/render backend identities so Retry rebuilds stale derivatives after an implementation upgrade.

### Why extraction and rendering use different engines

A PDF may render perfectly while its embedded font-to-Unicode map is broken or deliberately obfuscated. Text extraction therefore asks a different question from visual rendering.

```text
immutable PDF
    │
    ├── oxidize-pdf ──> text + semantic structure
    │                       │
    │                       └── extraction-quality gate
    │                                   │
    └── platform raster ──> faithful page image
          │
          ├── desktop: PDFium
          └── Android: PdfRenderer
                                    │
                                    └── platform OCR when native text is unreliable
```

This separation is intentional. oxidize-pdf remains useful for RAG-oriented semantic structure even when one font mapping is corrupt, while the platform renderer supplies a visual source for grounding and OCR.

## PDF extraction and malformed-text recovery

Raw per-page text extraction runs through oxidize-pdf. Independent physical pages may be extracted concurrently; each worker opens its own parser document rather than sharing one parser handle. Worker count is chosen from task count, logical CPUs, currently available RAM, a conservative 96 MiB per-worker estimate, and optional `OSTUDY_PDF_TEXT_WORKERS` cap. There is no hard-coded extraction worker ceiling.

The canonical structure-aware `ExtractionProfile::Rag` partition/chunk pass remains sequential so concurrency cannot reorder headings or semantic relationships.

Extracted text is untrusted until it passes quality validation. The validator detects at least Private Use Area mappings, replacement characters, invalid controls, the observed pathological high-Latin substitution pattern, native page extraction exceptions, sparse/missing page text, and corrupt structured semantics when raw page text remains readable.

Good raw page text with bad semantic structure uses page-text semantic fallback instead of OCR. The quality system deliberately has two levels: localized malformed glyph mappings invalidate structured semantics and are removed from the native page-text fallback, while only page-wide unusability may become an OCR candidate. **Readable native text is never replaced merely because it is short, image-heavy, table-heavy, or contains a minority of unmapped scientific-symbol/font glyphs.**

Desktop builds can ask PDFium for a second independent native-text extraction before OCR. If that rescue produces usable text, it is cleaned and used for semantic fallback. Android `PdfRenderer` does not expose an equivalent text layer, so an Android page whose primary native extraction is genuinely unusable is rasterized and sent directly to the bounded ML Kit OCR adapter. In both cases OCR remains reserved for pages where trustworthy native text is unavailable. Blank/image-only pages may remain visual evidence when no text is recoverable. Recovered text passes the same quality boundary again; ostudy does not hard-code a substitution alphabet for one PDF/font.

## Rendering backends and native runtime trust

### Desktop PDFium

Desktop rendering binds directly through `pdfium-render 0.9.3` against PDFium API build `7881`. The Rust dependency graph does **not** contain `pdfium-bundled` or another native-runtime downloader.

Native runtime acquisition belongs to reviewed ostudy scripts and the audited `scripts/native-deps-manifest.json`. The default binary path uses the immutable `bblanchon/pdfium-binaries` release `chromium/7881`, with a committed SHA-256 for every platform archive. An archive is not extracted until its digest matches the manifest. The staged native library is gitignored and has a provenance record containing the archive/source and extracted-library hash.

Builders can avoid the default binary distribution by setting `OSTUDY_PDFIUM_PATH` to a compatible system or self-built PDFium library. Runtime resolution also checks the packaged Tauri resource, the reviewed development staging directory, the executable directory, and finally the OS library search path.

Official-source PDFium builds are supported operationally through `OSTUDY_PDFIUM_PATH`, but are not the default bootstrap because PDFium uses Chromium's depot_tools/gclient/GN/Ninja build infrastructure.

Two desktop raster classes exist:

- **120 DPI durable pages** for visual grounding, backup, and provider image context;
- **300 DPI temporary OCR pages** used only while recognition is running.

PDFium rendering stays serialized within the process. Upstream PDFium does not guarantee concurrent calls; pdfium-render's thread-safe mode serializes access. CPU scaling is instead applied to independent extraction and OCR tasks.

The desktop render backend identity is versioned as `pdfium-render-0.9.3/pdfium-api-7881-external-v2-120dpi`, so Retry rebuilds derivatives created by earlier render contracts.

### Android PdfRenderer

Android does not load the desktop PDFium runtime. `OstudyDocumentPlugin` opens only app-private managed PDFs with Android `PdfRenderer`, rasterizes the requested bounded page into a white-background bitmap, encodes a PNG into app-private cache, and returns page number/dimensions/byte count to Rust. It enforces the same document/page safety boundary before the image can enter OCR or provider planning.

The Android renderer is deliberately narrow: Kotlin renders pages but does not select OCR candidates, read/write learner data, create cards, or contact providers. The active Android render identity is versioned separately so stale derivatives can be invalidated after adapter changes.

## Resource-aware OCR

### Desktop Tesseract

Desktop OCR uses Tesseract only after the extraction-quality gate selects affected pages. Normal PDFs never require OCR.

```text
PDFium 300-DPI producer
          │
          ▼
 RAM-bounded high-resolution queue
          │
   ┌──────┼────────┐
   ▼      ▼        ▼
Tesseract Tesseract Tesseract ...
worker 1 worker 2 worker N
```

There is no fixed eight-worker ceiling. The shared resource planner chooses `N` from candidate count, logical CPU count, available RAM, a conservative 384 MiB per-worker estimate, and the optional `OSTUDY_OCR_WORKERS` cap. It reserves memory for the application/system and sizes the high-resolution PNG queue from the remaining budget and the 64 MiB per-image safety limit. A high-core/high-memory workstation can therefore run more than eight OCR workers; a constrained machine automatically selects fewer.

Each Tesseract worker has its own provider instance and temporary workspace. Recognition uses explicit 300-DPI input and begins with page-segmentation mode 3. Weak/sparse output can retry with PSM 6 and PSM 11 inside the same bounded page timeout. The best result is selected using substantive text coverage and confidence, then revalidated before semantic reconstruction.

Desktop cancellation terminates active Tesseract children and prevents partial recovered text from being committed.

### Android ML Kit

Android packages the bundled ML Kit Text Recognition v2 Latin model and has no Tesseract/Tesseract4Android/tessdata lifecycle. Rust stages a bounded PNG in app-private cache and calls the private Android plugin. The plugin validates source size, decoded dimensions, pixel count, and timeout before recognition, closes the recognizer after each request, and returns text/confidence/language observations/dimensions/duration plus a versioned ML Kit engine identity.

Cancellation remains exact-document scoped in shared Rust. If cancellation occurs while a native recognition call is already executing, the task result is discarded and no canceled recovered text is committed or sent to a provider.

OCR/source text, provider prompts, credentials, and raw provider responses are not written to diagnostics. Safe metrics may include page number, engine identity, confidence bucket, recognized-character counts, CPU/RAM planning inputs, selected worker count, and queue capacity.

### Desktop native dependency installation

Linux/macOS use `scripts/install-native-deps.sh`; Windows uses `scripts/install-native-deps.ps1`.

PDFium is version/hash pinned by ostudy. Tesseract is intentionally installed through established platform channels instead of a custom ostudy binary bundle: apt on Debian/Ubuntu, Homebrew/MacPorts on macOS, and the Tesseract-documented UB Mannheim Windows distribution via winget when available. `OSTUDY_TESSERACT_PATH` supports non-PATH installations.

### iOS

iOS remains planned. A future native Tauri/Swift adapter can use Apple Vision behind the same OCR contract without changing ingestion, persistence, or generation code. Until the iOS project and adapter exist, iOS must fail locally before corrupted source reaches an AI provider rather than pretending the Android or desktop runtime is available.

## Hard-bounded semantic source windows

A physical PDF page is provenance/layout, not necessarily a semantic reasoning boundary. Document-aware AI consumes overlapping semantic windows derived from structure-aware chunks.

Provider source windows are deliberately conservative and provider-neutral. Heading changes remain semantic hierarchy metadata but do **not** force request boundaries. Large-context models can pack more adjacent source units into each bounded request; smaller-context models automatically receive smaller windows and therefore more requests. A pathological semantic chunk larger than the provider boundary is split into bounded continuation units while retaining original source/page provenance.

All providers are **window-bounded, not aggregate-job-bounded**. Whole-document token estimates are diagnostic/cost information rather than a provider-specific rejection rule. Every individual request must still fit the configured/reported model context after prompt, schema, response, image, and calibrated safety reserves.

## Hierarchical first pass and whole-document view

Before card generation, ostudy requests one concise schema-constrained outline fragment per bounded source window. Request packing is capacity-driven rather than heading-driven, so a heading-dense textbook does not degenerate into one provider call per section or page. Fragments still carry the headings, summary, learning objectives, concepts, and source spans needed to reconstruct the hierarchy.

The hierarchy is assembled deterministically in Rust. Native assembly bounds/sanitizes content, clamps source ranges, repairs reversed ranges, deduplicates objectives/concepts, orders sections by physical position, stitches compatible adjacent sections, and creates a bounded fallback when a fragment omits sections.

ostudy does not recursively send complete outline objects back through an unbounded model merge. A bounded whole-document digest samples coverage from beginning through end; optional schema-constrained global synthesis can improve the summary/objectives/concepts but cannot rewrite source hierarchy. If global synthesis fails, the already-valid native aggregate remains authoritative.

The persisted outline is reusable document intelligence for flashcards today and later study-guide, quiz, search, and tutoring tasks.

## Outline-guided flashcard generation

Every generation request receives a bounded whole-document summary, bounded global objectives/concepts, only outline sections relevant to the current source window, the bounded structured semantic source, and selected rendered pages when the configured provider has verified image capability.

The task prompt favors durable concepts, mechanisms, comparisons, applications, and supported cross-page relationships. It suppresses bibliography/reference lists, legal/publishing scaffolding, navigation/index content, citation metadata, and low-value trivia.

Generated forms include Basic conceptual questions, definitions, four-option multiple choice with explanation, critical-thinking/application/scenario questions, and Cloze. Definition/MCQ/critical-thinking forms normalize into existing safe Basic cards; Rust constructs Cloze markers from validated exact substrings.

After source windows complete, bounded curation batches select which cards remain. Cloud batches may overlap under the same four-request ceiling, but their results are consumed in original batch/index order; local/LAN curation remains serial. It cannot rewrite cards. Curation is also a semantic pair-quality gate: an answer must directly and completely answer its question, multi-part questions require all requested parts, headings/topic labels are not accepted as factual answers, and unsupported or ambiguous cards are dropped. Even a single generated candidate passes through this gate. Selected cards then pass through canonical generated-card authoring for restricted Markdown/content validation, Cloze syntax, tags, duplicate fingerprints, fresh schedules, and transactional deck/card creation. There is no generated-card review queue.

`Generate more cards` reuses retained extraction/OCR derivatives, outline, and current deck coverage. It should not rerender or OCR an unchanged source.

## Provider-enforced structured output

Prompts describe educational intent, but prompt prose is not the structural guarantee.

### llama.cpp / Unsloth

llama-server receives task JSON Schema through schema-constrained JSON output. Compatible retry forms remain grammar/schema constrained and never silently change providers. The server owns the model chat template; ostudy does not construct tokenizer-specific chat templates in the WebView or document feature.

### DeepSeek

DeepSeek document tasks use strict Function Calling. The native adapter converts internal schemas to DeepSeek's supported strict subset and Rust enforces tighter length/count/semantic rules afterward. Large jobs use the same per-request context/window bounds as other providers.

### Native validation remains authoritative

Provider structure is necessary but not sufficient. Rust still validates text/tag/card bounds, card direction rules, MCQ options, Cloze exact blanks, outline source ranges, curation indices, restricted Markdown, canonical card structure, duplicates, and scheduling.

A structurally valid response can contain one semantically unusable card; that candidate may be dropped while valid cards from the same bounded window continue. A malformed outer task object fails the request.

## Long-running tasks, close protection, and cancellation

A running document task owns one process-local cancellation signal keyed by document ID. The same registry spans extraction, platform rendering, OCR, and provider execution. Independent cloud outline/generation/curation batches execute with bounded concurrency of up to four; local and LAN model servers remain serial unless a future verified capacity capability says otherwise. Network work may finish out of order, but checkpoints, final card ordering, and SQLite writes remain deterministic. Provider resilience uses one shared inactivity contract rather than provider-name deadlines or total request timers: the profile timeout bounds silence between response headers/SSE events, while heartbeats, reasoning deltas, output deltas, tool arguments, and final usage keep an active request alive indefinitely.

**Stop generation** cancels only that document task and drops its exact provider stream/active OCR result. Durable completed checkpoints remain reusable; canceled documents remain retryable.

On desktop, application close is guarded natively. If the main window receives `CloseRequested` while any document task is active, ostudy prevents the close and shows a native warning that the running request will be interrupted and uncommitted work may be lost. **Keep working** leaves the process/job running; **Close ostudy** proceeds with shutdown. No warning appears when no document job is active.

On Android, a user-started document task acquires a native foreground `dataSync` service lease with a visible notification and bounded partial wake lock. Backgrounding the Activity therefore does not make the WebView the task owner. Full process death/reboot still ends the native process; startup/document reconciliation uses durable checkpoints and never resumes provider transmission invisibly.

## Progress and provenance

The structure extraction step determines the physical page count before durable rasterization. Documents records total page count and updates rendered progress as page PNGs are committed. OCR has separate candidate/completed counts, extraction mode, engine identity, and mean confidence.

The persistent successful-OCR warning appears only after OCR derivatives are committed: completed count must equal candidate count and committed confidence must exist. Failed/in-progress OCR never claims successful recovery.

## Managed document lifetime

Archiving a document is reversible and keeps source PDF, derivatives, and generated deck provenance. Permanent deletion is supported when no generation task is active.

Deletion removes the managed PDF, rendered pages, structured extraction, outline, generation jobs, and document-library provenance. Surviving generated decks remain ordinary study content and retain a detached human-readable source citation when appropriate. Soft-deleted decks are not citation destinations.

The delete workflow stages the managed document directory before the SQLite transaction. A database failure restores it; post-commit filesystem cleanup is best-effort and startup reconciliation removes unreachable staging residue.

## Privacy and trust

PDF import, extraction, semantic chunking, platform rendering, OCR, native outline assembly, and checkpoint persistence are local. Only explicit provider requests cross the configured AI endpoint boundary. There is no automatic provider fallback.

Source material is untrusted data and cannot replace system/task instructions. Credentials, Authorization headers, raw provider responses, reasoning content, and OCR/source text are not written to diagnostics.

The original PDF is not uploaded as an opaque file. Bounded source context and selected rendered pages are transmitted according to configured provider capabilities.

## Backup

Complete library backups preserve the coherent SQLite snapshot, managed card images, retained source PDFs, rendered pages, semantic/OCR provenance, outlines, and document-intelligence state. AI credentials remain excluded.

Backups are blocked while a document is actively processing so the database and managed files are captured coherently.

## Extension rule

Future study guides, quizzes, tutoring, document search, and learning-analysis features should reuse the managed document/semantic/outline platform rather than adding feature-specific PDF parsers or generic prompt IPC. New source formats supply ingestion adapters; new platforms supply renderer/OCR adapters; new providers implement the native provider boundary. Task code must never silently substitute another provider.
