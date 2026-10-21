# Project roadmap

ostudy is being developed in layers, but the roadmap is not a list of promises or a substitute for active source. Reliability, privacy, accessibility, data compatibility, and recovery problems take priority over adding another visible feature.

## Current foundation

The active codebase already includes:

- a local SQLite library for subjects, decks, notes, studyable card presentations, schedules, review history, sessions, a study activity ledger, settings, and documents;
- a five-destination shell (Home, Library, Tutor, Progress, Settings) with Search in the header: Home's Today strip, lessons to pick up, suggestions, week at a glance, subjects, Library organized by subject with its Overview, Lessons, Study guides, Documents, Notes and Flashcards (the Workbench) tabs, Session complete, and the Progress page;
- rich Basic, Reverse, and Fill in the blank authoring plus native single-choice, select-all, true/false, matching, grouping, ordering, and image-occlusion questions; validated math, managed image/audio/video content, reusable templates, durable drafts, tags, full-text card search and filters, duplicate detection, and bounded bulk work;
- FSRS-backed scheduling with recorded scheduler context and active study time, exact/deck/subject/scheduled/extra study scopes, suspend/bury/reset/reschedule controls, daily limits, a configurable study-day start, resumable practice sessions, and deterministic Progress statistics built from one set of learning-metric definitions;
- contextual `.studydeck` exchange, strict generated `.ostudy.json` import, format-4 complete `.ostudybackup` recovery, and explicit encrypted private-LAN/direct-USB trusted-device synchronization with three-way merge/conflict records;
- the release-v1 schema-1 baseline advanced by reviewed release migrations (currently schema 9) with a per-version upgrade preservation contract and pre-upgrade checkpoints, preserved historical readers/conversion, production/development isolation, and journaled same-process complete-library restoration;
- retained PDF, Word, PowerPoint, OpenDocument, EPUB, RTF, Excel/ODS, and CSV sources normalized into one canonical document model;
- native PDF extraction/rendering with ruled-table rebuilding, page images for table/figure/OCR pages, and OCR on Linux and Android, with Xberg as the current non-PDF parser boundary;
- a PDF annotator (ink, highlighter, shapes and text notes over the page renders, with sharp zoomed tiles) whose marks stay out of the source file and can be saved into an annotated copy;
- source-grounded AI-assisted flashcard generation and durable interactive Study Guides with stable learning topics, learner notes/section overrides, conflicts, linked-card Practice, cancellation/checkpoints, and local PDF export;
- a native lesson store, AI course writing for a subject (one lesson per top-level document topic), a step-by-step lesson player with native check grading, narration spoken by an OpenRouter voice and kept in the library, and a lecture view that shows a lesson as slides with the narration as speaker notes and saves it as an editable PowerPoint deck;
- implemented AI profiles for llama.cpp-compatible/Unsloth GGUF servers, DeepSeek, OpenRouter, and Claude, with native credential isolation/capability probing and no silent local-to-cloud fallback;
- a first-class conversational Tutor with source-optional chats, multi-source document/deck/guide/subject grounding, conversation history/projects, saved-knowledge retrieval, private-answer safeguards, and no authority to rewrite FSRS schedules or review evidence;
- one React/Rust application across desktop and Android, with wide/medium/compact navigation, shared accessible overlays/patterns, themes, local typography, reduced motion, safe areas, and keyboard/touch behavior;
- a qualified Ubuntu/Debian Linux path and a qualified Android source-build/debug-install path with API 36 emulator qualification;
- tiered exact-SHA qualification on an isolated self-hosted Linux/Android runner.

Historical stage plans under [`history/`](history/) explain how this foundation was built; they are not the current roadmap. Platform qualification always belongs to a named source/run and does not imply signed public distribution or untested physical-device behavior.

## In progress

The PR #106 foundation and the H-01–H-10 hardening scopes in the [post-#106 hardening register](development/post-106-hardening.md) are merged; the PR #106 [completion record](development/lk-followups/consolidation-qualification.md) separates automated qualification, recorded disposable installed-copy acceptance and the still-separate protected-library cutover.

Work under way in open pull requests, not on `main`, includes the OpenAI and Gemini provider adapters. None of it is current behavior until it merges. Podcasts and local voices remain planned; the [preserved lesson plan](development/lk-followups/lessons-narration.md) records an earlier design. Such work should consume the shared source/knowledge/guide boundaries rather than duplicate them.

## Current priorities

### 1. Harden document processing and generated-study quality

The document and Study Guide stack is one of the newest and most complex areas. Continue improving:

