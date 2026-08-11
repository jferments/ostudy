# Install and run ostudy on Linux

Ubuntu and Debian are the primary supported development environment. The repository setup script installs the packages and pinned toolchains expected by the local checks and the self-hosted Linux runner, provisions the verified PDF/OCR runtimes, and creates a standalone debug build.

## Prerequisites

Use an Ubuntu or Debian system with:

- an account that can run `sudo apt-get`;
- network access for operating-system packages, pinned language toolchains, JavaScript packages, Rust crates, and the verified PDFium archive;
- enough disk space for Node.js, Rust, Cargo build output, and application dependencies;
- a graphical session capable of running a WebKitGTK application.

The setup script installs the WebKitGTK, AppIndicator, SVG, OpenSSL, X11/WebDriver, SQLite, Python, and ordinary build packages used by ostudy. It installs the exact Node.js, pnpm, Rust, rustfmt, Clippy, and `tauri-driver` versions recorded in `versions.env`.

## One-time setup

From the repository root:

```bash
./scripts/setup-ubuntu.sh
```

The script is intentionally repeatable. Existing matching tool installations are reused, while version mismatches fail or are replaced explicitly rather than being ignored.

Setup also runs the reviewed native dependency installer. It stages the pinned PDFium runtime used to render PDF pages and installs Tesseract with English data for OCR fallback on PDFs whose embedded text is unusable. Normal text-based PDFs may never invoke OCR, but the runtime is available when a corrupted or image-only text layer requires recovery.

To re-provision only native document dependencies later:

```bash
./scripts/install-native-deps.sh --pdfium-only
./scripts/install-native-deps.sh --with-tesseract
```

## Start the application

After setup:

```bash
./run-app.sh
```

`run-app.sh` launches the existing standalone debug binary. It deliberately does not start Vite or rebuild the application. This keeps normal startup fast and makes it obvious when a build is stale.

After changing source code:

```bash
./scripts/build.sh
./run-app.sh
```

For a live development session instead:

```bash
pnpm desktop
```

The live development command is useful while editing the interface; the standalone build is the artifact used for final manual acceptance.

## Control compilation parallelism

Cargo normally uses all detected logical processors. On a machine where that competes with other work or memory, set an explicit limit:

```bash
CARGO_BUILD_JOBS=8 ./scripts/build.sh
```

The document extraction and OCR worker counts are separate runtime decisions. They are chosen from task size, CPU count, and available memory, and can be capped with the documented operator environment variables when diagnosing a machine-specific problem.

## Validate the checkout

Use the least expensive check that answers the current question:

```bash
./scripts/check-changed.sh
./scripts/check-static.sh
./scripts/check-fast.sh
./scripts/check.sh
```

The full gate builds the native application and exercises the real WebDriver persistence smoke test. See [Testing and CI](../development/testing-and-ci.md) before interpreting a narrower check as complete qualification.

## Common problems

### The launcher says the build is stale

Run:

```bash
./scripts/build.sh
```

The launcher compares the current source tree with the recorded build state and refuses to present an old binary as current.

### PDFium cannot be loaded

Re-run:

```bash
./scripts/install-native-deps.sh --pdfium-only
```

The installer verifies both the pinned archive and the staged native library. An operator-managed compatible library may be selected with `OSTUDY_PDFIUM_PATH`.

### OCR is unavailable

Confirm that Tesseract and English data are visible:

```bash
tesseract --version
tesseract --list-langs
```

The language list must contain `eng`. Re-run the installer with `--with-tesseract` when necessary.

### Environment diagnostics fail

Run:

```bash
./scripts/doctor.sh
```

The diagnostic output names the missing or mismatched tool instead of allowing a later build step to fail ambiguously.

## Other Linux distributions

The source is not intentionally tied to `apt`, but the complete bootstrap script currently is. On another distribution, install the equivalent Tauri/WebKitGTK build packages, the pinned language toolchains, PDFium, and Tesseract manually, then run the normal frozen/locked build commands. Such systems are not yet continuously qualified, so record distribution-specific fixes rather than adding silent fallbacks to the Ubuntu script.
