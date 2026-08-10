# Document platform stabilization testing

This guide qualifies the Stage 4 architecture changes in PR #39 before additional providers or document formats are introduced.

## Native runtime setup

Native PDF software is provisioned outside Cargo. `pdfium-bundled` is prohibited.

On Linux/macOS:

```bash
./scripts/install-native-deps.sh --with-tesseract
```

On Windows PowerShell:

```powershell
./scripts/install-native-deps.ps1 -WithTesseract
```

The installer reads `scripts/native-deps-manifest.json`, downloads only the pinned PDFium 7881 artifact for the selected platform, verifies its committed SHA-256 before extraction, and stages the native library under the ignored `src-tauri/resources/native/pdfium/` directory. It also verifies/installs desktop Tesseract through established OS channels.

To use a system or self-built PDFium instead, set `OSTUDY_PDFIUM_PATH` to the compatible PDFium 7881 library or its containing directory before starting ostudy. The standard build/check scripts use `--pdfium-only`, so they never require Tesseract unless the real application actually needs OCR.

## Local automated gates

During iteration:

```bash
./scripts/check-changed.sh
```

Required before handoff/review:

```bash
./scripts/check-fast.sh
./scripts/check.sh
./scripts/build.sh
```

Both fast and full checks must pass on the same exact source head. Build/check automation provisions and verifies PDFium itself; a developer should not need a system `libpdfium` package for the normal audited-binary path.

Ordinary pull-request CI is read-only. If the Cargo graph or rustfmt output changes, use the repository's explicit normalization action and review the resulting commit before qualification.

## Native dependency trust tests

Automated tests and manifest verification must prove:

1. `pdfium-bundled`, justpdf/justpdf-render, and rusty-tesseract are absent from the resolved Rust graph.
2. `pdfium-render` is pinned to `0.9.3` with the PDFium `7881` API feature.
3. `scripts/native-deps-manifest.json` pins PDFium build 7881 and an immutable release URL.
4. Every native artifact has a 64-hex SHA-256 digest.
5. Linux x64/arm64, macOS x64/arm64, and Windows x64/arm64 assets are present in the manifest.
6. A clean CI runner can download, verify, extract, bind, and render the real PDF fixture through the staged library.
7. An altered/mismatched archive digest fails before extraction.

The default binary source is a third-party distribution and must remain described as such. The official Google PDFium source-build path is an allowed higher-provenance alternative through `OSTUDY_PDFIUM_PATH`.

## Resource-aware concurrency

Document CPU parallelism is selected by `system_resources.rs`; there is no fixed OCR worker maximum.

Worker plans consider:

- logical CPU count;
- currently available RAM;
- number of pages/tasks;
- conservative per-worker memory estimates;
- optional operator caps.

Current estimates are 96 MiB per independent oxidize page-text worker and 384 MiB per Tesseract worker. OCR queue capacity is also limited by remaining memory and the 64 MiB per-image safety bound.

Optional caps:

```bash
export OSTUDY_OCR_WORKERS=4
export OSTUDY_PDF_TEXT_WORKERS=4
```

These are debugging/resource caps, not performance requirements. With no cap, a high-core/high-RAM system may select more than eight OCR workers.

Automated resource-planner tests must cover CPU-limited, RAM-limited, task-limited, explicit-cap, missing-memory-information, and queue-memory cases.

## PDF/OCR invariant regressions

The automated document/PDF lane must prove:

1. source descriptors/locators reject invalid source-unit identities;
2. semantic windows have globally unique identities before SQLite persistence;
3. local llama.cpp large documents remain per-window bounded instead of hitting the old aggregate 1.2M ceiling;
4. metered/cloud providers retain aggregate runaway-cost protection;
5. Private Use Area, replacement/control corruption, and the observed high-Latin substitution pattern are rejected before provider traffic;
6. normal accented prose is not falsely rejected;
7. corrupt semantic structure with good raw text uses native page fallback, not OCR;
8. OCR-recovered text replaces corrupt page/semantic content and retains page provenance;
9. cross-page semantic recovery does not drop clean neighboring source coverage;
10. Tesseract TSV parsing retains line boundaries/confidence;
11. OCR PSM fallback selects the strongest validated result;
12. durable page renders remain 120 DPI and OCR renders 300 DPI;
13. the external PDFium renderer processes the real test fixture;
14. a required missing OCR provider fails locally before AI traffic;
15. Stop/cancellation is visible to blocking OCR workers;
16. the active-document task registry drives native close protection;
17. Migration 21 preserves OCR/extraction metadata and forward upgrades remain integral.

