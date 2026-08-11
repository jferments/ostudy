# Installation and platform availability

ostudy is currently a pre-release project that is normally built from source. The repository contains one fully integrated setup path for Ubuntu/Debian Linux and lower-level native-dependency installers for other desktop targets. Platform support should be judged by qualification and packaging, not merely by whether Tauri can theoretically compile for a target.

## Current status

| Platform | Installation status | Qualification status |
| --- | --- | --- |
| Ubuntu/Debian Linux | Complete repository setup script and source build | Primary local and CI path |
| Other Linux distributions | Manual source build | Not continuously qualified |
| Windows | Source-build components are present | In progress; not continuously qualified |
| macOS | Source-build components are present | In progress; not continuously qualified |
| Android | No installable application | Planned |
| iOS | No installable application | Planned |

The platform-specific guides explain what is known to work and what still needs qualification:

- [Linux](linux.md)
- [Windows](windows.md)
- [macOS](macos.md)
- [Android and iOS](mobile.md)

## What every desktop build needs

A desktop build combines several layers:

- the pinned Node.js and pnpm toolchain for the React/TypeScript frontend;
- the pinned Rust toolchain and Tauri CLI for the native application;
- operating-system WebView and build packages;
- SQLite, which is bundled through Rust;
- a compatible PDFium runtime for PDF page rendering;
- Tesseract with English language data for desktop OCR fallback;
- the exact JavaScript and Rust lockfiles committed to the repository.

`versions.env` is the repository's authoritative list of pinned tool and dependency versions. The manifests, `.node-version`, `.nvmrc`, and `rust-toolchain.toml` are verified mirrors rather than independent version sources.

## Native PDF and OCR dependencies

ostudy does not allow a Rust build dependency to download PDFium implicitly. The repository's installers select an immutable archive from `scripts/native-deps-manifest.json`, verify its SHA-256 digest, and stage the native library under the ignored Tauri resources directory. Builders may instead provide a compatible library through `OSTUDY_PDFIUM_PATH`.

Desktop OCR uses Tesseract only when a PDF's text layer is unusable. The installer uses established operating-system package channels rather than maintaining a private executable bundle. See the [dependency audit](../development/dependency-audit.md) and [AI/document architecture](../architecture/ai-and-document-processing.md) for the trust and runtime boundaries.

## Release artifacts

The current release workflow produces Linux artifacts from a self-hosted Linux runner when a version tag is pushed. It does not yet produce Windows, macOS, Android, or iOS packages. Until those pipelines and their acceptance tests exist, documentation should describe those targets as source-build work or planned support rather than completed releases.

## After installation

Continue with [Getting started](../user-guide/getting-started.md) to create a deck, add cards, run a practice session, configure an optional AI provider, and make a first backup.
