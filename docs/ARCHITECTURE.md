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
- `src/screens/DecksScreen.tsx` manages decks and provides the paged card
  browser, full-text search, filters, sorting, move/copy, and import/export
  actions.
- `src/screens/CardEditorScreen.tsx` edits Basic, Reverse, Fill in the blank,
  and field-template notes and manages tags and template definitions.
- `src/screens/PracticeScreen.tsx` supports direct single-deck launches and
  manual multi-deck scheduled sessions.
- `src/screens/AnalyticsScreen.tsx` presents deterministic review aggregates.
- `src/screens/SettingsScreen.tsx` edits appearance and review preferences.
- `src/editor/` contains the Tiptap configuration, canonicalization, display,
  and safe link handling.
- `src/ipc.ts` is the only frontend module that invokes Rust commands.

### Rust

- `database.rs` opens SQLite, applies checksummed append-only migrations, and
  exposes the connection through one mutex-protected application state.
- `decks.rs` owns deck validation and persistence.
- `cards.rs` validates canonical Tiptap JSON and retains the compatibility CRUD
  path for Basic notes.
- `authoring.rs` owns note modes, generated-card identities, literal field
  templates, Fill in the blank parsing, tags, duplicate keys, FTS refresh, and
  browser queries.
- `assets.rs` imports supported local image files into the managed library.
- `scheduler.rs` is the sole adapter to `rs-fsrs`.
- `practice.rs` creates due-card snapshots, renders the selected generated card
  by card ID, and records graded reviews atomically.
- `analytics.rs` calculates due counts, accuracy, timing, rating distribution,
  per-deck results, and recent reviews using ordinary SQL.
- `transfer.rs` handles version 1 `.studydeck` compatibility and validates the
  legacy package boundary.
- `transfer_v2.rs` exports and imports the current exact-schema SQLite package,
  including templates, tags, generated cards, and managed images.
- `settings.rs` validates and persists graphical configuration.

## Notes, generated cards, and schedules

A **note** stores the editable source material. A **card** is one studyable
presentation generated from that note. A Basic note generates one forward card;
a Reverse note generates forward and reverse cards; a Fill in the blank note
generates one card per marked answer; and a field-template note generates one
card per template card definition.

Generated cards have stable IDs, directions, ordinals, and independent schedule
rows. Practice queues card IDs—not merely note IDs—so every generated card shows
the correct prompt. Editing a note updates its desired generated-card set in one
transaction while retaining matching card identities.

Moving a note preserves those generated-card identities and schedules while
changing the owning deck. Copying intentionally creates new note/card identities
and fresh New schedules, while retaining validated content, tags, and managed
image relationships.

Rich content is stored as versioned Tiptap/ProseMirror JSON rather than HTML.
Rust accepts only an explicit node, mark, attribute, link-scheme, and managed
image allowlist. Markdown-style shortcuts are an editing convenience, not the
durable format.

## Templates and Fill in the blank

The Stage 1 template language is intentionally data-only:

- `{{field name}}` is the only special template syntax;
- field names follow a bounded character grammar;
- expressions, property access, loops, executable HTML, JavaScript, CSS, and SQL
  are unsupported;
- substituted values are rendered into ordinary JSON text nodes.

Fill in the blank uses literal `[[answer]]` markers. Nested, empty, unmatched,
or excessive markers are rejected. The product calls this feature “Fill in the
blank”; the legacy SQLite enum value remains internal for schema compatibility.

See [`decisions/0004-secure-authoring-foundation.md`](decisions/0004-secure-authoring-foundation.md).

## Search and duplicate detection

SQLite FTS5 stores a content-only search row for each active note. Trusted Rust
transactions refresh that row after note, tag, or import changes. Search input
is tokenized into ordinary words, quoted, combined, and bound as a parameter so
user-entered FTS operators are not executed as query syntax.

Duplicate detection uses a stable application-derived key over normalized mode,
front, back, and field data. The key is an accelerator and warning mechanism;
the learner can explicitly keep both notes.

## Scheduling and analytics

`card_schedules` is the fast mutable view of current FSRS state.
`review_events` is the immutable historical ledger. Review submission is one
idempotent SQLite transaction. See
[`decisions/0003-fsrs-review-ledger.md`](decisions/0003-fsrs-review-ledger.md).

Stage 1 preserves the existing two-step review rating flow. Direct 0–3
Wrong/Hard/OK/Easy scoring and matching analytics are Stage 3 work, so the
scheduler and ledger change together rather than being partially migrated.

Analytics are deterministic. A future AI component may interpret these
aggregates, but it will not be responsible for calculating basic review facts.

## Local files and deck packages

Images are copied into the application data directory and referenced by stable
asset identifiers. Cards do not store arbitrary absolute paths or remote image
URLs.

A version 2 `.studydeck` package is an ordinary SQLite database with an exact,
bounded schema. Import opens it read-only, disables SQLite trusted-schema
features, runs integrity and object/column checks, validates every identifier,
relationship, rich document, template, tag, and image, and then inserts the
content in one local transaction. Packages omit personal review history and
settings; imported cards receive fresh schedules. Version 1 packages continue
through a constrained compatibility importer.

## Error containment

Rust commands return stable, user-safe error codes and messages. React error
boundaries prevent a single rendering exception from producing an unexplained
blank Tauri window. Optimistic revisions prevent silent overwrite conflicts,
and pending-operation guards prevent duplicate writes from fast repeated clicks.

## Comments and documentation

Comments explain invariants, security boundaries, transaction ordering, race
prevention, and non-obvious product decisions. They should not narrate obvious
syntax. Architecture Decision Records document durable cross-module choices.
