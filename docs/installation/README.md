# Installation and platform availability

ostudy is currently a pre-release project that is normally built from source. The repository contains fully integrated source-build and qualification paths for Ubuntu/Debian Linux and Android, plus lower-level native-dependency installers for other desktop targets. Platform support should be judged by qualification and packaging, not merely by whether Tauri can theoretically compile for a target.

## Current status

| Platform | Installation status | Qualification status |
| --- | --- | --- |
| Ubuntu/Debian Linux | Complete repository setup script and source build | Primary local and CI path |
| Other Linux distributions | Manual source build | Not continuously qualified |
| Windows | Source-build components are present | In progress; not continuously qualified |
| macOS | Source-build components are present | In progress; not continuously qualified |
| Android | Generated Tauri project, debug APK/AAB build tools, emulator tests, and physical-device development path | A1–A3 application qualified; PDF/document processing remains a stub |
| iOS | No installable application | Planned |

The platform-specific guides explain what is known to work and what still needs qualification:

- [Linux](linux.md)
- [Windows](windows.md)
- [macOS](macos.md)
- [Android](android.md)
- [Mobile platform status](mobile.md)

## What every desktop build needs

A desktop build combines several layers:

- the pinned Node.js and pnpm toolchain for the React/TypeScript frontend;
- the pinned Rust toolchain and Tauri CLI for the native application;
- operating-system WebView and build packages;
- SQLite, which is bundled through Rust;
- a compatible PDFium runtime for PDF page rendering;
- Tesseract with English language data for desktop OCR fallback;
- the exact JavaScript and Rust lockfiles committed to the repository.

Android instead uses the pinned JDK, Android SDK, Build Tools, NDK, Gradle wrapper, and Android Rust targets described in the [Android build guide](android.md). PDFium and Tesseract are deliberately excluded from A1–A3 Android artifacts.

`versions.env` is the repository's authoritative list of pinned tool and dependency versions. The manifests, `.node-version`, `.nvmrc`, `rust-toolchain.toml`, Gradle configuration, and Android scripts are verified mirrors rather than independent version sources.

## Native PDF and OCR dependencies

ostudy does not allow a Rust build dependency to download PDFium implicitly. The repository's desktop installers select an immutable archive from `scripts/native-deps-manifest.json`, verify its SHA-256 digest, and stage the native library under the ignored Tauri resources directory. Builders may instead provide a compatible library through `OSTUDY_PDFIUM_PATH`.

Desktop OCR uses Tesseract only when a PDF's text layer is unusable. Android A1–A3 shows an explicit Documents development placeholder and does not package PDFium, OCR, or document-generation support. See the [dependency audit](../development/dependency-audit.md) and [AI/document architecture](../architecture/ai-and-document-processing.md) for the trust and runtime boundaries.

## Release artifacts

The tagged release workflow currently produces Linux artifacts. Android workflows build unsigned debug/QA APKs and AABs through explicit qualification or manual dispatch; public distribution still requires the project's permanent signing key and Play App Signing setup. Windows, macOS, and iOS release artifacts remain future work.

## After installation

Continue with [Getting started](../user-guide/getting-started.md) to create a deck, add cards, run a practice session, configure an optional AI provider, and make a first backup.
