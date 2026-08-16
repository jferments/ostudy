# Project roadmap

ostudy is being developed in layers: first a dependable local study library, then document-aware AI tools, then broader platform and synchronization support. This roadmap describes direction rather than promising release dates. Priorities may change when testing exposes a more important reliability, privacy, accessibility, or data-compatibility problem.

## Current foundation

The following capabilities are already present in the active codebase:

- a local SQLite library for decks, notes, scheduled card presentations, review history, settings, and analytics;
- Basic, Reverse, Fill in the blank, and reusable-template authoring;
- rich card content with managed local images and strict validation;
- tags, full-text search, filters, bulk entry, duplicate detection, and move/copy workflows;
- FSRS scheduling, workload controls, voluntary and due-only study, and detailed learning analytics;
- portable deck exchange, generated JSON import, and complete personal-library backup including recoverable unfinished authoring drafts;
- configurable llama.cpp/Unsloth and DeepSeek provider profiles with native credential and network boundaries;
- a managed PDF library and document-to-flashcard generation on Linux and Android;
- Android PDF page rendering through `PdfRenderer` and bundled ML Kit Text Recognition v2 for selective on-device OCR;
- explicit encrypted private-LAN synchronization offers between trusted ostudy installations, with three-way merging and conflict records;
- responsive shared UI foundations, locally hosted selectable fonts, themes, keyboard behavior, and accessibility primitives;
- a qualified Android source-build/debug-install path with authoring, practice, analytics, file/share integration, process restoration, Keystore-backed credentials, documents, and QR pairing;
- tiered local and self-hosted Linux/Android CI validation, including an API 36 emulator smoke suite.

Historical stage plans are kept under [`history/`](history/) for project archaeology. They are not the current roadmap.

## Completed portability foundation (P0-P2)

The shared runtime exposes typed platform capabilities for external files, secure secrets, lifecycle, time zones, and resource observation. External imports are staged into managed storage, exports are prepared before publication, card images use opaque asset identities, and card/template/bulk/AI-profile metadata editors retain bounded device-local drafts across process loss. Final authoring saves retire the exact flushed draft inside the canonical SQLite transaction, and backup format version 3 restores persistent drafts with their staged images.

## Completed Android application and document foundation (A1-A7)

The Tauri Android shell boots the shared application and uses narrow Kotlin adapters for Android-only APIs. Android supports ordinary deck/card/template authoring, practice, analytics, durable drafts, interrupted-practice restoration, managed images, Storage Access Framework import/export, outgoing and confirmed incoming shares, complete backup/restore, Android Keystore credentials, connectivity observations, system Back, safe areas, and software-keyboard insets.

The document pipeline now retains the shared Rust validation/planning/checkpoint/transaction boundaries while using Android `PdfRenderer` for page rasterization and bundled ML Kit Text Recognition v2 for pages that need visual text recovery. Long-running user-started document work can hold an Android foreground-service lease with a visible notification; process interruption still relies on durable checkpoints rather than pretending process death is transparent.

Trusted-device synchronization now uses authenticated encrypted TCP over private LAN addresses. A protocol-v2 invitation is a one-way offer from the device that created it: Android can scan the offer and immediately receive the sender-selected study data without creating a second invitation. Ordinary application/appearance settings are excluded from sync; API provider settings and saved credentials are a sender-controlled opt-in for each offer.

The repository includes pinned SDK/NDK tooling, Android project/package verifiers, debug/release APK and AAB build paths, and an official API 36 emulator smoke suite. Public Android distribution still requires release signing/store work described below.

## Current priorities

### 1. Stabilize document and PDF processing across desktop and Android

PDF generation remains one of the newest large subsystems. Ongoing work includes improving extraction quality, OCR recovery, provider compatibility, progress reporting, cancellation, retries, diagnostic clarity, generated-card quality, and representative corpus coverage for large, rotated, low-contrast, multi-column, table-heavy, and otherwise unusual documents. Android additionally needs continued physical-device coverage across camera/storage vendors and resource-constrained devices.

### 2. Harden cross-device synchronization on real devices

