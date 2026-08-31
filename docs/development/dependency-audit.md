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

### Linux qualification display stack

Ubuntu native qualification uses distribution-provided Xvfb, Openbox, and
`x11-utils` only as host-side test infrastructure. Xvfb owns the isolated X11
display, Openbox supplies the EWMH window-management behavior required to
qualify maximize/restore and full-screen, and `x11-utils` supplies `xprop` for
the fail-closed `_NET_SUPPORTING_WM_CHECK` readiness probe. The unprivileged
runner starts Openbox only for the disposable Xvfb session; these packages are
not linked into or packaged with ostudy and add no host mounts, control sockets,
or elevated application privilege.

Openbox is distributed under the GPL-2.0-or-later family. X.Org server and
utility sources carry the MIT/X11-family and other component notices enumerated
by the Ubuntu package copyright files. The canonical Ubuntu 24.04 runner
snapshot and repository provisioning scripts own the installed package
versions and their `/usr/share/doc/*/copyright` records.

### Local font assets

IBM Plex Sans is the default interface and card family for new installations.
It is not a package-manager dependency: `scripts/font-sources.mjs` records the
Google Fonts provisioning source, requested Latin variable face, audited
`OFL-1.1` metadata, and shipped notice. `scripts/install-local-fonts.mjs`
downloads missing WOFF2 assets into the ignored source cache after validating
the HTTPS host, response size, and WOFF2 signature. A pristine source checkout
therefore needs provisioning network access once unless that cache is already
present.

Vite packages locally referenced faces as hashed frontend assets and copies
`public/licenses/IBM-Plex-Sans-OFL-1.1.txt` into the distribution. The
post-build verifier requires every declared hashed WOFF2 asset, its CSS
reference, the matching IBM Plex notice, and the absence of a Google Fonts or
other runtime font import. Installed desktop and Android frontend packages use
those local bytes offline. If the IBM face cannot be read, the semantic family
resolves through the operating-system sans-serif stack; the fallback contract
does not claim identical glyph metrics across platforms.

This audit covers browser UI/card packaging only. IBM Plex faces for the native
Study Guide PDF renderer, including print-specific styles and embedding, remain
Stage 9 work.

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
- pdfium-render `0.9.3`, compiled against PDFium API build `7881`
- sysinfo `0.39.6` for cross-platform available-memory observation used by document concurrency planning

`pdfium-bundled`, justpdf/justpdf-render, and rusty-tesseract are explicitly prohibited by repository dependency verification.

`rs-fsrs` remains isolated behind `src-tauri/src/scheduler.rs`; durable rows and IPC objects use ostudy-owned types.

`keyring` is isolated behind `src-tauri/src/ai/secrets.rs`. It stores credential bytes in the operating-system credential service when available. SQLite stores only nonsecret provider metadata and credential epochs. An explicit session-only backend keeps a credential in native process memory and never writes it to disk.

`reqwest` is isolated behind the bounded modules in `src-tauri/src/ai/gateway/`, with HTTP client construction confined to `gateway/client.rs`. The clients use Rustls, disabled redirects, bounded response bodies, no cookies, and no raw provider-body logging. Provider adapters use ostudy-owned wire DTOs rather than provider SDKs. The `url` crate owns normalized endpoint parsing and loopback/LAN/cloud policy.

`base64` is used only inside the native document-intelligence task to lower a verified local PNG page into an inline provider content part. The encoded page is transient request data. It is not stored in SQLite, logs, diagnostics, or frontend state.

## Native dependency trust boundary

PDF rendering and desktop OCR depend on native software that is intentionally **not downloaded by a Rust dependency or build script**. ostudy owns that provisioning layer under `scripts/` so sources, versions, hashes, target selection, and package-manager behavior are reviewable independently from Cargo.

`scripts/native-deps-manifest.json` pins PDFium API build `7881`, the matching pdfium-render binding version, one immutable archive per supported target, and a SHA-256 digest for every archive. Automated tests verify the build, immutable release URL, expected desktop targets, and digest shapes.

The default precompiled PDFium source is the established third-party `bblanchon/pdfium-binaries` distribution at immutable release `chromium/7881`. It is not represented as an official Google/Foxit distribution. A downloaded archive is never extracted until its SHA-256 matches the committed manifest. The extracted native library is stored under ignored `src-tauri/resources/native/pdfium/` with a local provenance record containing both archive and extracted-library hashes.

Builders who do not want to use that binary distribution can provide their own compatible PDFium `7881` library through `OSTUDY_PDFIUM_PATH` or install PDFium system-wide. Building PDFium from the official source remains the highest-provenance option, but it uses Chromium's depot_tools/GN/Ninja build infrastructure and is therefore not the default developer setup path.

### Platform installers

- Linux/macOS: `scripts/install-native-deps.sh`
- Windows PowerShell: `scripts/install-native-deps.ps1`

`--pdfium-only` provisions only the audited PDFium runtime and is used by build/check/release automation. The default desktop setup also verifies or installs Tesseract.

