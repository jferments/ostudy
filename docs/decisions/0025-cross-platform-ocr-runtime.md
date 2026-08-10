# ADR 0025: Cross-platform OCR runtime, PDFium raster source, and platform-native mobile adapters

- **Status:** Accepted for the PR #39 document-platform stabilization
- **Date:** 2026-08-09
- **Updated:** 2026-08-10
- **Scope:** OCR fallback portability and faithful PDF rendering across Linux, Windows, macOS, Android, and iOS

## Context

PR #39 added extraction-quality validation after real acceptance testing found a PDF whose visible glyphs were correct but whose text layer used malformed/private font-to-Unicode mappings. Native extraction and Poppler both returned corrupted text, so ostudy must render affected pages and recover text with OCR before any LLM request.

The first recovery implementation used Tesseract against a justpdf raster. The OCR engine was functional, but real testing showed that a clean digital PDF could render correctly in ordinary readers while the lightweight raster path still produced pages from which Tesseract could not recover trustworthy text. OCR is only as good as the pixels it receives, so renderer fidelity is part of the recovery contract.

A Tesseract child-process architecture is also not an acceptable universal mobile contract. Tauri mobile applications integrate native functionality through Kotlin/Java on Android and Swift on iOS. The document pipeline therefore must depend on ostudy rendering/OCR contracts, not on one PDF renderer or the presence of a `tesseract` executable.

## Decision

### Separate extraction, rendering, and OCR engines

The active desktop PDF pipeline intentionally uses different engines for different responsibilities:

1. **oxidize-pdf** extracts native text and semantic structure;
2. **PDFium** rasterizes the visible page;
3. **Tesseract** recognizes text from a PDFium raster when native extraction is unreliable.

These are adapters behind ostudy-owned contracts. Document identity, SQLite persistence, semantic planning, checkpoints, provider execution, and card authoring do not depend on a concrete parser/renderer/OCR vendor.

### PDFium is the canonical desktop raster source

Linux, Windows, and macOS use PDFium through `pdfium-bundled` / `pdfium-render` for both durable page renders and temporary OCR renders.

- durable page derivatives: 120 DPI;
- temporary OCR derivatives: 300 DPI;
- print-quality PDFium rendering with smoothing enabled;
- PDFium native text-rendering optimization disabled so visible glyphs are rasterized by the normal PDFium page renderer rather than depending on an alternate native text path.

The renderer backend identity is versioned. A document that records the former justpdf backend automatically rebuilds derivatives from its immutable source PDF on Retry.

`pdfium-bundled` is compiled in bundled mode on supported desktop targets. Cargo's build helper obtains the pinned PDFium build (`7881`) at build time when it is not already supplied, embeds it, and `bind_bundled()` loads the extracted local copy at runtime. Runtime document processing therefore does not depend on a system PDFium package or a network connection.

The build environment already requires `curl` and `tar`. An offline/controlled builder may supply the exact shared library through `PDFIUM_BUNDLE_LIB` during compilation.

### PDFium concurrency policy

Pdfium itself does not guarantee concurrent access. pdfium-render's thread-safe mode serializes Pdfium calls, so running many threads against one process-local Pdfium instance does not provide meaningful render parallelism and can create false expectations about throughput.

ostudy therefore keeps PDFium rendering bounded and serialized within a document. CPU scaling is applied around it:

- raw oxidize page text extraction uses independent parser documents across up to 16 workers;
- selected 300-DPI PDFium pages flow into a bounded OCR queue;
- up to 8 independent Tesseract worker processes recognize separate pages concurrently.

This producer/consumer design avoids retaining every high-resolution page in memory while still allowing OCR to occupy multiple CPU cores.

### OCR is a platform service, not a Tesseract assumption

The document ingestion layer owns these platform-neutral steps:

1. detect unreliable native text;
2. choose only affected pages;
3. request a high-fidelity OCR-resolution render;
4. request OCR with language/options/cancellation context;
5. receive normalized text, confidence, engine identity, and image dimensions;
6. validate recovered text again;
7. rebuild semantic coverage and persist extraction provenance;
8. stop before provider traffic if no trustworthy recovery exists.

The OCR provider is replaceable behind that contract. PDF semantics, SQLite persistence, AI planning, checkpoints, and card generation must not branch on a particular OCR vendor.

### Desktop: Tesseract adapter

Linux, Windows, and macOS use the current Tesseract adapter through oxidize-pdf's `OcrProvider` interface.

Development builds may discover `tesseract` from `PATH` or `OSTUDY_TESSERACT_PATH`. Release packaging should bundle a target-specific Tesseract sidecar plus required trained-data assets rather than making ordinary users install a system package manually.

Desktop OCR keeps:

- local/offline recognition;
- selected 300-DPI PDFium page renders;
- bounded multi-page worker concurrency;
- bounded TSV output;
- private temporary workspaces;
- exact-document cancellation that kills active child processes promptly;
- engine/confidence provenance;
- multipass PSM 3/6/11 recognition within one bounded per-page timeout when the default layout pass is weak.

### Android: native bundled on-device recognizer

Android must **not** spawn the desktop Tesseract executable.

