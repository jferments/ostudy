# ADR 0005: Note-level tags, FTS5 search, and explicit transfer semantics

- **Status:** Accepted
- **Applies from:** database schema 4

## Context

Phase 2 needs fast local card search, reusable tags, bulk entry, duplicate
warnings, and move/copy operations. These features must remain compatible with
future generated cards, preserve scheduling history where appropriate, and
avoid interpreting user search text as executable query syntax.

## Decision

1. Tags attach to notes through `note_tags`, not directly to generated cards.
2. `card_search_fts` is an SQLite FTS5 index whose rowid mirrors `notes.rowid`.
   Triggers synchronize content, deck membership, deletion state, and tag text.
3. Rust converts user search text to bounded quoted prefix tokens. Deck, tag,
   schedule, sorting, and paging values are separately bound SQL parameters.
4. Duplicate fingerprints are candidate indexes only. Complete normalized
   question and answer strings are compared before a duplicate is reported.
5. Move retains note/card identities, schedules, and review history. Copy
   creates new identities and fresh schedules while reusing managed assets.
6. Bulk entry and multi-card transfer are bounded immediate transactions.
7. `.studydeck` packages use optional tag tables so schema-3 packages remain
   importable and older readers may ignore the extension.

## Consequences

- Search remains entirely local and scales without loading every card into
  React.
- Future reverse or Fill in the blank cards inherit source tags naturally.
- FTS syntax injection is not possible through the interface.
- A hash collision cannot produce a false duplicate.
- Moving a card does not erase learning history; copying does not clone personal
  history.
- Optional tag package metadata requires a matching pass after the core import
  remaps IDs, but does not require a second incompatible deck format.
