# ostudy development and quality process

The process is intentionally small:

1. publish a focused branch and draft pull request from an exact base commit;
2. implement and push reviewable vertical commits to that branch;
3. run `./scripts/check.sh` locally and through GitHub Actions;
4. launch `./run-app.sh` and manually test the changed UI path;
5. merge only after automated and manual acceptance are satisfactory.

The corrected publication-first workflow is documented in
[`OSTUDY-GIT-AND-CODING-WORKFLOW.md`](OSTUDY-GIT-AND-CODING-WORKFLOW.md).

## Local commands

```bash
./scripts/setup-ubuntu.sh        # one-time tools, dependencies, and initial build
./scripts/build.sh               # rebuild after source changes
./run-app.sh                     # start the existing build without compiling
./scripts/check.sh               # the complete automated development gate
./scripts/export-diagnostics.sh  # create a privacy-conscious support archive
./scripts/release-check.sh       # repeat the complete gate from a clean source copy
```

Cargo's incremental cache is preserved across builds. The build scripts set
`CARGO_BUILD_JOBS` to the machine's logical CPU count unless the caller supplies
a value. A target marker records the absolute repository location; moving the
checkout clears the old target once.

## What the one-command gate checks

`./scripts/check.sh` performs, in order:

1. pinned environment and repository-manifest diagnostics;
2. Stage 2 migration, FTS5, tag, bulk, duplicate, and move/copy wiring checks;
3. Stage 3 migration, generated-card identity, cloze, template-safety,
   Search/Filter, Practice, responsive UI, and `.studydeck` wiring checks;
4. frozen pnpm installation and exact dependency verification;
5. Vitest/Testing Library React tests;
6. Rust formatting and warning-free Clippy;
7. Rust unit/integration tests, including every release-schema fixture;
8. a standalone Tauri build suitable for `run-app.sh`;
9. resolved Rust dependency verification;
10. a native Linux Tauri/WebDriver smoke test under an isolated data directory.

The native smoke test may be skipped temporarily with
`OSTUDY_SKIP_NATIVE_SMOKE=1`, but a pull request must state that it was skipped
and why. CI runs it normally.

## Testing layers

- **Repository verifiers:** inspect durable migration checksums, command
  registration, typed IPC, stale API removal, template safety constraints,
  generated-card rendering, and required UI/documentation wiring.
- **React component and helper tests:** important interaction and error-rendering
  behavior with mocked typed IPC, including independent reverse-card selection,
  cloze text marking and modes, template management, and generated-card saves.
- **Pure Rust tests:** rich-content validation, scheduling, tag normalization,
  duplicate matching, search-query construction, cloze parsing/rendering,
  template placeholder validation, and literal substitution.
- **SQLite integration tests:** migrations, FTS triggers, transactions, CRUD,
  bulk entry, generated-card lifecycle, schedule/history preservation,
  template deletion independence, move/copy, import/export, idempotency, and
  review-ledger invariants.
- **Release-schema fixtures:** load the immutable schema snapshot shipped by
  each release, preserve representative data, apply current migrations, and run
  SQLite's integrity check.
- **Native smoke test:** launch the built Linux Tauri binary through WebDriver
  and navigate through the real application shell.
- **Manual test:** exercise the changed user workflow and assess visual,
  keyboard, persistence, and theme behavior.

## Stage 3 manual acceptance

After `./scripts/check.sh` passes, manually verify at least:

- a normal Basic card with the reverse option disabled;
- a reversible card in both practice directions;
- disabling and re-enabling the reverse presentation without losing its prior
  due state;
- a two-blank cloze note in one-at-a-time mode;
- the same note in all-blanks-together mode and then switched back;
- creation, editing, preview, use, and deletion of a reusable template;
- generated cards remaining after template deletion;
- exact generated-card selection in Search/Filter;
- note-level tags, move/copy, and deletion when several presentations are
  selected;
- deck export/import for Basic, reverse, and both cloze modes;
- application restart after each persistent authoring workflow;
- Light, Dark, Console, and Synthwave themes at narrow and wide layouts.

## GitHub Actions

The check workflow installs the pinned requirements on Ubuntu and runs the same
`scripts/check.sh` used locally. It is a second environment, not a second
development process. A branch is not ready for merge merely because files were
written; the published head must pass the current workflow run.

## Diagnostics

Rust emits small JSON records for startup and command failures. `run-app.sh`
streams output to the terminal and appends it to a rotating JSON Lines log under
the user's XDG state directory. `export-diagnostics.sh` includes environment,
toolchain, Git state, and recent log records, but never includes the library
database, cards, search index, tags, templates, or managed images.

## Release artifacts

A release is created only from a clean tagged commit whose tag matches the three
application manifests. `scripts/release.sh` runs the clean-source gate and
produces consistently named Debian, AppImage, and source artifacts plus build
metadata and SHA-256 checksums.

## Database changes

Schema changes are append-only numbered SQL migrations with checksums embedded
in `database.rs`. Once a release is tagged, its migrations and fixture are
immutable. A new release that changes the schema adds a migration and a fixture
for the prior released schema; it does not edit old migrations.

Stage 3 migration `0006_card_types_and_templates.sql` adds generated-card source
metadata and reusable authoring-template tables without modifying prior
migrations. Existing Basic notes receive safe defaults and retain their existing
card IDs, schedules, and review histories.

## Honest validation

Reports must distinguish commands actually executed, CI results, manual UI
paths actually exercised, skipped checks, and remaining limitations. Static
inspection is not a native desktop test, and an older successful workflow run
does not validate a newer branch head.
