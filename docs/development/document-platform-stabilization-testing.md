# Document platform stabilization testing

This guide qualifies the Stage 4 architecture changes in PR #39 before additional providers or document formats are introduced.

## Local automated gates

During document-platform iteration:

```bash
./scripts/check-changed.sh
```

The fast gate contains focused frontend, document/domain/persistence, PDF extraction/render/OCR, and migration contracts:

```bash
./scripts/check-fast.sh
```

Before requesting merge review:

```bash
./scripts/check-fast.sh
./scripts/check.sh
```

Ordinary pull-request CI is read-only. Do not rely on CI to modify rustfmt output or `Cargo.lock`; if canonical generated Rust candidates are required, use the explicit workflow-dispatch action and review the resulting commit.

## PDFium provisioning

Desktop rendering uses the pinned `pdfium-bundled` / `pdfium-render` graph. There is no normal Ubuntu `libpdfium` prerequisite for ostudy development or an installed ostudy build.

A normal build:

```bash
./scripts/build.sh
```

runs Cargo with the committed lockfile. The `pdfium-bundled` build helper obtains the target-specific pinned PDFium build (`7881`) when necessary and embeds it into the native application. At runtime `bind_bundled()` loads the local extracted copy. Runtime document processing does not download PDFium.

The build machine needs the ordinary tools already checked by `scripts/doctor.sh`, including `curl` and `tar`.

For controlled/offline builds, supply the exact PDFium library to Cargo through `PDFIUM_BUNDLE_LIB` before running `./scripts/build.sh`. For Ubuntu x86-64 the corresponding upstream archive for the pinned build is `pdfium-linux-x64.tgz`; for Ubuntu arm64 use `pdfium-linux-arm64.tgz`. The archive must match PDFium build `7881` used by `pdfium-render 0.9.3`.

## Optional desktop OCR runtime

Normal PDFs do not require an OCR executable. If a PDF has a corrupt/obfuscated text layer, desktop fallback needs local Tesseract with English trained data. On Ubuntu/Debian:

```bash
sudo apt update
sudo apt install tesseract-ocr tesseract-ocr-eng
tesseract --version
tesseract --list-langs
```

`eng` must be present. If Tesseract is not on `PATH`, set `OSTUDY_TESSERACT_PATH` before launching ostudy.

The current fallback uses English OCR. Automatic language selection remains future work.

## Architecture invariants

Automated coverage must prove at least the following:

1. Source descriptors/locators reject invalid source-unit identities.
2. Semantic windows have unique global identities before SQLite persistence.
3. Local llama.cpp jobs are window-bounded, not rejected by the former aggregate 1.2M-token ceiling.
4. Metered/cloud providers retain conservative aggregate runaway-cost protection.
5. Checkpoint identity changes with semantic/planning/source changes but not credential rotation alone.
6. PDF ingestion records `source_format=pdf` and `source_unit_kind=page`.
7. oxidize-pdf is the structured extraction backend; raw page extraction worker count is bounded by CPU/page count and at most 16.
8. PDFium is the desktop render backend; renderer identity includes pdfium-render `0.9.3` / PDFium `7881`.
9. Durable page images remain 120 DPI and selected OCR images use temporary 300-DPI renders.
10. PUA/replacement/control/high-Latin corruption is detected before provider traffic.
11. Corrupt semantics with trustworthy raw text use native page-text fallback instead of unnecessary OCR.
12. OCR text replaces corrupt semantic content only after post-OCR validation.
13. Tesseract PSM fallback and TSV parsing retain bounded confidence/provenance.
14. OCR worker count is bounded by available parallelism/candidate count and can schedule multiple independent pages concurrently.
15. A missing Tesseract runtime fails only when required OCR is needed.
16. Failed/in-progress OCR never displays the successful persistent OCR-recovery warning.
17. The active document-task registry exposes whether any document work is running so native close protection can gate application shutdown.
18. Migration 21 and forward upgrade preserve OCR provenance and database integrity.

## Why PDFium rendering is serialized

Do not interpret the absence of a PDFium page worker pool as a performance regression. Upstream PDFium does not guarantee thread-safe concurrent access. pdfium-render's thread-safe mode serializes library calls behind a mutex, so many Rust threads around one Pdfium instance do not create real renderer parallelism.

Performance comes from pipelining independent work:

```text
oxidize raw page extraction: up to 16 independent parser workers
                         │
                         ▼
PDFium rendering: one canonical process-local render lane
                         │
                bounded 300-DPI queue
                         │
          ┌──────────────┼──────────────┐
          ▼              ▼              ▼
     Tesseract       Tesseract      Tesseract ...
     worker 1        worker 2       worker N
```

This avoids retaining every high-resolution page in memory while still allowing OCR to use multiple CPU cores.

## PR #39 real-provider acceptance order

Run expensive tests only after cheaper representative tests pass.

