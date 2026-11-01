# Installation and platform availability

ostudy 0.1.1 is released for Windows (an installer) and Linux (a `.deb`, `.rpm`, Flatpak, AppImage or portable `.tar.gz`): download them from [ostudy.io/install](https://ostudy.io/install/) or the [GitHub release](https://github.com/jferments/ostudy/releases/tag/v0.1.1), with `SHA256SUMS.txt` to check them. Android and every other platform are built from source for now. The repository has integrated setup, build, and qualification paths for Ubuntu/Debian Linux and Android. Other desktop targets share the application architecture, but support should be judged by repeatable qualification and packaging—not merely by whether Tauri can theoretically compile for a platform.

## Current status

| Platform | Installation status | Qualification status |
| --- | --- | --- |
| Linux (x86-64) | Released packages (0.1.1): `.deb`, `.rpm`, Flatpak, AppImage and portable folder, built in an Ubuntu 22.04 container so they run on it and newer distributions; complete source setup on Ubuntu/Debian | Ubuntu/Debian is the primary local and CI path; packages on other distributions are not continuously qualified |
| Android | Generated Tauri project, pinned toolchain, debug APK/AAB tooling, emulator tests, and physical-device install path | Shared study, document, AI, and sync application qualified through compile/package and API 36 emulator gates; public signed distribution not yet provided |
| Windows (x86-64) | Released (0.1.1): an NSIS installer (`ostudy-vX.Y.Z-windows-x86_64-setup.exe`) built from the release commit on the on-demand Windows build box (jferments/ostudy-io `docs/windows-build-box.md`); it installs for the current user, ships PDFium and links the C runtime statically. Not on Windows yet: device voices and speech input, USB phone sync, Tesseract bundling | Not in CI. The Rust lib suite passes as a Windows build under Wine, and the installer builds with MSVC on Windows 11 and on the build box; releases are not continuously qualified |
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

The setup wrapper installs the qualified system packages and pinned toolchains, installs locked project dependencies, provisions native PDF/runtime dependencies, and creates a standalone **oStudy Dev** build using `io.github.jferments.ostudy.dev`. `run-app.sh` launches that existing matching development build; it does not rebuild source and it does not open the production `io.github.jferments.ostudy` library. Production installation is a separate package/release path.

After source changes:

```bash
./scripts/build.sh
./run-app.sh
```

For live Tauri/Vite development (install the locked dependencies first, because pnpm refuses to run scripts when `node_modules` is older than the lockfile):

```bash
pnpm install --frozen-lockfile
pnpm desktop
```

`pnpm desktop` also runs as `io.github.jferments.ostudy.dev` with the development library. Do not run bare `pnpm tauri dev`: that debug build resolves the production identifier and startup refuses it.

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

The Android application uses platform document providers, Android Keystore, native `PdfRenderer`, bundled open-source Tesseract OCR with pinned English trained data, a local ZXing QR scanner, a foreground-service boundary for user-started long document work, and the private ostudy platform plugin for Android-only lifecycle/file behavior. It does not package desktop PDFium or depend on a system-installed Tesseract runtime.

Public Android distribution is not yet published. The planned direct path requires permanent release-signing-key custody, signed-release qualification, release metadata/checksums, and a signed APK; F-Droid is a later open-source distribution option. Google Play is not a prerequisite for direct APK distribution.

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
- Android: platform `PdfRenderer` plus bundled open-source Tesseract OCR with pinned English trained data.

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

A tagged release runs the full gate in CI, then the maintainer builds the Linux `.deb`, `.rpm`, AppImage, Flatpak bundle and portable tarball (on Ubuntu 22.04, in a container) with `scripts/release.sh --skip-gate` and drafts a public GitHub release for them; published releases are downloaded from <https://ostudy.io/install/>. The Debian release path verifies that the built package does not own or delete the user XDG learner-data directory; normal package removal is defined to preserve that data. The same command builds the universal Android APK and signs it with the release key, whose certificate fingerprint is committed ([Android: the public APK](android.md#the-public-apk)); CI workflows produce only unsigned QA APK/AAB artifacts. Windows, macOS, and iOS release artifacts remain future work.

## After installation

Continue with [Getting started](../user-guide/getting-started.md). Contributors should also read the [development guide](../development/guide.md), [Testing and CI](../development/testing-and-ci.md), and [canonical command runbook](../development/command-runbook.md).
