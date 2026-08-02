# Phase 2 — Complete the standalone flashcard core

Phase 2 is split into small vertical stages. Each stage is implemented, tested,
and reviewed before work starts on the next one.

## Stage 1 — Practice confidence and a compact session screen — complete

- One answer after reveal: **0 Wrong**, **1 Hard**, **2 Medium**, or **3 Easy**.
- Existing FSRS and review-ledger compatibility.
- Confidence analytics and recently practiced decks.
- Compact session setup and Practice-only shuffle control.
- Voluntary study when a deck has no cards due.
- Locked Cargo builds and the card-content security boundary.

## Stage 2 — Organize, find, and enter cards — complete

- Reusable tags attached to note source content.
- SQLite FTS5 search across questions, answers, and tags.
- A whole-library Search/Filter screen with deck, tag, and schedule filters.
- Sort controls, result paging, multi-selection, and keyboard navigation.
- Transactional bulk card entry.
- Exact normalized duplicate detection with explicit override.
- Move cards while preserving schedules and review history.
- Copy cards with fresh schedules.
- Optional tag metadata in backward-compatible `.studydeck` packages.

## Stage 3 — Card types and safe templates — complete in this branch

- A **Card template** selector that defaults to Basic.
- Reversible Basic cards with separate forward and reverse schedules.
- **Fill in the blank** authoring by selecting text and marking stable numbered
  blank regions.
- One independently scheduled card per blank or one card hiding all blanks.
- Stable generated-card identities that preserve schedules and review history
  when a direction or cloze mode is disabled and later re-enabled.
- A **Templates** sidebar workspace for reusable field definitions and
  question/answer rules.
- A bounded data-only `{{field name}}` substitution language with no evaluator,
  script, SQL, shell, raw HTML, filesystem, or network capabilities.
- Permanent ordinary Basic cards generated from one completed template form.
- Template deletion that never deletes or changes previously generated cards.
- Generated-card support across Search/Filter, Practice, analytics, tags,
  move/copy, duplicate handling, and `.studydeck` transfer.

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

CSV/TSV file import is not part of Phase 2. Deck interchange continues to use
versioned SQLite `.studydeck` files. Bulk entry is an in-application form rather
than a second file format. Math support is deferred to a separate, focused
math-flashcard update.
