# ostudy architecture

ostudy is a local-first Tauri desktop application. React and TypeScript render
the interface; Rust owns durable behavior and validation; SQLite stores
structured state and full-text indexes; the native filesystem stores managed
image files.

## Architectural boundary

```text
React screens and editors
        │ typed Tauri commands
        ▼
Rust command handlers
        │ validation and use-case orchestration
        ▼
Rust domain modules and SQLite transactions
        │
        ├── library.sqlite3
        └── managed image directory
```

The frontend displays state and communicates user intent. It never issues SQL,
opens arbitrary files, evaluates template expressions, selects authoritative
template placement, or calculates the authoritative schedule. Rust validates all
data crossing the IPC boundary and owns multi-step transactions.

## Main modules

### Frontend

- `src/App.tsx` owns top-level navigation, themes, dirty-state guards, and
  cross-screen practice launch requests.
- `src/screens/DecksScreen.tsx` manages decks and the per-deck source-note list.
- `src/screens/CardsScreen.tsx` is the whole-library Search/Filter workspace. It
  displays exact generated-card presentations, manages card-level selection,
  deduplicates note-level actions, and launches temporary practice sessions.
- `src/screens/CardEditorScreen.tsx` selects a built-in or reusable card
  template, edits Basic rich content, configures reverse cards, authors cloze
  notes, and fills reusable template forms.
- `src/screens/card-editor/ClozeEditor.tsx` owns selection-based blank marking
  and transient cloze preview state.
- `src/screens/card-editor/TemplateEntryForm.tsx` renders reusable template
  fields and generated-card previews.
- `src/screens/TemplatesScreen.tsx` manages reusable data-only field and rule
  definitions. Rule authors type placeholders such as `{{Subject}}`; the UI does
  not expose inline/block placement controls.
- `src/screens/PracticeScreen.tsx` runs one-step 0–3 review sessions.
- `src/screens/AnalyticsScreen.tsx` presents deterministic confidence,
  response-time, and workload aggregates.
- `src/screens/SettingsScreen.tsx` edits appearance and review preferences.
- `src/editor/` contains the Tiptap configuration, canonicalization, display,
  and link handling.
- `src/ipc.ts` is the only frontend module that invokes Rust commands.

### Rust

- `database.rs` opens SQLite, applies checksummed migrations, backfills stable
  duplicate fingerprints, runs idempotent forward normalization, and exposes one
  mutex-protected connection.
- `decks.rs` owns deck validation and persistence.
- `cards.rs` owns source-note hydration, per-deck list/delete behavior, and the
  canonical Tiptap validator.
- `authoring.rs` remains the card-authoring boundary. Its Stage 2 modules own
  tags, FTS5, duplicates, bulk entry, assets, and note-level actions. Its
  `authoring/stage3_v2/` modules own card-type validation, cloze parsing,
  generated-card lifecycle, template CRUD, typed-placeholder resolution,
  automatic rich-value placement, and generated-card rendering.
- `assets.rs` imports supported local image files into the managed library.
- `scheduler.rs` is the sole adapter to `rs-fsrs`.
- `practice.rs` records graded reviews atomically.
- `practice_selection.rs` selects exact generated-card IDs and hydrates the
  corresponding forward, reverse, or session-stable cloze presentation.
- `analytics.rs` calculates deterministic review facts using ordinary SQL.
- `transfer.rs` validates the core `.studydeck` content and images.
- `tagged_transfer.rs` preserves optional package tag metadata.
- `stage3_transfer.rs` preserves optional reverse/cloze metadata and generated
  card identities while keeping old Basic packages importable.
- `settings.rs` validates and persists graphical configuration.

## Notes, generated cards, and tags

A **note** stores editable source content. A **card** is one independently
scheduled study presentation generated from that note. Tags attach to the note
so every generated presentation shares the same classification.

Generated card identity is the durable tuple:

```text
(note_id, direction, template_ordinal)
```

Basic forward and reverse cards use `forward/0` and `reverse/0`. Every active
Fill in the blank note uses one canonical `forward/0` card, regardless of how
many marked blanks its source contains or which cloze mode is selected.

Disabling a reverse presentation soft-deletes its `cards` row. Its schedule and
append-only review events remain attached to the same card ID. Re-enabling the
same tuple reactivates that row rather than creating a replacement.

For one-at-a-time cloze, Rust chooses the next blank group from the canonical
card's accepted review count and stores that choice in the practice-session
snapshot. Viewing, revealing, leaving, or resuming the session cannot advance or
change the selected blank.

`tags.normalized_name` provides case-insensitive identity while preserving the
first human-readable spelling. `note_tags` is the many-to-many relationship.
Tag writes are bounded, deduplicated, and transactional with source-note
content.