The intended Android adapter is a Tauri mobile plugin implemented in Kotlin, using a statically bundled on-device text-recognition SDK. The initial target is the bundled ML Kit Text Recognition v2 Latin model rather than the Google-Play-services/downloaded variant, because the recovery path must be available offline immediately and must not depend on Play Services being present.

The Kotlin adapter will accept rendered page bytes from the Rust/document boundary and return the shared ostudy OCR result. It must preserve cancellation, text/confidence normalization, language/script identity, and bounded output semantics.

If a future Android distribution cannot or should not include the bundled ML Kit dependency, the same contract may instead be backed by a linked native Tesseract/libtess implementation without changing document ingestion.

The mobile PDF renderer is a separate target concern. PR #39 does not force the desktop bundled-PDFium binary model onto Android.

### iOS: Apple Vision text recognition

iOS must **not** spawn or package a command-line Tesseract process.

The intended iOS adapter is a Tauri mobile plugin implemented in Swift using Apple's Vision text-recognition API. Vision performs on-device image text recognition and returns recognized text observations with confidence information, fitting the existing ostudy OCR result contract without introducing an external executable.

The Swift adapter will receive the same platform-rendered page input and return normalized text/confidence/provenance through the same Rust-facing OCR interface.

The mobile PDF renderer is likewise target-specific; PR #39 does not claim the desktop PDFium bundle already provides an iOS renderer.

### Current PR #39 mobile behavior

The repository does not yet contain initialized Android/iOS Tauri native projects, so PR #39 does not pretend those native SDK integrations already exist.

Current target gating is explicit:

- desktop targets: bundled PDFium rendering + Tesseract CLI OCR;
- Android/iOS targets: desktop PDFium/Tesseract subprocess assumptions are disabled and the missing native adapter reports a local configuration/unavailable error before any AI provider sees corrupted text.

Adding the actual Kotlin/Swift renderer/OCR adapters belongs with the mobile platform port, when generated projects and target-specific CI lanes exist.

## Engine identity and derivative invalidation

A recovered document persists extraction mode, OCR page counts, confidence, OCR engine identity, extraction backend identity, and render backend identity. A parser/renderer/OCR change that can alter recovered semantic text must bump the appropriate contract identity so retained derivatives/checkpoints are rebuilt instead of mixing output produced by different pipelines.

Backups may legitimately carry already-recovered canonical document text from one platform to another. Re-OCR is required only when the active extraction/render contract changes or derivatives are incomplete/corrupt.

## Application close policy

An active document task spans local extraction, rendering, OCR, and provider execution. Closing the application interrupts the task and can invalidate work that has not reached a durable checkpoint.

The native Tauri `CloseRequested` path checks the same exact-document task registry used by cancellation. If any document job is active, the close request is prevented and a native warning asks whether to close ostudy or keep working. Confirming close force-destroys the main window; declining leaves the document task running. Normal close behavior is unchanged when no document task is active.

Startup reconciliation continues to mark any interrupted durable document/job state as retryable rather than automatically resuming provider traffic.

## Language policy

PR #39 uses English OCR for the acceptance source. The cross-platform contract carries language/script options, but automatic language detection is not part of this stage.

Future platform adapters should expose a common language-capability layer rather than leaking Tesseract language codes, Apple Vision language identifiers, or ML Kit script SDK names into document ingestion.

## Failure policy

OCR is a recovery mechanism, never permission to send suspect text onward.

- Corrupt non-empty native text requires trustworthy OCR recovery.
- Blank/image-only pages may remain visual-only when OCR is unavailable.
- A required OCR backend that is absent returns a platform-appropriate local error.
- Recovered text is quality-checked again before semantic planning/provider traffic.
- Diagnostics never contain source text or OCR text.

## Validation

Before PR #39 merge:

- fast and full desktop qualification must pass on the exact final head;
- the malformed-font 138-page acceptance PDF must exercise real PDFium + Tesseract recovery on Linux;
- a normal PDF must remain on native extraction without invoking OCR;
- multiple Tesseract workers must execute concurrently on a many-page OCR fallback;
- Stop during OCR must terminate active work promptly;
- application close during local processing or provider execution must show the native warning and Keep working must preserve the running task;
- accepted OCR source windows must contain readable text rather than PUA/high-Latin corruption.

During Android/iOS porting:

- add target-specific compile/build CI lanes;
- add native rendering/OCR adapter tests using the same recovery fixtures;
- prove corrupted source text never reaches an LLM when a native adapter is unavailable;
- verify offline behavior and packaged model/framework availability;
- benchmark OCR accuracy and memory on representative phone/tablet hardware before enabling large-document automatic OCR by default.

## Consequences

- Document AI is less sensitive to malformed font/text-layer PDFs because visual rasterization is delegated to PDFium rather than the lightweight renderer.
- The document core remains engine-neutral despite using three specialized desktop engines.
- Parallel OCR uses real independent processes instead of pretending PDFium itself scales through threads.
- Desktop builds provision the pinned PDFium runtime automatically at build time.
- Android/iOS avoid an unnatural subprocess/sidecar model and remain free to use platform-native render/OCR engines.
- The system fails safely on a platform whose native recovery adapter has not shipped yet.
