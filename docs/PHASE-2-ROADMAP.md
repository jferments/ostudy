# Phase 2 roadmap — standalone flashcard core

Phase 2 is intentionally split into small, testable stages. Each stage must pass
`./scripts/check.sh`, receive a local desktop smoke test, and merge before the
next stage begins. CSV/TSV import is not planned; deck interchange remains a
single versioned SQLite `.studydeck` file. Math support is deferred to a later,
dedicated math-flashcard phase.

## Stage 1 — secure authoring foundation

Implemented by the first Phase 2 pull request:

- Basic and reverse notes, where one note may generate one or two review cards.
- **Fill in the blank** notes using literal `[[answer]]` markers.
- Field-based card templates using literal `{{field name}}` placeholders,
  including a built-in Pharmacology template.
- Tags, duplicate warnings, a paged card browser, filters, and SQLite FTS5
  search across card text, tags, and template fields.
- Move or copy a note—and all review cards generated from it—between decks.
  Moving preserves schedules; copying creates fresh New schedules.
- Version 2 SQLite deck packages that preserve tags, templates, generated-card
  definitions, and managed images without carrying personal review history.
- A strict untrusted-content boundary: rich text is allowlisted JSON, templates
  are data rather than executable code, SQL is parameterized, package schemas
  are allowlisted, and imported SQLite files are opened read-only with
  `trusted_schema` disabled.
- A Cargo.lock guard that prevents Tauri build commands from silently changing
  the committed Rust dependency lockfile.

## Stage 2 — faster authoring and deck organization

- Bulk card entry.
- Autosaved editor drafts and draft recovery.
- Image sizing and alignment controls.

These features depend on the Stage 1 note/card model so they can work uniformly
with basic, reverse, Fill in the blank, and template-generated cards.

## Stage 3 — study controls and direct confidence scoring

- Replace the current two-step answer flow with one 0–3 choice:
  **Wrong**, **Hard**, **OK**, or **Easy**.
- Update analytics to report the same four confidence choices.
- Suspend, bury, reset, and reschedule controls.
- Undo the last review.
- New-card and review limits plus interval previews.
- Detect repeatedly unsuccessful cards and label them **Struggling cards**
  rather than “leeches.”
- Bury related generated cards when appropriate.
- Replace the oversized Session options column with a compact **Shuffle decks**
  checkbox and add recent-deck confidence summaries.

## Stage 4 — keyboard, accessibility, and final polish

- Complete keyboard navigation across the deck browser, editor, and practice
  flow.
- Perform a documented accessibility audit covering focus, dialogs, labels,
  contrast, zoom, reduced motion, and screen-reader semantics.
- Resolve audit findings and run final regression testing across all Phase 2
  workflows.
