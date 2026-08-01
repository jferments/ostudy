# ostudy architecture

ostudy is a local-first Tauri desktop application. React and TypeScript render
the interface; Rust owns durable behavior and validation; SQLite stores
structured state; the native filesystem stores managed image files.

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
- `src/screens/DecksScreen.tsx` manages decks and opens the card editor.
- `src/screens/PracticeScreen.tsx` starts single- or multi-deck sessions,
  presents the one-step 0–3 confidence scale, and shows recent deck results.
- `src/screens/AnalyticsScreen.tsx` presents deterministic confidence,
  response-time, and workload aggregates.
- `src/screens/SettingsScreen.tsx` edits appearance and review preferences.
- `src/editor/` contains the Tiptap configuration, canonicalization, display,
  and link handling.
- `src/ipc.ts` is the only frontend module that invokes Rust commands.

### Rust

- `database.rs` opens SQLite, applies checksummed migrations, and exposes the
  connection through one mutex-protected application state.
- `decks.rs` owns deck validation and persistence.
- `cards.rs` validates Tiptap JSON, derives search text, and writes notes/cards
  transactionally.
- `assets.rs` imports supported local image files into the managed library.
- `scheduler.rs` is the sole adapter to `rs-fsrs`.
- `practice.rs` creates due-card snapshots and records graded reviews atomically.
- `analytics.rs` calculates due counts, correctness, timing, canonical rating
  distribution, per-deck results, and recent reviews using ordinary SQL.
- `transfer.rs` validates and imports/exports `.studydeck` packages.
- `settings.rs` validates and persists graphical configuration.

## Note and card separation

A **note** stores editable content. A **card** is one studyable presentation
generated from a note. The current basic note creates one forward card. The
separation is retained so reverse cards, Fill in the blank cards, and field-based
templates can generate several cards without duplicating source content.

Rich content is stored as versioned Tiptap/ProseMirror JSON rather than HTML.
Markdown-style shortcuts are editing conveniences, not the durable format.

## Practice confidence and scheduling

The learner gives one score after revealing an answer:

| Score | Learner wording | FSRS rating |
| --- | --- | --- |
| 0 | Wrong | Again |
| 1 | Hard | Hard |
| 2 | Medium | Good |
| 3 | Easy | Easy |

The existing durable review ledger stores the canonical FSRS rating and its
historical outcome string. The frontend maps the 0–3 score to that stable format,
so changing the interaction does not rewrite prior reviews or invalidate deck
libraries.

`card_schedules` is the fast mutable view of current FSRS state.
`review_events` is the immutable historical ledger. Review submission is one
idempotent SQLite transaction. The frontend converts each canonical 1–4 rating
to the displayed 0–3 confidence score by subtracting one; the underlying facts
remain deterministic.

## Untrusted card content

All card and imported-deck content is untrusted until Rust validates it.

- Cards are structured JSON, never executable HTML or script source.
- Node types, mark types, attributes, nesting, sizes, colors, and image
  references use explicit allowlists and bounds.
- Links are limited to complete `http`, `https`, `mailto`, and `tel` addresses;
  JavaScript, data, file, and malformed schemes are rejected.
- Images must reference an asset already copied into the managed image library.
  Cards cannot request arbitrary local paths or remote tracking images.
- Search text is derived from the validated document in Rust rather than trusted
  from a frontend snapshot.
- SQLite statements use bound parameters for user-controlled values.
- Unknown document nodes and formatting fail validation instead of being passed
  through to the WebView.

The future template language follows the same boundary: field names and values
are data. Templates may select and substitute fields, but they may not evaluate
JavaScript, SQL, shell commands, raw HTML, filesystem operations, network
requests, or general-purpose expressions.

## Local files and deck packages

Images are copied into the application data directory and referenced by stable
asset identifiers. `.studydeck` packages contain content and managed images but
omit personal review history and settings. Imported cards receive fresh local
schedules.

## Error containment

Rust commands return stable, user-safe error codes and messages. React error
boundaries prevent a single rendering exception from producing an unexplained
blank Tauri window. Pending-operation guards prevent duplicate writes from fast
repeated clicks.

## Comments and documentation

Comments explain invariants, security boundaries, transaction ordering, race
prevention, and non-obvious product decisions. Architecture Decision Records
document durable cross-module choices.
