# ostudy architecture and implementation plan

Current documentation is split by purpose:

- [`README.md`](README.md): product scope, launch instructions, and the quality
  gate;
- [`AGENTS.md`](AGENTS.md): repository-wide instructions for agents and LLMs;
- [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md): implemented component
  boundaries and data flow;
- [`docs/CARD-FORMAT-AND-DATA-MODEL.md`](docs/CARD-FORMAT-AND-DATA-MODEL.md):
  card content, SQLite tables, FSRS state, review ledger, and deck packages;
- [`docs/DEVELOPMENT-AND-QUALITY.md`](docs/DEVELOPMENT-AND-QUALITY.md): testing,
  CI, release checks, comments, and documentation policy;
- [`docs/GIT-WORKFLOW.md`](docs/GIT-WORKFLOW.md): the branch/PR workflow for
  future development;
- [`docs/agents/`](docs/agents/): implementation, code-style, and validation
  guidance for automated maintainers;
- [`docs/decisions/`](docs/decisions/): Architecture Decision Records.

## Implemented baseline

Revision 5.4 includes persistent decks and rich-text cards, managed images,
`.studydeck` import/export, direct single-deck and manual multi-deck scheduled
practice, FSRS review scheduling, an immutable review ledger, deterministic
analytics, themes, typography, and graphical settings.

## Near-term sequence after the Git baseline

1. Add React component tests for deck, editor, Practice, Analytics, and Settings
   interactions.
2. Add a Tauri WebDriver end-to-end test covering create → review → restart →
   persistence → export/import.
3. Add autosaved editor drafts and undo-safe orphaned-image cleanup.
4. Add reverse-card generation behind explicit note/card migration tests.
5. Add suspend/bury controls and daily review/new-card limits.
6. Add cloze notes after basic/reverse generation is protected by domain and
   end-to-end tests.
7. Build the local PDF document library before adding local AI generation.

Each item should be a small vertical change based on an exact Git commit. Whole
application replacement archives should no longer be the normal development
mechanism after this baseline is accepted.
