# Card format, scheduling, and data model

## Canonical card content

ostudy stores validated ProseMirror/Tiptap JSON rather than HTML. Markdown-style
shortcuts may be used while editing; they are not the durable representation.

Each note stores:

- `content_format = "tiptap-json"`
- `content_schema_version = 1`
- `front_doc` and `back_doc`: validated structured documents
- `front_text` and `back_text`: plain text derived by Rust
- `duplicate_hash`: a stable candidate fingerprint of normalized text
- timestamps, soft-deletion state, and an optimistic revision

Supported content is deliberately allowlisted. Links are limited to complete
`http`, `https`, `mailto`, and `tel` addresses. Images must use
`study-asset://<id>` and refer to an asset already copied into the managed image
directory. Arbitrary HTML, JavaScript URLs, remote images, local file paths,
unknown nodes, unknown marks, and unknown attributes are rejected.

## Notes, cards, and tags

A **note** is editable source content. A **card** is a studyable presentation
generated from that note. The current basic note creates one forward card.

Core content tables:

- `decks`: deck identity and metadata
- `notes`: versioned source documents, derived text, and duplicate fingerprints
- `cards`: studyable prompt identities
- `tags`: global display names and case-insensitive normalized identity
- `note_tags`: note-to-tag relationships
- `assets`: managed image metadata and relative paths
- `note_assets`: note-to-image relationships by side

Tags attach to notes so future reverse and Fill in the blank cards can share the
same source tags. Each note is limited to 32 unique tags of 64 visible
characters each.

## Full-text search

`card_search_fts` is an SQLite FTS5 virtual table containing note ID, deck ID,
question text, answer text, and joined tag text. Existing notes are indexed by
migration and triggers keep the index synchronized after supported writes.

Search commands support:

- all decks or one deck;
- one or more required tags;
- all, due, or not-due cards;
- relevance, recently updated, question, or due-date ordering;
- bounded pagination.

The user query is tokenized and quoted by Rust. It is never accepted as raw
FTS5 grammar.

## Duplicate detection

Question and answer text are trimmed, internal whitespace is collapsed, and
case is folded before the stable fingerprint is calculated. The fingerprint
narrows the candidate set; complete normalized text is compared before a match
is returned.

The rich editor requires an explicit override to save an exact duplicate. Bulk
entry can skip duplicates both within the submitted rows and against the
destination deck. Image-only cards are not considered duplicates merely
because their searchable text is empty.

## Bulk entry

Bulk entry accepts up to 500 in-application rows. Each row has a question,
answer, and optional comma-separated tags. Rust converts the plain text into
canonical paragraph documents, validates every row, and inserts all accepted
cards in one immediate transaction. There is no CSV/TSV file-import path.

## Move and copy

Moving updates `notes.deck_id` and all generated `cards.deck_id` values while
preserving card IDs, schedules, and review events. Copying creates new note and
card identities, copies tag and asset relationships, and creates fresh local
schedules. Both operations are bounded and transactional.

## Practice confidence and FSRS

After revealing an answer, the learner records one confidence score:

| Score | Display label | Stored outcome | FSRS rating |
| --- | --- | --- | --- |
| 0 | Wrong | `wrong` | Again (1) |
| 1 | Hard | `not_confident` | Hard (2) |
| 2 | Medium | `somewhat_confident` | Good (3) |
| 3 | Easy | `very_confident` | Easy (4) |

Scheduling data remains split between mutable `card_schedules` and append-only
`review_events`.

## `.studydeck` packages

A `.studydeck` file is a standalone SQLite interchange package containing one
deck, notes, cards, relationships, and managed image bytes. Stage 2 exports add
optional `tags` and `note_tags` tables. Older packages without those tables
remain importable, and older readers can ignore the additional tables.

Imported cards receive fresh schedules and do not inherit another learner's
review history. Packages exclude application settings and personal reviews.

## Future field templates

The Phase 2 template language is data-only. A template may define named fields
such as `{{medication name}}` or `{{side effects}}`, but it will not expose an
expression evaluator or allow JavaScript, SQL, shell commands, raw HTML,
filesystem access, or network access. Generated output must pass the same Rust
document validator.
