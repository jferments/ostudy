# Android and iOS status

Android and iOS remain separate delivery targets over one shared ostudy application core.

## Android

Android is implemented as a source-build Tauri application. The repository contains a generated Tauri Android project, a private Kotlin platform plugin, pinned Android SDK/NDK tooling, APK/AAB build wrappers, official-emulator smoke tests, and self-hosted GitHub workflows.

The Android application supports:

- deck, card, tag, template, search, and bulk authoring;
- managed card/template images;
- practice, scheduling controls, interrupted-session restoration, and analytics;
- durable authoring drafts after backgrounding or process death;
- `.studydeck`, generated `.ostudy.json`, complete `.ostudybackup`, and PDF import;
- system document-picker export and restore;
- Android Sharesheet send and confirmed incoming share import;
- Android Keystore-backed AI-provider credentials;
- cloud and LAN AI-provider profile connectivity;
- native PDF page rendering through Android `PdfRenderer`;
- selective on-device PDF OCR through bundled ML Kit Text Recognition v2;
- foreground document processing for user-started long-running PDF jobs;
- encrypted same-LAN device synchronization with QR pairing, common-base merging, and explicit conflicts;
- optional encrypted AI-credential transfer between trusted devices.

Android does not package Tesseract or `tessdata`. Shared Rust keeps usable embedded PDF text and calls the Android ML Kit adapter only for pages that require visual text recovery.

See [Build and install ostudy for Android](android.md), [Android build and install](../development/android-build-and-install.md), and [Android development architecture](../development/android.md).

## iOS

iOS remains planned. The shared runtime and Android work validate the ports, merge protocol, document boundaries, and mobile UI patterns that a future Swift implementation can reuse, but there is no generated iOS project, installable application, native adapter set, or supported iOS build procedure yet.

## Shared portability foundation

The following behavior remains common across desktop and mobile:

- card, deck, tag, template, scheduling, and analytics domain rules;
- SQLite migrations and data repairs;
- validated rich-content and import formats;
- provider-profile metadata and narrow AI task contracts;
- React feature views, controllers, and adaptive shared UI;
- staged-import, prepared-export, durable-draft, backup, and operation-identity state machines;
- document extraction planning, OCR selection, checkpoints, and generation semantics;
- trusted-device snapshot, merge, conflict, and encrypted-transport contracts.

## Native mobile adapters

Platform-specific code remains limited to operating-system boundaries:

- Android content URIs and future iOS document-provider URLs;
- lifecycle, process restoration, system Back, safe areas, and software keyboards;
- protected credential storage;
- Sharesheets and incoming intents;
- connectivity and local-network access;
- PDF rasterization and OCR;
- QR camera acquisition;
- foreground/background execution policy;
- signing, store packaging, and upgrade testing.

Kotlin and future Swift code do not write the ostudy database or implement scheduling, analytics, package validation, synchronization merge semantics, or authoring transactions.

## Mobile acceptance requirements

A mobile application is considered usable only after end-to-end qualification for:

- first launch, migration, restart, and upgrade persistence;
- compact navigation, overlays, focus, keyboard, and screen-reader behavior;
- card authoring, practice, scheduling controls, and analytics;
- import, export, backup, restore, and system sharing;
- provider credentials and mobile network policy;
- PDF rendering, OCR, cancellation, and durable recovery;
- device pairing, encrypted transfer, merge conflicts, and asset synchronization;
- cancellation and recovery when backgrounded or terminated;
- battery, memory, storage, and representative phone/tablet layouts;
- package permissions and native-library compatibility.

The maintained Android qualification path combines shared Linux/Tauri checks, Android compile/package checks, and the official API 36 emulator smoke suite on the unified self-hosted runner.
