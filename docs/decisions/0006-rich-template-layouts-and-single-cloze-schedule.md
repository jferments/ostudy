# ADR 0006: Rich template layouts and one schedule per cloze note

- **Status:** Accepted, amended by the automatic-placement UI refinement
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

The initial rich-template UI then exposed two implementation choices—**Insert
inline** and **Insert as block**—to the user. This kept document structure safe,
but cluttered every rule editor and forced the person authoring a template to
predict the shape of values that would be entered later. Placement is a property
of the submitted rich value, not a useful authoring decision.

## Decision

### Rich reusable templates

Template rule sides are validated Tiptap documents. Users refer to fields by
typing complete double-brace placeholders such as `{{Subject}}` or
`{{Important findings}}` directly into a paragraph or heading.

Before a template is saved, Rust:

1. assigns stable identifiers to new fields and rules;
2. resolves each typed field name case-insensitively to the field's stable ID;
3. stores one neutral inert placeholder node rather than a user-selected
   placement mode; and
4. rejects incomplete braces, unknown field names, duplicate field names, or
   placeholders outside supported text blocks.

At preview and generation time, Rust validates every submitted field document
and classifies its structure:

- **Inline-compatible:** exactly one paragraph, no managed images, and only text
  or hard breaks inside that paragraph. Formatting marks on the text are
  preserved.
- **Block:** ordered or bullet lists, images, headings, block quotes, code
  blocks, horizontal rules, multiple paragraphs, or any other supported
  non-inline document structure.

Inline-compatible values are spliced into the surrounding paragraph. A block
value causes the surrounding paragraph or heading to be split into valid
before/value/after blocks. Whitespace-only wrapper paragraphs are omitted. The
value's original top-level nodes are inserted unchanged, so lists remain lists
and images remain managed images rather than being flattened into text.

The renderer never infers placement from whitespace, field names, or user-entered
plain-text snapshots. It bases the decision only on the validated Tiptap
structure. Final generated documents pass through the ordinary card validator
and are stored as ordinary Basic notes and cards. Generated content contains no
placeholder nodes and no template foreign key.

The old `templateFieldBlock` node remains readable only for compatibility with
templates saved by the first schema-7 build. The existing idempotent startup
normalization rewrites those explicit block nodes to neutral placeholders. No
new database migration is required because the stored columns and document
schema version do not change.

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

The same normalization boundary now canonicalizes schema-7 template documents
that used explicit block placeholders. Typed placeholders and automatic
placement do not require schema 8.

Older Stage 3 deck packages containing multiple cloze card rows are accepted and
normalized immediately after import. New exports contain one active card for
each cloze note. Reusable authoring templates are not included in `.studydeck`
packages.

## Consequences

- Template authors use one familiar `{{Field}}` syntax instead of placement
  controls.
- Rich fields preserve lists, images, formatting, and multi-block structure
  without HTML or expression evaluation.
- Placement follows the actual validated value and can differ between two uses
  of the same template without changing the template definition.
- Existing schema-7 templates remain usable and are normalized forward without
  rewriting migration history.
- A cloze note cannot appear several times in one practice session merely
  because it has several blanks.
- The single cloze FSRS state represents performance on the complete concept,
  while the tested blank rotates across reviews.
- Historical schema-6 sibling schedules remain inspectable but inactive.
- Template-generated cards remain independent of later template edits or
  deletion.
