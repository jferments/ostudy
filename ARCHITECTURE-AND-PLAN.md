# ostudy architecture and implementation plan

Canonical documentation is split by purpose:

- `README.md`: product introduction, setup, and everyday commands;
- `docs/PHASE-2-ROADMAP.md`: the staged standalone-flashcard roadmap;
- `docs/ARCHITECTURE.md`: implemented boundaries and data flow;
- `docs/CARD-FORMAT-AND-DATA-MODEL.md`: durable card and SQLite formats;
- `docs/DEVELOPMENT-AND-QUALITY.md`: tests, CI, diagnostics, and releases;
- `docs/GIT-WORKFLOW.md`: branch, pull-request, and rollback workflow;
- `AGENTS.md` and `docs/agents/`: instructions for automated maintainers;
- `docs/decisions/`: accepted Architecture Decision Records.

## Implemented product baseline

The application is a local-first Tauri desktop program with persistent decks,
validated rich-text content, managed images, SQLite deck packages, FSRS review
scheduling, an immutable review ledger, deterministic analytics, and local
appearance/settings storage.

Phase 2 Stage 1 extends that baseline with a secure note-generation layer:

- Basic and Reverse notes;
- Fill in the blank notes using literal `[[answer]]` markers;
- field templates using literal `{{field name}}` placeholders;
- tags, duplicate warnings, SQLite FTS5 search, browser filters, sorting, and
  pagination;
- version 2 `.studydeck` packages that preserve templates, tags, generated-card
  identities, and managed images;
- an immutable-Cargo.lock build guard.

## Phase 1 — repository stabilization and CI

Phase 1 established the repeatable development foundation:

- release versioning begins at 0.1.0;
- setup creates a standalone binary and `run-app.sh` launches without compiling;
- `scripts/check.sh` is the single local and CI quality gate;
- Vitest/Testing Library, Rust, migration-fixture, and native WebDriver tests are
  part of that gate;
- developer runs produce structured local logs and privacy-conscious diagnostic
  archives;
- tag builds produce consistently named Linux artifacts and checksums;
- moved repositories invalidate stale Cargo targets automatically.

## Phase 2 — complete the standalone flashcard core

Phase 2 is deliberately delivered as four independently testable stages rather
than one oversized branch:

1. **Secure authoring foundation:** reverse, Fill in the blank, tags, templates,
   search/browser improvements, duplicate detection, move/copy between decks,
   safe package import/export, and the Cargo.lock fix.
2. **Faster authoring and organization:** bulk entry, autosaved drafts, and image
   sizing/alignment.
3. **Study controls and confidence scoring:** direct 0–3 ratings and analytics,
   suspend/bury/reset/reschedule, undo, limits, previews, Struggling cards, and a
   compact practice setup.
4. **Keyboard, accessibility, and regression polish.**

CSV/TSV import is intentionally omitted; deck interchange remains SQLite-based.
Math support is deferred to a dedicated later phase. The detailed acceptance
boundaries are in [`docs/PHASE-2-ROADMAP.md`](docs/PHASE-2-ROADMAP.md).

Each stage should start from the merged `main` branch, remain one coherent pull
request, pass `./scripts/check.sh`, and receive a local desktop acceptance test
before the next stage begins.
