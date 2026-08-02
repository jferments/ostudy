# ADR 0006: Keep templates independent from generated cards

- **Status:** Accepted
- **Applies from:** Phase 2, Stage 3

## Context

Reverse and Fill in the blank notes genuinely have several presentations of one
source record. Those presentations should share edits while retaining separate
schedules. Reusable templates solve a different problem: entering several
related cards quickly from one structured form.

Treating a template-generated card as a special durable type would couple normal
cards to an authoring tool, complicate deletion, export, analytics, and schedule
semantics, and make later template edits capable of unexpectedly changing saved
study material.

## Decision

Basic reverse and cloze cards use the existing note-to-many-cards relationship:

- Basic forward uses `(direction = forward, ordinal = 0)`;
- Basic reverse uses `(direction = reverse, ordinal = 0)`; and
- cloze presentations use `(direction = cloze, ordinal = group number)`, or
  ordinal `0` for the all-together presentation.

Disabled presentations are soft-deleted and revived by the same direction and
ordinal so their schedules and immutable review events remain attached.

A custom template is instead a data-only bulk-generation definition. Submitting
it renders each rule once and inserts one ordinary Basic note/card per rule in a
single transaction. Generated cards store no template reference. Editing or
soft-deleting the template has no effect on existing cards.

The template language is restricted to literal text plus validated
`{{field name}}` substitutions. Generated documents pass through the same Rust
validator and duplicate checks used by normal authoring.

## Consequences

- Template deletion is safe and never requires deleting or converting cards.
- Template-generated cards use normal search, scheduling, analytics, transfer,
  and edit code without special branches.
- Reverse and cloze cards still learn independently while sharing one editable
  source note.
- Existing Basic card data migrates forward through defaults; no obsolete API or
  ignored-field compatibility shim is retained.
- Template edits affect only future submissions, which is explicit and
  predictable.
