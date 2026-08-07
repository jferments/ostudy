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
- `note_type`: `basic` or `cloze` for active authoring
- `reverse_enabled`: whether a Basic note currently exposes a reverse card
- `cloze_source`: the complete source statement and numbered blank markers
- `cloze_mode`: `one_at_a_time` or `all_together`
- timestamps, soft-deletion state, and an optimistic revision

Supported rich content is deliberately allowlisted. Links are limited to
complete `http`, `https`, `mailto`, and `tel` addresses. Images must use
`study-asset://<id>` and refer to an asset already copied into the managed image
directory. Arbitrary HTML, JavaScript URLs, remote images, local file paths,
unknown nodes, unknown marks, and unknown attributes are rejected.

## Authoring command contract

The bundled editor submits a strict Basic-or-Cloze tagged request. Basic writes
carry canonical rich question/answer documents plus reverse configuration;
Cloze writes carry the bounded cloze source and mode. Type-inappropriate and
unknown properties are rejected. Browser plain-text snapshots are not part of
the write contract: Rust derives `front_text` and `back_text` from the validated
documents and enforces the authoritative text limits there.

The older Basic-only `create_organized_card` and `update_organized_card` IPC
commands are removed rather than retained as compatibility shims because bundled
Tauri IPC is an internal application contract.

## Notes, generated cards, and tags

A **note** is editable source content. A **card** is one independently scheduled
presentation generated from that note. A Basic note may expose forward and
reverse cards. A Fill in the blank note exposes exactly one active card even
when its source contains several marked blanks.

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

Tags attach to notes, so forward/reverse directions and every rotating cloze
presentation share the same classification. Each note is limited to 32 unique
tags of 64 visible characters each.

## Generated card identity

The `cards` table enforces:

```text
UNIQUE(note_id, direction, template_ordinal)
```

Active Stage 3 identities are:

| Presentation | `direction` | `template_ordinal` |
| --- | --- | --- |
| Basic forward | `forward` | `0` |
| Basic reverse | `reverse` | `0` |
| Fill in the blank | `forward` | `0` |

A disabled reverse presentation is soft-deleted. Its card row, schedule, and
review history remain in the database. Re-enabling the same tuple clears
`deleted_at` and reuses the existing card ID.

Every Fill in the blank mode and marked-blank rotation uses the same canonical
card identity. Adding, removing, or renumbering blanks therefore does not create
sibling schedules or search rows.

## Fill-in-the-blank source format

The editor marks selected text with a bounded data marker:

```text
{{c1::sinoatrial node}} initiates the {{c2::cardiac impulse}}.
```

The number after `c` identifies one presentation group within the source. Rust
validates the complete marker grammar, group range, nesting, blank text, and
total source size.

For `one_at_a_time`, the canonical card's accepted review count selects which
group will be hidden in the next practice session. The selected group is stored
in `practice_session_cards.presentation_variant`, so an active or resumed
session cannot change questions after viewing or revealing the card. For
`all_together`, the same card hides every marked group.

Cloze source is data, not HTML or an expression language.

## Reusable card-template tables

The reusable template language is **data-only**. Authoring templates are stored
separately from notes and cards:

- `card_templates`: display name, description, timestamps, deletion state, and
  optimistic revision
- `card_template_fields`: ordered stable field IDs, display names, and optional
  descriptions
- `card_template_rules`: ordered names plus rich question and answer documents
- `card_template_assets`: managed images referenced by saved rule layouts

Users refer to fields by typing complete placeholders such as `{{Subject}}` or
`{{Important details}}` in a rule layout. Field names are normalized for
case-insensitive lookup but retain their display spelling. Rust resolves the
name to the field's stable ID before storage. Incomplete braces and unknown
field names are rejected.

Saved rule documents use one neutral inert placeholder node. The older
`templateFieldBlock` node remains readable only for compatibility with templates
created by the first schema-7 build. A checksummed one-time data repair rewrites
those nodes to the neutral representation; normal canonical startup does not
repeat that write.

## Automatic field placement

Template field values are complete validated Tiptap documents. Placement is
chosen from the submitted value's structure at preview/generation time:

- **Inline:** one paragraph, no managed images, and only text or hard breaks
  within that paragraph.
- **Block:** supported lists, images, headings, block quotes, code blocks,
  horizontal rules, multiple paragraphs, or another supported non-inline
  structure.

Inline values are spliced into the surrounding paragraph. For block values,
Rust splits a surrounding paragraph or heading into valid before/value/after
blocks, inserts the value's top-level nodes unchanged, and drops whitespace-only
wrapper paragraphs. Placement is never inferred from whitespace, a field name,
or the untrusted plain-text snapshot.

Final rendered questions and answers pass through the ordinary card-content
validator. A generated card does **not** store a template ID. Template edits or
deletion cannot modify previously generated cards.

## Template safety and limits

The template language exposes no evaluator or general expression syntax.
Validation includes:

- 1–32 fields and 1–32 card rules per template
- bounded names, descriptions, rich documents, field values, and generated
  output
- unique normalized field and rule names
- complete, non-nested `{{field name}}` placeholders
- stable field/rule IDs and rejection of unknown fields
- managed asset existence and ownership checks
- rejection of unknown IPC properties
- structural rendering followed by canonical card-content validation

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
forward, reverse, or current cloze presentation before serialization. A
multi-blank cloze note appears once because it owns one active card.

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
completed rich field form renders all rules, previews them in the UI, and
inserts accepted results as permanent Basic cards. Preview and generation use
the same automatic-placement and validation functions. Neither path introduces
a CSV/TSV file format.

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
Older Stage 3 packages with several cloze card rows are normalized immediately
to one active card per cloze note. Imports do not inherit another learner's
review history. Packages exclude application settings, personal reviews, and
the user's reusable authoring-template library.
