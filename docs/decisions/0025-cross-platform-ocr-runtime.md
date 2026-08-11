# ADR 0025: Cross-platform PDF rendering, OCR runtimes, and audited native provisioning

- **Status:** Accepted for the PR #39 document-platform stabilization
- **Date:** 2026-08-10
- **Scope:** PDF rendering/OCR portability and native dependency trust across Linux, Windows, macOS, Android, and iOS

## Context

PR #39 found a real PDF whose visible glyphs are correct but whose embedded font-to-Unicode mapping is malformed. Native extraction and Poppler both return corrupted text, so ostudy must render affected pages and recover text before any LLM request.

The earlier lightweight renderer did not faithfully rasterize every malformed-font page, so desktop rendering moved to Chromium PDFium. A first PDFium implementation used a very new Rust convenience crate that downloaded/bundled PDFium automatically. That made native-runtime acquisition opaque inside the Rust dependency graph and did not meet ostudy's desired trust boundary.

OCR has the same portability problem if it is modeled as a universal command-line executable: desktop Tesseract is appropriate, while Android/iOS should use native on-device OCR APIs.

## Decision

### Native binary acquisition is owned by ostudy, not Cargo dependencies

Rust dependencies may provide bindings, but they must not silently download or select the native PDF/OCR runtime.

Desktop PDFium uses the mature `pdfium-render` Rust bindings directly. The actual PDFium library is provisioned separately by ostudy's reviewed scripts or by the operator/system. Repository checks explicitly reject `pdfium-bundled`.

The audited native manifest is:

```text
scripts/native-deps-manifest.json
```

It pins PDFium API build `7881`, pdfium-render `0.9.3`, one immutable archive name per supported target, a SHA-256 digest for every archive, and the expected native library path inside each archive.

The default precompiled source is the established third-party `bblanchon/pdfium-binaries` project at immutable release `chromium/7881`. It is deliberately documented as third-party, not as an official Google/Foxit binary distribution. ostudy verifies the committed SHA-256 before extracting anything.

For the strongest provenance, an operator may build PDFium from the official Google source and set `OSTUDY_PDFIUM_PATH`, or install a compatible system PDFium. The official PDFium build is Chromium-scale (depot_tools, gclient, GN, Ninja and target toolchains), so it is an optional high-assurance path rather than the default developer bootstrap.

### Runtime lookup order

Desktop `pdf_render_backend` binds PDFium in this order:

1. explicit `OSTUDY_PDFIUM_PATH` file/directory;
2. the Tauri-bundled `native/pdfium` resource;
3. the reviewed development runtime under `src-tauri/resources/native/pdfium`;
4. the executable directory;
5. the normal operating-system PDFium library search path.

A missing/incompatible runtime fails locally with an actionable error before document processing proceeds.

### Platform provisioning scripts

Unix-like hosts use `scripts/install-native-deps.sh`. Windows uses `scripts/install-native-deps.ps1`.

The scripts support PDFium-only provisioning for CI/build/release and the desktop OCR setup path for end-user/developer machines. Downloaded PDFium archives are HTTPS-only, SHA-256 checked before extraction, and staged into a gitignored native-runtime directory. A local provenance file records the immutable source, archive digest, and extracted-library digest so repeated runs can verify and reuse the exact staged binary.

### Tesseract desktop installation uses established OS channels

ostudy does not maintain a custom Tesseract binary bundle.

Desktop setup prefers the installation channels documented by the Tesseract project:

- Debian/Ubuntu: apt (`tesseract-ocr`, `tesseract-ocr-eng`);
- macOS: Homebrew or MacPorts;
- Windows: the Tesseract-documented UB Mannheim distribution, installed through its winget package when available.

`OSTUDY_TESSERACT_PATH` remains available when the executable is not on `PATH`.

### Rendering and OCR are separate platform services

The document ingestion layer owns the platform-neutral sequence:

1. detect unreliable native text;
2. choose affected pages;
3. render selected pages at OCR resolution;
4. invoke a platform OCR provider;
5. receive normalized text/confidence/engine identity;
6. quality-check recovered text again;
7. rebuild semantic coverage and persist provenance;
8. stop before provider traffic if no trustworthy recovery exists.

PDF semantics, SQLite persistence, AI planning, checkpoints, and card generation do not branch on a specific OCR vendor.

### Desktop PDFium rendering

Linux, Windows, and macOS use PDFium through `pdfium-render`.

