# Phase 2 — Complete the standalone flashcard core

Phase 2 is split into small vertical stages. Each stage is implemented, tested,
and reviewed before work starts on the next one.

## Stage 1 — Practice confidence and a compact session screen — complete

- Replace the two-step Right/Wrong → confidence flow with one answer:
  **0 Wrong**, **1 Hard**, **2 Medium**, or **3 Easy**.
- Preserve the existing FSRS and review-ledger formats through a documented
  mapping at the application boundary.
- Update analytics to use the same 0–3 language and show average confidence.
- Replace the oversized Session options column with a small **Shuffle decks**
  checkbox beside the session count and Start button.
- Keep the shuffle choice on the Practice screen rather than duplicating it in
  application Settings.
- Keep every non-empty deck selectable. Each selected deck contributes due cards
  first, or all active cards when it has nothing due.
- Enable direct deck Practice actions whenever the deck contains at least one
  card, even when its due count is zero.
- Show recently practiced decks and their average confidence.
- Improve keyboard and focus behavior for revealing and rating cards.
- Make Tauri builds use Cargo's locked mode so builds cannot silently rewrite
  `src-tauri/Cargo.lock`.
- Document the card-content security boundary and the remaining Phase 2 order.

## Stage 2 — Organize, find, and enter cards — implemented in this branch

- Reusable, case-insensitive tags with per-deck tag counts.
- SQLite FTS5 search across validated question text, answer text, and tags.
- Server-side status/tag filtering, sorting, paging, and result counts.
- Keyboard card navigation and selection, including Arrow keys, Space,
  Ctrl/Command+A, Enter, and `/` to focus search.
- Bulk entry for up to 500 tab-separated or `::`-separated rows with optional
  comma-separated tags.
- Exact duplicate detection in the editor, bulk entry, and destination decks.
- Transactional move/copy between decks. Moves preserve schedules and review
  history; copies receive fresh schedules and no review history.

See [`PHASE-2-STAGE-2.md`](PHASE-2-STAGE-2.md) for behavior and test coverage.

## Stage 3 — Card types and safe templates

- Reverse cards.
- **Fill in the blank** cards.
- A small data-only template language with named fields such as
  `{{medication name}}` and `{{side effects}}`.
- Reusable card templates that generate multiple cards from one set of fields.
- Validation that prevents template content from executing JavaScript, SQL,
  shell commands, raw HTML, filesystem operations, or network requests.

## Stage 4 — Reliable editing and media layout

- Autosaved editor drafts and interrupted-edit recovery.
- Image sizing and alignment controls.
- Safe image cleanup that does not delete assets still referenced by drafts,
  copied cards, or imported decks.

## Stage 5 — Scheduling controls and workload management

- Suspend and bury.
- Reset and reschedule.
- Undo the last review.
- Daily new-card and review limits.
- Interval previews before a rating is submitted.
- Repeatedly missed-card detection under the user-facing name
  **Needs more practice**.

## Stage 6 — Whole-application quality pass

- Complete keyboard navigation.
- Accessibility audit and fixes.
- Shared-deck and template security regression audit.
- Responsive layout and visual polish.
- Final user and architecture documentation updates.

## Deferred or excluded

CSV/TSV file import is not part of Phase 2. Stage 2 bulk entry accepts pasted
TSV-style rows directly in the application. Deck interchange continues to use
versioned SQLite `.studydeck` files. Math support is deferred to a separate,
focused math-flashcard update.
