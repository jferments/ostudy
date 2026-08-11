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

## Stage 3 — Card types and safe templates — complete

- A **Card template** selector that defaults to Basic.
- **Reverse cards** generated from Basic notes, with separate forward and reverse
  schedules.
- **Fill in the blank** authoring with one canonical scheduled card whose selected
  blank is stable for the complete practice session.
- Stable generated-card identities that preserve schedules and review history
  when a direction or card type is disabled and later re-enabled.
- A **Templates** sidebar workspace for reusable field definitions and rich
  question/answer rules.
- A bounded data-only `{{field name}}` substitution language with no evaluator,
  script, SQL, shell, raw HTML, filesystem, or network capabilities.
- Automatic inline or block placement from the submitted rich field value.
- Permanent ordinary Basic cards generated from one completed template form.
- Template deletion that never deletes or changes previously generated cards.
- Generated-card support across Search/Filter, Practice, analytics, tags,
  move/copy, duplicate handling, and `.studydeck` transfer.

## Stage 4 — Reliable editing and media layout — deferred

- Autosaved editor drafts and interrupted-edit recovery.
- Image sizing and alignment controls.
- Safe image cleanup that does not delete assets still referenced by drafts,
  copied cards, or imported decks.

## Stage 5A — Scheduling controls and workload management — complete in PR #16

- Exact-card suspend/resume and bury/unbury controls.
- Reset to a fresh New schedule without deleting review history.
- Manual rescheduling that changes the next due time without recording a review.
- Global daily new-card and review limits.
- Scheduled, extra, and custom session modes.
- Due-review backlog protection that pauses normal new-card introduction.
- Four FSRS interval previews before a rating is submitted.
- Repeatedly missed-card detection under the user-facing name
  **Needs more practice**.
- Compact controls in Practice, Search/Filter, and Settings.

## Stage 5B — Learning analytics and progress visualization — implemented in PR #17

- Mixed progress visualization with unique cards or focused study time as bars
  and remembered-answer rate as a line.
- Thirty-day, ninety-day, one-year, and all-time calendar ranges.
- Whole-library or selected deck/tag scope, with deck union and tag intersection
  semantics matching Search/Filter.
- Summary metrics for practice volume, recall, confidence, study time, due cards,
  and Needs more practice cards.
- Topic-level performance, trends, current workload, and due-only practice entry.
- Deck performance, recent sessions, and recent-review details.
- Accessible chart descriptions and a keyboard-accessible semantic data table.
- Session-completion navigation into deck-scoped analytics with the completed
  session highlighted.
- Theme-aware Chart.js rendering with animations disabled by Reduce motion.

## Deferred Stage 5 item

- Undo the last review is intentionally deferred. Exact reversal requires its own
  complete schedule-snapshot and append-only reversal design.

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
