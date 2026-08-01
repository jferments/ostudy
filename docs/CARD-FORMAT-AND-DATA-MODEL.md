# Card format, scheduling, and data model

## Canonical card content

ostudy uses the open-source Tiptap editor core, but stores validated
ProseMirror/Tiptap JSON rather than HTML. Markdown-style shortcuts may be used
while editing; they are not the durable representation.

Each note currently stores:

- `content_format = "tiptap-json"`
- `content_schema_version = 1`
- `front_doc` and `back_doc`: validated structured documents
- `front_text` and `back_text`: plain text derived by Rust from those documents

Supported content is deliberately allowlisted. It includes paragraphs, headings
1–4, text, hard breaks, dividers, quotes, code blocks, lists, list items, and
managed local images. Supported formatting includes bold, italic, underline,
strike, inline code, safe links, highlight, bounded font styling, colors,
alignment, indentation, and line height.

Links are limited to complete `http`, `https`, `mailto`, and `tel` addresses.
Images must use `study-asset://<id>` and refer to an asset already copied into
the managed image directory. Arbitrary HTML, JavaScript URLs, remote images,
local file paths, unknown nodes, unknown marks, and unknown attributes are
rejected by Rust.

## Notes and cards

A **note** is editable source content. A **card** is a studyable presentation
generated from that note. The current basic note creates one forward card. This
separation lets later stages add reverse cards, Fill in the blank cards, and
field-based templates without duplicating source content.

Core content tables:

- `decks`: deck identity and metadata
- `notes`: versioned source documents and derived text
- `cards`: studyable prompt identities
- `assets`: managed image metadata and relative paths
- `note_assets`: note-to-image relationships by side

Card creation, editing, deletion, and asset relationships are transactional.
Optimistic revisions prevent silent overwrites.

## Practice confidence and FSRS

After revealing an answer, the learner records one confidence score:

| Score | Display label | Stored outcome | FSRS rating |
| --- | --- | --- | --- |
| 0 | Wrong | `wrong` | Again (1) |
| 1 | Hard | `not_confident` | Hard (2) |
| 2 | Medium | `somewhat_confident` | Good (3) |
| 3 | Easy | `very_confident` | Easy (4) |

The stored outcome strings and canonical FSRS ratings remain compatible with
existing schema-3 libraries. Analytics displays `rating - 1`, which gives the
same 0–3 score shown during practice.

Scheduling data is split deliberately:

- `card_schedules` stores the current mutable FSRS state used by due-card
  queries.
- `review_events` is append-only and records every accepted review, including
  learner outcome, canonical rating, response time, and complete before/after
  schedule state.

Each review request carries a unique `submission_id`. A uniqueness constraint
and idempotent command handling prevent double clicks or IPC retries from
recording the same review twice. The review event, updated schedule, session-card
state, and session counters commit in one immediate SQLite transaction.

## Practice sessions and analytics

A direct deck Practice action starts a scheduled session for that deck. The
Practice screen can also select multiple decks. Sessions contain only cards due
at or before the session start time.

`practice_sessions` records selected decks and summary counts.
`practice_session_cards` records fixed order plus viewed, revealed, and rated
timestamps. A card must be revealed before it can be rated.

Analytics are deterministic facts over schedules and reviews: due count, review
count, correctness, response time, canonical rating distribution, per-deck
results, and recent reviews. The interface derives the displayed 0–3 average
confidence and recent-deck summaries from those stored ratings.

## `.studydeck` packages

A `.studydeck` file is a standalone, versioned SQLite interchange package
containing one deck, notes, cards, relationships, and managed image bytes.
Import validates metadata, row counts, IDs, timestamps, document structure,
image signatures, and relationships before one transactional insertion.

The human-readable deck title is preserved; internal ID collisions are remapped.
Imported cards receive fresh local schedules and do not inherit another
learner's review history. Packages exclude application settings and personal
reviews.

## Future field templates

The Phase 2 template language is intentionally data-only. A template may define
named fields such as `{{medication name}}` or `{{side effects}}`, and may use
those fields to generate validated card documents. It will not expose an
expression evaluator or allow JavaScript, SQL, shell commands, raw HTML,
filesystem access, or network access.

Template output passes through the same Rust document validator as manually
created and imported cards before it can be stored or rendered.
