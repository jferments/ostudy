# Build ostudy on Windows

Windows 10 and 11 (x86-64) get an NSIS installer with each release, built from the release commit on the on-demand Windows build box (jferments/ostudy-io `docs/windows-build-box.md`). Windows is not yet continuously qualified: CI runs on Linux and Android only. The rest of this page is the source-build path for contributors and testers, the script that builds and installs the installer on a Windows PC, and the startup diagnostic. Treat platform-specific failures as useful qualification work.

## Install a release

Download `ostudy-vX.Y.Z-windows-x86_64-setup.exe` from [ostudy.io/install](https://ostudy.io/install/) or the GitHub release, and check it against the release's `SHA256SUMS.txt`. The installer is not code-signed yet, so SmartScreen asks first: choose **More info**, then **Run anyway**. It installs for the current user under `%LOCALAPPDATA%\ostudy` without administrator rights, ships PDFium, and links the C runtime statically, so no Visual C++ Redistributable is needed. WebView2 comes with current Windows.

Not on Windows yet:

- device voices (Kokoro) and speech input: Settings and the AI setup wizard do not offer them, and the Tutor shows no microphone;
- USB phone sync;
- OCR for scanned pages without a separate Tesseract: oStudy uses a Tesseract installed under `%ProgramFiles%` or on `PATH` (set `OSTUDY_TESSERACT_PATH` for another location), and is not bundled with it.

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
pnpm tauri build --config src-tauri/tauri.dev.conf.json --debug --no-bundle --ci -- --locked
```

This is a development build. To build the release installer, install it and start it in one step, see the next section.

For a live development session:

```powershell
pnpm tauri dev --config src-tauri/tauri.dev.conf.json -- --locked
```

Both commands pass the `src-tauri/tauri.dev.conf.json` overlay, so the debug build runs as `io.github.jferments.ostudy.dev` with its own library and credential namespace. Without it, a debug build resolves the production identifier `io.github.jferments.ostudy`, and startup refuses it before opening any library. This is the same command the Bash `pnpm desktop` wrapper (`scripts/dev.sh`) runs on Linux.

## Build, install and check the installer on a Windows PC

`scripts/windows-build-install.ps1` builds the NSIS installer from the checked-out commit, installs it silently for the current user, checks that `ostudy.exe` and `native\pdfium\pdfium.dll` landed, and starts the installed app through the startup diagnostic. Check out the commit to test first, then from the repository root:

```powershell
powershell -ExecutionPolicy Bypass -File .\scripts\windows-build-install.ps1 -ExpectedCommit <sha>
```

It stops at the first failing step and names it, and refuses to build if `HEAD` is not `<sha>`. Tracked files checked out with CRLF line endings are rewritten with LF first (oStudy's migrations and contract files must be LF), which discards uncommitted changes to tracked files. The first build takes 20 to 40 minutes. `-KeepLibrary` starts the app on your existing library instead of an empty one.

`scripts/diagnose-windows-startup.ps1` collects why a Windows build stops at startup into one report, printed and saved to `%TEMP%\ostudy-startup-report.txt`. It closes running oStudy processes and moves the library aside (renamed, never deleted) so the app starts on an empty library; `-KeepLibrary` keeps it, and `-Exe <path>` starts that executable instead of the first one it finds among the build and install locations.

## What still needs qualification

The installer is released, but Windows still needs:

- a repeatable full setup command equivalent to `scripts/setup-ubuntu.sh`;
- automated TypeScript, Rust, native-build, and application-launch checks on a Windows runner;
- acceptance on a clean PC with no development tools: file pickers, backup, import/export, the credential store, close guard and document generation;
- a code-signed installer;
- device voices, speech input, USB phone sync and bundled Tesseract.

Report Windows defects with the exact Windows version, CPU architecture, WebView2 version, commands run, failing phase, and relevant diagnostics. See [Contributing](../../CONTRIBUTING.md) for the issue template and privacy rules.