### 1. Normal ~11-page llama.cpp / Unsloth regression

Use the source/profile that previously produced the duplicate generation-window database error but whose text layer extracts normally.

Expected:

- no duplicate-window database error;
- unique generation window identities;
- `extractionMode=native` and no OCR warning;
- Tesseract is not required;
- PDFium renders all source pages;
- generation commits through canonical authoring.

### 2. Malformed-font ~138-page PDFium + OCR regression

Use the known Elsevier pharmacology source whose body text renders visually but extracted as PUA/high-Latin substitutions.

Because the render backend identity changed from justpdf to PDFium, Retry must rebuild retained rendered derivatives automatically.

Expected:

- oxidize-pdf identifies unreliable native text before provider traffic;
- PDFium renders all pages faithfully;
- Documents progresses through normal rendering and then `Recovering text with OCR (X/Y pages)`;
- 300-DPI OCR renders are produced only for selected pages;
- several Tesseract page workers execute concurrently on a multi-page fallback (verify with `htop`, `ps`, or equivalent if desired);
- accepted OCR text contains readable English rather than PUA/high-Latin substitution output;
- completed OCR count reaches the selected page count and mean confidence is persisted;
- the persistent lower-quality OCR warning appears only after successful recovery;
- source windows in the llama.cpp/Unsloth request viewer contain readable recovered text;
- local diagnostics remain `ceiling=windowed`; the old aggregate rejection does not return;
- generation continues through bounded windows and commits one canonical deck.

If OCR still rejects a page, capture only safe diagnostics such as `document_ocr_layout_selected`, `document_ocr_text_rejected`, and the PDFium render error/status. Do not add raw source/OCR text to diagnostic logs.

### 3. Application-close protection

Run a PDF generation and test close behavior during each long phase that is practical:

- oxidize extraction;
- PDFium rendering;
- Tesseract OCR;
- an active LLM request.

Expected whenever the exact-document task registry is active:

- the normal close request is prevented;
- a native warning titled `PDF processing is still running` appears;
- the warning explains that the running request will be interrupted and uncommitted progress can be lost;
- **Keep working** leaves the application and job running;
- **Close ostudy** exits the application;
- after a confirmed close, restart reconciliation marks interrupted durable state retryable and never resumes provider traffic automatically.

With no document job active, application close should remain normal and should not display the warning.

### 4. Stop during parallel OCR

During a many-page OCR fallback, click **Stop generation**.

Expected:

- active OCR workers observe the exact-document cancellation state;
- active Tesseract children are terminated promptly;
- no corrupt/partial OCR text is committed;
- the retained PDF remains retryable.

### 5. Small DeepSeek baseline

Re-run the representative small text source. Compare planner estimate, actual aggregate usage, provider calls, final card quality, and any cache behavior. A large unexplained regression blocks the more expensive tests.

### 6. Large local-model windowing/resume

On the 138-page OCR-backed source:

- do not reject because aggregate estimate exceeds the former 1.2M local ceiling;
- each provider request remains within conservative/live context limits;
- diagnostics report `ceiling=windowed`;
- completed checkpoints are reusable after interruption;
- recovered PDFium/OCR derivatives are reused after a later AI interruption rather than OCR being repeated;
- curation/global synthesis remain bounded;
- one canonical deck is committed.

### 7. Generate More

On a successfully generated document:

- Generate More appends to the exact existing deck;
- it reuses retained outline and OCR-backed semantics;
- it does not rerender/re-OCR an unchanged source;
- cancellation/failure leaves existing cards intact.

### 8. Large DeepSeek source

Only after cheaper cases pass, test the large metered provider case. The aggregate metered-provider safety ceiling must remain active.

## Malformed-PDF edge checks

Exercise representative cases for:

- blank/image-only page;
- native extraction exception while PDFium can still render the page;
- corrupt semantic structure with good raw page text;
- structural partition failure with usable native page text;
- U+FFFD and invalid controls;
- mixed PDF where only some pages need OCR;
- required OCR page whose Tesseract output remains untrustworthy;
- malformed font resources that oxidize cannot map but PDFium can visibly render;
- page rotations/crop boxes and tables/figures where renderer fidelity matters.

## Lifecycle regression checks

Re-test:

- Stop / Retry generation;
- Open source PDF;
- Archive/restore document;
- generated-deck deletion followed by regeneration;
- permanent PDF deletion with preserved deck citation;
- interrupted PDF deletion recovery;
- backup/restore with source, PDFium page derivatives, recovered text, and OCR provenance;
- Practice navigation from generated deck.

## Merge criteria

PR #39 remains draft until both automated gates pass on the exact final head. Do not merge until the normal 11-page regression, malformed-font 138-page PDFium/OCR recovery, close warning, parallel OCR/cancellation, windowed local resume behavior, and representative provider acceptance tests pass.
