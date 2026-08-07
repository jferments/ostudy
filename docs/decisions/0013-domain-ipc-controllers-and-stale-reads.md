# ADR 0013: Domain IPC, controllers, stale reads, and screen decomposition

- **Status:** Accepted
- **Date:** 2026-08-07
- **Scope:** Stage 3.12

## Context

Stage 3.11 established a canonical Basic/Cloze authoring boundary, removed obsolete Basic-only writes, and split generated import into explicit schema, parser, preparation, and apply modules. The frontend still exposes older architectural shapes:

- editable source notes and rendered study presentations share `Flashcard` / `OrganizedCard` transport types;
- `src/ipc.ts` remains a broad gateway containing unrelated domains while analytics, generated import, and diagnostics use separate gateways;
- major screens own asynchronous orchestration, mutation reconciliation, launch-state handling, and large render trees in the same components;
- stale-response protection exists in several screens but is implemented repeatedly with ad-hoc sequence refs and is incomplete for some reads;
- durable mutation success and subsequent view reconciliation are handled correctly in some workflows but are not an explicit shared controller contract;
- generated `.ostudy.json` import has focused parser and transaction tests but no executable conformance corpus spanning valid syntax, rejection cases, limits, duplicate behavior, atomicity, fresh schedules, and cleanup.

The changes in this stage are architectural. User-visible study, authoring, template, import, scheduling, analytics, and backup behavior must remain stable.

## Decision

### Source notes and study presentations

Frontend/native transport will distinguish editable source notes from rendered scheduled presentations.

- `CardNote` is the complete editable Basic or Cloze source-note model, including note revision, tags, card-type-specific authoring fields, and generated-card count.
- `CardPresentation` is a rendered study identity keyed by card ID and note ID with rendered front/back documents/text plus direction and presentation metadata.
- practice, search, and scheduling use presentation/card identities; authoring edits source notes.

The old `get_organized_card`, `create_generated_card`, `update_generated_card`, and `get_card_type_info` command surface is replaced by `get_card_note`, `create_card_note`, and `update_card_note`. Bundled Tauri IPC is internal, so no compatibility aliases are retained.

### Domain IPC gateways

Frontend transport is organized under `src/ipc/` by domain. Screens, controllers, components, and tests import only the domains they use. Direct Tauri `invoke()` calls are limited mechanically to the gateway directory.

The repository IPC verifier continues to enforce exact registered-command / wrapper parity without depending on file names or source-layout wording.

### Controllers and request lanes

Workflow-heavy screens move asynchronous orchestration and transition state into focused hooks/controllers. Controllers own persistence calls, stale-read handling, mutation reconciliation, modal/editor workflow state, and domain transitions. Presentational components receive state and events rather than directly coordinating native workflows.

A shared latest-request lane provides monotonically increasing request tokens and invalidation. Obsolete read results may finish in Rust but may not update current UI state. Independent reads use independent lanes.

Mutations are not treated as cancellable reads. A committed mutation remains successful even if later reconciliation fails; controllers expose a distinct recoverable reconciliation failure and preserve committed success.

### Screen decomposition

Components are extracted only when they own an independently meaningful view, workflow, or test surface. Stage 3.12 does not impose line-count limits or create prop-forwarding fragments merely to reduce file size.

### Generated-import conformance corpus

The repository owns one deterministic conformance corpus describing valid cards, invalid cards/files, and expected outcomes. The harness materializes real `.ostudy.json` files in a temporary application-data root and exercises the real path-based import boundary.

Coverage includes:

- every supported Basic restricted-Markdown block and inline construct;
- safe literal parser edge cases and Unicode/line-ending cases;
- Basic/reverse and Cloze modes/marker variants;
- schema/type/unknown-field failures;
- unsafe links, HTML, images, tables, malformed Markdown, and Cloze grammar failures;
- deck create/append resolution and ambiguity;
- boundary values around declared limits;
- duplicates within a file and against the destination;
- whole-file atomicity and fresh generated-card schedules;
- cleanup of all temporary files and database state even when an assertion fails.

The test corpus never operates on the user's normal application data directory and does not add production test-only commands.

## Non-goals

Stage 3.12 does not add a database migration, generated-import v2, import preview UI, a new card type, template import/export, Redux/Zustand/query frameworks, a network layer, a Markdown dependency, scheduling-algorithm changes, analytics semantic changes, a database connection pool, modal accessibility redesign, or a broad CSS rewrite.

## Validation

Behavior is protected by domain IPC tests, controller transition tests, reverse-order stale-response tests, Rust serde/DTO tests, the generated-import conformance corpus, existing authoring/template/practice/analytics regression suites, mechanical IPC parity, and the full native qualification gate. A green CI result qualifies only the exact tested commit SHA.