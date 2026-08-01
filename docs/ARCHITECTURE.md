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
opens arbitrary files, or calculates the authoritative schedule. Rust validates
all data crossing the IPC boundary and owns multi-step transactions.

## Main modules

### Frontend

- `src/App.tsx` owns top-level navigation, themes, and cross-screen launch
  requests.
- `src/screens/DecksScreen.tsx` manages decks and the original per-deck card
  list.
- `src/screens/CardsScreen.tsx` is the whole-library card browser. It sends
  search/filter/page requests to Rust, manages keyboard and multi-selection,
  and launches bulk entry or move/copy.
- `src/screens/CardEditorScreen.tsx` edits validated rich content, tags, and
  explicit duplicate overrides.
- `src/screens/PracticeScreen.tsx` runs one-step 0–3 review sessions.
- `src/screens/AnalyticsScreen.tsx` presents deterministic confidence,
  response-time, and workload aggregates.
- `src/screens/SettingsScreen.tsx` edits appearance and review preferences.
- `src/editor/` contains the Tiptap configuration, canonicalization, display,
  and link handling.
- `src/ipc.ts` is the only frontend module that invokes Rust commands.

### Rust

- `database.rs` opens SQLite, applies checksummed migrations, backfills stable
  duplicate fingerprints, and exposes one mutex-protected connection.
- `decks.rs` owns deck validation and persistence.
- `cards.rs` owns shared card hydration, per-deck list/delete behavior, and
  canonical Tiptap validation.
- `authoring.rs` is the only card create/update boundary; it owns tags, FTS5
  queries, duplicate checks, assets, deck changes, bulk entry, and move/copy.
- `assets.rs` imports supported local image files into the managed library.
- `scheduler.rs` is the sole adapter to `rs-fsrs`.
- `practice.rs` records graded reviews atomically.
- `practice_selection.rs` creates scheduled or voluntary session snapshots.
- `analytics.rs` calculates deterministic review facts using ordinary SQL.
- `transfer.rs` validates the core `.studydeck` content and images.
- `tagged_transfer.rs` adds and restores optional package tag metadata while
  keeping old packages readable.
- `settings.rs` validates and persists graphical configuration.

## Notes, cards, and tags

A **note** stores editable source content. A **card** is one studyable
presentation generated from a note. Tags attach to the note, not the generated
card, so reverse and Fill in the blank cards can later share one source
classification.

`tags.normalized_name` provides case-insensitive identity while preserving the
first human-readable spelling. `note_tags` is the many-to-many relationship.
Tag writes are bounded, deduplicated, and transactional with card content.

## FTS5 search

`card_search_fts` indexes active note questions, answers, deck IDs, and tag text.
Its rowid mirrors `notes.rowid`. SQLite triggers keep it synchronized when note
content, deck membership, deletion state, tags, or tag names change.

The interface never forwards raw FTS5 syntax. Rust extracts bounded
alphanumeric tokens, quotes them, binds the resulting expression as a
parameter, and applies deck, tag, due-state, sort, limit, and offset constraints
as separate bound values.

## Duplicate detection

Each note stores a stable FNV-1a fingerprint of normalized question and answer
text. The hash is only a candidate index: Rust always compares the complete
normalized strings before declaring a duplicate, so a hash collision cannot
produce a false duplicate. Historical notes are backfilled after migration.

Image-only cards are not treated as duplicates from empty text alone. The
editor warns before an exact duplicate is saved, and the write command repeats
the check inside an immediate transaction to close the race between warning and
save. Bulk entry can skip duplicates within the batch and destination deck.

## Move and copy semantics

- **Move** changes the note and generated card deck IDs in one transaction.
  Card IDs, schedules, and review history remain intact.
- **Copy** creates new note/card IDs, reuses managed asset records, copies tags,
  and creates fresh schedules without personal review history.
- The optional duplicate guard can leave matching cards in their source deck
  rather than creating destination duplicates.

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
idempotent SQLite transaction.

## Untrusted card content

All card and imported-deck content is untrusted until Rust validates it.

- Cards are structured JSON, never executable HTML or script source.
- Node types, marks, attributes, nesting, sizes, colors, and image references
  use explicit allowlists and bounds.
- Links are limited to complete `http`, `https`, `mailto`, and `tel` addresses.
- Images must reference managed local assets.
- Search text is derived from validated documents in Rust.
- SQLite statements use bound parameters for user-controlled values.
- Search input cannot inject FTS5 operators because Rust constructs the query.
- Unknown document nodes and formatting fail validation.

The future template language follows the same boundary: field names and values
are data, not executable expressions.
