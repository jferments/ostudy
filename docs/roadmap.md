# Project roadmap

ostudy is being developed in layers, but the roadmap is not a list of promises or a substitute for active source. Reliability, privacy, accessibility, data compatibility, and recovery problems take priority over adding another visible feature.

## Current foundation

The active codebase already includes:

- a local SQLite library for folders, decks, notes, studyable card presentations, schedules, review history, sessions, settings, and analytics;
- rich Basic, Reverse, and Fill in the blank authoring, answer-key/task-list support, managed images, reusable templates, durable drafts, tags, full-text Search/Filter, duplicate detection, and bounded bulk work;
- FSRS-backed scheduling, exact/deck/scheduled/extra study scopes, suspend/bury/reset/reschedule controls, daily limits, resumable practice sessions, and deterministic analytics;
- contextual `.studydeck` exchange, strict generated `.ostudy.json` import, complete `.ostudybackup` recovery, and explicit encrypted private-LAN trusted-device synchronization with three-way merge/conflict records;
- retained PDF, Word, PowerPoint, OpenDocument, EPUB, RTF, Excel/ODS, and CSV sources normalized into one canonical document model;
- native PDF extraction/rendering and selective OCR on Linux and Android, with Xberg as the current non-PDF parser boundary;
- source-grounded AI-assisted flashcard generation and durable interactive Study Guides with stable learning topics, learner notes/section overrides, conflicts, linked-card Practice, cancellation/checkpoints, and local PDF export;
- implemented AI profiles for llama.cpp-compatible/Unsloth GGUF servers, DeepSeek, and OpenRouter, with native credential isolation/capability probing and no silent local-to-cloud fallback;
- one React/Rust application across desktop and Android, with wide/medium/compact navigation, shared accessible overlays/patterns, themes, local typography, reduced motion, safe areas, and keyboard/touch behavior;
- a qualified Ubuntu/Debian Linux path and a qualified Android source-build/debug-install path with API 36 emulator qualification;
- tiered exact-SHA qualification on an isolated self-hosted Linux/Android runner.

Historical stage plans under [`history/`](history/) explain how this foundation was built; they are not the current roadmap.

## Current priorities

### 1. Harden document processing and generated-study quality

The document and Study Guide stack is one of the newest and most complex areas. Continue improving:

- extraction/provenance across representative PDF, Office/OpenDocument, EPUB, RTF, and spreadsheet files;
- OCR recovery for scanned, rotated, low-contrast, multi-column, table-heavy, and unusual documents;
- provider compatibility, structured-output reliability, cancellation, timeout/retry/checkpoint behavior, and safe diagnostics;
- source-grounded card quality, guide concision, topic coverage, linked-card relevance, and export fidelity;
- large-document memory, concurrency, progress, battery, and foreground/background behavior;
- explicit failure/degradation when the source/parser/model cannot support a trustworthy result.

Assisted authoring should surface uncertainty rather than turn imperfect extraction or model output into unquestionable facts.

### 2. Refine everyday Flashcards and Study Guide ergonomics

Recent UI work removed permanent low-frequency workspaces, quieted empty states, made transfer/actions contextual, and moved templates into Settings. Preserve that hierarchy while improving:

- large-library folder/deck organization and selection;
- narrow/tablet/keyboard/touch/assistive-technology behavior;
- richer managed-image sizing/alignment;
- multi-draft visibility/recovery;
- Study Guide reading, annotation, conflicts, source navigation, and linked-card workflows;
- a carefully designed undo-last-review operation that restores exact prior schedule state without falsifying historical evidence.

### 3. Qualify synchronization on more real devices and libraries

The protocol, encrypted transport, package validation, merge engine, baselines, and conflicts are implemented. Continue with:

- broader physical-device/network combinations;
- clearer conflict review/resolution;
- large libraries, long histories, many assets, interruption/expiry/low-storage conditions;
- transfer efficiency without weakening explicit one-way offers;
- a separately versioned extension before Study Guide/topic/document artifact graphs become part of peer synchronization.

Synchronization remains user initiated: pairing must not become a permanent background subscription and credential transfer must require fresh sender opt-in for each offer.

