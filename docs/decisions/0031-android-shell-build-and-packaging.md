# ADR 0031: Android shell, build, and packaging

- **Status:** Accepted
- **Date:** 2026-08-12
- **Scope:** Android phases A1–A3

## Context

The P0–P2 portability foundation keeps React feature views, Rust application and domain services, SQLite migrations, managed files, scheduling, analytics, imports, backups, and durable drafts shared. Android now needs a real application shell and native adapters without creating a Kotlin implementation of the product.

## Decision

ostudy will ship one generated Tauri Android project under `src-tauri/gen/android/`. Kotlin is limited to Android APIs at the sandbox boundary: lifecycle, system Back, window insets, Storage Access Framework document access, Sharesheet integration, Android Keystore, connectivity observations, and native intent delivery. Kotlin must not open or mutate the ostudy SQLite database or implement study rules.

The production package identifier remains `io.github.jferments.ostudy`. Debug builds use an application-ID suffix so they can coexist with later release builds. The initial supported Android range is API 28 and newer. The project compiles and targets API 36 and uses JDK 17, a Tauri-compatible Android Gradle Plugin, and NDK r28 or newer so native libraries can satisfy 16 KiB page-size requirements.

The first Android feature release includes deck/card/template authoring, practice, analytics, settings, managed images, `.studydeck` and generated-JSON import, complete backup export/restore, Android document sharing, incoming share handling, secure provider credentials, and cloud/LAN provider connectivity tests. PDF import, rendering, OCR, and AI document generation remain an explicit development stub during A1–A3.

Android Auto Backup is disabled initially. ostudy already has a coherent versioned backup format; raw platform backup of the SQLite database, staged files, encrypted secret records, or device-local navigation state could produce an inconsistent restore or ciphertext without its Keystore key.

All Android manifests, Gradle files, wrapper versions, SDK/NDK requirements, and package metadata are committed and verified by repository tooling. Pull requests continue to run the complete Linux gate and add Android cross-build, emulator, process-restoration, file-provider, Sharesheet, and package-alignment gates on self-hosted Android-capable runners.

## Consequences

- Android remains an adapter set for the existing application rather than a product fork.
- The generated project becomes reviewable and reproducible.
- Later iOS work can reuse the same lifecycle, navigation, external-file, secret, and feature-capability contracts.
- PDF/OCR work can be added later without blocking an otherwise useful Android application.
- Debug and release data remain isolated.

## Validation

A1–A3 are complete only when the shared application builds into an installable Android APK, persists an upgraded library across restart and process death, supports ordinary authoring/practice/analytics and Android import/export/share/secret workflows, passes API 28 and API 36 emulator checks, and passes static 16 KiB native-package verification while the Linux full gate remains green.
