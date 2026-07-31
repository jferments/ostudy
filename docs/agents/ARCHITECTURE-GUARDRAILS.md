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

All frontend-to-Rust invocation belongs in `src/ipc.ts`. Screens and components
should call typed wrapper functions rather than invoke arbitrary command names.

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

Released migration files are immutable. Add a new numbered migration, checksum
it through the existing mechanism, and test both a fresh database and an
upgraded database.

### Managed files

Image bytes belong in the managed application-data directory. Persist stable
asset identifiers and managed relative locations, never arbitrary absolute paths
or remote image URLs in card content.

## Durable data invariants

- A note stores editable front/back content; a card stores a studyable
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
- Imported cards receive fresh local schedules.
- Analytics are deterministic SQL aggregates over authoritative stored state.

## Local-first constraints

Core application behavior must remain useful without an account, hosted
database, telemetry service, paid API, or network connection. A future optional
network or AI feature must be isolated behind a clear interface and must fail
without corrupting or blocking local study data.

Never add remote loading of card images or rich content as a convenience. That
would weaken reproducibility, privacy, and the current validation boundary.

## Compatibility constraints

Treat the following as compatibility surfaces:

- Tauri command names and typed frontend wrappers;
- stable error codes;
- database schemas and migration ordering;
- Tiptap schema version and validation taxonomy;
- `.studydeck` format and import behavior;
- FSRS rating mapping and review idempotency;
- the Tauri application identifier and application-data location;
- user-visible keyboard, accessibility, and theme behavior.

A change to any of these requires explicit acceptance criteria, regression
coverage, migration or compatibility analysis, and documentation. Durable
cross-module changes require an ADR.

## Change-impact questions

Before implementation, answer:

1. Which layer owns the requested behavior?
2. Does this change cross the React/Rust, Rust/SQLite, or database/filesystem
   boundary?
3. Does it alter a persistent format, stable identifier, command, or error code?
4. Can a retry or double click duplicate data?
5. Can a partial failure leave structured data and files inconsistent?
6. Does it affect existing imported decks, schedules, reviews, themes, or
   keyboard behavior?
7. What is the narrowest test that proves the invariant, and what higher-level
   test proves the complete user workflow?