## Reusable data-only templates

Reusable templates are authoring shortcuts, not note types and not ownership
parents. A template contains ordered fields and ordered rich question/answer
rules. Authors type complete placeholders such as `{{Subject}}` or
`{{Important findings}}` directly in a layout.

When a template is saved, Rust assigns stable child IDs, resolves each typed
field name to its stable ID, and stores a neutral inert placeholder. At preview
or generation time, Rust chooses placement from the validated field document:

- one text-only paragraph is inserted inline;
- lists, images, headings, block quotes, code blocks, horizontal rules, and
  multi-paragraph values are inserted as blocks.

If a block value occurs inside a paragraph or heading, Rust splits the layout
into valid before/value/after blocks and removes empty wrappers. It never
flattens rich content or infers placement from whitespace or field names.

The old explicit block placeholder remains readable only for compatibility with
initial schema-7 templates. Startup normalization converts it to the neutral
representation without a new database migration.

Every final document passes through the ordinary Basic-card validator and write
transaction. Generated notes do not store a template ID. Editing or deleting a
reusable template cannot modify or delete cards already generated from it.
Templates are local authoring preferences and are not included in deck exports.

## FTS5 search and exact presentation rendering

`card_search_fts` indexes active note source text, deck IDs, and tag text. Its
rowid mirrors `notes.rowid`; SQLite triggers keep it synchronized after supported
writes.

Search filtering happens over notes and schedules, but each matching active
`cards` row is returned separately. Rust renders the exact presentation by card
ID before sending it to the frontend. Basic reverse cards display swapped sides;
a multi-blank cloze note appears once and displays the session-selected blank.

The interface never forwards raw FTS5 syntax. Rust extracts bounded
alphanumeric tokens, quotes them, binds the resulting expression as a
parameter, and applies deck, tag, due-state, sort, limit, and offset constraints
as separate bound values.

## Duplicate detection

Each note stores a stable FNV-1a fingerprint of normalized source question and
answer text. The hash is only a candidate index: Rust compares complete
normalized strings before declaring a duplicate.

The Basic editor warns before an exact duplicate is saved, and Rust repeats the
check inside an immediate transaction. Bulk entry and reusable-template
generation skip exact duplicates within the submitted batch and destination
deck. Image-only cards are not considered duplicates merely because searchable
text is empty.

## Move and copy semantics

- **Move** changes the note and every generated card's deck ID in one
  transaction. Card IDs, schedules, and review history remain intact.
- **Copy** creates a new note and new IDs for every active generated
  presentation, copies tags and asset relationships, and creates fresh schedules
  without personal review history.
- Search/Filter deduplicates selected presentations by note ID before invoking a
  note-level move, copy, tag, or delete operation.

## Practice confidence and scheduling

The learner gives one score after revealing an answer:

| Score | Learner wording | FSRS rating |
| --- | --- | --- |
| 0 | Wrong | Again |
| 1 | Hard | Hard |
| 2 | Medium | Good |
| 3 | Easy | Easy |

`card_schedules` is the fast mutable view of current FSRS state.
`review_events` is the immutable historical ledger. Review submission is one
idempotent SQLite transaction. Basic forward/reverse presentations own separate
card IDs; every cloze blank shares the note's one canonical cloze card ID.

## Deck interchange

`.studydeck` packages contain notes, active generated cards, tags, managed
assets, and optional data-only card-type metadata. The importer validates the
complete card graph and creates fresh local schedules. Personal review events,
application settings, and reusable authoring templates are excluded.

Older Basic packages without optional Stage 2 or Stage 3 tables remain
importable. Older Stage 3 packages with several cloze rows are normalized to one
active card immediately after import. Older readers can ignore additional
optional tables in new exports.

## Untrusted content and template safety

All card, template, and imported-deck content is untrusted until Rust validates
it.

- Cards are structured JSON, never executable HTML or script source.
- Node types, marks, attributes, nesting, sizes, colors, and image references
  use explicit allowlists and bounds.
- Links are limited to complete `http`, `https`, `mailto`, and `tel` addresses.
- Images must reference managed local assets.
- Cloze marker grammar, group numbers, nesting, and source size are validated.
- Template field names, values, rules, placeholders, stable IDs, and output sizes
  are bounded and validated.
- Template rendering performs structural literal substitution; it has no
  evaluator, JavaScript, SQL, shell, raw HTML, filesystem, network, or general
  expression capability.
- Search text is derived by Rust and SQLite statements use bound parameters.
- Unknown structured content, unknown fields, or malformed placeholders fail
  visibly.
