# Phase 2 — Complete the standalone flashcard core

Phase 2 is split into small vertical stages. Each stage is implemented, tested,
and reviewed before work starts on the next one.

## Stage 1 — Practice confidence and a compact session screen

- Replace the two-step Right/Wrong → confidence flow with one answer:
  **0 Wrong**, **1 Hard**, **2 Medium**, or **3 Easy**.
- Preserve the existing FSRS and review-ledger formats through a documented
  mapping at the application boundary.
- Update analytics to use the same 0–3 language and show average confidence.
- Replace the oversized Session options column with a small **Shuffle decks**
  checkbox beside the due count and Start button.
- Show recently practiced decks and their average confidence.
- Improve keyboard and focus behavior for revealing and rating cards.
- Make Tauri builds use Cargo's locked mode so builds cannot silently rewrite
  `src-tauri/Cargo.lock`.
- Document the card-content security boundary and the remaining Phase 2 order.

## Stage 2 — Organize, find, and enter cards

- Tags.
- SQLite FTS5 search.
- Better card browsing, filtering, and keyboard selection.
- Bulk card entry.
- Duplicate detection.
- Move or copy cards between decks.

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

CSV/TSV import is not part of Phase 2. Deck interchange continues to use
versioned SQLite `.studydeck` files. Math support is deferred to a separate,
focused math-flashcard update.
