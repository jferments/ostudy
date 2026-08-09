# Document platform stabilization testing

This guide qualifies the Stage 4 architecture changes in PR #39 before additional providers or document formats are introduced.

## Local automated gates

During document-platform iteration:

```bash
./scripts/check-changed.sh
```

The fast gate contains the focused cross-layer document/domain/persistence and migration contracts:

```bash
./scripts/check-fast.sh
```

Before requesting merge review:

```bash
./scripts/check-fast.sh
./scripts/check.sh
```

Ordinary pull-request CI is read-only. Do not rely on CI to modify rustfmt output or `Cargo.lock`; if canonical generated Rust candidates are required, use the explicit workflow-dispatch action and review the resulting commit.

## Optional OCR runtime

Normal PDFs do not require an OCR executable. If a PDF has a corrupt/obfuscated text layer, the current fallback needs local Tesseract with English trained data. On Ubuntu/Debian, install and verify it before running the malformed-font acceptance case:

```bash
sudo apt update
sudo apt install tesseract-ocr tesseract-ocr-eng
tesseract --version
```

If Tesseract is installed somewhere that is not on `PATH`, set `OSTUDY_TESSERACT_PATH` to the executable path before launching ostudy.

The current fallback uses English OCR (`eng`). Do not claim automatic multilingual OCR support in PR #39; automatic language selection remains future work.

## Invariant regressions

The automated document lane must prove all of the following:

1. A source descriptor rejects zero source units.
2. A source locator rejects zero/out-of-range/duplicate source-unit numbers.
3. Semantic windows are globally numbered `0..N-1`, including multiple windows inside one partition.
4. The 11-page/single-partition/two-window plan persists two distinct `document_generation_windows` rows.
5. Invalid window identity is rejected before a generation job row is inserted.
6. Provider-reported context can only shrink conservative source windows.
7. A provider context too small for schema/task/output reserves fails locally before traffic.
8. A 138-page-style llama.cpp plan whose aggregate estimate exceeds the former 1.2M local ceiling is accepted and remains window-bounded.
9. llama.cpp source totals above the cloud-only 250k source-token guard are accepted when the source-unit and per-window limits remain valid.
10. DeepSeek/current metered providers still reject conservatively excessive aggregate jobs before provider traffic.
11. Checkpoint contract identity changes with semantic model/capability/planning/source changes but not credential rotation alone.
12. Checkpoint cache write/read failures are non-canonical and degrade to recomputation.
13. Migration 21 upgrades old schemas while preserving foreign-key integrity and exposes OCR extraction metadata.
14. Durable provider adapter keys are no longer closed SQLite enums.
15. PDF ingestion explicitly records `source_format=pdf` and `source_unit_kind=page`.
16. Private Use Area characters are detected as unreliable extracted text.
17. Extreme Latin-1 font substitution like the real `Õâó...` case is detected without flagging ordinary accented prose.
18. Unicode replacement/control corruption is rejected before AI provider traffic.
19. Corrupt structured semantics with trustworthy raw page text use page-text fallback instead of unnecessary OCR.
20. OCR-recovered text replaces corrupt page/semantic content and retains page provenance.
21. Tesseract TSV parsing preserves line boundaries and computes bounded mean word confidence.
22. OCR page rendering uses temporary 300-DPI selected-page renders while durable page derivatives remain 120 DPI.
23. A missing OCR executable produces a safe `document_ocr_unavailable` error only when fallback is actually needed.

## Empirical usage benchmark

The committed baseline corpus is:

```text
src-tauri/tests/fixtures/document-generation/usage-baselines.json
```

It stores only observed aggregate acceptance facts and the required benchmark matrix. Do not add estimated values as if they were observations.

After a real-provider acceptance run, analyze one or more diagnostic logs:

```bash
node scripts/analyze-document-usage.mjs /path/to/ostudy-diagnostic.log
```

Machine-readable form:

```bash
node scripts/analyze-document-usage.mjs --json /path/to/ostudy-diagnostic.log
```

For every benchmark run, record externally in the acceptance notes:

- provider and exact model identifier;
- configured/reported context limit if available;
- source format;
- source-unit count;
- text-only versus multimodal;
- extraction mode (`native`, `page_text_fallback`, or `ocr_fallback`);
- OCR page count and mean confidence when applicable;
- planner source-token estimate;
- outline/generation window counts;
- planner aggregate estimate;
- aggregate budget mode (`windowed` for local llama.cpp or the numeric ceiling for metered providers);
- actual aggregate prompt/completion tokens reported by the provider;
- reasoning/cache token details when available;
- success/failure and the exact safe failure code.

### Required matrix before changing planner coefficients

At minimum collect repeatable observations for:

- small text-only source;
- medium text-only source;
- large text-only source;
- small multimodal source;
- medium multimodal source.

Prefer multiple runs for a provider/model before modifying calibration defaults. Treat provider-declared context as a hard per-request constraint regardless of empirical observations.

## PR #39 real-provider acceptance order

Run expensive tests only after cheaper representative tests pass.

### 1. Normal 11-page llama.cpp / Unsloth regression

Use the exact source/profile that previously produced the generic local database error but whose text layer extracts normally.

