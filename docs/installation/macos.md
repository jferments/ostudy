# Build ostudy on macOS

macOS is an intended desktop target, but it is not yet continuously qualified or packaged by the repository's release workflow. These instructions describe the current source-build path for contributors and testers; they do not represent a finished notarized macOS release.

## Prerequisites

Install:

- Xcode Command Line Tools;
- Git;
- the exact Node.js and pnpm versions listed in `versions.env`;
- Rust through rustup, using the toolchain pinned in `rust-toolchain.toml`;
- the ordinary Tauri macOS build prerequisites;
- Homebrew or MacPorts when using the repository installer to obtain Tesseract.

The repository does not yet provide a complete macOS bootstrap script equivalent to `scripts/setup-ubuntu.sh`.

## Install JavaScript dependencies

From the repository root:

```bash
pnpm install --frozen-lockfile
pnpm run verify:versions
```

## Provision PDFium and Tesseract

Run:

```bash
./scripts/install-native-deps.sh --with-tesseract
```

The installer selects the correct Intel or Apple Silicon PDFium archive from `scripts/native-deps-manifest.json`, verifies its SHA-256 digest, and stages the library for Tauri. It installs Tesseract through Homebrew or MacPorts when OCR support is requested.

A compatible system or self-built PDFium library may be supplied through `OSTUDY_PDFIUM_PATH`. A nonstandard Tesseract executable may be supplied through `OSTUDY_TESSERACT_PATH`.

## Build and run

Build the frontend and native application with locked dependencies:

```bash
pnpm run build
pnpm tauri build --debug --no-bundle --ci -- --locked
```

For a live development session:

```bash
pnpm tauri dev
```

The repository's Linux-specific `scripts/build.sh`, `run-app.sh`, and native WebDriver harness should not be assumed to provide complete macOS packaging or acceptance. A macOS contributor may use the underlying pnpm/Tauri commands while platform support is being completed.

## What still needs qualification

Before macOS can be described as a supported release target, the project needs:

- a repeatable setup command for Intel and Apple Silicon systems;
- automated frontend, Rust, native-build, and launch checks on macOS;
- verified PDFium and Tesseract resource packaging inside the application bundle;
- Keychain, file-picker, backup, import/export, close-guard, and document-generation acceptance tests;
- code signing, notarization, and reproducible release artifacts;
- troubleshooting documentation based on qualified hardware and OS versions.

Report macOS defects with the OS version, hardware architecture, commands run, failing phase, and relevant privacy-conscious diagnostics. See [Contributing](../../CONTRIBUTING.md).
