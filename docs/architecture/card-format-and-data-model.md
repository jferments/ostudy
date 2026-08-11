# Card format and data model

ostudy separates what a learner edits from what the scheduler reviews. A **note** is the durable source of a question, answer, cloze statement, tags, and managed media. A **card** is one studyable presentation generated from that note. This distinction allows a Basic note to create forward and reverse cards with independent schedules while keeping shared content and tags in one place.

## Canonical rich content

Questions and answers are stored as validated, versioned Tiptap/ProseMirror JSON rather than HTML. Markdown-style editor shortcuts are an authoring convenience; HTML is not the durable representation.

A rich Basic note records:

- `content_format = "tiptap-json"`;
- `content_schema_version = 1`;
- canonical front and back documents;
- plain-text front and back values derived by Rust;
- a stable duplicate-candidate fingerprint;
- Basic/Reverse configuration;
- timestamps, soft-deletion state, and optimistic revision.

Supported node types, marks, attributes, nesting, sizes, colors, and link schemes are allowlisted. Images use managed asset identifiers such as `study-asset://<id>` and must already belong to the application-managed image store. Remote images, arbitrary filesystem paths, raw HTML, JavaScript URLs, unknown nodes, and unknown attributes are rejected.

The native validator derives searchable text from the canonical document. Browser-provided plain-text snapshots are never authoritative for limits, duplicate matching, or search.

## Notes and cards

The main ownership graph is:

```text
deck
  └── note
       ├── card presentation(s)
       │    ├── current schedule
       │    └── review events
       ├── tags
       └── managed assets
```

Core tables include:

- `decks`: deck identity, metadata, ordering, archive state, and deletion lifecycle;
- `notes`: editable source content and card-type metadata;
- `cards`: stable study-presentation identities;
- `card_schedules`: current mutable schedule state for each card;
- `review_events`: append-only review evidence;
- `tags` and `note_tags`: global tag identity and note relationships;
- `assets` and note/template asset relationships: managed file ownership;
- practice-session tables: stable snapshots of the cards and variants shown in a session.

Tags attach to notes. Forward and reverse cards from one note therefore share classification even though each card has its own schedule and review ledger.

## Basic and Reverse presentations

A normal Basic note exposes one card:

```text
(note_id, direction = forward, template_ordinal = 0)
```

When Reverse is enabled, it also exposes:

```text
(note_id, direction = reverse, template_ordinal = 0)
```

The database enforces stable uniqueness for the note, direction, and template ordinal. Disabling Reverse soft-deletes that presentation rather than destroying its identity. Re-enabling it restores the same card, schedule, and review history.

Reverse is a presentation decision, not a second copy of the note. Editing the shared source affects both directions, while due dates and review outcomes remain independent.

## Fill in the blank

A Fill in the blank note stores one bounded source string with numbered data markers:

```text
The {{c1::sinoatrial node}} initiates the {{c2::cardiac impulse}}.
```

The marker grammar, group numbers, nesting, hidden text, and total source size are validated natively. Markers are inert data; they are not HTML or an expression language.

A cloze note owns one stable scheduled card:

```text
(note_id, direction = forward, template_ordinal = 0)
```

In **one at a time** mode, accepted-review history selects the next group to hide. The chosen group is recorded in the practice-session snapshot so revealing, navigating away, or resuming cannot change the question mid-session. In **all together** mode, the same card hides every marked group.

Adding, removing, or renumbering blanks and switching modes changes presentation but does not create sibling schedules or reset the card's history.

## Reusable templates

Reusable templates are authoring tools, not runtime programs and not owners of generated cards. Their tables store:

- template name, description, timestamps, deletion state, and revision;
- ordered stable field identifiers and display names;
- ordered card-generation rules with rich question and answer layouts;
- managed assets referenced by saved layouts.

A rule refers to a field through a complete placeholder such as `{{Subject}}`. Field names are matched by normalized identity while retaining display spelling. Unknown, incomplete, nested, or ambiguous placeholders are rejected.

Field substitution is structural and literal. It exposes no JavaScript, SQL, shell, HTML, filesystem, network, or general expression capability.

### Inline and block placement

A submitted field value remains inline when it contains one paragraph made only of text and hard breaks and contains no managed image. Lists, images, headings, block quotes, code blocks, horizontal rules, multiple paragraphs, and other supported block structures are inserted as blocks.

