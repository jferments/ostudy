# Card format, scheduling, and data model

## Canonical rich content

Editable rich content is stored as serialized Tiptap/ProseMirror JSON with:

- `content_format = 'tiptap-json'`;
- an explicit schema version;
- derived `front_text` and `back_text` for browsing, duplicate detection, and
  search;
- managed image references rather than arbitrary URLs or absolute paths.

Rust validates every document before persistence or import. The allowlist covers
known block and inline nodes, known marks, safe URL schemes, bounded text and
document sizes, managed asset IDs, predefined image sizes, and left/center/right
image alignment. The frontend display component is read-only and uses the same
schema extensions.

## Note modes and generated cards

The public note modes are:

| Mode | Editable source | Generated cards |
| --- | --- | --- |
| Basic | front + back rich documents | one forward card |
| Reverse | front + back rich documents | forward and reverse cards |
| Fill in the blank | plain source containing `[[answer]]` markers | one card per marker |
| Template | selected template + field values | one card per template card specification |

The historical database values `note_type='cloze'` and `direction='cloze'`
remain for compatibility. They are never presented as the feature name.

A note records its owning deck, mode, canonical documents/text, optional template
ID, normalized field-value JSON, duplicate key, timestamps, deletion state, and
revision. A card records the note, deck, generated direction, template ordinal,
display name, timestamps, suspension, burial, Struggling state, and optional
source-card relationship.

Edits synchronize generated cards in one transaction. Cards that no longer
correspond to the note are soft-deleted; existing matching card identities and
schedules are preserved.

## Safe card templates

`card_templates` stores:

- a 32-character local ID;
- name and description;
- ordered normalized field names as JSON;
- ordered `{name, front, back}` card specifications as JSON;
- built-in flag, revision, timestamps, and soft-deletion state.

Only exact `{{field name}}` placeholders are recognized. Every placeholder must
reference a declared field. Patterns have bounded length and cannot contain
unbalanced braces or executable constructs. Substitution is literal. Rendering
produces ordinary text documents; values cannot introduce JSON nodes, HTML,
JavaScript, SQL, or CSS.

The seeded Pharmacology template declares medication name, drug class, medical
conditions, and side effects, and generates multiple question directions.

## Fill in the blank

The parser recognizes nonempty, bounded `[[answer]]` spans and rejects unmatched,
nested, or empty markers. For each marker, the selected answer is replaced with
a visible blank while other text remains visible. The answer side contains the
complete source text. Multiple markers generate multiple independently
scheduled cards.

## Tags and search

`tags` stores display and normalized names. `note_tags` is the many-to-many
relationship. Tags are note-level metadata and therefore apply to every card
generated from the note.

`notes_fts` is an FTS5 content-only table containing note ID, deck ID, derived
front/back text, tags, and template field JSON. Rust explicitly deletes and
reinserts the row after trusted note/tag changes. Search terms are tokenized and
quoted before `MATCH` to prevent user input from becoming an FTS expression.

## Duplicate keys

A duplicate key is a deterministic application-owned hash over normalized note
mode and source content. Whitespace and case normalization reduce accidental
near-identical entries. The browser confirms candidate matches against stored
content and lets the learner either cancel or explicitly save anyway. Bulk entry
can skip duplicate rows. Import never trusts package-provided keys; it
recomputes them locally.

## Editor drafts

`editor_drafts` stores a bounded JSON payload by `draft_key`, deck, optional note,
and update time. Draft payload is transient editor state, not authoritative card
content. Loading a draft still passes through normal frontend state checks and
saving the note still passes through full Rust validation. Saving a note clears
its draft.

## Card schedules

`card_schedules` stores current mutable FSRS state:

- state, due time, stability, and difficulty;
- elapsed and scheduled days;
- repetition and lapse counters;
- last-review time;
- scheduler name/version and update time.

Every generated card receives its own schedule. New and imported cards start in
the New state and are due locally. Reset recreates a fresh New schedule;
reschedule changes due time without fabricating a review.

## Review events, snapshots, and Undo

`review_events` remains append-only. Existing compatibility columns retain the
canonical FSRS rating 1–4 and historical confidence text; the public score is
`rating - 1` and is shown as Wrong, Hard, OK, or Easy.

`review_schedule_snapshots` stores complete before/after scheduler state for each
Phase 2 review. `review_card_state_snapshots` stores suspension and Struggling
state. `review_sibling_buries` stores prior sibling burial values.
`review_event_reversals` records Undo without deleting the review event.

Undo restores the exact pre-review schedule and related card state and marks the
event reversed. Queries for active analytics and daily limits exclude reversed
events.

## Struggling cards

A card becomes **Struggling** after the configured number of active Wrong reviews.
The state stores when it began and a human-readable reason. Automatic suspension
is optional. The card browser can filter Struggling cards so content can be
revised, reset, or resumed. The term replaces the specialist label “leech” in
all user-facing surfaces.

## Practice sessions and limits

`practice_sessions` stores selected deck IDs, shuffle choice, queue totals,
view/reveal/rating counters, start time, and completion time.
`practice_session_cards` stores the immutable queue position and interaction
timestamps.

The queue excludes deleted, suspended, and currently buried cards. It applies
local-day new/review limits and, when enabled, retains only one generated card
per note so reverse, Fill in the blank, and template siblings do not appear in
the same initial session. A card must be revealed before a score can be accepted.

## State history

`card_state_events` records manual state changes with bounded JSON details:
suspend, resume, bury, unbury, reset, reschedule, move, copy, Struggling, and
clear-Struggling. It is an audit aid and does not replace current card/schedule
state.

## `.studydeck` version 2

A version 2 package is a standalone SQLite database with an exact interchange
schema for:

- metadata and one deck;
- templates used by exported notes;
- notes and generated card metadata;
- tags and note-tag relationships;
- managed assets and note-asset relationships.

It deliberately excludes schedules, review events, reversals, session state,
drafts, settings, analytics, and state-event history.

Import validates exact schema objects and declared column types, metadata shape,
row limits, IDs, timestamps, relationships, templates, generated-card counts,
rich content, asset references, image signatures, and total byte limits before
one transaction. Local ID collisions are remapped. A package template with an ID
matching a different local template is imported under a fresh ID and unique name
rather than replacing local content.

## Migration policy

Phase 2 is migration `0004_standalone_flashcard_core.sql`. Released migrations
and release fixtures are append-only. `database.rs` verifies each migration by
SHA-256 checksum and tests upgrades from the immutable 0.1.0 schema fixture.