- extraction/provenance across representative PDF, Office/OpenDocument, EPUB, RTF, and spreadsheet files;
- OCR recovery for scanned, rotated, low-contrast, multi-column, table-heavy, and unusual documents;
- provider compatibility, structured-output reliability, cancellation, timeout/retry/checkpoint behavior, and safe diagnostics;
- source-grounded card quality, guide concision, topic coverage, linked-card relevance, and export fidelity;
- large-document memory, concurrency, progress, battery, and foreground/background behavior;
- explicit failure/degradation when the source/parser/model cannot support a trustworthy result.

Assisted authoring should surface uncertainty rather than turn imperfect extraction or model output into unquestionable facts. Keep generation progress, elapsed time, Stop and useful token/cache cost observations discoverable; optional technical detail is not a reason to hide user cost control.

### 2. Refine everyday Flashcards and Study Guide ergonomics

Recent UI work replaced the old Flashcards, Documents and Analytics destinations with Home, the Library Workbench, Review and Progress, quieted empty states, made transfer/actions contextual, and moved templates into Settings. Preserve that hierarchy while improving:

- large-library subject/deck organization and selection;
- narrow/tablet/keyboard/touch/assistive-technology behavior;
- richer managed-image sizing/alignment;
- multi-draft visibility/recovery;
- Study Guide reading, annotation, conflicts, source navigation, and linked-card workflows;
- a carefully designed undo-last-review operation that restores exact prior schedule state without falsifying historical evidence.

### 3. Qualify synchronization on more real devices and libraries

The protocol, encrypted transport, package validation, merge engine, baselines, and conflicts are implemented. Package v3 already has explicit Documents and Study Guides scopes. Continue with:

- broader physical-device/network/USB combinations;
- clearer conflict review/resolution;
- large libraries, long histories, many assets, interruption/expiry/low-storage conditions;
- transfer efficiency without weakening explicit one-way offers;
- separately reviewed, versioned extensions for additional graph entities beyond the current allowlist, such as PDF page annotations and lessons, without including private Tutor state or derived caches by default.

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

The project has explicit frontend/native/platform boundaries and one ordinary branch-and-PR workflow for people and coding agents. Keep:

- one writer per task branch;
- no force-pushes to shared branches and no direct pushes to `main`;
- source-read-only Actions;
- exact-SHA product `Check` results that end with a focused failure summary;
- focused failure reproduction before broad reruns;
- completed stage terminology, checkpoint workflows, and temporary orchestration machinery out of current architecture.

## Next product capabilities

These are directions, not current feature claims. They should reuse managed documents, learning topics, scheduling, analytics, and provider boundaries rather than creating parallel pipelines.

### Practice tests and quizzes

Generate configurable question sets, including multiple choice/application/scenario questions, with explanations and source grounding. Quiz attempts need their own explicit evidence model and must not silently contaminate the flashcard review ledger.

### Lessons

A subject's course can be written, played step by step, listened to, viewed as a lecture and saved as a PowerPoint deck. Two-host podcasts, local voices (a local Kokoro server and the phone's own speech engine), rewriting an existing course, keeping lessons current when their sources change, and sync of lessons remain planned. Lesson progress must stay a resume point that never writes reviews or changes schedules.

### Tutor refinement and guided study

Tutor already provides conversational study with optional source grounding, projects/history, saved-knowledge retrieval, and contextual entry from supported workflows. Continue improving source-grounded explanations, multi-source navigation, question practice inside conversation, retrieval transparency, interruption/retry behavior, and learner-controlled memory. Tutor must continue using the explicitly selected provider, identify missing source evidence when the request requires grounding, and remain unable to rewrite scheduling/analytics state as an unreviewable side effect.

### Learning recommendations

Use deterministic Progress statistics plus optional model interpretation to identify weak areas, overloaded schedules, neglected topics, and useful next actions. Stored review evidence remains authoritative; model advice is explanatory rather than a hidden scoring system.

### Additional provider adapters

Dedicated Ollama, OpenAI, and Gemini integrations remain planned; llama.cpp, DeepSeek, OpenRouter, and Claude are implemented. A provider is not supported merely because its API resembles an existing endpoint. Each adapter needs fixed trust rules, credential handling, discovery, structured-output evidence, diagnostics redaction, fixtures, and opt-in live testing.

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

Potential later work includes recording and speech-grading workflows, narration/TTS-assisted study, richer collaborative sharing, additional imports, and optional local feedback effects. These future additions require their own acceptance criteria, durable ownership, privacy analysis, accessibility review, and compatibility plan. Mathematical/scientific notation and managed audio/video card content are already part of the current foundation.

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
