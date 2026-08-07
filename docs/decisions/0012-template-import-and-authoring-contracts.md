# ADR 0012: Template, generated-import, and authoring contract cleanup

- **Status:** Accepted
- **Date:** 2026-08-07
- **Scope:** Stage 3.11

## Context

Stages 3.9 and 3.10 replaced source-shape quality gates with behavior contracts and converted the authoring subsystem from lexical `include!` composition to ordinary Rust modules. Those changes exposed several semantic boundaries that were intentionally left behavior-preserving during the module conversion:

- template update still seeds temporary child rows so newly supplied field/rule IDs can pass ownership validation before the real replacement graph is written;
- generated JSON import remains behind a transitional path/rustfmt wrapper and combines external schema parsing, restricted Markdown parsing, card preparation, duplicate handling, destination resolution, and transaction application in one implementation file;
- ordinary editor authoring, reusable-template generation, and generated-file import share behavior through private helpers rather than an explicit canonical note-preparation boundary;
- several internal request contracts still accept strings or redundant browser text snapshots even though Rust derives authoritative card text from validated rich documents;
- obsolete Basic-only authoring commands remain registered despite the generated-card authoring API superseding them;
- current architecture and agent documents still describe pre-3.9/3.10 module and validation workflows.

The generated `.ostudy.json` version-1 format is already a deliberate durable compatibility surface. Its one-step Import deck workflow, `create`/`append` destination semantics, duplicate skipping, fresh schedules, strict unknown-field rejection, and no-preview behavior must not be changed by this cleanup.

## Decision

### Canonical card-note preparation

Rust will expose one internal card-note preparation model for Basic and Cloze source notes. Editor commands, reusable-template generation, and generated-file import map their boundary-specific inputs into this model before canonical validation, plain-text derivation, generated-presentation planning, duplicate normalization, and persistence.

Closed internal domains use enums rather than open-ended strings. Database and external-format strings remain stable representations at their boundaries.

### Template graph validation before persistence

Template create/update will normalize and validate a complete submitted graph in memory. Database-dependent preconditions (parent revision/name ownership, child-ID ownership, and referenced assets) are checked without creating placeholder rows. A validated replacement graph is persisted once inside the optimistic-revision transaction.

The temporary/pending child-row update strategy is removed.

### Generated import as a first-class subsystem

Generated import becomes an ordinary Rust module with explicit responsibilities for:

- the immutable version-1 external JSON schema;
- the deliberately restricted Markdown grammar;
- conversion to canonical Tiptap documents;
- complete-file preparation and bounded structured errors;
- destination and duplicate planning;
- one transactional application boundary.

The restricted Markdown parser decides syntax support. The canonical card validator remains authoritative for safe rich-document semantics. Literal comparison characters must not be mistaken for raw HTML, while actual HTML/images/tables/executable or unknown constructs continue to fail closed.

No generic Markdown runtime dependency is added in this stage.

### IPC cleanup

The generated-card authoring request becomes a strict Basic/Cloze tagged contract. Basic requests carry rich front/back documents and reverse configuration; Cloze requests carry cloze source/mode. Redundant `frontText`/`backText` request snapshots are removed because Rust derives authoritative text.

The obsolete `create_organized_card` and `update_organized_card` commands, frontend wrappers, DTOs, tests, and registrations are removed end to end. No compatibility shim is retained because bundled Tauri IPC is an internal contract.

The broader source-note/detail versus rendered-presentation DTO split is intentionally deferred to a later focused stage.

### Compatibility and historical code

Active implementation names use semantic terminology instead of stage numbers where practical. Released migrations, release fixtures, versioned repair records/checksums, legacy `.studydeck` readers, and other durable compatibility paths are not rewritten merely to remove historical names.

## Non-goals

Stage 3.11 does not:

- change the database schema;
- change generated JSON format version 1;
- add generated-import preview/inspect/commit UI;
- add duplicate-policy UI or destination selection outside the JSON file;
- change `.studydeck` behavior;
- add template import/export;
- change generated-card identities, schedules, review history, or template ownership semantics;
- perform the frontend-wide source-note/presentation DTO split;
- add a new card type or image-occlusion workflow;
- rewrite released migrations or historical repair checksums.

## Validation

Behavior is protected by Rust/SQLite and frontend tests rather than a Stage 3.11 source-shape verifier. Required coverage includes template graph validation/rollback/stable IDs, generated-import grammar and transaction fixtures, strict Basic/Cloze request serialization, obsolete command removal, IPC parity, exact dependency/migration contracts, and the existing full native qualification gate.
