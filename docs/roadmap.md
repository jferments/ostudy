# Project roadmap

ostudy is being developed in layers: first a dependable local study library, then document-aware AI tools, then broader platform and synchronization support. This roadmap describes direction rather than promising release dates. Priorities may change when testing exposes a more important reliability, privacy, accessibility, or data-compatibility problem.

## Current foundation

The following capabilities are already present in the active codebase:

- a local SQLite library for decks, notes, scheduled card presentations, review history, settings, and analytics;
- Basic, Reverse, Fill in the blank, and reusable-template authoring;
- rich card content with managed local images and strict validation;
- tags, full-text search, filters, bulk entry, duplicate detection, and move/copy workflows;
- FSRS scheduling, workload controls, voluntary and due-only study, and detailed learning analytics;
- portable deck exchange, generated JSON import, and complete personal-library backup;
- configurable llama.cpp/Unsloth and DeepSeek provider profiles with native credential and network boundaries;
- a managed PDF library and an initial document-to-flashcard generation workflow;
- responsive shared UI foundations, themes, keyboard behavior, and accessibility primitives;
- tiered local and self-hosted CI validation.

Historical stage plans are kept under [`history/`](history/) for project archaeology. They are not the current roadmap.

## Current priorities

### 1. Stabilize document and PDF processing

PDF generation is the newest large subsystem and remains the most important reliability target. Work in this area includes improving extraction quality, OCR recovery, provider compatibility, progress reporting, cancellation, retries, diagnostic clarity, generated-card quality, and behavior on large or unusual documents. The goal is not merely to make one test PDF succeed, but to make failures understandable and recoverable without corrupting the local library.

### 2. Complete everyday authoring and review ergonomics

Planned improvements include autosaved editor drafts, interrupted-edit recovery, richer image sizing and alignment, and a carefully designed undo-last-review operation that restores an exact prior schedule state without rewriting historical evidence incorrectly.

### 3. Broaden desktop qualification

Ubuntu/Debian Linux is currently the primary qualified path. Windows and macOS need repeatable setup, native dependency packaging, automated checks, release artifacts, and platform-specific acceptance testing before they can be presented as equally supported desktop releases.

### 4. Continue accessibility and responsive refinement

The shared UI foundation already supports compact layouts, keyboard operation, reduced motion, safe-area-aware overlays, and theme tokens. The remaining work is a systematic screen-by-screen audit, especially around dense analytics, document workflows, touch interaction, high interface scaling, and assistive technology.

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

Synchronization between a learner's own devices is planned. The design must preserve local ownership, support reliable conflict resolution, avoid requiring a single proprietary service, and protect credentials and private study material. The exact transport and encryption design has not yet been accepted, so current documentation does not claim that sync exists.

### Android and iOS

Native mobile applications are planned after the document, storage, and desktop boundaries are stable enough to support platform adapters cleanly. Mobile work includes filesystem/content-URI handling, native PDF rendering and OCR, lifecycle-safe long-running tasks, backup/share sheets, touch-first navigation, and platform packaging. No installable Android or iOS release is currently available.

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