- durable visual derivatives: 120 DPI;
- temporary OCR rasters: 300 DPI;
- print-quality/text/image/path smoothing enabled;
- native glyph rendering is used even when the PDF text mapping is corrupt.

PDFium calls remain serialized in one process. Upstream PDFium does not guarantee concurrent access, and pdfium-render's thread-safe mode serializes calls. Parallel CPU scaling is therefore placed where work is independent: raw text extraction and page OCR.

### Resource-aware parallelism replaces fixed worker ceilings

There is no fixed `8`-worker OCR limit and no fixed extraction-thread maximum.

The shared Rust resource planner observes logical CPU count, currently available RAM, task/page count, a conservative per-worker working-memory estimate, and an optional operator cap. Worker count is the minimum of those real constraints. High-core/high-memory machines may use more than eight OCR workers. Lower-memory systems reduce concurrency before they swap or OOM.

Current conservative estimates are:

- raw oxidize page extraction: 96 MiB per independent worker;
- desktop Tesseract OCR: 384 MiB per worker;
- queued OCR PNG: bounded by the existing 64 MiB page-image safety limit.

The planner reserves system/application memory and sizes the producer queue from the remaining budget. Operator caps are available through `OSTUDY_OCR_WORKERS` and `OSTUDY_PDF_TEXT_WORKERS`. These values can lower concurrency for debugging/thermal/resource policies; they do not bypass RAM/task safety bounds.

### Android

Android does not spawn the desktop Tesseract executable.

The intended OCR adapter is a Tauri mobile plugin implemented in Kotlin using the bundled/offline ML Kit Text Recognition v2 Latin model. A linked native Tesseract implementation remains an alternate future adapter if distribution constraints change.

The native-dependency manifest already records pinned PDFium Android artifacts for future platform packaging, but PR #39 does not claim the Android renderer bridge is implemented. Mobile rendering/OCR must be qualified as part of the Android port before enabling corrupted-PDF recovery there.

### iOS

iOS does not spawn a command-line Tesseract process.

The intended OCR adapter is Swift + Apple Vision. The manifest records PDFium iOS device/simulator/catalyst artifacts for future renderer packaging, but the current Rust mobile renderer bridge remains pending until the iOS Tauri project exists.

### Engine identity and derivative invalidation

A recovered document persists extraction mode, OCR page counts, confidence, OCR engine identity, extraction backend identity, and render backend identity.

The active desktop render identity is:

```text
pdfium-render-0.9.3/pdfium-api-7881-external-v2-120dpi
```

Changing rendering/extraction contracts invalidates stale derivatives so Retry rebuilds them instead of mixing output from old engines.

## Failure policy

OCR is recovery, never permission to send suspect text onward.

- corrupt non-empty native text requires trustworthy OCR recovery;
- blank/image-only pages may remain visual-only when OCR is unavailable;
- missing native PDF/OCR runtimes return actionable local errors;
- OCR output is quality-checked before semantic planning/provider traffic;
- native artifacts are not used until their pinned digest is verified;
- diagnostics never contain source or OCR text.

## Validation

Before PR #39 merge:

- fast and full desktop qualification must pass on the exact final head;
- repository checks must prove `pdfium-bundled`/justpdf/rusty-tesseract are absent;
- native manifest tests must validate PDFium build, immutable source and SHA-256 pins;
- a clean self-hosted runner must provision and bind the pinned external PDFium library;
- the malformed-font 138-page acceptance PDF must complete PDFium + Tesseract recovery on Linux;
- a normal PDF must remain on native extraction without OCR;
- a many-page OCR run should use the CPU/RAM-selected worker count rather than a fixed ceiling;
- Stop during OCR must terminate active child processes promptly;
- closing ostudy during any active document job must show the native close warning.

During Android/iOS porting:

- add target-specific compile/build CI lanes;
- implement/qualify native renderer and OCR adapters;
- prove corrupt source text never reaches an LLM when a native adapter is unavailable;
- verify offline behavior and packaged runtime/model availability;
- benchmark memory/thermal behavior on representative devices before enabling large automatic OCR jobs by default.

## Consequences

- The Rust graph contains only PDFium bindings, not a hidden native downloader.
- Native runtime trust is explicit, versioned, checksum-audited, and independently replaceable.
- Builders can choose audited precompiled artifacts, system packages, or official-source builds without changing document code.
- OCR/extraction can use large workstations well while remaining RAM-aware on smaller systems.
- Android/iOS remain free to use platform-native OCR rather than inheriting a desktop subprocess architecture.
