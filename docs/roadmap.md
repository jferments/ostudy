# Project roadmap

ostudy is being developed in layers, but the roadmap is not a list of promises or a substitute for active source. Reliability, privacy, accessibility, data compatibility, and recovery problems take priority over adding another visible feature.

## Current foundation

ostudy 0.1.0 (tag `v0.1.0`) is released for Linux as `.deb`, `.rpm`, Flatpak, AppImage and a portable folder. `main` has moved on since then; this list describes `main`.

- **Study shell.** Five destinations (Home, Library, Notes, Tutor, Progress) with Search and Settings at the foot of the side rail or in a phone's top bar. Home holds the question of the day, the Today strip and Start review, the current lesson's slide, lessons to pick up, suggestions, the week at a glance and the subjects. Library opens on subject tiles; a subject's pages are its Lessons, Flashcards (the Workbench), Study guides, Podcasts and Documents tabs. New study set is the one place a subject starts.
- **Library and scheduling.** A local SQLite library (release schema 26, migrated forward from the immutable release-v1 baseline with pre-upgrade checkpoints) for subjects, decks, notes, card presentations, FSRS schedules, review history, a study activity ledger, documents, guides, lessons, notebooks and settings. Exact, deck, subject, scheduled and extra study scopes; suspend, bury, reset and reschedule; daily limits; a configurable study-day start; resumable sessions; deterministic Progress statistics from one set of metric definitions.
- **Cards.** Rich Basic, Reverse and Fill in the blank authoring; native single best answer, select all, true/false, matching, grouping, ordering and image-occlusion questions checked in Rust; math and chemistry notation; managed images, audio and video; templates, drafts, tags, search, filters, duplicate checks and bulk work.
- **Documents.** PDF, Word, PowerPoint, OpenDocument, EPUB, RTF, spreadsheets, CSV and image sets kept as managed copies and read into one canonical model on the device, with ruled-table rebuilding, OCR on desktop and Android, and figure finding in PDFs.
- **Annotation and notes.** A PDF annotator (pen, highlighter that snaps to text, shapes, curves, text and sticky notes, lasso, partial erase, clips) whose marks stay out of the source and can be saved into an annotated copy; flashcards from marks and Study marked cards. Notes: notebooks of pages and endless boards written with the same tools, pictures, links to study materials, folders and pins, mind maps, lines that stay attached, a document beside the notebook, and PDF export.
- **AI-made study material** (optional). Flashcards sized to each page's key ideas; designed study guides with boxes and tables beside the text, source figures, one Sources line per section, a cram sheet, cited key points, self-test and Typst PDF export, read as an annotatable canvas; lessons shown as natively typeset lecture slides with a teacher's lecture and questions, PowerPoint export and slide cards; two-host podcasts. A New study set can start from documents, a prompt, or both.
- **Tutor** (optional). Source-optional chats grounded in documents, decks, guides and subjects; projects and history; quiz question cards graded in Rust where possible; picture and text attachments; figures from documents; read aloud; on-device speech input; flashcards from an answer; no authority over schedules or review evidence.
- **AI services.** llama.cpp / Unsloth GGUF servers, DeepSeek, OpenRouter and Claude, with a first-run setup wizard, per-task models, model tests, native credential isolation and no silent local-to-cloud fallback. Voices through OpenRouter (Drama 3 with saved voices, Kokoro) or free on the device (Kokoro). Builds made with `VITE_OSTUDY_AI=off` show no AI features, and every build hides them until a service is set up.
- **Your data.** Contextual `.studydeck` exchange, Anki `.apkg` and Quizlet text import, strict `.ostudy.json` import, complete `.ostudybackup` backup with same-process restore, and encrypted one-way device sync over the local network or a USB cable, with selectable Flashcards, Documents, Study guides, Notes, Lessons, AI services and saved-key scopes and three-way merge.
- **Platforms and qualification.** One React/Rust application on Linux and Android with wide, medium and compact layouts, four themes, reduced motion, safe areas and keyboard/touch support. Tiered exact-SHA checks on an isolated self-hosted Linux/Android runner, with Android emulator coverage.

Historical stage plans under [`history/`](history/) explain how this foundation was built; they are not the current roadmap. Platform qualification always belongs to a named source/run and does not imply signed public distribution or untested physical-device behavior.

