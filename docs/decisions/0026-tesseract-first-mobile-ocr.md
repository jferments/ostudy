# ADR 0026: Tesseract-first OCR with platform-native fallback

- Status: Accepted
- Date: 2026-08-15
- Scope: PDF page OCR on desktop, Android, and future Apple targets

## Decision

ostudy uses a provider chain rather than a single cross-platform OCR binary:

1. Preserve usable embedded PDF text without OCR.
2. Use Tesseract as the first OCR engine wherever the target can package and
   execute it reliably.
3. Fall back to the platform-native recognizer only when Tesseract cannot
   initialize, times out, errors, or returns a result below the bounded quality
   threshold.
4. Persist the actual engine and model identity with processing diagnostics.

The initial platform matrix is:

| Platform | Primary OCR | Fallback |
| --- | --- | --- |
| Linux, Windows, macOS desktop | managed Tesseract executable/sidecar, then a validated system Tesseract | no proprietary fallback required initially |
| Android | Tesseract4Android with bundled `tessdata_fast` English model | bundled ML Kit Text Recognition v2 Latin model |
| iOS | Tesseract native adapter when qualified | Apple Vision |

## Tauri sidecars

Tauri `externalBin` sidecars are appropriate for desktop executables. They are
not the Android packaging boundary: Android distributes ABI-specific native
libraries inside the application package and restricts arbitrary executable
launches. Therefore ostudy will use the Tauri sidecar mechanism to make desktop
Tesseract reproducible, while Android calls Tesseract through a native library
adapter in the private platform plugin.

## Android behavior

The Android adapter bundles the English `tessdata_fast` model at build time,
copies it into no-backup app-private storage on first use, and runs bounded page
segmentation passes. A result is considered strong only when it contains enough
non-whitespace and alphabetic content and reaches the confidence threshold.
Weak or failed Tesseract results are compared with ML Kit output and the stronger
result is returned.

The adapter never exposes app-private paths to React. Rust retains ownership of
page limits, image-size limits, cancellation, persistence, and downstream
transactions.

## Consequences

- Android OCR remains offline after installation.
- Tesseract behavior is consistent with desktop while ML Kit provides a robust
  phone-native recovery path.
- The APK becomes larger because it includes two OCR engines and a trained-data
  model.
- Additional languages require explicitly pinned and tested trained-data models.
- OCR remains selective: digitally generated PDFs with usable text do not pay
  the rendering and recognition cost.
