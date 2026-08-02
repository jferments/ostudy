# Card format, scheduling, and data model

## Canonical card content

ostudy stores validated ProseMirror/Tiptap JSON rather than HTML. Markdown-style
shortcuts may be used while editing; they are not the durable representation.

Each note stores:

- `content_format = "tiptap-json"`
- `content_schema_version = 1`
- `front_doc` and `back_doc`: validated structured source documents
- `front_text` and `back_text`: plain text derived by Rust
- `duplicate_hash`: a stable candidate fingerprint of normalized source text
- `note_type`: `basic` or `cloze` for active Stage 3 authoring
- `reverse_enabled`: whether a Basic note currently exposes a reverse card
- `cloze_source`: the complete source statement and numbered blank markers
- `cloze_mode`: `one_at_a_time` or `all_together`
- timestamps, soft-deletion state, and an optimistic revision

Supported rich content is deliberately allowlisted. Links are limited to
complete `http`, `https`, `mailto`, and `tel` addresses. Images must use
`study-asset://<id>` and refer to an asset already copied into the managed image
directory. Arbitrary HTML, JavaScript URLs, remote images, local file paths,
unknown nodes, unknown marks, and unknown attributes are rejected.

## Notes, generated cards, and tags

A **note** is editable source content. A **card** is one independently scheduled
presentation generated from that note. One note may therefore have more than one
active row in `cards`.

Core content tables:

- `decks`: deck identity and metadata
- `notes`: versioned source documents, card-type metadata, derived text, and
  duplicate fingerprints
- `cards`: stable study-presentation identities
- `card_schedules`: current mutable schedule for each card ID
- `review_events`: append-only review history for each card ID
- `tags`: global display names and case-insensitive normalized identity
- `note_tags`: note-to-tag relationships
- `assets`: managed image metadata and relative paths
- `note_assets`: note-to-image relationships by side

Tags attach to notes, so all directions and blanks generated from one note share
the same classification. Each note is limited to 32 unique tags of 64 visible
characters each.

## Generated card identity

The `cards` table enforces:

```text
UNIQUE(note_id, direction, template_ordinal)
```

Stage 3 uses the tuple as follows:

| Presentation | `direction` | `template_ordinal` |
| --- | --- | --- |
| Basic forward | `forward` | `0` |
| Basic reverse | `reverse` | `0` |
| First one-at-a-time blank | `forward` | numbered blank ID |
| Additional one-at-a-time blank | `cloze` | numbered blank ID |
| All blanks together | `forward` | `0` |

A disabled presentation is soft-deleted. The card row, schedule, and review
history remain in the database. Re-enabling the same tuple clears `deleted_at`
and reuses the existing card ID. A genuinely new tuple receives a new card ID
and a fresh schedule.

The all-blanks presentation uses a separate identity from every individual
blank. Switching modes therefore cannot accidentally assign an individual
blank's schedule to the combined question. Switching back reactivates the prior
individual blank identities.

## Fill-in-the-blank source format

The editor marks selected text with a bounded data marker:

```text
{{c1::ACE inhibitors}} reduce formation of {{c2::angiotensin II}}.
```

The number after `c` is the stable blank identity. Rust validates the complete
marker grammar, group range, nesting, blank text, and total source size.

For `one_at_a_time`, one card is generated per distinct group. A question hides
only the target group and leaves other groups visible. For `all_together`, one
card hides every marked group. The answer includes the hidden text and the
complete unhidden statement.

Cloze source is data, not HTML or an expression language.

## Reusable card-template tables

The reusable template language is **data-only**. Authoring templates are stored
separately from notes and cards:

- `card_templates`: display name, description, timestamps, deletion state, and
  optimistic revision
- `card_template_fields`: ordered field names and optional input guidance
- `card_template_rules`: ordered names plus question and answer patterns

Patterns use complete placeholders such as `{{name}}`, `{{drug class}}`, or
`{{side effects}}`. Field names are normalized for case-insensitive identity but
retain the user's display spelling.

A generated card does **not** store a template ID. Template generation renders
literal field substitution, validates the resulting question and answer through
the ordinary content boundary, and inserts each accepted result as an ordinary
Basic note/card with a fresh schedule. Consequently, template edits or deletion
cannot modify or remove previously generated cards.

## Template safety and limits

The template language exposes no evaluator or general expression syntax.
Validation includes:

- 1–32 fields and 1–32 card rules per template
- bounded names, descriptions, patterns, field values, and generated output
- unique normalized field and rule names
- complete, non-nested `{{field name}}` placeholders
- rejection of unknown fields and unknown IPC properties
- literal substitution followed by canonical card-content validation

Templates cannot execute JavaScript, SQL, shell commands, raw HTML, filesystem
operations, network requests, or arbitrary local paths.

## Full-text search

`card_search_fts` is an SQLite FTS5 virtual table containing note ID, deck ID,
source question text, source answer text, and joined tag text. Existing notes are
indexed by migration and triggers keep the index synchronized after supported
writes.

Search commands support:

- all decks or one or more selected decks
- one or more required tags
- all, due, or not-due cards
- last-studied filters
- relevance, recently updated, recently studied, question, or due-date ordering
- bounded pagination and bounded all-results selection

Each active generated `cards` row is returned separately. Rust renders its exact
forward, reverse, individual-blank, or all-blanks question before serialization.
The user query is tokenized and quoted by Rust; raw FTS5 grammar is never
accepted.

## Duplicate detection

Source question and answer text are trimmed, internal whitespace is collapsed,
and case is folded before the stable fingerprint is calculated. The fingerprint
narrows the candidate set; complete normalized text is compared before a match
is returned.

The Basic rich editor requires an explicit override to save an exact duplicate.
Bulk entry and reusable-template generation skip duplicates within the submitted
batch and against the destination deck. Image-only cards are not considered
duplicates merely because searchable text is empty.

## Bulk entry and reusable-template generation

Plain bulk entry accepts up to 500 in-application rows. Each row has a question,
answer, and optional tags. Rust converts text into canonical paragraph
documents, validates every row, and inserts accepted cards transactionally.

Reusable-template generation is a second in-application bulk-entry path. One
completed field form renders all rules, previews them in the UI, and inserts the
accepted results as permanent Basic cards. Neither path introduces a CSV/TSV
file format.

## Move and copy

Moving updates `notes.deck_id` and every active generated `cards.deck_id` while
preserving card IDs, schedules, and review events. Copying creates a new note and
new card identities for every active presentation, copies tag and asset
relationships, and creates fresh local schedules. Both operations are bounded
and transactional.

## Practice confidence and FSRS

After revealing an answer, the learner records one confidence score:

| Score | Display label | Stored outcome | FSRS rating |
| --- | --- | --- | --- |
| 0 | Wrong | `wrong` | Again (1) |
| 1 | Hard | `not_confident` | Hard (2) |
| 2 | Medium | `somewhat_confident` | Good (3) |
| 3 | Easy | `very_confident` | Easy (4) |

Scheduling remains split between mutable `card_schedules` and append-only
`review_events`. Every active generated presentation is selected and submitted
by its exact card ID.

## `.studydeck` packages

A `.studydeck` file is a standalone SQLite interchange package containing one
deck, notes, active generated cards, relationships, and managed image bytes.
Optional extension tables preserve:

- Stage 2 tags and note-tag relationships
- Stage 3 reverse/cloze metadata

Older Basic packages without these tables remain importable, and older readers
can ignore the additional tables in new exports.

Imports validate the complete note/card graph and create fresh local schedules.
They do not inherit another learner's review history. Packages exclude
application settings, personal reviews, and the user's reusable authoring
template library.
