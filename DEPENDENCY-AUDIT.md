# Dependency audit

ostudy requires no paid service, hosted database, account system, or cloud runtime. Direct application and development dependencies are pinned exactly. AI features connect only to endpoints that the learner explicitly configures; ostudy does not install or run a model server.

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

pnpm is configured to reject unreviewed dependency lifecycle scripts. Only the exact esbuild installation script is allowed. Tiptap packages are overridden to one patch version so the editor cannot resolve a mixed internal graph.

No OpenAI, DeepSeek, Ollama, Anthropic, Gemini, or PDF JavaScript SDK is installed. Provider networking and PDF parsing are not available to the WebView.

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
- justpdf `0.1.2`

`rs-fsrs` remains isolated behind `src-tauri/src/scheduler.rs`; durable rows and IPC objects use ostudy-owned types.

`keyring` is isolated behind `src-tauri/src/ai/secrets.rs`. It stores credential bytes in the operating-system credential service when available. SQLite stores only nonsecret provider metadata and credential epochs. An explicit session-only backend keeps a credential in native process memory and never writes it to disk.

`reqwest` is isolated behind `src-tauri/src/ai/gateway.rs`. The client uses Rustls, disabled redirects, bounded timeouts and response bodies, no cookies, and no raw provider-body logging. Provider adapters use ostudy-owned wire DTOs rather than provider SDKs. The `url` crate owns normalized endpoint parsing and loopback/LAN/cloud policy.

`base64` is used only inside the native document-flashcard task to lower a verified local PNG page into an inline provider content part. The encoded page is transient request data. It is not stored in SQLite, logs, diagnostics, or frontend state.

`justpdf` is the native PDF parser, text extractor, and page renderer for the managed Documents workflow. The exact crate version and its transitive image/codec graph are captured by `src-tauri/Cargo.lock`. ostudy applies its own source-size, page-count, extracted-text, rendered-image, and generated-card limits around the library.

The PDF path intentionally does not enable or install OCR. It packages no Tesseract, Pdfium, Python, external command-line PDF utility, or platform-specific PDF runtime. Scanned pages can still be rendered; a vision-capable configured model may receive the rendered page image when current capability evidence permits it.

No Python runtime, model weights, CUDA runtime, llama.cpp binary, Ollama binary, or other local inference infrastructure is packaged by these stages.

## Managed files and backups

Card images, immutable source PDFs, and rendered PDF pages live under application-controlled paths. SQLite records their relative paths, sizes, MIME types, and integrity metadata. Full-library backup version 2 validates and packages these managed files alongside one coherent SQLite snapshot. Credentials and provider request/response bodies remain excluded.

## Lockfile policy

`pnpm-lock.yaml` and `src-tauri/Cargo.lock` are committed source-of-truth files. Normal setup, checks, and releases use frozen/locked dependency resolution. Dependency upgrades should be reviewed separately from feature behavior.

The same-repository PR normalization job resolves the exact Cargo graph and applies canonical rustfmt before ordinary qualification. The gate still verifies that the resulting lockfile and source formatting are committed; normal qualification and release commands remain locked and format-clean.
