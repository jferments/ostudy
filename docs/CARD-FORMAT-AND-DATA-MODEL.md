# Card format, scheduling, and data model

## Editor choice

ostudy uses the open-source Tiptap editor core rather than implementing a
home-grown `contenteditable` editor. Correct selection handling, nested lists,
paste normalization, undo/redo, links, image nodes, and stable serialization
would otherwise become a separate editor-engine project. Tiptap is headless:
ostudy still owns the toolbar, layout, themes, validation rules, and storage
model.

## Canonical rich content

Markdown-style shortcuts are accepted while typing, but Markdown is not the
canonical representation because it cannot reliably round-trip the supported
rich formatting and managed image identifiers.

Each note stores:

- `content_format = "tiptap-json"`;
- `content_schema_version = 1`;
- `front_doc` and `back_doc`: validated editor JSON;
- `front_text` and `back_text`: Rust-derived plain text for search and display;
- optional template/field data and a duplicate-detection key.

Supported nodes include paragraphs, headings 1–4, text, hard breaks, dividers,
quotes, code blocks, bullet/numbered lists, list items, and managed local images.
Supported marks include bold, italic, underline, strike, inline code, links,
highlight, font family, bounded font size, colors, and line height. Paragraphs
and headings can be aligned and indented.

Links are limited to `http`, `https`, `mailto`, and `tel`. Images must reference
an asset already imported into the managed image directory. Persisted image
nodes use `study-asset://<id>` rather than arbitrary local or remote URLs.

## Notes and generated cards

A **note** is editable source content. A **card** is one studyable presentation
with its own schedule. Stage 1 supports four user-facing note modes:

| Note mode | Generated cards |
| --- | --- |
| Basic | One forward card |
| Reverse | One forward and one reverse card |
| Fill in the blank | One card for each `[[answer]]` marker |
| Template | One card for each card definition in the selected field template |

Core content tables include:

- `decks`: deck identity and metadata;
- `notes`: source documents, mode data, fields, duplicate key, and revision;
- `cards`: generated prompt identity, direction, ordinal, and display name;
- `card_templates`: field definitions and generated-card patterns;
- `tags` and `note_tags`: normalized reusable labels;
- `notes_fts`: content-only SQLite FTS5 search rows;
- `assets` and `note_assets`: managed image metadata and relationships.

Note creation and editing validate the desired generated-card set and update the
note, cards, schedules, tags, assets, duplicate key, and FTS row in one SQLite
transaction. Optimistic revisions prevent silent overwrites.

Moving a note updates the note and every generated card to the destination deck
without replacing card IDs, schedules, or review history. Copying creates a new
note and new generated-card IDs, reuses safe local tag/asset relationships, and
creates fresh New schedules without copying personal review history.

## Literal card-template language

A card template contains:

- a name and description;
- between 1 and 64 field names;
- between 1 and 64 generated-card definitions;
- a front and back literal pattern for each generated card.

`{{field name}}` is the only special syntax. Field names are normalized and
bounded. Every card side must reference a declared field. Values are substituted
as ordinary text and converted to canonical JSON text nodes; they are never
parsed as HTML, JavaScript, CSS, SQL, or another expression language.

The built-in Pharmacology template demonstrates the model with medication name,
drug class, medical conditions, and side effects. Templates that are already in
use are immutable in Stage 1 so existing generated cards cannot silently change
meaning.

## Fill in the blank format

Fill in the blank notes use literal `[[answer]]` markers. For example:

```text
[[Warfarin]] inhibits [[vitamin K epoxide reductase]].
```

This produces two cards. Each prompt hides one marked answer while showing the
other marked answers, and the revealed side displays the complete sentence.
Markers must be balanced, nonempty, nonnested, and within the documented size
and count limits.

## Full-text search and duplicate warnings

Each active note has one FTS5 row containing searchable source text, tags, field
names/values, and rendered template-card text. Search strings are converted by
Rust into quoted token-prefix terms and passed to SQLite through a bound
parameter, so SQL and FTS operators entered by a deck author are treated as
data.

A normalized duplicate key covers the note mode and source data. Saving a match
in the same deck produces a warning; the learner can explicitly choose to keep
both.

## FSRS schedules and immutable reviews

Scheduling data is deliberately split:

- `card_schedules` stores the current mutable FSRS state used by due-card
  queries;
- `review_events` is append-only and records every accepted review, including
  the user-facing outcome, canonical FSRS rating, response time, and complete
  before/after schedule state.

Stage 1 retains the existing learner-facing mapping:

| Learner choice | FSRS rating |
| --- | --- |
| Wrong | Again (1) |
| Right → Not confident | Hard (2) |
| Right → Somewhat confident | Good (3) |
| Right → Very confident | Easy (4) |

The direct 0–3 Wrong/Hard/OK/Easy flow is planned for Phase 2 Stage 3 and will
update review input, analytics, and tests together.

Each review request carries a unique `submission_id`. The database uniqueness
constraint and idempotent command handling prevent double clicks or IPC retries
from recording the same answer twice. One `BEGIN IMMEDIATE` transaction validates
the active session, calculates the FSRS transition, appends the review, updates
the current schedule, marks the session card rated, updates session totals, and
commits.

## Practice sessions

The sidebar Practice screen lets the learner select multiple decks. The Practice
action on an individual deck immediately starts a scheduled session for that
deck. Sessions contain cards whose current `due_at` is at or before the session
start time.

`practice_sessions` records the selected decks and summary counts.
`practice_session_cards` records the fixed card order plus viewed, revealed, and
rated timestamps. A card must be revealed before it can be graded. Practice
renders the selected generated card ID, not merely the parent note, so reverse,
Fill in the blank, and template prompts remain distinct.

## Analytics

Analytics are deterministic SQL aggregates over `card_schedules` and
`review_events`: due count, completed reviews, correctness, response time,
rating distribution, per-deck results, and recent reviews. A future local AI
feature may interpret these facts, but it will not calculate or replace the
underlying ledger.

## `.studydeck` packages

A `.studydeck` is a standalone, versioned SQLite interchange package containing
one deck, notes, generated cards, rich documents, tags, templates, relationships,
and managed image bytes. Version 2 preserves Stage 1 authoring semantics; version
1 remains readable through a compatibility importer.

Before import, Rust opens the package read-only with `trusted_schema=OFF`, checks
file and row limits, runs SQLite integrity checks, requires the exact package
objects and columns, validates metadata and declared types, and revalidates every
ID, timestamp, rich document, template, field map, tag, asset signature, and
relationship. Only then is content inserted in one local transaction.

The human-readable deck title is preserved exactly; internal ID collisions are
remapped separately. Duplicate keys are recomputed locally. Imported cards
receive fresh schedules and do not inherit the exporting learner's review
history. Packages exclude application settings and personal reviews.

## Validation error taxonomy

Rust validates child content before checking whether a known child is legal in
its parent. This preserves stable compatibility errors:

- unknown nodes/marks → `unsupported_card_content`;
- known nodes in an illegal structure → `invalid_card_document`;
- noncanonical or remote image references → `invalid_image_reference`.

These codes are part of the import/content contract and are protected by
regression tests. See ADR 0002, ADR 0003, and ADR 0004.