## In progress

Draft pull requests, not on `main` and not current behavior until they merge:

- mind-map webs: cross-links between topics and floating topics (#285);
- OpenAI (#158) and Gemini (#159) provider adapters;
- a second runner lane for draft checks (#185).

Release work continues outside the code: a signed Android download, then Windows and macOS packages.

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

Recent UI work replaced the old Flashcards, Documents and Analytics destinations with Home, Library's subject pages, Notes and Progress, folded Review into Home, quieted empty states, made transfer/actions contextual, and moved templates into Settings. Preserve that hierarchy while improving:

- large-library subject/deck organization and selection;
- narrow/tablet/keyboard/touch/assistive-technology behavior;
- richer managed-image sizing/alignment;
- multi-draft visibility/recovery;
- Study Guide reading, annotation, conflicts, source navigation, and linked-card workflows;
- a carefully designed undo-last-review operation that restores exact prior schedule state without falsifying historical evidence.

### 3. Qualify synchronization on more real devices and libraries

The protocol, encrypted transport, package validation, merge engine, baselines, and conflicts are implemented, with explicit Documents, Study guides, Notes and Lessons scopes that carry annotations, figures, notebooks and lessons. Continue with:

- broader physical-device/network/USB combinations;
- clearer conflict review/resolution;
- large libraries, long histories, many assets, interruption/expiry/low-storage conditions;
- transfer efficiency without weakening explicit one-way offers;
- separately reviewed, versioned extensions for additional graph entities beyond the current allowlist, such as Notes folders and pins, without including private Tutor state or derived caches by default.

Synchronization remains user initiated: pairing must not become a permanent background subscription and credential transfer must require fresh sender opt-in for each offer.

### 4. Broaden desktop qualification and release packaging

Linux packages for every maintained distribution ship from one release script (built in an Ubuntu 22.04 container) and are published on ostudy.io and GitHub Releases. Windows and macOS need repeatable setup, native-dependency packaging, automated checks, signed artifacts, upgrade testing, and target-specific acceptance before they are presented as equally supported releases.

### 5. Continue Android product and distribution work

Android already builds, installs, and runs the shared study/document/sync application from source. Remaining work includes:

- broader physical phone/tablet coverage;
- accessibility, camera, storage-provider, background-processing, thermal, memory, and battery testing;
- permanent release-key policy and signing custody;
- Play App Signing, store metadata, privacy declarations, and signed-release qualification;
- F-Droid and upgrade testing once public distribution exists.

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

Lessons, lectures, the teacher's questions, PowerPoint export, podcasts, on-device Kokoro voices and lesson sync are implemented. The phone's own speech engine as a voice remains planned. Lesson progress must stay a resume point that never writes reviews or changes schedules.

### Tutor refinement and guided study

Tutor already provides conversational study with optional source grounding, projects/history, saved-knowledge retrieval, and contextual entry from supported workflows. Continue improving source-grounded explanations, multi-source navigation, question practice inside conversation, retrieval transparency, interruption/retry behavior, and learner-controlled memory. Tutor must continue using the explicitly selected provider, identify missing source evidence when the request requires grounding, and remain unable to rewrite scheduling/analytics state as an unreviewable side effect.

### Learning recommendations

Use deterministic Progress statistics plus optional model interpretation to identify weak areas, overloaded schedules, neglected topics, and useful next actions. Stored review evidence remains authoritative; model advice is explanatory rather than a hidden scoring system.

### Notes and mind maps

Mind-map webs are in review (#285). AI-made mind maps from documents are deferred until the maintainer asks for them. Handwriting recognition and sync of Notes folders remain later work.

### Additional provider adapters

Dedicated OpenAI and Gemini adapters are in draft pull requests and Ollama remains planned; llama.cpp, DeepSeek, OpenRouter, and Claude are implemented. A provider is not supported merely because its API resembles an existing endpoint. Each adapter needs fixed trust rules, credential handling, discovery, structured-output evidence, diagnostics redaction, fixtures, and opt-in live testing.

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

Potential later work includes recording and speech-grading workflows, richer collaborative sharing, additional imports, and optional local feedback effects. These future additions require their own acceptance criteria, durable ownership, privacy analysis, accessibility review, and compatibility plan. Mathematical/scientific notation and managed audio/video card content are already part of the current foundation.

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
