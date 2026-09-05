# Android and iOS status

Android and iOS are separate delivery targets over one shared ostudy application core. Android is implemented and qualified from source; iOS remains planned.

## Android

The repository contains a Tauri Android project, private Kotlin platform plugin, pinned SDK/NDK/JDK/Rust tooling, APK/AAB wrappers, physical-device install path, official API 36 emulator smoke, and self-hosted Actions qualification.

The Android application supports:

- deck/folder/card/tag/template authoring, Search/Filter, bulk operations, and managed images;
- Practice, scheduling controls, Analytics, and interrupted-session restoration;
- durable authoring drafts and complete personal backup/restore;
- `.studydeck` and generated `.ostudy.json` import, system picker/export, Sharesheet send, and confirmed incoming shares;
- Android Keystore-backed AI credentials plus local/LAN/cloud provider profiles;
- PDF, Word, PowerPoint, OpenDocument, EPUB, RTF, Excel/ODS, and CSV document ingestion through the shared canonical document model;
- native PDF rendering through Android `PdfRenderer` and selective bundled ML Kit Text Recognition v2 OCR;
- source-grounded flashcard generation and Study Guides through shared Rust/provider task boundaries;
- foreground execution for user-started long document work with durable restart/checkpoint semantics;
- encrypted same-LAN trusted-device synchronization with one-way offers, common-base merge, explicit conflicts, and optional per-offer API credential transfer;
- Google Code Scanner through Play services for device-sync QR acquisition, with pasted invitation fallback.

Android does not package desktop PDFium or Tesseract/tessdata. Shared Rust owns document planning, OCR selection, provider task semantics, validation, scheduling, synchronization merge, and durable database transactions. Kotlin is an operating-system adapter rather than a separate study-domain implementation.

See [Android installation](android.md), [Android build/install](../development/android-build-and-install.md), and [Android architecture](../development/android.md).

## iOS

iOS remains planned. Shared runtime/platform ports and Android experience provide reusable boundaries, but there is no generated iOS project, installable application, native adapter set, signing pipeline, or supported iOS build procedure yet.

A future iOS target needs file/document-provider integration, protected credentials, lifecycle/background behavior, system share/Back-equivalent navigation behavior, native PDF/OCR decisions, network permissions, signing, store packaging, and exact platform qualification without forking shared study semantics.

## Shared portability foundation

Desktop and Android share:

- deck/note/card/tag/template, scheduling, review history, analytics, and deletion rules;
- SQLite migrations/data repairs and managed-file lifecycle;
- validated rich-content/import/backup formats;
- provider-profile metadata and narrow AI task contracts;
- React feature views/controllers and adaptive UI foundations;
- staged-import, prepared-export, durable-draft, operation/retry, and startup-recovery state machines;
- canonical document source/learning-plan/topic/generation semantics;
- Study Guide revision/annotation/topic-link behavior;
- trusted-device package, merge, conflict, and encrypted-transport contracts.

Platform expansion should implement the required ports and qualify them rather than duplicate these rules.

## Native mobile boundaries

Mobile-specific adapters can own:

- Android content URIs and future iOS document-provider URLs;
- Activity/application lifecycle, foreground/background execution, safe areas, software keyboards, and system navigation;
- protected credential storage;
- Sharesheets/incoming intents;
- camera/QR acquisition and local-network permissions;
- native PDF rasterization/OCR;
- signing/store packaging and upgrade behavior.

They do not own SQLite migrations, FSRS scheduling, analytics attribution, package validation, authoring transactions, document-learning schemas, provider fallback, or synchronization merge/conflict policy.

## Mobile acceptance

Representative mobile qualification includes:

- first launch, migration, restart, upgrade/persistence;
- compact navigation, overlays, focus, keyboard/IME, system Back, and accessibility;
- authoring, Practice, scheduling controls, and Analytics;
- import/export/backup/restore/sharing;
- provider credentials and mobile network policy;
- representative document parsing, PDF rendering/OCR, cancellation, foreground/background recovery;
- device pairing, encrypted transfer, conflicts, assets, and credential opt-in;
- battery, memory, storage, and representative phone/tablet layouts;
- package permissions/native-library compatibility and signing where applicable.

The maintained automated Android baseline combines compile/package checks and API 36 emulator smoke on the unified runner. Real camera/Google Code Scanner, OEM file providers, thermal/battery pressure, physical-device screen-reader behavior, and public signing/store distribution remain manual or separately qualified boundaries unless actually exercised.
