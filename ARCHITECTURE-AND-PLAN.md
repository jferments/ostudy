# ostudy architecture and implementation plan

Canonical documentation is split by purpose:

- `README.md`: product scope and everyday commands;
- `docs/ARCHITECTURE.md`: implemented boundaries and data flow;
- `docs/CARD-FORMAT-AND-DATA-MODEL.md`: durable card and SQLite formats;
- `docs/DEVELOPMENT-AND-QUALITY.md`: tests, CI, diagnostics, and releases;
- `docs/GIT-WORKFLOW.md`: branch, pull-request, and rollback workflow;
- `AGENTS.md` and `docs/agents/`: instructions for automated maintainers;
- `docs/decisions/`: accepted Architecture Decision Records.

## Implemented product baseline

The current application includes persistent decks and rich-text cards, managed
images, `.studydeck` import/export, single- and multi-deck practice, FSRS review
scheduling, an immutable review ledger, deterministic analytics, themes,
typography, and graphical settings.

## Phase 1 — repository stabilization and CI

Phase 1 establishes a small, repeatable development foundation:

- release versioning begins at 0.1.0 and archive revision labels are removed;
- the Tauri application is named and identified as ostudy;
- setup creates a standalone binary and `run-app.sh` launches without compiling;
- `scripts/check.sh` is the single local quality gate;
- GitHub Actions repeats that gate on Ubuntu;
- Vitest/Testing Library component tests and a native WebDriver smoke test are
  present;
- each released schema has an upgrade fixture;
- developer runs produce structured local logs and a privacy-conscious
  diagnostic archive;
- tag builds produce consistently named Linux artifacts and checksums;
- moved repositories invalidate stale Cargo targets automatically;
- a short pull-request checklist records acceptance criteria and validation.

## Near-term product sequence

1. Add autosaved editor drafts and undo-safe orphaned-image cleanup.
2. Add reverse-card generation behind note/card and migration tests.
3. Add suspend/bury controls and daily review/new-card limits.
4. Add cloze notes after the basic and reverse flows are stable.
5. Build the local PDF document library before local AI generation.

Each item should remain a small vertical change based on an exact Git commit.
