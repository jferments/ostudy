# Phase 2 Stage 2 — Organize, find, and enter cards

Stage 2 adds organization workflows without weakening the local-first data or
card-content security boundaries established in Phase 1.

## Tags

Tags are normalized by trimming and collapsing whitespace and are unique without
regard to case. A card can have up to 50 tags, each no longer than 80 characters.
The database stores one canonical tag row plus a many-to-many `note_tags` link.
Orphaned tag rows are removed after editing. Search and deck-level tag counts are
updated automatically.

Tags belong to the editable note rather than to one generated card. That choice
will let reverse, fill-in-the-blank, and template-generated cards inherit the
same organization metadata in Stage 3.

## FTS5 search and browsing

Migration 4 creates a SQLite FTS5 projection of active note question text,
answer text, and tags. SQLite triggers keep the projection synchronized when a
note is created, edited, moved, deleted, tagged, or untagged. The durable note
remains the source of truth.

Search input is converted to quoted prefix terms in Rust. Raw FTS syntax is
never passed through from the WebView. The browser command also validates and
binds deck IDs, tag IDs, status, sort, limit, and offset. Results can be filtered
to due, new, or reviewed cards and sorted by relevance, update time, creation
time, question text, or due time.

The deck browser loads at most 100 cards per page. Arrow Up and Arrow Down move
focus, Space toggles selection, Enter opens the focused card, Ctrl/Command+A
selects the visible page, and `/` focuses search when the user is not already
typing in a control.

## Bulk entry and duplicates

Bulk entry accepts up to 500 non-empty rows. A tab is the preferred delimiter;
`::` is accepted for easier manual typing. A third column contains optional
comma-separated tags. Rust creates ordinary validated Tiptap documents from the
plain text and writes the batch in one immediate SQLite transaction.

The user explicitly chooses whether exact duplicates are skipped, allowed, or
cause the entire batch to roll back. Duplicate comparison trims the two sides
and compares their Unicode-lowercased question-and-answer pair. The normal card
editor performs the same library-wide check and requires a deliberate **Save
anyway** action when a match exists.

## Move and copy semantics

Both operations are one SQLite transaction and skip a selected note when the
destination already contains the same normalized question-and-answer pair.

- **Move** changes the note and generated cards' deck IDs. It preserves card IDs,
  tags, managed-image links, FSRS schedule state, and review history.
- **Copy** creates new note and card IDs, copies content, tags, and managed-image
  links, and creates a fresh FSRS schedule. It intentionally does not copy the
  learner's review history.

## Automated coverage

The repository gate covers:

- schema 3 → schema 4 fixture migration and FTS indexing;
- migration idempotence and the expected Stage 2 tables;
- tag normalization and invalid-tag rejection;
- FTS query sanitization;
- integrated bulk creation, duplicate skipping, tag indexing, search, copy, and
  move behavior against an in-memory SQLite database;
- bulk-row parsing in React tests;
- indexed browser IPC requests and selection actions in component tests;
- the existing TypeScript, Rust formatting, Clippy, Tauri build, and native
  smoke-test gates.
