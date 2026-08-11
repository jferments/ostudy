# Architecture overview

ostudy is a local-first Tauri application. React and TypeScript render the interface; Rust owns durable behavior and validation; SQLite stores structured state; and application-managed directories store images, source documents, rendered pages, and other controlled file bytes. Optional AI features connect to a provider that the learner explicitly configures. ostudy does not install a model server or make AI a prerequisite for ordinary study.

This document explains the major ownership boundaries and how they fit together. Detailed card, UI, AI/document, dependency, and decision records are linked where the implementation needs a narrower reference.

## Design goals

The architecture is shaped by several product commitments:

- core flashcards, practice, search, analytics, import, and backup work without an account or AI provider;
- local data remains authoritative and inspectable;
- model output cannot bypass ordinary validation or write directly to storage;
- a local provider is never silently replaced with a cloud provider;
- scheduling and analytics are deterministic consequences of stored ostudy data;
- imports and rich content are treated as untrusted data rather than executable application code;
- desktop and future mobile targets share domain behavior while isolating operating-system capabilities;
- released migrations and interchange formats evolve through explicit compatibility paths.

## Runtime layers

```text
React feature screens
        ↓
feature controllers
        ↓
typed IPC gateways
        ↓
Tauri command boundary
        ↓
Rust domain and task services
        ↓
SQLite + managed files + native platform services
```

The direction is deliberate. Screens express user intent and render state. Controllers coordinate workflows and protect the interface from stale asynchronous results. Typed IPC gateways are the only frontend modules that invoke native commands. Rust validates every request, applies domain rules, owns transactions, and returns bounded DTOs. SQLite and the managed filesystem are implementation details of the native layer, not data stores that React accesses directly.

## Frontend

### Feature screens

`src/screens/` contains product workspaces such as Decks, Search/Filter, Practice, Analytics, Templates, Documents, and Settings. Screens compose domain controllers and shared UI primitives; they do not execute SQL, calculate authoritative schedules, or contact an AI provider.

### Controllers

`src/controllers/` owns workflow-level behavior that is too stateful for a presentational component:

- native reads and mutations;
- independent stale-read lanes;
- operation and retry identity;
- committed-success versus failed-reconciliation reporting;
- navigation and launch transitions;
- close or practice-exit guards.

An obsolete read may finish in Rust, but it may not overwrite newer interface state. Durable writes are different: once a mutation commits, a later refresh failure is reported as reconciliation trouble rather than inviting the same write to be submitted again.

### Typed IPC gateways

Direct Tauri `invoke()` calls live under `src/ipc/`, split by domain. Repository verification compares the frontend command names with the native `generate_handler!` registry and rejects feature code that bypasses the gateway layer.

The AI gateway exposed to React remains task-specific. There is no generic “send any prompt” command, no credential readback command, and no frontend provider networking.

### Shared UI and platform adapters

`src/ui/` provides accessibility and interaction primitives: overlays, confirmation, tabs, menus, buttons, fields, panels, notices, loading/empty states, semantic feedback, and design foundations. Shared UI cannot import IPC, feature controllers, or Tauri APIs.

`src/platform/` isolates operating-system and WebView capabilities such as file selection, external links, platform Back, pointer/capability detection, audio, and haptics. This allows a future Android content-URI adapter or mobile feedback implementation to replace a platform service without rewriting the Decks or Settings workflow.

See [UI architecture](ui.md) for overlay lifecycle, responsive layouts, accessibility, themes, and styling rules.

## Native composition and startup

`src-tauri/src/lib.rs` is the native composition root. Startup performs durable recovery before exposing the application:

1. resolve application paths;
2. validate and apply a pending whole-library restore when present;
3. open and migrate SQLite;
4. run missing versioned data repairs once;
5. initialize the AI runtime without contacting a provider;
6. finalize expired deletion recovery windows;
7. purge history owned by finalized deletions;
8. reconcile managed files after interrupted operations;
9. publish application, database, and native service state;
10. register the typed command surface.

A durable startup failure stops initialization instead of exposing a partially migrated or reconciled library. A temporarily unavailable credential store does not prevent non-AI study features from starting; affected secret operations fail explicitly or use an intentional session-only credential.

## Domain model

The most important ownership distinction is between editable source and scheduled presentation:

```text
deck
  └── note (editable source)
       ├── card presentation(s)
       │    └── current schedule
       ├── tags
       └── managed asset references

practice session
  └── stable session-card snapshot

review event
  └── immutable reviewed card/presentation identity
```

A Basic note may own forward and reverse cards with independent schedules. A Fill in the blank note owns one stable card whose selected blank is fixed for the active practice session. Notes own editable content and tags; cards own the identities that can be due, suspended, buried, selected, and reviewed.

See [Card format and data model](card-format-and-data-model.md) for rich content, templates, search, generated imports, and interchange formats.

## Persistence and transactions

SQLite stores structured domain state. Managed directories store controlled file bytes such as card images, retained source PDFs, and rendered document pages. SQLite records relative paths, ownership, size, MIME type, and integrity metadata rather than allowing arbitrary local paths in card or import content.

Command or service orchestrators own logical transactions. Helpers accept a connection or transaction and do not commit secretly. Multi-card scheduling operations, generated imports, deck transfer, review submission, and generated-card persistence validate their complete logical unit before commit.

Long-lived compatibility is split into two mechanisms:

