# ostudy development and quality process

The process is intentionally small:

1. publish one focused branch and draft pull request from an exact base commit;
2. implement and push reviewable vertical commits to that branch;
3. iterate with changed/static/fast checks and consume exact-head CI failures;
4. run `./scripts/check.sh`, launch `./run-app.sh`, and manually test the affected workflow;
5. mark the PR ready so the self-hosted runner qualifies the exact head with the full gate;
6. merge only after automated and human acceptance are satisfactory.

The corrected publication-first workflow is documented in
[`OSTUDY-GIT-AND-CODING-WORKFLOW.md`](OSTUDY-GIT-AND-CODING-WORKFLOW.md).

## Local commands

```bash
./scripts/setup-ubuntu.sh        # one-time tools, dependencies, and initial build
./scripts/build.sh               # rebuild after source changes
./run-app.sh                     # start the existing build without compiling
./scripts/check-changed.sh       # diff-aware iteration aid
./scripts/check-static.sh        # cheap repository/IPC/type contracts
./scripts/check-fast.sh          # frontend tests + Rust compile
./scripts/check.sh               # complete merge qualification
./scripts/export-diagnostics.sh  # create a privacy-conscious support archive
./scripts/release-check.sh       # repeat the complete gate from a clean source copy
```

Cargo's incremental cache is preserved across builds. The build scripts set
`CARGO_BUILD_JOBS` to the machine's logical CPU count unless the caller supplies
a value. A target marker records the absolute repository location; moving the
checkout clears the old target once.

## Qualification tiers

`check-static.sh` verifies durable environment/repository contracts: exact tool and
dependency versions, migration checksums, IPC registry/wrapper parity, repository
policy, test policy, script syntax, TypeScript, and canonical Rust formatting.
It also enforces the Stage 3.12 frontend transport boundary: direct Tauri
`invoke()` calls must live under `src/ipc/` and every registered command must
have exactly one frontend wrapper.

`check-fast.sh` reuses an exact-tree static result when possible and adds the full
frontend suite plus locked Rust compile checking. Draft pull requests use this
tier. Controller race tests deliberately resolve older/newer reads in reverse
order to verify that obsolete Search, Analytics, Template, Practice, and Deck
results cannot overwrite current state.

`check.sh` reuses an exact-tree fast result when possible and adds native
environment qualification, production frontend build, warning-free Clippy, all
Rust/unit/integration/migration tests, standalone Tauri build, exact dependency
verification, and the native create → review → restart → persistence smoke test.
The complete Rust suite also runs the generated-import conformance corpus.
Ready pull requests use this tier. See `docs/development/testing-and-ci.md` for
the authoritative mechanics.

The native smoke test may be skipped temporarily with
`OSTUDY_SKIP_NATIVE_SMOKE=1`, but a pull request must state that it was skipped
and why. CI runs it normally.

## Testing layers

- **Repository contracts:** verify durable migration checksums, exact command
  registry/frontend-wrapper parity, typed IPC-gateway ownership, stale API
  removal, self-hosted immutable workflow policy, dependency manifests, and
  test-policy invariants. Feature behavior is protected by executable tests
  rather than private source shapes.
- **Controller/read-race tests:** cover independent request lanes and resolve
  competing requests in reverse order. Obsolete successes and errors must not
  mutate newer screen state. Mutation tests preserve committed success even when
  a later reconciliation step fails.
- **React component and helper tests:** important interaction and error-rendering
  behavior with mocked typed IPC, including independent reverse-card selection,
  cloze text marking and modes, template management, generated-card saves,
  extracted Search/Filter controls, and extracted Analytics scope controls.
- **Pure Rust tests:** rich-content validation, scheduling, tag normalization,
  duplicate matching, search-query construction, cloze parsing/rendering,
  template placeholder validation, literal substitution, CardNote transport,
  and restricted generated-import Markdown parsing.
- **Generated-import conformance tests:** materialize real temporary
  `.ostudy.json` files from the checked-in corpus; exercise supported Basic/
  Reverse/Cloze/Markdown/Unicode forms, broad schema/content rejection cases,
  declared boundaries, duplicate handling, exact-name create/append semantics,
  fresh schedules, and whole-file atomicity. The test database is disposable,
  the imported test graph is explicitly deleted, and RAII removes the temporary
  filesystem root even when a test fails.
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
  keyboard, persistence, stale-read behavior, and theme behavior.

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
- rapid Search/Filter, deck-card query, template-selection, Practice-scope, and
  Analytics-scope changes without old responses replacing the current view;
- successful mutations followed by refresh/reconciliation still reporting the
  durable write as successful;
- generated JSON import for representative Basic, reverse, Cloze, links, lists,
  code, Unicode, duplicate append, and invalid atomic-rejection cases;
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