For Tesseract, ostudy prefers the established operating-system channels documented by the Tesseract project instead of maintaining its own ad-hoc executable bundle:

- Debian/Ubuntu: `tesseract-ocr` + `tesseract-ocr-eng` through apt;
- macOS: Homebrew or MacPorts;
- Windows: the Tesseract-documented UB Mannheim distribution, installed through its winget package when available.

Android/iOS do not use the desktop Tesseract subprocess. Their OCR implementations remain platform-native adapters (planned bundled/offline ML Kit on Android and Apple Vision on iOS).

## PDF processing boundary

PDF processing deliberately uses separate engines for separate jobs:

1. **oxidize-pdf** parses and extracts text/semantic structure;
2. **PDFium** renders the visible PDF into page images;
3. **Tesseract** is the current desktop OCR engine when extracted text fails quality validation.

No third-party PDF/OCR type crosses the document boundary. `pdf_backend.rs`, `pdf_render_backend.rs`, and `pdf_ocr_backend.rs` map those implementations into ostudy-owned pages, chunks, image bytes, OCR text/confidence, and safe error/provenance records.

### Structured PDF extraction

`oxidize-pdf` remains the native structured extraction backend. ostudy uses its RAG extraction profile for reading order, headings, semantic elements, geometry, tables/lists, and structure-aware chunks.

Raw per-page text extraction is independent and parallelized using the shared resource planner. Each worker opens its own parser document, while the canonical structure-aware partition/chunk pass remains sequential so parallel execution cannot reorder headings or semantic structure. Worker selection is bounded by task count, logical CPU count, available RAM, and the optional `OSTUDY_PDF_TEXT_WORKERS` operator cap. There is no hard-coded CPU-count ceiling.

The crate is compiled with default features disabled. Its optional `rusty-tesseract` wrapper is not enabled; OCR is owned by ostudy's platform OCR service instead.

### PDFium page rendering

Desktop rendering uses direct dynamic binding through `pdfium-render`. Runtime lookup order is:

1. `OSTUDY_PDFIUM_PATH` when explicitly configured;
2. the PDFium resource packaged with an installed ostudy build;
3. the audited development runtime under `src-tauri/resources/native/pdfium/`;
4. the application executable directory;
5. the operating-system PDFium search path.

Durable visual-grounding pages are rendered at 120 DPI. Temporary OCR pages are rendered independently at 300 DPI and are deleted after recognition. The renderer backend identity is versioned, so Retry rebuilds derivatives when the renderer contract changes.

PDFium calls remain serialized within one application process. pdfium-render's thread-safe mode serializes calls because upstream PDFium does not guarantee concurrent access. CPU scaling is instead applied to independent extraction and OCR work.

### Resource-aware OCR

Normal PDFs do not require OCR. When native extracted text is corrupt or unusable, ostudy selects only affected pages and feeds their PDFium 300-DPI renders to the platform OCR service.

Desktop Tesseract OCR uses a bounded producer/consumer pipeline, but there is no fixed `8`-worker ceiling. The shared resource planner calculates workers from:

- selected OCR page count;
- logical CPU count;
- currently available RAM;
- a conservative per-worker working-set estimate;
- the optional `OSTUDY_OCR_WORKERS` operator cap.

The planner reserves memory for the rest of the application/system and also sizes the high-resolution PNG queue from remaining RAM. This permits high-core/high-memory machines to exceed eight concurrent OCR workers without turning a large document into an unbounded memory allocation.

## Document intelligence

The document database stores engine-neutral semantic elements/chunks and a validated hierarchical document outline. Flashcard generation uses overlapping token/page-bounded semantic windows rather than one request per physical page. Image-only pages are preserved as visual-evidence chunks; verified image-capable llama.cpp profiles can use managed PDFium page renders during generation, while text-only profiles are prevented from treating missing text as source facts.

A final model-assisted relevance pass may reject low-value or redundant generated cards, but cannot rewrite or commit them directly. Canonical generated-card authoring remains authoritative for Markdown, Cloze syntax, tags, duplicate fingerprints, scheduling, and transactional persistence.

## Managed files and backups

Card images, immutable source PDFs, and rendered PDF pages live under application-controlled paths. SQLite records their relative paths, sizes, MIME types, and integrity metadata. Full-library backup version 2 validates and packages these managed files alongside one coherent SQLite snapshot. Semantic elements, semantic chunks, outlines, OCR provenance, and generation-window state are included in that snapshot. Credentials and provider request/response bodies remain excluded.

## Lockfile policy

`pnpm-lock.yaml` and `src-tauri/Cargo.lock` are committed source-of-truth files. Normal setup, checks, and releases use frozen/locked dependency resolution. Dependency upgrades should be reviewed separately from feature behavior.

The explicit same-repository normalization action may resolve the exact Cargo graph and apply canonical rustfmt before ordinary qualification. Normal qualification and release commands remain locked and format-clean; ordinary CI is read-only.
