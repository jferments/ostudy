# Architecture guardrails for agents

This document translates ostudy's current architecture into concrete implementation constraints. Read the [comprehensive architecture](../architecture/overview.md), relevant focused architecture documents, and accepted ADRs before changing a boundary described here.

## Trust and ownership boundaries

### React and TypeScript

The frontend owns presentation, transient interaction state, and communication of user intent. It may derive view-only values, but it must not become the authoritative implementation of persistence, scheduling, validation, analytics, AI transport, or synchronization merge.

Frontend-to-Rust invocation belongs only in typed domain gateway modules under `src/ipc/`. Screens, controllers, components, and tests must not call Tauri `invoke()` directly. `src/platform/` owns presentation-facing OS/WebView capability adapters. Shared UI under `src/ui/` must not import feature IPC/controllers or Tauri APIs.

Editable notes and study presentations are different contracts:

- `CardNote` represents editable source and owns note revision, tags, and card-type-specific fields;
- `CardPresentation` represents one rendered study/search identity with its card ID and independently scheduled presentation.

Do not create another generic `Flashcard`/`OrganizedCard` DTO that mixes source editing with scheduling/presentation.

Workflow-heavy screens use focused controllers. General controllers live under `src/controllers/`; feature-local controllers may be colocated when that is the established ownership pattern. Independent asynchronous reads use independent latest-request lanes. Obsolete successes **and** errors must not overwrite current state.

Do not apply stale-read semantics to durable mutations. Once Rust reports a committed mutation, a later projection refresh failure is a separate recoverable reconciliation problem. Never tell the user the write rolled back or allocate a new logical operation merely because the following read failed.

Presentational components should represent independently meaningful views or controls. Do not split components merely to satisfy a line count or create prop-forwarding fragments with no behavior/test boundary.

### Rust/application services

Rust owns untrusted-input validation, use-case orchestration, durable domain behavior, operation/revision policy, safe errors, and multi-step transaction boundaries. Treat every IPC payload, imported package, rich document, external-file handle, provider result, peer package, and identifier as untrusted until validated.

Application-owned DTOs/domain types isolate durable behavior from external crate representations. `rs-fsrs` remains behind `src-tauri/src/scheduler.rs`. Third-party parser/render/OCR/provider types are converted into ostudy-owned contracts before crossing their subsystem boundary.

### SQLite

SQLite owns durable structured state. Enforce invariants with schema constraints, explicit SQL, transactions, revisions, ledgers, and tests rather than relying on UI state.

Released public migration files/checksums and supported compatibility fixtures are immutable. Add a new numbered migration/repair and test fresh and supported upgrade paths. A historical migration name remains durable identity even if its implementation technology was later replaced.

### Managed files and secrets

Application-managed files own card/template/draft images, retained documents, canonical evidence/renders, staging, prepared exports, backup/sync workspaces, and diagnostics. Persist application-owned IDs and contained relative locations, never arbitrary absolute paths, Android content URIs, or remote image URLs as durable study content.

Filesystem and SQLite changes require staging/reconciliation; a SQL transaction does not roll back arbitrary file I/O. Startup recovery is part of the contract.

Saved provider credentials live behind the native secure-secret boundary; session-only keys live in native process memory. A newly typed key necessarily exists transiently in the UI form/write request, but saved credentials are not read back through ordinary profile DTOs or stored in learner SQLite/backups/deck packages/routine diagnostics.

### Platform code

Platform/native Kotlin or future Swift owns OS APIs such as document providers, protected storage, lifecycle, system Back, foreground/background execution, Sharesheets, camera/QR acquisition, PDF rendering/OCR, and signing/package behavior.

Platform code does **not** own SQLite migrations, card validation, FSRS scheduling, analytics attribution, package/import validation, document-learning prompts/schemas, provider fallback, or synchronization merge/conflict policy.

## Durable data invariants

- A note stores editable source; a card stores one studyable presentation generated from a note.
- Canonical rich content is validated versioned Tiptap/ProseMirror JSON. The transport-envelope version and canonical rich-document root version are independent compatibility contracts.
- Rust uses explicit node/mark/attribute allowlists, derives authoritative plain text, and validates managed assets/links.
- `card_schedules` stores mutable current FSRS state.
- `review_events` stores append-only historical review evidence while its owning material is retained.
- One review submission atomically validates session/card/reveal/schedule revision/submission identity, updates current schedule/session state, appends review evidence, and commits.
- Operation/submission identity prevents duplicate durable work after retries/lost acknowledgements.
- Scheduling controls such as suspend/bury/reset/reschedule do not fabricate review events.
- `.studydeck` shares one deck's content/managed assets with fresh local schedules and no donor review history.
- Generated `.ostudy.json` is strict content-only input, whole-file validated and transactional.
- `.ostudybackup` is complete personal-library recovery and excludes credential bytes.
- Trusted-device sync is an explicit encrypted one-way offer over an allowlisted graph, not a live database copy or automatic inclusion of every table.
- Analytics are deterministic projections over ostudy-owned schedules, sessions, review evidence, current relationships, and historical attribution.

