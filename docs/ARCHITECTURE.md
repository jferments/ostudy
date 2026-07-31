# ostudy architecture

ostudy is a local-first Tauri desktop application. React and TypeScript display
state and collect user intent; Rust validates and executes application behavior;
SQLite stores structured state; the native filesystem stores managed images.

## Trust boundary

```text
Untrusted editor/import/search input
                │ typed Tauri IPC
                ▼
Rust command handlers
  validation • normalization • authorization-by-scope
                │
                ▼
Rust domain transactions
  decks • notes/cards • study • transfer • analytics
                │
       ┌────────┴─────────┐
       ▼                  ▼
library.sqlite3    managed image directory
```

The frontend never executes SQL, writes arbitrary application files, or owns the
authoritative schedule. All IPC inputs, card documents, template patterns,
search strings, image metadata, and imported package rows are treated as
untrusted.

## Frontend modules

- `src/App.tsx` owns navigation, settings application, dirty-editor protection,
  and cross-screen practice launch requests.
- `src/screens/DecksScreen.tsx` owns deck navigation, card browsing, FTS/tag
  filters, bulk entry, schedule/state actions, and move/copy dialogs.
- `src/screens/CardEditorScreen.tsx` owns basic, reverse, Fill in the blank, and
  template authoring; draft recovery; duplicate confirmation; and card preview.
- `src/screens/PracticeScreen.tsx` owns compact multi-deck setup, recent practice
  summaries, answer reveal, direct 0–3 ratings, interval previews, keyboard
  shortcuts, undo, and completion results.
- `src/screens/AnalyticsScreen.tsx` presents deterministic rating, deck, timing,
  due, suspended, and Struggling-card aggregates.
- `src/screens/SettingsScreen.tsx` edits appearance, daily limits, desired
  retention, related-card burying, and Struggling-card behavior.
- `src/editor/` contains the Tiptap extension allowlist, canonicalization, image
  size/alignment attributes, and read-only rendering.
- `src/components/Modal.tsx` provides labelled modal semantics, Escape handling,
  focus trapping, and focus restoration.
- `src/ipc.ts` is the only frontend module that invokes Rust commands.

## Rust modules

- `database.rs` opens SQLite with `trusted_schema=OFF`, applies checksummed
  append-only migrations, and owns the connection mutex.
- `decks.rs` validates and persists deck metadata.
- `cards.rs` validates the canonical Tiptap JSON format and managed image/link
  references. Legacy basic-card commands remain for compatibility.
- `phase2.rs` owns templates, generated cards, tags, FTS5 indexing, browsing,
  duplicate keys, drafts, bulk entry, state actions, and note move/copy.
- `scheduler.rs` is the only adapter to `rs-fsrs`; durable records use
  application-owned types.
- `study.rs` owns current practice-session construction, direct 0–3 scoring,
  interval previews, daily limits, sibling burying, Struggling-card state, exact
  snapshots, and undo.
- `practice.rs` retains small session lifecycle commands such as viewed,
  revealed, and completion timestamps.
- `analytics.rs` computes deterministic aggregates and recent per-deck rating
  mixes from unreversed review events.
- `assets.rs` imports recognized image bytes into the managed directory.
- `transfer_v2.rs` writes and reads current SQLite `.studydeck` packages under a
  strict trust boundary.
- `transfer.rs` is the read-only compatibility path for version-1 packages.
- `settings.rs` validates and persists appearance and study settings.

## Note, generated-card, and template model

A **note** contains the editable source. A **card** is a studyable presentation
generated from that note.

- Basic notes generate one forward card.
- Reverse notes generate forward and reverse cards.
- Fill in the blank notes generate one cloze-direction card per `[[answer]]`
  marker. The word “cloze” remains only in internal compatibility values.
- Template notes store a template ID plus a validated field-value map and
  generate one card per template card specification.

Editing one note synchronizes its generated cards transactionally. Removed
card directions are soft deleted; retained cards preserve their identities and
schedules; newly generated cards receive fresh schedules.

## Safe template language

Templates consist only of literal text and exact placeholders such as
`{{medication name}}`. The parser rejects empty placeholders, malformed braces,
triple braces, property access, control characters, undeclared fields, and
patterns beyond bounded sizes. Rendering performs literal lookup and creates
plain text nodes; substituted values are never reparsed as markup or code.

Fill in the blank syntax is similarly non-executable: `[[answer]]` marks a
literal answer span. Nested, empty, malformed, or excessive markers are
rejected.

See ADR 0004.

## Search and duplicate detection

`notes_fts` is an explicitly refreshed FTS5 table containing derived front/back
text, tags, and template fields. Rust tokenizes a search string into ordinary
word tokens and quotes every token before constructing the bound `MATCH`
parameter. User-entered FTS operators therefore remain literal terms rather
than query syntax.

Duplicate detection uses a stable normalized hash of the note mode, derived
front/back text, and template fields. It is an assistance feature, not a unique
constraint: the learner may explicitly save a duplicate.

## Scheduling, limits, and undo

`card_schedules` is the mutable current FSRS state. `review_events` remains the
append-only historical ledger. The UI's 0–3 scores map to the schema's canonical
1–4 FSRS ratings.

A review transaction:

1. verifies the session/card and revealed state;
2. checks idempotency by submission ID;
3. snapshots the exact schedule and all generated-card state for the note;
4. calculates the FSRS transition;
5. appends the review event;
6. updates the current schedule and session counters;
7. buries related cards when enabled;
8. recalculates Struggling-card and optional auto-suspend state;
9. commits atomically.

Undo does not edit or delete the review event. It appends a reversal, restores
the schedule and note-card state from exact snapshots, restores previous sibling
bury values, clears the session rating, and recalculates counters. Analytics and
daily limits exclude reversed events.

Session construction globally orders due cards, applies review/new limits, and,
when related-card burying is enabled, includes only one generated card per note.
This prevents a sibling already captured in the immutable queue from appearing
later in the same session.

See ADR 0003 and ADR 0005.

## SQLite packages

Current `.studydeck` exports use package version 2 and remain one standalone
SQLite database. They include one deck, notes, generated cards, referenced
custom templates, tags, note relationships, and image bytes. They exclude
settings, schedules, review events, reversals, drafts, and local state flags.

The importer:

- enforces a file-size limit before opening;
- opens read-only and enables `query_only` and `trusted_schema=OFF`;
- runs `PRAGMA integrity_check`;
- rejects triggers, views, indexes, virtual tables, or any object outside the
  package allowlist;
- verifies every table is `STRICT` and every column name, declared type,
  nullability, primary-key ordinal, order, and hidden flag exactly;
- validates exact bounded metadata and row-count limits;
- validates IDs, timestamps, revisions, templates, fields, rich documents,
  generated-card relationships, tags, image signatures, and asset links;
- remaps conflicting identities without replacing local templates;
- writes assets and database rows transactionally, removing created files after
  a failed import;
- creates fresh local schedules.

The Content Security Policy also disables objects and frames, limits connections
to local Tauri IPC, and permits images only from the application, managed asset
protocol, or data URLs needed by trusted editor behavior.

## Build boundary

All normal Cargo/Tauri builds go through
`scripts/with-immutable-cargo-lock.mjs`. The wrapper validates the committed
lockfile with `cargo metadata --locked`, records its bytes, runs the requested
command, and restores/fails if the command changes or deletes the lockfile. This
keeps dependency resolution separate from feature builds and prevents a dirty
`Cargo.lock` from unexpectedly blocking Git operations.