When a block value appears inside a surrounding paragraph, the renderer splits the wrapper into valid before/value/after blocks and removes empty wrappers. It does not flatten a list or image to text simply because the placeholder was typed on a line with other content.

Preview and generation call the same rendering and validation functions. Accepted outputs become ordinary permanent Basic notes with fresh schedules. They do not retain a template ID, so editing or deleting the authoring template cannot mutate cards that were already generated.

## Search and projections

SQLite FTS5 indexes source question text, source answer text, deck identity, and joined tag text. Triggers keep the index synchronized after supported writes.

Search accepts bounded filters such as:

- one or more decks;
- one or more required tags;
- due or not-due state;
- last-studied windows;
- relevance, update, study, question, or due ordering;
- bounded pages and bounded select-all identities.

Rust tokenizes and quotes the user query; raw FTS5 grammar is never accepted directly. Each active card presentation is returned as its exact rendered direction. Forward and reverse cards therefore appear separately, while a multi-blank cloze note appears once because it owns one active card.

List operations use summary projections instead of hydrating every rich document and asset. Detail and practice views request the bounded content they actually need.

## Duplicate detection

Source question and answer text are trimmed, internal whitespace is collapsed, and case is folded before a stable fingerprint narrows the candidate set. Complete normalized text is compared before declaring an exact match.

The normal rich editor requires an explicit override to save an exact duplicate. Bulk entry, reusable-template generation, and generated JSON import skip duplicates within the submitted batch and against the destination deck. Image-only cards are not treated as duplicates solely because their searchable text is empty.

## Move, copy, delete, and archive

Moving changes the note's deck and the current ownership of its active card presentations while preserving card IDs, schedules, and review events. Historical review evidence follows the moved card to its current deck ownership.

Copying creates a new note, new card identities, copied tags and asset relationships, and fresh schedules. Review history is not cloned.

Note-level actions are deduplicated when several presentations from the same note are selected. Deleting a shared source removes all of its presentations. A finalized deck deletion removes the material and historical analytics it owns; an archived deck remains recoverable and visible in historical analytics.

Managed file cleanup is reference aware. A file is not deleted merely because one note or template stopped referencing it while another durable or draft owner remains.

## Scheduling and review ledger

After answer reveal, the learner records one confidence choice:

| Choice | Stored outcome | FSRS rating |
| --- | --- | --- |
| Wrong | `wrong` | Again (1) |
| Hard | `not_confident` | Hard (2) |
| Medium | `somewhat_confident` | Good (3) |
| Easy | `very_confident` | Easy (4) |

The current schedule and historical event are intentionally separate. Review submission validates the previewed card revision and commits both the new schedule state and immutable event in one transaction. Retried submissions are idempotent through stable operation and submission identity.

Suspend, bury, reset, and manual reschedule are schedule-control operations; they do not masquerade as reviews. Reset preserves historical events while returning current state to a fresh schedule. Manual reschedule changes the next due time without inventing recall evidence.

## Portable deck packages

A `.studydeck` package contains one deck's notes, active cards, relationships, and managed images. Extension tables preserve tags and current Reverse/Cloze metadata while allowing older Basic packages to remain readable.

Import validates the complete graph, assigns local identities where needed, and creates fresh schedules. It does not import another learner's review history. Older packages with multiple cloze rows are normalized into the canonical one-card cloze model.

Reusable templates, application settings, provider profiles, practice sessions, and personal analytics are not part of a shared deck package.

## Generated JSON import

A `.ostudy.json` file is content-only external input. It can create one named deck or append to exactly one active deck with an exact name. Basic and cloze content use restricted Markdown and the canonical cloze grammar.

The importer validates the whole file before one transaction, reports errors by card path, skips exact duplicates automatically, and gives every accepted card a fresh schedule. It rejects internal IDs, timestamps, schedules, history, analytics, settings, template IDs, local paths, HTML, images, and deletion or overwrite instructions.

See the [generated JSON reference](../reference/generated-json-import.md) for the complete versioned format.

## Complete-library backup

A `.ostudybackup` captures the canonical SQLite snapshot and managed files needed to restore the personal library, including schedules, reviews, sessions, analytics sources, templates, images, retained documents, and durable document-processing state. Credential bytes are excluded.

The backup boundary is broader than deck sharing because its purpose is recovery of one learner's complete state. See [Backups, imports, and sharing](../user-guide/backups-imports-and-sharing.md).