- **schema migrations** append SQL structure changes and are checksum registered;
- **versioned data repairs** rewrite already stored data once, under their own stable ID, version, checksum, and transaction.

Released migrations and release fixtures are immutable evidence. Current code adapts old storage into the canonical model rather than rewriting historical files.

## Scheduling and practice

`src-tauri/src/scheduler.rs` is the adapter around `rs-fsrs`. The external library does not define the application's durable schema or IPC contracts.

- `card_schedules` stores mutable current scheduling state.
- `review_events` is the append-only historical ledger.
- review submission validates the previewed schedule revision and commits the review plus schedule update transactionally;
- operation and submission IDs make ambiguous retries idempotent;
- session-card snapshots keep the practiced identity and cloze variant stable;
- studyability is determined by archive, suspension, burial, and due rules before a timestamp is treated as ready.

Local-calendar behavior is owned natively so “today,” “tomorrow,” and longer date windows respect time zones, daylight-saving transitions, leap days, and calendar semantics rather than fixed browser-duration arithmetic.

## Analytics

Analytics are deterministic projections of stored sessions and review evidence. Current inventory and historical evidence remain separate so that library changes have predictable effects:

- moving a note transfers its cards and retained review evidence to the new current owner;
- renaming a deck or tag displays the current name;
- archived decks remain visible in historical analytics;
- finalized deleted material and its owned analytics evidence are removed according to the deletion contract.

React renders analytics data but does not invent an independent scoring model.

## Imports, exports, and backup

`.studydeck` is the portable single-deck format. It transfers content and managed images but gives the importing learner fresh schedules and no inherited review history.

`.ostudy.json` is strict data-only generated authoring input. The entire file is validated before one transaction, exact duplicates are skipped, and unsupported or unknown syntax is rejected rather than ignored.

`.ostudybackup` is the complete personal-library recovery format. It combines one coherent SQLite snapshot with the managed files required by that snapshot. Provider metadata may be backed up; credential bytes are excluded because they never enter SQLite.

See [Backups, imports, and sharing](../user-guide/backups-imports-and-sharing.md) for the user-facing distinction.

## AI and managed documents

`src-tauri/src/ai/` owns provider metadata, credentials, endpoint policy, HTTP transport, provider adapters, capability evidence, and task execution. Source PDFs and their derived text, semantics, outline, renders, OCR results, and generation checkpoints belong to the managed document subsystem.

Document processing is divided into narrow capabilities rather than one PDF-specific monolith. Parser, renderer, OCR, provider, and authoring types do not cross each other's boundaries. A model receives bounded context and returns strict task data; native validation and the existing authoring transaction remain authoritative.

See [AI and document processing](ai-and-document-processing.md) for provider isolation, extraction, PDFium rendering, OCR, resource planning, structured output, retries, cancellation, and document lifetime.

## Untrusted content and security

Card content, templates, imported files, provider responses, and source documents are untrusted until Rust validates them.

- Rich cards use versioned, allowlisted Tiptap/ProseMirror JSON, not arbitrary HTML.
- Links are limited to complete approved schemes.
- Images reference managed asset identifiers rather than remote or local filesystem paths.
- Templates perform literal structural substitution and expose no evaluator, script, SQL, shell, network, or filesystem capability.
- Search statements use bound parameters and Rust-owned query construction.
- Provider redirects are rejected; public cleartext HTTP is forbidden.
- Credentials, raw provider bodies, reasoning content, and source text are excluded from diagnostics.
- HTTP success never grants provider output authority over storage.

## Quality contracts

The repository's static gate verifies durable boundaries rather than historical source shape. It checks exact tools and dependencies, lockfiles, migrations, IPC parity, module and UI dependency direction, workflow runner policy, test policy, documentation links, and top-level organization.

Behavior remains protected by executable tests: React/controller tests, pure Rust tests, SQLite integration and migration fixtures, generated-import conformance, provider contract fixtures, document acceptance suites, and a native WebDriver persistence smoke test.

See [Development guide](../development/guide.md) and [Testing and CI](../development/testing-and-ci.md).

## Extending the system

New work should enter through the narrowest existing boundary:

- a new provider implements the native provider adapter and gateway contracts;
- a new document format supplies an ingestion adapter that produces ostudy-owned semantics;
- a new platform supplies file, credential, rendering, OCR, and lifecycle adapters;
- a new study tool reuses the document outline, authoring validation, scheduling, and analytics data rather than creating a generic prompt shortcut;
- a durable cross-module rule receives an Architecture Decision Record under [`../decisions/`](../decisions/).

The purpose of these boundaries is not architectural ceremony. They make local ownership, recovery, platform expansion, and testing possible without allowing every feature to invent its own storage, network, or interaction model.

## Cross-platform UI dependency boundary

ostudy's product UI is intentionally platform-neutral. React screens, feature controllers, responsive layouts, and shared UI primitives must not branch on desktop/mobile operating-system identity. Native differences are exposed through typed IPC under `src/ipc/` or capability-focused adapters under `src/platform/`. Responsive presentation is selected from viewport, pointer, safe-area, and accessibility capabilities rather than user-agent strings.

The dependency direction is one-way: shared UI primitives do not import feature controllers or IPC; feature screens may depend on controllers/shared UI; controllers may depend on typed IPC; direct Tauri imports are confined to the IPC/platform boundary. This lets Android, iOS, Windows, macOS, and Linux supply platform integrations without forking the study workflows themselves.