## Current document and AI invariants

Document/AI/synchronization are implemented optional subsystems; do not describe them as merely hypothetical future networking.

- Core study remains usable without an account or AI provider.
- Supported source families normalize into ostudy-owned canonical document structures.
- PDF extraction/render/OCR and current non-PDF Xberg parsing remain isolated adapter boundaries.
- A reusable learning plan/stable topics can connect generated flashcards and Study Guides without using mutable display tags as provenance.
- Model output is untrusted assisted-authoring input and cannot write the database directly.
- Provider HTTP stays inside the native gateway; profile persistence, secure secrets, probes, provider dialects, and document tasks are separate owners.
- Implemented provider families are llama.cpp-compatible/Unsloth GGUF, DeepSeek, and OpenRouter. Catalog/planned entries are not implementation evidence.
- A local/LAN provider never silently falls back to cloud or another model/provider.
- Study Guide generated revisions and learner notes/overrides/conflicts have separate ownership; regeneration publishes atomically.
- The current Study Guide/document learning graph is broader than the present peer-sync allowlist; adding it requires an explicit versioned sync extension.

## UI and accessibility invariants

Current top-level destinations are Flashcards, Study Guides, Documents, Analytics, and Settings. Practice is an internal contextual workflow. Flashcards has Decks and Search/Filter; Card templates live in Settings; transfer is contextual.

Responsive presentation can move an action but cannot change its meaning, stable identity, authorization, destructive status, accessible name, or domain operation. Primary next steps stay discoverable. Empty states omit controls with no meaningful target rather than presenting filler tiles/disabled toolbars.

Use shared Dialog/Sheet/Menu/confirmation/selection/action patterns. Preserve keyboard, focus, platform Back, touch/coarse pointer, non-drag alternatives, safe areas, software-keyboard containment, themes, enlarged typography, reduced motion, and forced-colors behavior.

## Local-first and privacy constraints

Core study must remain useful without a network, hosted database, telemetry service, or paid API. Optional network/AI features are explicit, bounded, and must fail without corrupting/blocking unrelated local study.

Never add remote loading of card images/rich content merely as a convenience. Do not log raw prompts, provider responses, reasoning, source/OCR text, credential bytes, or personal managed files in routine diagnostics.

Privacy claims must name their channel. Validated source/generated learning content is intentionally stored in the local domain graph; saying it “never enters SQLite” would be false. The relevant rule is that raw transport/secrets and unrelated private content do not cross unnecessary boundaries.

## Compatibility constraints

Durable compatibility surfaces include:

- database lineage, migrations, repairs, and supported upgrade/adoption readers;
- rich-content schema/version and validation taxonomy;
- `.studydeck`, `.ostudy.json`, `.ostudybackup` versions;
- device-sync protocol/package/allowlist/baselines/conflicts;
- UI checkpoint readers;
- scheduler rating mapping/review history/idempotency;
- canonical document/checkpoint/prompt/Study Guide revision contracts;
- Tauri application identifier/data location;
- user-visible keyboard/accessibility/theme semantics where documented.

The bundled React/Rust IPC layer is internal unless explicitly documented externally. Unreleased/internal behavior can be removed end-to-end after tracing callers. Do not keep an obsolete command/component/test/workflow merely because it existed in an old PR. Conversely, do not remove a released reader, migration, historical fixture, or real platform fallback because its name looks old.

Durable compatibility changes require explicit acceptance criteria, regression coverage, migration/reader/rollback analysis, and documentation. Durable cross-module decisions require an ADR or explicit supersession of an existing ADR.

## Change-impact questions

Before implementation, answer:

1. Which entity/layer owns the behavior?
2. Does it cross React/Rust, Rust/SQLite, database/filesystem, secure-store, provider, peer, or platform boundaries?
3. Does it alter a persistent/versioned format, stable identifier, command, error, or checkpoint?
4. Can a retry/double click/late callback duplicate or misapply durable work?
5. Can partial failure leave database/files/secrets out of agreement?
6. Can an older async read overwrite newer scope/state?
7. Does a successful mutation remain distinguishable from a failed follow-up refresh?
8. Does it affect import/export/backup/sync/deletion/recovery?
9. Does it affect keyboard/touch/compact/Back/focus/motion/assistive behavior?
10. What is the narrowest regression, what architecture tripwire is appropriate, and what broader platform/manual evidence is required?
