# Build ostudy on Windows

Windows is an intended desktop target, but it is not yet continuously qualified or packaged by the repository's release workflow. The instructions below describe the current source-build path for contributors and testers. Treat platform-specific failures as useful qualification work rather than as evidence that a Windows release already exists.

## Prerequisites

Install:

- Git for Windows;
- Microsoft Visual Studio Build Tools with the C++ desktop workload and a current Windows SDK;
- Microsoft Edge WebView2 Runtime;
- the exact Node.js and pnpm versions listed in `versions.env`;
- Rust through rustup, using the toolchain pinned in `rust-toolchain.toml`;
- PowerShell 7 or Windows PowerShell with permission to run the repository installer;
- `tar`, which is included in current Windows installations and is used to extract the verified PDFium archive.

The repository does not yet provide a complete Windows bootstrap script equivalent to `scripts/setup-ubuntu.sh`. Version mismatches should be corrected explicitly rather than bypassing the manifest checks.

## Install JavaScript dependencies

From PowerShell in the repository root:

```powershell
pnpm install --frozen-lockfile
pnpm run verify:versions
```

The frozen lockfile and exact version verification prevent a source build from silently resolving a different dependency graph.

## Provision PDFium and Tesseract

Run:

```powershell
powershell -ExecutionPolicy Bypass -File .\scripts\install-native-deps.ps1 -WithTesseract
```

The script downloads only the PDFium archive selected by `scripts/native-deps-manifest.json`, verifies its SHA-256 digest, and stages the native library under the ignored Tauri resource directory. For OCR fallback, it locates Tesseract or installs the Tesseract-documented UB Mannheim distribution through `winget` when available.

To use a compatible PDFium library that you built or manage yourself, set `OSTUDY_PDFIUM_PATH` before building. If Tesseract is installed outside `PATH`, set `OSTUDY_TESSERACT_PATH` to its executable.

## Build the application

Run the frontend build, then ask Tauri to produce the native application:

```powershell
pnpm run build
pnpm tauri build --debug --no-bundle --ci -- --locked
```

A packaged Windows release process is not yet committed. The command above is therefore a development build path, not a guarantee that a distributable installer has passed platform acceptance.

For a live development session:

```powershell
pnpm tauri dev
```

## What still needs qualification

Before Windows can be described as a supported release target, the project needs:

- a repeatable full setup command;
- automated TypeScript, Rust, native-build, and application-launch checks on a Windows runner;
- verified PDFium and Tesseract packaging in the installed application;
- file-picker, backup, import/export, credential-store, close-guard, and document-generation acceptance tests;
- signed and reproducible release artifacts;
- Windows-specific troubleshooting documentation based on actual failures.

Report Windows defects with the exact Windows version, CPU architecture, WebView2 version, commands run, failing phase, and relevant diagnostics. See [Contributing](../../CONTRIBUTING.md) for the issue template and privacy rules.
