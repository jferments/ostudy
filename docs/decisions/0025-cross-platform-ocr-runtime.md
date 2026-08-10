# ADR 0025: Cross-platform OCR runtime and platform-native mobile adapters

- **Status:** Accepted for the PR #39 document-platform stabilization
- **Date:** 2026-08-09
- **Scope:** OCR fallback portability across Linux, Windows, macOS, Android, and iOS

## Context

PR #39 added extraction-quality validation after real acceptance testing found a PDF whose visible glyphs were correct but whose text layer used malformed/private font-to-Unicode mappings. Native extraction and Poppler both returned corrupted text, so ostudy must render affected pages and recover text with OCR before any LLM request.

The first recovery implementation used a Tesseract command-line process. That is appropriate for desktop development, but a child-process architecture is not an acceptable universal mobile contract. Tauri mobile applications integrate native functionality through Kotlin/Java on Android and Swift on iOS. The document pipeline therefore must depend on an ostudy OCR contract, not on the presence of a `tesseract` executable.

## Decision

### OCR is a platform service, not a PDF/Tesseract assumption

The document ingestion layer owns these platform-neutral steps:

1. detect unreliable native text;
2. choose only affected pages;
3. render selected pages at OCR resolution;
4. request OCR for PNG bytes with language/options/cancellation context;
5. receive normalized text, confidence, engine identity, and image dimensions;
6. validate recovered text again;
7. rebuild semantic coverage and persist extraction provenance;
8. stop before provider traffic if no trustworthy recovery exists.

The OCR provider is replaceable behind that contract. PDF semantics, SQLite persistence, AI planning, checkpoints, and card generation must not branch on a particular OCR vendor.

### Desktop: Tesseract adapter

Linux, Windows, and macOS use the current Tesseract adapter through oxidize-pdf's `OcrProvider` interface.

Development builds may discover `tesseract` from `PATH` or `OSTUDY_TESSERACT_PATH`. Release packaging should bundle a target-specific Tesseract sidecar plus the required trained-data assets rather than making ordinary users install a system package manually.

Desktop OCR keeps:

- local/offline recognition;
- selected 300-DPI temporary page renders;
- bounded TSV output;
- private temporary workspaces;
- exact-document cancellation that kills the child process promptly;
- engine/confidence provenance.

### Android: native bundled on-device recognizer

Android must **not** spawn the desktop Tesseract executable.

The intended Android adapter is a Tauri mobile plugin implemented in Kotlin, using a statically bundled on-device text-recognition SDK. The initial target is the bundled ML Kit Text Recognition v2 Latin model rather than the Google-Play-services/downloaded variant, because the recovery path must be available offline immediately and must not depend on Play Services being present.

The Kotlin adapter will accept rendered page bytes from the Rust/document boundary and return the shared ostudy OCR result. It must preserve cancellation, text/confidence normalization, language/script identity, and bounded output semantics.

If a future Android distribution cannot or should not include the bundled ML Kit dependency, the same contract may instead be backed by a linked native Tesseract/libtess implementation without changing document ingestion.

### iOS: Apple Vision text recognition

iOS must **not** spawn or package a command-line Tesseract process.

The intended iOS adapter is a Tauri mobile plugin implemented in Swift using Apple's Vision text-recognition API. Vision performs on-device image text recognition and returns recognized text observations with confidence information, fitting the existing ostudy OCR result contract without introducing an external executable.

The Swift adapter will receive the same rendered page input and return normalized text/confidence/provenance through the same Rust-facing OCR interface.

### Current PR #39 mobile behavior

The repository does not yet contain initialized Android/iOS Tauri native projects, so PR #39 does not pretend those native SDK integrations already exist.

Instead, the current OCR backend is target-gated:

- desktop targets: Tesseract CLI is implemented and available when its executable/trained data can be found;
- Android/iOS targets: subprocess execution is disabled and OCR reports a local configuration/unavailable error before any AI provider sees corrupted text.

This makes the current code safe to compile as the mobile ports are introduced and creates one explicit integration point for the native adapters. Adding the actual Kotlin/Swift code belongs with the mobile platform port, when those generated projects and target-specific CI lanes exist.

## Engine identity and derivative invalidation

A recovered document persists `extraction_mode`, OCR page counts, confidence, and OCR engine identity. A future OCR-engine or normalization change that can alter recovered semantic text must also bump the document extraction-backend contract identity so retained derivatives/checkpoints are rebuilt instead of mixing semantic output produced by different extraction contracts.

Backups may legitimately carry already-recovered canonical document text from one platform to another. Re-OCR is only required when the extraction contract changes or derivatives are incomplete/corrupt.

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

- desktop fast and full qualification must pass;
- the malformed-font 138-page acceptance PDF must exercise real Tesseract recovery on Linux;
- a normal PDF must remain on native extraction without invoking OCR;
- Stop during OCR must terminate promptly;
- the practice-navigation test must wait for the active exit-guard precondition rather than racing React effect timing.

During Android/iOS porting:

- add target-specific compile/build CI lanes;
- add native adapter tests using the same OCR contract fixtures;
- prove corrupted source text never reaches an LLM when the native OCR adapter is unavailable;
- verify offline behavior and packaged model/framework availability;
- benchmark OCR accuracy and memory on representative phone/tablet hardware before enabling large-document automatic OCR by default.

## Consequences

- Document AI stays portable even though OCR implementations differ by OS.
- Desktop development can use mature Tesseract immediately.
- Android/iOS avoid an unnatural subprocess/sidecar model.
- Mobile package size and native SDK work are isolated to the mobile adapters rather than contaminating the document core.
- The system fails safely on a platform whose OCR adapter has not been shipped yet.
