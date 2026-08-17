# ADR 0035: Structured local diagnostics

## Decision

ostudy uses a bounded, local-only JSONL diagnostics pipeline with stable event names, hierarchical subsystem targets, operation-safe metadata, dynamic filters, and a non-blocking writer queue. Normal releases persist sparse information/warning/error events; debug and trace output is enabled explicitly with `OSTUDY_LOG`, script flags, or a temporary in-app session.

Diagnostics never automatically upload data. Credentials, Authorization headers, prompts, provider responses, document/OCR text, card content, and QR payloads are prohibited fields and are centrally redacted. Current and previous files are each capped at 2 MiB. Queue saturation drops diagnostics rather than blocking study/database operations and records a bounded drop summary.

## Consequences

- Rust, webview, script, and Android boundaries use one event taxonomy.
- `--debug=db,ai,qr`, `--trace=<targets>`, and exact filters work across development and qualification scripts.
- Failure artifacts are bounded and intentionally omit the database and user content.
- Diagnostic logging is never a correctness or transaction boundary.
