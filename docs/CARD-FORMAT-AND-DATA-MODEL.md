# Card format, scheduling, and data model

## Editor choice

Study App uses the open-source Tiptap editor core rather than implementing a
home-grown `contenteditable` editor. Correct selection handling, nested lists,
paste normalization, undo/redo, links, image nodes, and stable serialization
would otherwise become a separate editor-engine project. Tiptap is headless:
Study App still owns the complete toolbar, layout, themes, validation rules, and
storage model.

## Canonical card content

Markdown-style shortcuts are accepted while typing, but Markdown is not the
canonical representation because it cannot round-trip colors, exact font sizes,
font families, alignment, indentation, or managed image identifiers.

Each note stores:

- `content_format = "tiptap-json"`
- `content_schema_version = 1`
- `front_doc`: validated question JSON
- `back_doc`: validated answer JSON
- `front_text` and `back_text`: Rust-derived plain text for search and analytics

Supported nodes include paragraphs, headings 1–4, text, hard breaks, dividers,
quotes, code blocks, bullet/numbered lists, list items, and managed local images.
Supported marks include bold, italic, underline, strike, inline code, links,
highlight, font family, 8–96 px font size, colors, and line height. Paragraphs
and headings can be aligned and indented.

Links are limited to `http`, `https`, `mailto`, and `tel`. Images must reference
an asset already imported into the managed image directory. Persisted image
nodes use `study-asset://<id>` rather than arbitrary local or remote URLs.
Tiptap's optional `width` and `height` attributes are normalized to either
`null` or a bounded numeric pixel value before Rust validates and stores them.

## Notes and cards

A **note** is the editable question/answer content. A **card** is one studyable
presentation generated from the note. Revision 5.4 creates one forward card per
note. The separation is retained so reverse, cloze, and image-occlusion cards
can be added without duplicating the underlying content.

Core content tables:

- `decks`: deck identity and metadata
- `notes`: versioned front/back editor documents and derived plain text
- `cards`: studyable prompt identity
- `assets`: managed image metadata and relative file location
- `note_assets`: note-to-image relationships by card side

Card creation, editing, deletion, and asset relationships are transactional.
Optimistic note/deck revisions prevent silent overwrites.

## FSRS schedule and immutable reviews

Scheduling data is deliberately split:

- `card_schedules` stores the current mutable FSRS state used by due-card
  queries.
- `review_events` is append-only and records every accepted review, including
  the user-facing outcome, canonical FSRS rating, response time, and complete
  before/after schedule state.

The learner-facing mapping is:

| Learner choice | FSRS rating |
| --- | --- |
| Wrong | Again (1) |
| Right → Not confident | Hard (2) |
| Right → Somewhat confident | Good (3) |
| Right → Very confident | Easy (4) |

Each review request carries a unique `submission_id`. The database uniqueness
constraint and idempotent command handling prevent double clicks or IPC retries
from recording the same answer twice. In one `BEGIN IMMEDIATE` transaction the
backend validates the active session, calculates the FSRS transition, appends
the review, updates the current schedule, marks the session card rated, updates
session totals, and commits.

## Practice sessions

The sidebar Practice screen lets the learner select multiple decks. The
Practice action on an individual deck bypasses that setup and immediately
starts a scheduled session for that deck. Sessions contain only cards whose
current `due_at` is at or before the session start time.

`practice_sessions` records the selected decks and summary counts.
`practice_session_cards` records the fixed card order plus viewed, revealed,
and rated timestamps. A card must be revealed before it can be graded.

## Analytics

Analytics are deterministic SQL aggregates over `card_schedules` and
`review_events`: due count, completed reviews, correctness, response time,
rating distribution, per-deck results, and recent reviews. A future local AI
feature may interpret these facts, but it will not calculate or replace the
underlying ledger.

## `.studydeck` packages

A `.studydeck` is a standalone, versioned SQLite interchange package containing
one deck, notes, cards, rich documents, relationships, and image bytes. Import
validates metadata, row counts, IDs, timestamps, rich-text structure, image
signatures, and relationships before one transactional insertion.

The human-readable deck title is preserved exactly; internal ID collisions are
remapped separately. Imported cards receive new local schedules and do not
inherit the exporting learner's review history. Packages exclude application
settings and personal reviews, making them suitable for content sharing and as
the content layer of a later sync protocol.

## Validation error taxonomy

Rust validates child content before checking whether a known child is legal in
its parent. This preserves stable compatibility errors:

- unknown nodes/marks → `unsupported_card_content`
- known nodes in an illegal structure → `invalid_card_document`
- noncanonical or remote image references → `invalid_image_reference`

These codes are part of the import/content contract and are protected by
regression tests. See ADR 0002 and ADR 0003.
