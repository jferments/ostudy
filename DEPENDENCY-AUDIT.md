# Dependency audit

ostudy requires no paid service, hosted database, account system, or cloud runtime. Direct application and development dependencies are pinned exactly. AI features connect only to endpoints that the learner explicitly configures; ostudy does not install or run an LLM server.

## Desktop and frontend

- React / React DOM `19.2.8`
- Tauri JavaScript API `2.11.1` and CLI `2.11.4`
- Tauri dialog/opener plugins `2.7.2` / `2.5.4`
- Tiptap editor packages `3.28.0`
- Vite `7.3.6`, TypeScript `5.8.3`, and esbuild `0.28.1`
- Vitest `4.1.10`
- Testing Library React `16.3.2` and DOM `10.4.1`
- jsdom `29.1.1`

The React testing dependencies run only during development and CI. The native smoke test uses Python's standard library with `tauri-driver`; it does not add a second JavaScript end-to-end framework.

pnpm rejects unreviewed dependency lifecycle scripts. Only the exact esbuild installation script is allowed. Tiptap packages are overridden to one patch version so the editor cannot resolve a mixed internal graph.

No OpenAI, DeepSeek, Ollama, Anthropic, Gemini, or PDF JavaScript SDK is installed. Provider networking and PDF parsing/rendering are not available to the WebView.

## Rust and local data

- Tauri `2.11.5` and build helper `2.6.3`
- `tauri-driver` `2.0.6` as a development tool
- rusqlite `0.40.1` with bundled SQLite `3.53.2`
- serde `1.0.229`, serde_json `1.0.145`, chrono `0.4.45`
- rs-fsrs `1.2.1`
- keyring `4.1.6`
- reqwest `0.13.4` with default features disabled and Rustls/JSON enabled
- url `2.5.7`
- base64 `0.22.1`
- oxidize-pdf `4.2.2`, default features disabled, with `compression` and `semantic`
- image `0.25.10` for PNG encoding of PDFium bitmaps on desktop
- pdfium-bundled `0.1.1+pdfium-7881`, resolving pdfium-render `0.9.3` and PDFium build `7881` on desktop

`rs-fsrs` remains isolated behind `src-tauri/src/scheduler.rs`; durable rows and IPC objects use ostudy-owned types.

`keyring` is isolated behind `src-tauri/src/ai/secrets.rs`. It stores credential bytes in the operating-system credential service when available. SQLite stores only nonsecret provider metadata and credential epochs. An explicit session-only backend keeps a credential in native process memory and never writes it to disk.

`reqwest` is isolated behind `src-tauri/src/ai/gateway.rs`. The client uses Rustls, disabled redirects, bounded response bodies, no cookies, and no raw provider-body logging. Provider adapters use ostudy-owned wire DTOs rather than provider SDKs. The `url` crate owns normalized endpoint parsing and loopback/LAN/cloud policy.

`base64` is used only inside the native document-intelligence task to lower a verified local PNG page into an inline provider content part. The encoded page is transient request data. It is not stored in SQLite, logs, diagnostics, or frontend state.

## PDF processing boundary

PDF processing deliberately uses separate engines for separate jobs:

1. **oxidize-pdf** parses and extracts text/semantic structure;
2. **PDFium** renders the visible PDF into page images;
3. **Tesseract** is the current desktop OCR engine when extracted text fails quality validation.

No third-party PDF/OCR type crosses the document boundary. `pdf_backend.rs`, `pdf_render_backend.rs`, and `pdf_ocr_backend.rs` map those implementations into ostudy-owned pages, chunks, image bytes, OCR text/confidence, and safe error/provenance records.

### Structured PDF extraction

`oxidize-pdf` remains the native structured extraction backend. ostudy uses its RAG extraction profile for reading order, headings, semantic elements, geometry, tables/lists, and structure-aware chunks. Raw per-page text extraction is independent and parallelized with up to 16 workers; each worker opens its own parser document. The structure-aware partition/chunk pass remains canonical/sequential so parallel execution cannot reorder headings or semantic structure.

