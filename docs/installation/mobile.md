# Android and iOS status

Android and iOS are planned ostudy targets. No installable mobile application or supported mobile build procedure is currently included in the repository. The shared P0-P2 runtime, opaque file-transfer layer, and durable authoring drafts are now implemented, but Android/iOS native adapters and generated projects remain future work. The presence of cross-platform Tauri abstractions, accepted platform-service decisions, and native dependency metadata is preparation for future work, not evidence that the mobile applications have shipped.

## Portability foundation

The implementation sequence begins before an Android project is generated:

1. **P0 — decisions and inventory:** accept the platform, file, secret, lifecycle/time/resource, and durable-draft boundaries; reject new untracked platform debt.
2. **P1 — runtime ports:** move current Linux behavior behind typed capability, file, secret, lifecycle, clock/time-zone, and resource interfaces.
3. **P2 — path-free files and durable drafts:** stage imports, prepare exports, serve managed images by ID, and recover interrupted authoring safely.

See [Platform services](../architecture/platform-services.md) and the [portability inventory](../development/platform-portability-inventory.md).

## What can be shared

The architecture is designed so that the following behavior remains common across desktop and mobile:

- card, deck, tag, template, scheduling, and analytics domain rules;
- SQLite migrations and data repairs;
- validated rich-content and import formats;
- provider-profile metadata and narrow AI task contracts;
- document semantics, source grounding, and generated-card validation;
- React feature views, controllers, and responsive shared UI where WebView behavior is portable;
- staged-import, prepared-export, and durable-draft state machines.

## What needs mobile adapters

Mobile applications require platform-specific implementations for capabilities that desktop obtains from Tauri plugins, native libraries, or subprocesses:

- document and backup selection through Android content URIs and iOS document providers;
- application lifecycle and long-running task interruption;
- native PDF rendering and packaging;
- OCR without a desktop Tesseract subprocess;
- operating-system credential storage;
- share sheets, backup destinations, and external-link handling;
- hardware Back on Android, safe areas, virtual keyboards, touch interaction, audio, and haptics;
- application signing, store packaging, upgrades, and data-directory compatibility.

The preferred future OCR direction is a platform-native adapter: bundled/offline ML Kit Text Recognition on Android and Apple Vision on iOS. This remains a design direction until implemented and qualified.

## Mobile acceptance requirements

A mobile release should not be announced merely because a project compiles. It needs end-to-end testing for:

- first launch, migration, restart, and upgrade persistence;
- compact navigation, overlays, focus, keyboard, and screen-reader behavior;
- card authoring, practice, scheduling controls, and analytics;
- import, export, backup, restore, and device share workflows;
- local and cloud provider connections under mobile network policies;
- cancellation and recovery when the application is backgrounded or terminated;
- document ingestion, rendering, OCR, AI generation, and storage cleanup;
- battery, memory, storage, and large-document behavior on representative devices.

Follow the [roadmap](../roadmap.md) for progress. Until a mobile project and its validation path are committed, README and release documentation should describe Android and iOS as planned.
