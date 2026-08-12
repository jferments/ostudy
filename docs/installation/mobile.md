# Android and iOS status

Android and iOS remain separate delivery targets over one shared ostudy application core.

## Android

Android phases A1–A3 are implemented as a source-build application. The repository contains a generated Tauri Android project, a private Kotlin platform plugin, pinned Android SDK/NDK tooling, APK/AAB build wrappers, official-emulator smoke tests, and self-hosted GitHub workflows.

The Android application supports:

- deck, card, tag, template, search, and bulk authoring;
- managed card/template images;
- practice, scheduling controls, interrupted-session restoration, and analytics;
- durable authoring drafts after backgrounding or process death;
- `.studydeck`, generated `.ostudy.json`, and complete `.ostudybackup` import;
- system document-picker export and restore;
- Android Sharesheet send and confirmed incoming share import;
- Android Keystore-backed AI-provider credentials;
- cloud and LAN AI-provider profile connectivity.

PDF import, PDF rendering, OCR, and AI document generation intentionally remain an Android development stub during A1–A3. Existing desktop document rows and managed files are preserved when a complete backup is restored on Android.

See [Build and install ostudy for Android](android.md) and [Android development architecture](../development/android.md).

## iOS

iOS remains planned. The shared P0–P2 runtime and the Android work validate the ports and mobile UI patterns that a future Swift implementation will reuse, but there is no generated iOS project, installable application, native adapter set, or supported iOS build procedure yet.

## Shared portability foundation

The following behavior remains common across desktop and mobile:

- card, deck, tag, template, scheduling, and analytics domain rules;
- SQLite migrations and data repairs;
- validated rich-content and import formats;
- provider-profile metadata and narrow AI task contracts;
- React feature views, controllers, and adaptive shared UI;
- staged-import, prepared-export, durable-draft, backup, and operation-identity state machines.

## Native mobile adapters

Platform-specific code remains limited to operating-system boundaries:

- Android content URIs and future iOS document-provider URLs;
- lifecycle, process restoration, system Back, safe areas, and software keyboards;
- protected credential storage;
- Sharesheets and incoming intents;
- connectivity and future local-network permissions;
- future mobile PDF rendering and OCR;
- signing, store packaging, and upgrade testing.

Kotlin and future Swift code do not write the ostudy database or implement scheduling, analytics, package validation, or authoring transactions.

## Mobile acceptance requirements

A mobile application is considered usable only after end-to-end qualification for:

- first launch, migration, restart, and upgrade persistence;
- compact navigation, overlays, focus, keyboard, and screen-reader behavior;
- card authoring, practice, scheduling controls, and analytics;
- import, export, backup, restore, and system sharing;
- provider credentials and mobile network policy;
- cancellation and recovery when backgrounded or terminated;
- battery, memory, storage, and representative phone/tablet layouts;
- package permissions and native-library compatibility.

Android A1–A3 has automated shared, compile/package, and official-emulator smoke paths for the supported non-document feature set. PDF/OCR/document-generation acceptance belongs to later Android phases.
