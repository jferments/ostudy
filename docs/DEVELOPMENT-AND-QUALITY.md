# ostudy development and quality process

The process is intentionally direct:

1. make one coherent change on a branch;
2. run `./scripts/check.sh`;
3. launch `./run-app.sh` and manually test the affected workflow;
4. commit, push, and merge only after automated and human checks are satisfactory.

## Local commands

```bash
./scripts/setup-ubuntu.sh        # one-time tools, dependencies, and initial build
./scripts/build.sh               # rebuild after source changes
./run-app.sh                     # launch the existing build without compiling
./scripts/check.sh               # complete automated development gate
./scripts/export-diagnostics.sh  # privacy-conscious support archive
./scripts/release-check.sh       # repeat the gate from a clean source copy
```

Cargo's incremental cache is preserved. The build scripts set
`CARGO_BUILD_JOBS` to the machine's logical CPU count unless the caller supplies
a value. A target marker records the absolute repository location so moving the
checkout clears stale absolute build-script paths once.

## Immutable Cargo.lock behavior

Normal setup, development, check, and release builds must not modify
`src-tauri/Cargo.lock`. `scripts/with-immutable-cargo-lock.mjs`:

1. rejects an unstaged pre-existing lockfile change with recovery instructions;
2. runs `cargo metadata --locked` to verify manifest/lock agreement;
3. snapshots the current reviewed lockfile bytes;
4. runs the requested Tauri command;
5. restores and fails if the command modifies or deletes the lockfile.

The Node test suite for this wrapper covers unchanged, pre-dirty, modified, and
deleted lockfile cases. Dependency updates remain intentional separate changes.

## One-command gate

`./scripts/check.sh` performs, in order:

1. pinned environment and repository-manifest diagnostics;
2. Cargo-lock wrapper tests;
3. frozen pnpm installation and exact dependency verification;
4. Vitest/Testing Library component tests;
5. Rust formatting and warning-free Clippy;
6. Rust unit and integration tests, including released-schema upgrades;
7. a standalone Tauri build suitable for `run-app.sh`;
8. resolved Rust dependency verification;
9. a native Linux Tauri/WebDriver smoke test under isolated application data.

The native smoke may be skipped temporarily with
`OSTUDY_SKIP_NATIVE_SMOKE=1`, but a pull request must say that it was skipped and
why. CI runs it normally.

## Phase 2 test layers

- **React component tests:** compact Practice setup and recent confidence,
  Settings persistence, modal focus trap/Escape/restoration, notifications, and
  rich-text image canonicalization.
- **Pure Rust tests:** safe template grammar, Fill in the blank parsing, quoted
  FTS input, duplicate key stability, 0–3 score mapping, one-card-per-note queue,
  and local-day validation.
- **SQLite integration tests:** note generation, browsing, drafts, state actions,
  review idempotency, exact Undo, recent deck analytics, package schema
  validation, conflicting template IDs, and transactions.
- **Released-schema fixture:** load the immutable 0.1.0 schema, preserve
  representative data, apply migration 4, and run SQLite integrity checks.
- **Native smoke:** launch the built Tauri binary through WebDriver and navigate
  through the shell.
- **Manual Phase 2 test:** follow
  `docs/PHASE-2-LOCAL-TEST-PLAN.md`, including restart persistence and all four
  themes.

## Security review checklist

Changes to cards, templates, search, or import/export must verify:

- no arbitrary HTML or script execution path;
- explicit rich-content nodes, marks, attributes, URL schemes, image sizes, and
  alignments;
- parameterized SQL and quoted FTS tokens;
- read-only imported databases with trusted schema disabled;
- exact package schema and bounded rows/bytes;
- relationship, asset, and template validation before insertion;
- no imported schedule, review, settings, or draft state;
- transactional rollback on any validation or write failure.

Durable changes to these boundaries require an ADR.

## GitHub Actions

`.github/workflows/check.yml` installs the pinned environment on Ubuntu 24.04
and runs the same `scripts/check.sh`. GitHub Actions is an independent clean
environment, not a substitute for testing the native application on the user's
workstation.

## Diagnostics and releases

Rust emits small structured startup and command-failure records. The diagnostic
archive includes environment, toolchain, Git state, and recent logs but excludes
the library database, card content, and managed images.

A release is created only from a clean tagged commit whose tag matches all three
application manifests. The release script repeats the clean-source gate and
produces consistently named Debian, AppImage, source, metadata, and checksum
artifacts.

## Database changes

Schema changes are append-only numbered SQL migrations with SHA-256 checksums in
`database.rs`. Once a release is tagged, its migrations and fixture are
immutable. A later schema change adds a migration and an appropriate historical
fixture; it never edits a released migration.

## Honest validation language

Reports distinguish:

- **Passed:** the named command completed successfully in the named environment;
- **Inspected:** source or output was reviewed without executing the native path;
- **Not run:** the check was unavailable or intentionally deferred;
- **Manually verified:** the named behavior was actually exercised.

Parsing, static inspection, or a frontend build must never be described as a
successful native desktop test.
