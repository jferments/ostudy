# ostudy architecture and implementation plan

Canonical documentation is split by purpose:

- `README.md`: product scope and everyday commands;
- `docs/ARCHITECTURE.md`: implemented boundaries and data flow;
- `docs/CARD-FORMAT-AND-DATA-MODEL.md`: durable card, review, and package
  formats;
- `docs/DEVELOPMENT-AND-QUALITY.md`: tests, CI, diagnostics, and releases;
- `docs/PHASE-2-ACCESSIBILITY-AUDIT.md`: Phase 2 keyboard and accessibility
  review;
- `docs/GIT-WORKFLOW.md`: branch, pull-request, and rollback workflow;
- `AGENTS.md` and `docs/agents/`: instructions for automated maintainers;
- `docs/decisions/`: accepted Architecture Decision Records.

## Implemented baseline

ostudy is a local-first Tauri application with a React/TypeScript frontend, a
Rust command layer, embedded SQLite, managed local images, FSRS scheduling, an
append-only review ledger, deterministic analytics, checksummed migrations, and
a reproducible Ubuntu build and test gate.

## Phase 1 — repository stabilization and CI

Phase 1 established the development foundation:

- release versioning begins at 0.1.0;
- setup creates a standalone binary and `run-app.sh` launches without compiling;
- `scripts/check.sh` is the single local and CI quality gate;
- frontend, Rust, migration-fixture, and native WebDriver tests are present;
- local diagnostics and repeatable Linux release artifacts are available;
- moved checkouts invalidate stale Cargo targets automatically;
- committed lockfiles and exact dependency pins define the build.

## Phase 2 — complete standalone flashcard core

Phase 2 is implemented as one vertical product layer rather than a collection of
frontend-only toggles.

### Card creation and organization

- Basic and reverse cards share one editable note.
- Fill in the blank notes use literal `[[answer]]` markers and generate one card
  per marker.
- Safe field templates use exact `{{field name}}` placeholders and can generate
  several cards from one structured note.
- A built-in Pharmacology template demonstrates medication name, drug class,
  conditions, and side-effect fields.
- Tags, FTS5 search, sorting, status/type filtering, bulk entry, duplicate
  checks, and autosaved drafts support larger libraries.
- A note and all cards generated from it can be moved or copied between decks.

### Practice and scheduling

- The learner gives one score after revealing an answer: 0 Wrong, 1 Hard, 2 OK,
  or 3 Easy.
- Interval previews show the resulting next-review interval before a score is
  submitted.
- Daily new-card and review limits are enforced from unreversed review events.
- Related cards can be buried until the next local day.
- Repeated Wrong scores mark a note as **Struggling** and may automatically
  suspend it.
- Undo appends a reversal marker and restores exact schedule/card-state
  snapshots without deleting review history.
- Practice setup uses a compact **Shuffle decks** checkbox and shows recent
  per-deck Wrong/Hard/OK/Easy results.

### Content and sharing safety

- Rich card content remains validated Tiptap JSON, never arbitrary HTML.
- Templates are literal substitution only and cannot execute code.
- FTS input is tokenized and quoted before `MATCH`.
- `.studydeck` version 2 remains a single SQLite file and is opened read-only,
  integrity checked, exact-schema checked, bounded, and revalidated.
- Personal schedules and review history are never imported with shared decks.

### Explicit Phase 2 exclusions

- CSV/TSV import is not added. Bulk entry covers fast local entry, while
  `.studydeck` remains the supported portable deck format.
- Math rendering and math-oriented templates are deferred to a focused math
  card phase.
- Image occlusion remains reserved in the durable schema but is not exposed as
  a Phase 2 authoring mode.

## Next product sequence

1. Qualify and tag the first release that includes the Phase 2 schema.
2. Implement the dedicated math-card phase, including safe math syntax,
   rendering, editing, and export compatibility.
3. Build the local document/PDF library and citation model.
4. Add local AI-assisted card generation behind the same validation and
   provenance boundaries used by manual authoring.
5. Evaluate optional sync only after package, migration, conflict, and personal
   learning-state boundaries are mature.

Future work should continue as reviewable vertical pull requests based on exact
commits. Released migrations and fixtures remain append-only.
