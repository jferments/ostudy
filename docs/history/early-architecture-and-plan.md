# ostudy architecture and implementation plan

Canonical documentation is split by purpose:

- `README.md`: product scope and everyday commands;
- `docs/ARCHITECTURE.md`: implemented boundaries and data flow;
- `docs/CARD-FORMAT-AND-DATA-MODEL.md`: durable card and SQLite formats;
- `docs/DEVELOPMENT-AND-QUALITY.md`: tests, CI, diagnostics, and releases;
- `docs/GIT-WORKFLOW.md`: branch, pull-request, and rollback workflow;
- `docs/PHASE-2-ROADMAP.md`: the current staged product sequence;
- `AGENTS.md` and `docs/agents/`: instructions for automated maintainers;
- `docs/decisions/`: accepted Architecture Decision Records.

## Implemented product baseline

The application includes persistent decks and validated rich-text cards,
managed images, `.studydeck` import/export, FSRS scheduling, an immutable review
ledger, deterministic analytics, themes, typography, and graphical settings.

Phase 2 Stage 1 added the one-step 0–3 confidence flow, compact practice setup,
recent results, voluntary deck practice, and locked builds.

Phase 2 Stage 2 adds:

- note-level tags;
- SQLite FTS5 question, answer, and tag search;
- a whole-library card browser with filters, sorting, paging, multi-selection,
  and keyboard navigation;
- transactional bulk entry;
- exact duplicate warnings and skip behavior;
- schedule-preserving moves and fresh-schedule copies;
- optional tag preservation in `.studydeck` packages.

## Development foundation

- Release versioning begins at 0.1.0.
- Setup creates a standalone binary; `run-app.sh` launches without compiling.
- `scripts/check.sh` is the single local and CI quality gate.
- Vitest/Testing Library, Rust unit tests, migration fixtures, and a native
  WebDriver smoke test cover the supported boundaries.
- Every schema migration is checksummed and historical release fixtures upgrade
  forward.
- Developer runs produce structured local logs and privacy-conscious
  diagnostics.
- Build commands keep Cargo.lock immutable.

## Near-term product sequence

1. Add reverse cards and the safe field/template substrate.
2. Add Fill in the blank cards on the same substrate.
3. Add autosaved editor drafts and image layout controls.
4. Add suspend, bury, reset, reschedule, undo, limits, interval previews, and
   **Needs more practice** detection.
5. Finish whole-application keyboard, accessibility, security, and responsive
   quality passes.

Each stage remains a reviewable vertical change based on an exact Git commit.
