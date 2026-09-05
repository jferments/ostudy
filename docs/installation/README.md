# Installation and platform availability

ostudy is a pre-release project normally built from source. The repository has integrated setup, build, and qualification paths for Ubuntu/Debian Linux and Android. Other desktop targets share the application architecture, but support should be judged by repeatable qualification and packaging—not merely by whether Tauri can theoretically compile for a platform.

## Current status

| Platform | Installation status | Qualification status |
| --- | --- | --- |
| Ubuntu/Debian Linux | Complete repository setup, standalone debug build, live development, and release tooling | Primary local and CI path |
| Other Linux distributions | Manual source build using equivalent native dependencies | Not continuously qualified |
| Android | Generated Tauri project, pinned toolchain, debug APK/AAB tooling, emulator tests, and physical-device install path | Shared study, document, AI, and sync application qualified through compile/package and API 36 emulator gates; public signed distribution not yet provided |
| Windows | Source-build components/native dependency support present | In progress; not continuously qualified or released |
| macOS | Source-build components/native dependency support present | In progress; not continuously qualified or released |
| iOS | No generated project or installable application | Planned |

Platform guides:

- [Linux](linux.md)
- [Android](android.md)
- [Android and iOS status](mobile.md)
- [Windows](windows.md)
- [macOS](macos.md)

## Linux quick start

On a supported Ubuntu/Debian development system, run from the repository root:

```bash
./scripts/setup-ubuntu.sh
./run-app.sh
```

The setup wrapper installs the qualified system packages and pinned toolchains, installs locked project dependencies, provisions native PDF/runtime dependencies, and creates a standalone debug build. `run-app.sh` launches that existing matching build; it does not rebuild source.

After source changes:

```bash
./scripts/build.sh
./run-app.sh
```

For live Tauri/Vite development:

```bash
pnpm desktop
```

Desktop OCR fallback for scanned or damaged PDFs additionally requires Tesseract and appropriate trained data:

```bash
./scripts/install-native-deps.sh --with-tesseract
```

See [Linux installation](linux.md) for prerequisites and troubleshooting.

## Android quick start

Android uses the repository-pinned JDK, Android SDK, Build Tools, NDK, Gradle wrapper, Rust targets, and emulator image. Follow [Build and install ostudy for Android](android.md) rather than constructing ad-hoc Tauri/Gradle/ADB commands.

Maintained entry points include:

```bash
pnpm android:doctor:emulator
pnpm android:test
pnpm android:smoke
pnpm android:install
```

The Android application uses platform document providers, Android Keystore, native `PdfRenderer`, bundled ML Kit Text Recognition v2, a foreground-service boundary for user-started long document work, and the private ostudy platform plugin for QR acquisition and Android-only lifecycle/file behavior. It does not package desktop PDFium or Tesseract.

Public Android distribution still requires permanent release-signing custody, Play App Signing, store metadata, privacy declarations, and signed-release qualification.

## Desktop build layers

A desktop build combines:

- pinned Node.js and pnpm for the React/TypeScript interface;
- pinned Rust and Tauri CLI for the native application;
- OS WebView/build packages;
- SQLite bundled through Rust;
- a compatible provisioned PDFium runtime for PDF rendering;
- Tesseract where desktop OCR fallback is required;
- exact JavaScript and Rust lockfiles committed to the repository.

`versions.env` is the authoritative tool/dependency version inventory. Tool-discovered files such as `.node-version`, `.nvmrc`, `rust-toolchain.toml`, manifests, and Gradle configuration are verified mirrors for their respective tools rather than competing version sources.

## Document processing by platform

The shared document system accepts PDF, Word, PowerPoint, OpenDocument, EPUB, RTF, Excel/ODS, and CSV sources. Non-PDF formats are normalized in process through the current Xberg adapter into the same ostudy-owned canonical source model.

PDF visual recovery uses narrow platform adapters:

- desktop: pinned PDFium rendering and optional Tesseract OCR;
- Android: platform `PdfRenderer` and bundled ML Kit Text Recognition v2.

Shared Rust owns source planning, canonical provenance, checkpoints, provider tasks, generated-card validation, Study Guide publication, and durable state. Platform adapters do not reimplement those rules.

## Validate a checkout

Use the smallest maintained tier that answers the current question:

```bash
./scripts/check-changed.sh
./scripts/check-static.sh
./scripts/check-fast.sh
./scripts/check.sh
```

Android-affecting work also uses its compile/package and emulator gates. A successful narrow command is not a substitute for required exact-head pull-request qualification.

## Release artifacts

The tagged release workflow currently produces Linux artifacts. Android workflows can produce unsigned QA APK/AAB artifacts, but the repository does not yet publish a signed public Android release. Windows, macOS, and iOS release artifacts remain future work.

## After installation

Continue with [Getting started](../user-guide/getting-started.md). Contributors should also read the [development guide](../development/guide.md), [Testing and CI](../development/testing-and-ci.md), and [canonical command runbook](../agents/COMMAND-RUNBOOK.md).
