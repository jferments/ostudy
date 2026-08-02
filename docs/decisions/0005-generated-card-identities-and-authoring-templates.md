# ADR 0005: Preserve generated-card identities and keep templates as authoring tools

- **Status:** Accepted
- **Applies from:** Phase 2, Stage 3

## Context

One editable source note may produce more than one independently scheduled study
card. A reversible Basic note has a forward and reverse presentation. A
fill-in-the-blank note may produce one presentation for each marked blank or one
presentation that hides every blank together.

Changing these options must not silently transfer one presentation's schedule or
review history to a different question. At the same time, temporarily disabling
a reverse direction or changing a cloze mode should not permanently destroy its
previous learning state if the user later enables the same presentation again.

Reusable field templates solve a different problem. They are shortcuts for
entering a group of facts once and generating several ordinary cards. Making the
generated cards children of the reusable template would allow later template
editing or deletion to unexpectedly change or delete study data.

## Decision

### Generated card identities

A generated card retains the existing durable identity tuple:

```text
(note_id, direction, template_ordinal)
```

The active Stage 3 mappings are:

- Basic forward: `direction = "forward"`, `template_ordinal = 0`;
- Basic reverse: `direction = "reverse"`, `template_ordinal = 0`;
- one-at-a-time cloze primary blank: `direction = "forward"`,
  `template_ordinal = <blank number>`;
- each additional one-at-a-time cloze blank: `direction = "cloze"`,
  `template_ordinal = <blank number>`;
- all-blanks-together cloze: `direction = "forward"`,
  `template_ordinal = 0`.

Blank numbers are stored in the source marker syntax and remain stable while the
note is edited. The all-blanks presentation deliberately has a different
identity from every individual blank.

When a generated presentation is disabled, its `cards` row is soft-deleted. Its
`card_schedules` row and append-only `review_events` remain attached to the same
card ID. Re-enabling the same identity clears `deleted_at` instead of inserting a
replacement card. New identities receive fresh schedules.

Search/Filter and Practice operate on exact card IDs, so generated presentations
can be selected, reviewed, and scheduled independently. Note-level actions such
as tags, move/copy, and delete intentionally deduplicate selected presentations
by `note_id`.

### Reusable authoring templates

A reusable template stores only:

- a display name and description;
- ordered field names and optional input guidance;
- ordered question and answer patterns using complete `{{field name}}`
  placeholders.

Rust validates field names, placeholder grammar, sizes, and all submitted values.
Rendering performs literal substitution only. There is no evaluator, expression
language, raw HTML path, JavaScript, SQL, shell, filesystem, or network access.

Each rendered result passes through the ordinary card-content validator and is
inserted as an independent Basic note/card with its own schedule. Generated
notes have no foreign key, source-template ID, or ownership relationship back to
the reusable template. Editing or deleting a template therefore cannot modify
or delete cards already generated from it.

### Deck interchange

`.studydeck` packages continue to contain notes, generated cards, tags, and
managed assets, but not the user's reusable authoring-template library. Stage 3
exports add optional data-only card-type metadata so reverse and cloze notes can
be reconstructed safely. Imports validate the complete generated-card graph and
create fresh local schedules without importing another learner's review history.

## Consequences

- Forward, reverse, individual-blank, and all-blanks questions cannot
  accidentally inherit one another's schedules.
- Disabling and re-enabling the same presentation preserves its previous
  schedule and review ledger.
- Switching cloze modes may temporarily hide one set of presentation identities,
  but switching back restores those exact identities.
- Moving a note preserves every generated card's identity and history; copying a
  note creates new card IDs and fresh schedules for every active presentation.
- Template deletion is safe because generated cards are ordinary permanent deck
  content.
- Template capabilities remain intentionally less expressive than a scripting
  system, making them deterministic, inspectable, and suitable for untrusted
  local data.
