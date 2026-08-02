# ADR 0006: Rich template layouts and one schedule per cloze note

- **Status:** Accepted
- **Applies from:** database schema 7

## Context

Stage 3 adds two distinct authoring behaviors:

1. Reusable templates collect structured field values once and materialize one
   or more permanent Basic cards.
2. Fill in the blank notes may contain several marked blanks, but the learner
   regards the complete statement as one logical card.

The first Stage 3 implementation stored template fields and rule layouts as
plain strings. That flattened lists and could not preserve images. It also
created one `cards` row and FSRS schedule for every one-at-a-time cloze blank,
which made alternate blanks appear as separate search results and allowed them
into the same practice session.

## Decision

### Rich reusable templates

Template rule sides are validated Tiptap documents. They may contain all normal
card formatting plus two inert placeholder nodes:

- `templateFieldInline` inserts a short, single-paragraph field inside a
  paragraph or heading.
- `templateFieldBlock` occupies its own block position and structurally inserts
  the field's top-level nodes.

Block insertion preserves ordered lists, bullet lists, headings, paragraphs,
and managed images. A field containing block content cannot be inserted inline;
Rust returns a visible validation error rather than flattening or creating
invalid document structure.

Template field values are also validated Tiptap documents. Rust validates every
layout, field value, placeholder field ID, managed asset reference, and final
rendered output. Final generated documents pass through the ordinary card
validator and are stored as ordinary Basic notes and cards. Generated content
contains no placeholder nodes and no template foreign key.

Templates may own managed image references through `card_template_assets`.
Generated notes receive their own `note_assets` references for the images that
actually appear in their final question and answer documents. Deleting a
template never deletes generated cards or images still referenced by cards.

### One scheduled cloze card

Every active Fill in the blank note owns exactly one active card identity:

```text
(note_id, direction = forward, template_ordinal = 0)
```

Both cloze modes use that one card ID, one `card_schedules` row, and one review
history.

- **Hide one blank per review:** the presentation rotates through the marked
  blank groups according to the canonical card's accepted review count.
- **Hide all blanks together:** every marked blank is hidden in the one
  presentation.

When a practice session is created, Rust stores the chosen cloze presentation
in `practice_session_cards.presentation_variant`. Rehydrating the session
therefore shows the same blank. Only an accepted, idempotent review advances the
FSRS schedule and consequently the next rotation position. Viewing, revealing,
or abandoning a session does not advance it.

Search, deck counts, analytics card totals, move/copy, tags, export, and practice
selection see one cloze card. Reverse Basic cards remain genuinely separate
card identities with independent schedules.

### Migration and compatibility

Migration 0006 remains immutable. Migration 0007 adds rich rule documents,
template asset relationships, and practice presentation snapshots. An
idempotent Rust normalization step converts schema-6 cloze notes by retaining
one canonical card and soft-deleting additional cloze presentation rows.
Existing schedules and immutable review events for the retired rows remain in
the database; history is not rewritten or mathematically merged.

Older Stage 3 deck packages containing multiple cloze card rows are accepted and
normalized immediately after import. New exports contain one active card for
each cloze note. Reusable authoring templates are not included in `.studydeck`
packages.

## Consequences

- Rich fields can safely preserve lists and images without HTML or expression
  evaluation.
- Inline and block placement are explicit rather than inferred from whitespace.
- A cloze note cannot appear several times in one practice session merely
  because it has several blanks.
- The single cloze FSRS state represents performance on the complete concept,
  while the tested blank rotates across reviews.
- Historical schema-6 sibling schedules remain inspectable but inactive.
- Template-generated cards remain independent of later template edits or
  deletion.