## Real acceptance: malformed 138-page PDF

Use the known Elsevier pharmacology PDF whose visible body text is clean but whose text mapping produced PUA/high-Latin garbage.

Expected sequence:

1. **Retry generation** rebuilds old rendering derivatives because the PDFium render backend identity changed.
2. oxidize-pdf identifies corrupt text pages before source text reaches the AI provider.
3. PDFium renders normal durable pages and 300-DPI OCR pages without the previous justpdf page failure.
4. the Documents UI changes to `Recovering text with OCR (X/Y pages)`.
5. safe diagnostic `document_ocr_parallel_started` reports CPU/RAM inputs, selected worker count, and queue capacity.
6. On a capable workstation, multiple Tesseract processes should run concurrently; the count may exceed eight when CPUs/RAM permit.
7. OCR progresses beyond the formerly failing page 8.
8. accepted OCR text is readable English and does not contain the known PUA/high-Latin substitution pattern.
9. the request viewer shows readable recovered text in affected semantic windows.
10. `extractionMode=ocr_fallback`, completed OCR count reaches the candidate count, and mean confidence is persisted.
11. the persistent OCR warning appears only after successful committed recovery.
12. local planner diagnostics remain `ceiling=windowed`.
13. the document completes through bounded outline/generation windows and commits one canonical deck.

During that run, inspect CPU utilization and optionally:

```bash
pgrep -a tesseract
```

The selected worker count is resource-dependent; do not assert a particular count without checking the `document_ocr_parallel_started` diagnostic.

## Stop/retry and close behavior

During a many-page OCR run:

- press **Stop generation** and verify active Tesseract children terminate promptly;
- Retry must remain available;
- completed/committed document derivatives should be reused when valid.

During extraction, PDFium rendering, OCR, and later provider execution, attempt to close the main ostudy window.

Expected:

- **Keep working** prevents close and leaves the job running;
- **Close ostudy** exits;
- reopening reconciles interrupted durable state to a retryable state rather than silently resuming provider traffic;
- closing with no active document job shows no processing warning.

## Normal-source regression

Re-run the ~11-page source whose text layer is healthy.

Expected:

- PDFium renders successfully;
- `extractionMode=native`;
- Tesseract is not required or launched;
- no duplicate generation-window database error;
- generation completes through canonical authoring.

## Provider / resume regression order

After the PDFium/OCR case passes:

1. small DeepSeek baseline;
2. representative medium llama.cpp/Unsloth source;
3. large local-model interruption/resume using the 138-page source;
4. Generate More on the same successfully processed document;
5. expensive large DeepSeek case last.

Verify throughout that local sources are window-bounded, cloud cost ceilings remain intact, completed checkpoints are reused, and no recursive retry loop appears.

## Lifecycle regression checks

Spot-check:

- Open source PDF;
- archive/restore document;
- generated-deck deletion followed by regeneration;
- permanent PDF deletion with preserved detached citation;
- interrupted PDF deletion reconciliation;
- complete backup/restore with source, rendered derivatives, recovered text and OCR provenance;
- Practice navigation from a generated deck.

## Merge criteria

PR #39 remains draft until both automated gates pass on the exact final head. Do not merge until the real 138-page PDFium/OCR test, native close warning, normal-source regression, representative local/provider resume behavior, and essential lifecycle checks pass.

After acceptance, mark PR #39 Ready for review one final time, let the unchanged head complete the full self-hosted gate, then squash-merge into `main`.
