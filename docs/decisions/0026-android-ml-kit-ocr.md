# ADR 0026: Use ML Kit for Android document OCR

- Status: **Superseded by ADR 0052**
- Date: 2026-08-15
- Scope: Android PDF-page OCR

## Historical decision

This decision originally selected bundled ML Kit Text Recognition v2 for Android OCR while keeping OCR behind the shared platform boundary.

PR #101 introduced a stricter dependency requirement for application-controlled non-AI functionality: it must use free/open-source software and remain locally executable. On-device execution alone is not sufficient when the runtime dependency itself is proprietary. ML Kit therefore no longer satisfies the accepted interactive-question release contract.

The current Android implementation and rationale are defined by [ADR 0052](0052-open-source-android-imaging.md): Tesseract4Android with a pinned packaged `tessdata_fast` English model replaces ML Kit, while the Rust-facing OCR contract and selective document-pipeline behavior remain intact.

This ADR is retained only to record why the older implementation existed; it is not the current architecture.
