# Architecture guardrails for agents

This document translates ostudy's architecture into concrete constraints for
implementation agents. Read the canonical architecture and data-model documents
before changing a boundary described here.

## Trust and ownership boundaries

### React and TypeScript

The frontend owns presentation, transient interaction state, and communication
of user intent. It may derive view-only values, but it must not become the
authoritative implementation of persistence, scheduling, validation, or
analytics.

Frontend-to-Rust invocation belongs only in typed domain gateway modules under
`src/ipc/`. Screens, controllers, components, and tests must not call Tauri
`invoke()` directly. The repository IPC contract check verifies exact parity
between every gateway command name and the Rust registry and rejects direct
invocation outside the gateway directory.

Editable source notes and rendered study presentations are different frontend
contracts:

- `CardNote` is the Basic/Cloze authoring model and owns note revision, tags, and
  card-type-specific editable fields;
- `CardPresentation` is the rendered card identity used by practice/search and
  carries a card ID plus rendered question/answer content.

Do not create another generic `Flashcard`/`OrganizedCard` DTO that mixes these
responsibilities.

Workflow-heavy screens use focused controllers under `src/controllers/` for
native read/write orchestration. Independent asynchronous reads use independent
latest-request lanes. Rust work does not need to be physically cancelled, but a
completion that is no longer current must not overwrite newer screen state.
Obsolete errors are ignored along with obsolete successes.

Do **not** apply stale-read semantics to mutations. Once Rust reports a durable
mutation as committed, later view refresh/reconciliation failure is a separate,
recoverable display problem. Never tell the user the write failed or create a
new operation/retry ID merely because reconciliation failed.

Presentational components should represent independently meaningful views or
controls. Do not split components solely to satisfy a line-count target or create
prop-forwarding fragments with no semantic/test boundary.

### Rust

Rust owns validation, use-case orchestration, durable domain behavior, stable
error responses, and multi-step transactions. Treat every IPC payload, imported
package, rich-text document, path, and identifier as untrusted input.

Application-owned DTOs and domain types must isolate durable data from external
crate representations. In particular, `rs-fsrs` remains behind
`src-tauri/src/scheduler.rs`.

### SQLite

SQLite owns durable structured state. Enforce invariants with schema constraints,
transactions, and tests rather than relying only on UI behavior.

Released migration files and release-schema fixtures are immutable. Add a new
numbered migration, checksum it through the existing mechanism, and test both a
fresh database and upgrades from every released fixture.

### Application identity

The product name is `ostudy`, release versioning begins at `0.1.0`, and the
Tauri identifier is `io.github.jferments.ostudy`. A later identifier change is a
storage-compatibility decision and requires an explicit migration plan.

### Managed files

Image bytes belong in the managed application-data directory. Persist stable
asset identifiers and managed relative locations, never arbitrary absolute paths
or remote image URLs in card content.

## Durable data invariants

- A note stores editable source content; a card stores a studyable
  presentation generated from a note.
- Canonical rich content is versioned Tiptap/ProseMirror JSON.
- The Rust validator uses an explicit node and mark allowlist.
- Validation error ordering is compatibility behavior. Preserve the distinction
  between unsupported content, malformed structure, and invalid image
  references.
- `card_schedules` stores current mutable FSRS state.
- `review_events` is append-only historical evidence.
- One review submission atomically validates the session, calculates the
  transition, appends the event, updates the schedule, updates session state,
  and commits.
- `submission_id` idempotency must prevent duplicate reviews caused by retries or
  repeated clicks.
- `.studydeck` packages contain shareable deck content and managed assets, not
  application settings or personal review history.
- Generated `.ostudy.json` import is whole-file validated, transactional, strict
  about unknown fields, and creates fresh local schedules. Its conformance tests
  run only against disposable in-memory/temporary test storage and remove the
  imported graph and temporary files when they finish.
- Analytics are deterministic SQL aggregates over authoritative stored state.

## Local-first constraints

Core application behavior must remain useful without an account, hosted
database, telemetry service, paid API, or network connection. A future optional
network or AI feature must be isolated behind a clear interface and must fail
without corrupting or blocking local study data.

Never add remote loading of card images or rich content as a convenience. That
would weaken reproducibility, privacy, and the current validation boundary.

## Compatibility constraints

Durable user data and documented external formats are compatibility surfaces:

- database schemas and migration ordering;
- stable error codes for active durable operations;
- Tiptap schema version and validation taxonomy;
- `.studydeck` format and import behavior;
- `.ostudy.json` format version and strict generated-import behavior;
- FSRS rating mapping, review history, and review idempotency;
- the Tauri application identifier and application-data location;
- user-visible keyboard, accessibility, and theme behavior.

The bundled React/Rust IPC layer is an internal contract unless an external API
is explicitly documented. When an unreleased or internal feature is removed,
delete its gateway wrapper, command, request field, tests, and documentation
together. Do not silently accept obsolete fields; revised request DTOs should
reject them when acceptance would hide a caller defect.

Changes to durable compatibility surfaces require explicit acceptance criteria,
regression coverage, migration or compatibility analysis, and documentation.
Durable cross-module changes require an ADR.

## Change-impact questions

Before implementation, answer:

1. Which layer owns the requested behavior?
2. Does this change cross the React/Rust, Rust/SQLite, or database/filesystem
   boundary?
3. Does it alter a persistent format, stable identifier, command, or error code?
4. Can a retry or double click duplicate data?
5. Can a partial failure leave structured data and files inconsistent?
6. Can an older async read finish after a newer scope and overwrite current UI
   state?
7. Does a successful mutation remain distinguishable from a failed follow-up
   refresh?
8. Does it affect existing imported decks, schedules, reviews, themes, or
   keyboard behavior?
9. What is the narrowest test that proves the invariant, and what higher-level
   test proves the complete user workflow?
