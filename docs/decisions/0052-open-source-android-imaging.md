# ADR 0052: Keep Android QR scanning and OCR local and open source

- **Status:** Accepted
- **Date:** 2026-09-17
- **Pull request:** #101

## Context

The interactive-question release contract requires application-controlled non-AI functionality to use free/open-source software and to remain locally executable. The inherited Android implementations violated that dependency policy even though they executed on-device: QR pairing used Google Code Scanner / Google Play services and document OCR used Google ML Kit Text Recognition.

The shared Rust architecture already isolates both functions behind narrow platform ports. Replacing the Android implementations therefore does not require changing device-sync cryptography, document ingestion, durable checkpoints, SQLite ownership or React application semantics.

## Decision

Android uses local open-source implementations for both capabilities.

### QR pairing

Use JourneyApps **ZXing Android Embedded 4.3.0** with **ZXing core 3.5.4**. Both are Apache-2.0 projects. oStudy owns the Android camera permission and launches the scanner through Tauri's Activity Result boundary.

The scanner:

- requests QR format only;
- disables barcode-image retention;
- bounds returned text to 16 KiB;
- guards concurrent/duplicate terminal callbacks;
- returns only decoded text or cancellation to the existing Rust scanner contract; and
- never logs decoded pairing payloads.

Rust remains authoritative for pairing-envelope parsing, protocol/expiry checks and cryptographic validation. A camera is optional hardware because LAN text invitations and direct USB remain separate supported paths.

### Document OCR

Use **Tesseract4Android 4.9.0** (Apache-2.0), whose current native dependency set includes Tesseract OCR 5.5.1, Leptonica 1.85.0, libjpeg v9f and libpng 1.6.48. Their upstream licenses remain independently applicable and must be included in release notice generation/packaging where required.

The initial Android build packages only English `tessdata_fast`. To make the model reproducible rather than accepting a mutable download, Gradle fetches it from immutable `tesseract-ocr/tessdata_fast` commit `87416418657359cb625c412a48b6e1d6d41c29bd`, requires exact byte size `4,113,088`, and verifies Git blob SHA-1 `bbef4675053b5b468cdb477053e28b1c698ba08e` before placing it in the generated asset tree.

The APK contains `assets/tessdata/eng.traineddata`. Recognition does not download a model or contact a hosted OCR service. On first use the packaged file is copied to app-private storage, the only location Tesseract reads. Requests for an unbundled language fail explicitly rather than falling back to a network provider.

The existing byte, dimension, pixel and timeout budgets remain in the Android adapter. Rust still owns selective page choice, cancellation/checkpoint policy, semantic document processing and publication.

## Build and supply-chain boundary

JitPack is used only as the Maven distribution path for the pinned open-source Tesseract4Android release. It is a build-time artifact source, not a runtime application service.

Repository verification enforces the exact dependency/model boundary:

- `verify-android-qr-scanner.mjs` requires the pinned ZXing/Tesseract versions and model identity and rejects Google Code Scanner/ML Kit text-recognition references;
- `android-test.sh` verifies the resolved Gradle graph and compiles the private Kotlin plugin;
- `verify-android-package.sh` requires camera permission, rejects broad storage/install permissions, requires packaged `eng.traineddata`, and retains native 16 KiB alignment checks; and
- emulator smoke exercises real packaged OCR while physical-device acceptance covers camera decoding and touch/device behavior that an emulator cannot faithfully represent.

No model or dependency is fetched by the application at runtime.

## Consequences

- ADR 0026 (ML Kit OCR) and ADR 0036 (Google Code Scanner) are superseded.
- QR pairing and OCR no longer require proprietary Google imaging runtimes.
- Android QR now requires the ordinary camera permission when the user invokes scanning.
- Offline OCR remains immediately available after installation for English; adding another language requires an explicit packaged-model change and the same immutable provenance review.
- APK/native footprint increases because Tesseract and its native image-processing dependencies are bundled. This is accepted in exchange for the local FOSS boundary and reproducible offline behavior.
- The Rust/frontend contracts do not depend on ZXing or Tesseract, so another qualified local implementation can replace either adapter later without changing the application model.