Expected:

- no duplicate-window database error;
- at least two generation windows are allowed when source/window bounds require them;
- window identities remain unique;
- `extractionMode=native` and no OCR warning;
- Tesseract is never required for this source;
- generation proceeds through canonical authoring;
- diagnostic output contains no `document_generation_window_insert_failed` event.

### 2. Malformed-font OCR fallback regression

Use the known 138-page Elsevier pharmacology source whose body text renders visually but previously extracted as Private Use Area characters in Poppler and `Õâó...` substitutions in oxidize-pdf.

Expected:

- Retry forces derivative rebuild because the extraction backend identity changed;
- the native detector identifies suspect pages before any document source text is sent to the AI provider;
- Documents switches from normal rendering progress to `Recovering text with OCR (X/Y pages)` when fallback begins;
- only suspect/sparse pages are rendered again at OCR resolution;
- accepted OCR text contains readable English rather than the known PUA/high-Latin substitution pattern;
- `extractionMode=ocr_fallback`, `ocrPageCount > 0`, completed count reaches the candidate count, and mean confidence is persisted;
- the Documents row retains the lower-quality OCR warning after completion/restart;
- the success toast explicitly mentions OCR fallback;
- the prompt/request viewer shows readable recovered English in affected semantic windows;
- the local planner remains `ceiling=windowed`; the old 1.2M aggregate rejection must not return;
- generation continues through bounded semantic windows and commits one canonical deck.

Also verify the negative dependency path once: temporarily run without Tesseract available, retry the corrupt source, and confirm ostudy returns the actionable `document_ocr_unavailable` message while retaining the PDF. Restore Tesseract before the full acceptance run.

### 3. Small DeepSeek baseline

Re-run the ~11-page text source previously observed around ~45k aggregate tokens.

Compare:

- planner estimate;
- actual aggregate token usage;
- number of provider calls;
- any cache-hit behavior;
- final card count/quality.

A large unexplained regression should block the expensive large-source tests.

### 4. Medium local-model source

Re-run the representative ~48-page llama.cpp/Unsloth source.

Verify:

- conservative request windows;
- current model-context capability is honored if available;
- no runaway retry loop;
- interruption can be retried without redoing validated retained work unnecessarily.

### 5. Large local-model windowing/resume

On the 138-page OCR-backed source, verify the large-job behavior independently of extraction:

- **do not reject the document because its aggregate estimate exceeds 1.2M**;
- diagnostics report `ceiling=windowed`;
- the source is processed as multiple bounded outline and generation windows;
- each provider request remains within the conservative/live context-aware window limit;
- completed outline/generation checkpoints are reusable after interruption;
- no recursive compact-retry loop;
- final global outline synthesis remains bounded to the compact digest;
- curation remains bounded in small batches;
- successful generation commits one canonical deck for the original document.

Interrupt once after multiple windows have completed, retry, and verify already validated recovered document derivatives do not re-run OCR unless the extraction backend/source changed.

### 6. Large DeepSeek source

Only after the cheaper cases pass, re-run the ~138-page source that previously consumed roughly 1.45M aggregate DeepSeek tokens before late truncation.

Expected behavior:

- the metered-provider aggregate safety ceiling remains active;
- the planner may reject the job locally before another runaway paid sequence, or a bounded plan that falls below the ceiling may complete;
- completed outline/generation windows can be reused after interruption;
- no recursive compact-retry loop;
- diagnostics provide partition/window/source-unit context without raw source text.

### 7. Generate More

On a successfully generated document:

- Generate More appends to the exact existing deck;
- it reuses the retained final outline;
- it uses the same shared generation executor/state transition rules as initial generation;
- large local documents remain window-bounded rather than aggregate-job-bounded on the second pass too;
- OCR-backed document semantics are reused as retained source data rather than re-OCRing the original PDF;
- it does not create a replacement deck;
- failure/cancellation leaves existing cards intact.

## Malformed-PDF edge checks

In addition to the known font-substitution file, exercise at least representative fixtures or unit cases for:

- a blank/image-only page;
- a page whose native text extractor throws while the file remains renderable;
- corrupt structured metadata with good raw page text;
- a structural partitioner failure with usable native page text;
- U+FFFD replacement characters;
- invalid embedded control characters;
- a mixed PDF where only some pages need OCR;
- a required OCR page whose OCR output remains empty/untrustworthy, which must fail before LLM traffic rather than silently passing corrupt content.

## Lifecycle regression checks

Re-test:

- Stop generation;
- Retry generation;
- Open source PDF;
- Archive/restore document;
- generated-deck deletion followed by regeneration;
- permanent PDF deletion with preserved deck citation;
- interrupted PDF deletion recovery;
- complete backup/restore with source, rendered derivatives, recovered text, and OCR provenance;
- Practice navigation from generated deck.

## Merge criteria

PR #39 should remain draft until the fast and full automated gates pass. Mark it ready only when the code is stable enough for the full qualification gate. Do not merge until the normal 11-page llama.cpp/Unsloth regression, the malformed-font 138-page OCR regression, windowed local resume behavior, and representative DeepSeek/local acceptance tests pass in addition to automated qualification.
