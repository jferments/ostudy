# ADR 0026: Use ML Kit for Android document OCR

- Status: Accepted
- Date: 2026-08-15
- Scope: Android PDF-page OCR

## Decision

ostudy keeps OCR behind the shared platform boundary and chooses the implementation per operating system.

The Android implementation uses bundled **ML Kit Text Recognition v2** as its only OCR engine. Android does not package Tesseract, Tesseract4Android, a Tesseract sidecar, or separately managed `tessdata` files.

The document pipeline remains selective:

1. Preserve usable text extracted directly from the PDF.
2. Render only pages that require visual recovery.
3. Send those bounded PNG pages to the Android ML Kit adapter.
4. Return normalized text, language observations, confidence, image dimensions, elapsed time, and a versioned engine identifier to Rust.
5. Keep page limits, cancellation, durable checkpoints, AI generation, and SQLite transactions in shared Rust code.

Desktop platforms may continue using the existing Tesseract CLI adapter. A future Apple implementation can use its own qualified native adapter without changing the document-ingestion contract.

## Why Android does not use a Tauri Tesseract sidecar

Tauri sidecars are a useful option for reproducible desktop executables, but Android is already providing OCR through a maintained mobile SDK with a compact Kotlin boundary. Shipping Tesseract on Android would add another native dependency, ABI packaging, trained-data provisioning, timeout/cancellation plumbing, and an additional recognition-quality arbitration layer.

Using ML Kit alone removes that Android-specific maintenance burden while preserving the same Rust-facing OCR contract.

## Android behavior

The Android platform plugin depends on the bundled Text Recognition v2 artifact (`com.google.mlkit:text-recognition`). The model is packaged with the application, so ordinary recognition does not depend on downloading a model at first use.

The native adapter:

- accepts only app-private image paths prepared by shared Rust;
- enforces byte, dimension, pixel, and timeout bounds before recognition;
- closes the recognizer after each bounded request;
- reports the precise ML Kit engine identifier with the result;
- never exposes Android filesystem paths to React.

## Consequences

- Android has one document OCR implementation instead of a Tesseract/ML Kit fallback chain.
- No JitPack repository, Tesseract native library, `tessdata` download task, or trained-data copy lifecycle is required.
- APK contents and Android build logic are simpler.
- OCR remains available offline after installation through the bundled ML Kit model.
- Desktop OCR remains independently replaceable because the shared application depends on the platform OCR contract rather than ML Kit directly.
