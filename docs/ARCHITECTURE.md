# ostudy architecture

ostudy is a local-first Tauri desktop application. The frontend is React and
TypeScript; durable behavior is implemented in Rust; SQLite stores structured
state; the native filesystem stores managed image files.

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
- `src/screens/PracticeScreen.tsx` supports direct single-deck launches and
  manual multi-deck scheduled sessions.
- `src/screens/AnalyticsScreen.tsx` presents deterministic review aggregates.
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
- `analytics.rs` calculates due counts, accuracy, timing, rating distribution,
  per-deck results, and recent reviews using ordinary SQL.
- `transfer.rs` validates and imports/exports `.studydeck` packages.
- `settings.rs` validates and persists graphical configuration.

## Note and card separation

A **note** stores editable question and answer content. A **card** is a
studyable presentation generated from that note. Version 0.3 creates one forward
card per note. This separation permits reverse, cloze, and image-occlusion cards
later without duplicating the underlying content.

Rich content is stored as versioned Tiptap/ProseMirror JSON rather than HTML.
Rust accepts only an explicit node/mark allowlist. Markdown-style shortcuts are
an editing convenience, not the durable format.

## Scheduling and analytics

`card_schedules` is the fast mutable view of current FSRS state.
`review_events` is the immutable historical ledger. Review submission is one
idempotent SQLite transaction. See
[`decisions/0003-fsrs-review-ledger.md`](decisions/0003-fsrs-review-ledger.md).

Analytics are deterministic. A future AI component may interpret these
aggregates, but it will not be responsible for calculating basic review facts.

## Local files and deck packages

Images are copied into the application data directory and referenced by stable
asset identifiers. Cards do not store arbitrary absolute paths or remote image
URLs. `.studydeck` packages contain content and managed images but omit personal
review history and settings. Imported cards receive fresh schedules.

## Error containment

Rust commands return stable, user-safe error codes and messages. React error
boundaries prevent a single rendering exception from producing an unexplained
blank Tauri window. Pending-operation guards prevent duplicate writes from fast
repeated clicks.

## Comments and documentation

Comments explain invariants, security boundaries, transaction ordering, race
prevention, and non-obvious product decisions. They should not narrate obvious
syntax. Architecture Decision Records document durable cross-module choices.