The crate is compiled with default features disabled. Its optional `rusty-tesseract` wrapper is not enabled and repository dependency checks explicitly reject it. OCR is owned by ostudy's platform OCR service instead.

### PDFium page rendering

Desktop page rendering uses Chromium PDFium through `pdfium-bundled`/`pdfium-render`. PDFium replaces the former justpdf renderer because real malformed-font acceptance documents demonstrated that the lightweight renderer was not a sufficiently faithful OCR raster source.

The desktop build enables `pdfium-bundled`'s bundled mode. During Cargo compilation the crate resolves the target-specific PDFium build, embeds the library in the ostudy binary, and `bind_bundled()` extracts/binds it from the local application cache at runtime. A normal installed ostudy desktop build therefore does not require a separate system `libpdfium` installation or a runtime network download.

Build-time automatic provisioning requires the normal development tools already enforced by `scripts/doctor.sh`, including `curl` and `tar`. Advanced/offline builders may supply the exact PDFium library through the crate's `PDFIUM_BUNDLE_LIB` build environment override rather than allowing the build helper to download it.

Durable visual-grounding pages are rendered at 120 DPI. Temporary OCR pages are rendered independently at 300 DPI and are deleted after recognition. The renderer backend identity is versioned, so Retry automatically rebuilds old justpdf derivatives through PDFium.

PDFium calls remain serialized within one application process. pdfium-render's thread-safe mode serializes calls because upstream PDFium does not guarantee concurrent access, so pretending to parallelize one Pdfium instance would add synchronization without real renderer throughput. CPU scaling is instead applied to independent text extraction and OCR work.

### OCR

Normal PDFs do not require OCR. When native extracted text is corrupt or unusable, ostudy selects only the affected pages and feeds their PDFium 300-DPI renders to the platform OCR service.

Desktop currently uses local Tesseract. Development builds discover `tesseract` from `PATH` or `OSTUDY_TESSERACT_PATH`; release packaging still needs a target-specific Tesseract/trained-data packaging step before desktop distribution is considered self-contained. OCR work is pipelined through a bounded multi-worker pool so several independent Tesseract processes can recognize different pages concurrently while PDFium produces the next renders.

Android/iOS do not spawn the desktop executable. The planned adapters are bundled/offline ML Kit Text Recognition v2 on Android and Apple Vision on iOS, behind the same ostudy OCR contract. Until those native projects are implemented, mobile OCR fails locally before corrupt text can reach an AI provider.

## Document intelligence

The document database stores engine-neutral semantic elements/chunks and a validated hierarchical document outline. Flashcard generation uses overlapping token/page-bounded semantic windows rather than one request per physical page. Image-only pages are preserved as visual-evidence chunks; verified image-capable llama.cpp profiles can use managed PDFium page renders during generation, while text-only profiles are prevented from treating missing text as source facts.

A final model-assisted relevance pass may reject low-value or redundant generated cards, but cannot rewrite or commit them directly. Canonical generated-card authoring remains authoritative for Markdown, Cloze syntax, tags, duplicate fingerprints, scheduling, and transactional persistence.

## Managed files and backups

Card images, immutable source PDFs, and rendered PDF pages live under application-controlled paths. SQLite records their relative paths, sizes, MIME types, and integrity metadata. Full-library backup version 2 validates and packages these managed files alongside one coherent SQLite snapshot. Semantic elements, semantic chunks, outlines, OCR provenance, and generation-window state are included in that snapshot. Credentials and provider request/response bodies remain excluded.

## Lockfile policy

`pnpm-lock.yaml` and `src-tauri/Cargo.lock` are committed source-of-truth files. Normal setup, checks, and releases use frozen/locked dependency resolution. Dependency upgrades should be reviewed separately from feature behavior.

The explicit same-repository normalization action may resolve the exact Cargo graph and apply canonical rustfmt before ordinary qualification. Normal qualification and release commands remain locked and format-clean; ordinary CI is read-only.