### 4. Broaden desktop qualification and release packaging

Ubuntu/Debian Linux remains the primary qualified desktop path. Windows and macOS need repeatable setup, native-dependency packaging, automated checks, signed artifacts, upgrade testing, and target-specific acceptance before they are presented as equally supported releases.

### 5. Continue Android product and distribution work

Android already builds, installs, and runs the shared study/document/sync application from source. Remaining work includes:

- broader physical phone/tablet coverage;
- accessibility, camera, storage-provider, background-processing, thermal, memory, and battery testing;
- permanent release-key policy and signing custody;
- Play App Signing, store metadata, privacy declarations, and signed-release qualification;
- upgrade testing once public distribution exists.

### 6. Maintain architecture and development reliability

The project has explicit frontend/native/platform boundaries and a GitHub-direct development path that does not depend on custom developer MCP availability. Keep:

- one writer per task branch;
- expected-parent non-force publication and remote verification;
- source-read-only Actions;
- `PR Contract` metadata validation independent from exact-SHA product `Check`;
- focused failure reproduction before broad reruns;
- completed stage terminology, checkpoint workflows, and temporary orchestration machinery out of current architecture.

## Next product capabilities

These are directions, not current feature claims. They should reuse managed documents, learning topics, scheduling, analytics, and provider boundaries rather than creating parallel pipelines.

### Practice tests and quizzes

Generate configurable question sets, including multiple choice/application/scenario questions, with explanations and source grounding. Quiz attempts need their own explicit evidence model and must not silently contaminate the flashcard review ledger.

### Interactive AI tutoring

Provide conversational tutoring over selected documents and learner context through a narrow task interface. Tutoring must use the explicitly selected provider, show when source evidence is missing, and avoid becoming an unreviewable writer of scheduling/analytics state.

### Learning recommendations

Use deterministic analytics plus optional model interpretation to identify weak areas, overloaded schedules, neglected topics, and useful next actions. Stored review evidence remains authoritative; model advice is explanatory rather than a hidden scoring system.

### Additional provider adapters

Dedicated Ollama, OpenAI, Gemini, and Claude integrations remain planned. A provider is not supported merely because its API resembles an existing endpoint. Each adapter needs fixed trust rules, credential handling, discovery, structured-output evidence, diagnostics redaction, fixtures, and opt-in live testing.

## Platform direction

### iOS

iOS still needs a generated project, Swift/native adapters, secure credentials, file/share behavior, lifecycle/background execution, native PDF/OCR decisions, signing, and Xcode packaging. It should reuse shared domain behavior and the platform ports proven by Linux/Android rather than becoming a separate product fork.

### User-controlled synchronization

Future sync work can improve peer ergonomics, discovery, efficiency, conflict review, and allowed entity scope while preserving:

1. no mandatory proprietary cloud account;
2. no silent continuous replication;
3. no automatic transfer of appearance/device preferences;
4. no credential transfer without fresh sender opt-in;
5. no unvalidated live-database copying;
6. explicit conflicts when both devices changed the same durable state.

## Later possibilities

Potential later work includes mathematical/scientific notation, audio-assisted cards, richer collaborative sharing, additional imports, and optional local feedback effects. These require their own acceptance criteria, durable ownership, privacy analysis, accessibility review, and compatibility plan.

## Principles that constrain future work

1. Core study remains usable without an account or AI provider.
2. The learner controls durable data and which provider receives a task.
3. Local/LAN profiles never silently fall back to cloud.
4. Model output is untrusted assisted-authoring input and cannot write directly to the database.
5. Scheduling and analytics remain reproducible from ostudy-owned evidence.
6. Released migrations, backups, and interchange formats remain readable through explicit compatibility paths.
7. Platform expansion reuses shared domain behavior rather than forking scheduling/import/analytics rules.
8. Long-running work uses bounded resources, cancellation, checkpoints, and startup reconciliation.
9. Responsive simplification changes presentation, not feature meaning, authorization, data ownership, or accessibility.
10. Repository workflow separates source publication, PR metadata, exact-head qualification, manual acceptance, and human merge authority.