The encrypted one-way-offer protocol and three-way merge engine are implemented. The next work is broader physical-device qualification, clearer conflict UX, more representative asset/history test matrices, performance measurement on large libraries, and future transfer-efficiency improvements that preserve the explicit user-controlled send/receive model. Sync must continue to keep device-local appearance/application settings separate and require fresh sender consent before transferring API configuration or credentials.

### 3. Refine everyday authoring and review ergonomics

Autosaved card, template, bulk-entry, and AI-profile metadata drafts recover after interruption and participate in complete-library backup. Remaining improvements include richer image sizing and alignment, clearer multi-draft management, and a carefully designed undo-last-review operation that restores an exact prior schedule state without rewriting historical evidence incorrectly.

### 4. Broaden desktop qualification

Ubuntu/Debian Linux is the primary qualified desktop path. Windows and macOS need repeatable setup, native dependency packaging, automated checks, release artifacts, and platform-specific acceptance testing before they can be presented as equally supported desktop releases.

### 5. Continue accessibility and responsive refinement

The shared UI foundation supports compact layouts, keyboard operation, reduced motion, safe-area-aware overlays, Android system insets, touch targets, theme tokens, and configurable typography. Remaining work is a systematic screen-by-screen audit, especially around dense analytics, document workflows, high interface scaling, and assistive technology on both desktop and mobile.

## Next product capabilities

These features should reuse the existing managed-document, semantic-outline, authoring, scheduling, and analytics boundaries rather than introducing separate one-off AI pipelines.

### Study guides

Generate structured, source-grounded study guides from one or more managed documents. Guides should retain source references, remain editable, and avoid presenting unsupported model additions as document facts.

### Practice tests and quizzes

Create configurable question sets, including multiple choice and application questions, with explanations and source grounding. Quiz results should complement rather than contaminate the flashcard review ledger unless a future design explicitly connects them.

### Interactive AI tutoring

Provide a conversational tutor that can answer questions from the learner's selected documents and study history. Tutoring must use a narrow task interface, respect the selected provider, show when evidence is missing, and avoid becoming an unreviewable source of durable scheduling or analytics state.

### Learning recommendations

Use deterministic analytics and optional AI interpretation to identify weak areas, overloaded schedules, neglected topics, and useful next actions. Stored review evidence remains authoritative; model-generated advice is explanatory rather than a hidden scoring system.

## Cross-device and platform direction

### User-controlled synchronization

The first synchronization implementation is local-first and explicit: one device creates a short-lived encrypted send offer and the other accepts it. Stable peer identity and saved baselines support later three-way merging without turning pairing into a permanent background subscription.

Future sync work may improve trusted-peer ergonomics, transfer efficiency, discovery, large-library performance, and additional platform adapters, but should preserve the current constraints: no mandatory proprietary cloud account, no silent credential transfer, no automatic synchronization of theme/UI preferences, and explicit conflict handling when both devices changed the same durable study state.

### iOS, Windows, and macOS

iOS still needs its Swift platform adapters and Xcode packaging. Windows and macOS need repeatable native qualification and signed packaging. All should reuse the platform/runtime boundaries proven by Linux and Android rather than fork shared study behavior.

### Android release distribution

Android can be built, installed, and emulator-tested from the repository. Public Play distribution still requires a permanent release signing key, Play App Signing enrollment, store metadata, privacy declarations, and a signed release qualification workflow.

## Later possibilities

Potential later work includes math and scientific notation, richer collaborative deck sharing, additional import formats, more provider adapters, audio-assisted cards, and optional local feedback effects. These ideas require their own acceptance criteria and privacy/accessibility review before they become committed work.

## Principles that constrain the roadmap

Future features should preserve the following commitments:

1. Core study behavior remains usable without an account or AI provider.
2. The learner controls where durable data lives and which provider receives an AI request.
3. Local configuration never silently falls back to a cloud service.
4. Model output is validated and cannot write directly to the database.
5. Scheduling and analytics remain reproducible from ostudy-owned data.
6. Released migrations and interchange formats remain readable through explicit compatibility paths.
7. Platform expansion should reuse shared domain behavior rather than fork the product into unrelated implementations.
8. Device synchronization must keep local UI/appearance preferences local and transfer sensitive API configuration only through explicit sender opt-in.
