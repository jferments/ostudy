# Phase 2 validation and acceptance record

Phase 2 completes the standalone flashcard core. This file records the required
coverage and the validation boundary; the pull request records the exact final
commit and GitHub Actions result. Human workstation acceptance remains required
before merge.

## Automated repository gate

```bash
./scripts/check.sh
```

Run the gate from the repository root without skipping individual stages; the
matching GitHub Actions run is the clean-environment acceptance record. A green
result is required before this pull request is marked ready for local testing.

The gate covers:

- project identity, command registration, migration checksums, security
  guardrails, and exact lockfiles;
- immutable-Cargo-lock wrapper regression tests;
- TypeScript build and React component tests;
- Rust formatting, Clippy with warnings denied, and all Rust tests;
- fresh and released-schema migration upgrades with SQLite integrity checks;
- safe template and Fill in the blank parsing;
- FTS query quoting and duplicate-key behavior;
- 0–3 score mapping, related-card queueing, review submission, and exact Undo;
- recent-practice analytics;
- strict version 2 package schema and import-security cases;
- standalone Tauri build and native WebDriver smoke.

## Phase 2 acceptance areas

1. Create and edit Basic, Reverse, Fill in the blank, and Template notes.
2. Confirm generated-card counts and independent schedules.
3. Add tags; browse using FTS text, tag, mode, state, and sort filters.
4. Recover and discard autosaved drafts.
5. Confirm duplicate warning, explicit override, and bulk duplicate skipping.
6. Move and copy notes between decks.
7. Suspend/resume, bury/unbury, reset, and reschedule.
8. Practice with compact **Shuffle decks**, daily limits, recent confidence, and
   0 Wrong / 1 Hard / 2 OK / 3 Easy.
9. Confirm interval previews, exact Undo, related-card burying, and Struggling
   behavior.
10. Confirm analytics use the four direct ratings and exclude undone reviews.
11. Export/import version 2 `.studydeck` files and import a legacy package.
12. Confirm managed image size/alignment and safe links.
13. Exercise mouse and keyboard paths, focus restoration, and all four themes.
14. Restart and confirm persistent data; confirm imported decks use fresh local
    schedules and no foreign review history.

The exact workstation procedure is in
[`docs/PHASE-2-LOCAL-TEST-PLAN.md`](docs/PHASE-2-LOCAL-TEST-PLAN.md).

## Compatibility and migration

- Database schema advances from 3 to 4 through the append-only
  `0004_standalone_flashcard_core.sql` migration.
- The released 0.1.0 schema fixture upgrades in place and preserves existing
  decks, notes, cards, schedules, and reviews.
- Existing Basic cards remain readable and editable.
- Legacy `.studydeck` version 1 import remains available; new exports use
  version 2.
- Historical database direction value `cloze` remains internal compatibility
  data while the UI says Fill in the blank.
- Review ledger columns remain compatible; the public direct score is represented
  by canonical FSRS rating minus one.

## Security boundary

Templates and card content are non-executable data. Shared packages are treated
as hostile SQLite files and are opened read-only with trusted schema disabled,
then checked for exact schema, metadata, bounds, relationships, rich content,
assets, and template correctness before a transactional import. No package can
supply executable JavaScript, SQL, HTML, arbitrary CSS, local file paths, remote
images, local schedules, personal reviews, settings, or drafts.

## Remaining human gate

A green GitHub Actions run proves the automated repository gate in a clean
Ubuntu environment. It does not prove visual behavior on the learner's actual
GPU, desktop, data directory, or existing library. The pull request must remain
unmerged until the user completes the local test plan and accepts the result.
