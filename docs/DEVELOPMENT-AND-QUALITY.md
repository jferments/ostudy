# ostudy development and quality process

The process is intentionally small:

1. make a focused change on a branch;
2. run `./scripts/check.sh`;
3. launch `./run-app.sh` and manually test the changed UI path;
4. commit, push, and merge after both are satisfactory.

## Local commands

```bash
./scripts/setup-ubuntu.sh     # one-time tools, dependencies, and initial build
./scripts/build.sh            # rebuild after source changes
./run-app.sh                  # start the existing build without compiling
./scripts/check.sh            # the complete automated development gate
./scripts/export-diagnostics.sh  # create a privacy-conscious support archive
./scripts/release-check.sh    # repeat the complete gate from a clean source copy
```

Cargo's incremental cache is preserved across builds. The build scripts set
`CARGO_BUILD_JOBS` to the machine's logical CPU count unless the caller supplies
a value. A target marker records the absolute repository location; moving the
checkout clears the old target once.

## What the one-command gate checks

`./scripts/check.sh` performs, in order:

1. pinned environment and repository-manifest diagnostics;
2. frozen pnpm installation and exact dependency verification;
3. Vitest/Testing Library React tests;
4. Rust formatting and warning-free Clippy;
5. Rust unit/integration tests, including every release-schema fixture;
6. a standalone Tauri build suitable for `run-app.sh`;
7. resolved Rust dependency verification;
8. a native Linux Tauri/WebDriver smoke test under an isolated data directory.

The native smoke test may be skipped temporarily with
`OSTUDY_SKIP_NATIVE_SMOKE=1`, but a pull request must state that it was skipped
and why. CI runs it normally.

## Testing layers

- **React component tests:** important interaction and error-rendering behavior
  with mocked typed IPC.
- **Pure Rust tests:** validation, scheduling, settings, and deterministic rules.
- **SQLite integration tests:** transactions, CRUD, import/export, idempotency,
  and review-ledger invariants.
- **Release-schema fixtures:** load the immutable schema snapshot shipped by
  each release, preserve representative data, apply current migrations, and run
  SQLite's integrity check.
- **Native smoke test:** launch the built Linux Tauri binary through WebDriver and
  navigate from the main shell to Settings.
- **Manual test:** exercise the changed user workflow and assess visual behavior.

## GitHub Actions

A GitHub Actions workflow is a YAML recipe executed by GitHub on a clean hosted
machine when a configured event occurs. The check workflow installs the pinned
requirements on Ubuntu and runs the same `scripts/check.sh` used locally. It is
a second environment, not a second development process.

## Diagnostics

Rust emits small JSON records for startup and command failures. `run-app.sh`
streams output to the terminal and appends it to a rotating JSON Lines log under
the user's XDG state directory. `export-diagnostics.sh` includes environment,
toolchain, Git state, and recent log records, but never includes the library
database, cards, or managed images.

## Release artifacts

A release is created only from a clean tagged commit whose tag matches the three
application manifests. `scripts/release.sh` runs the clean-source gate and
produces consistently named Debian, AppImage, and source artifacts plus build
metadata and SHA-256 checksums. The tag-triggered release workflow uploads these
files to its GitHub Actions run.

Phase 1 does not introduce signing, automatic GitHub Release publication,
updaters, or a multi-platform matrix. Those can be added when actual release
needs justify them.

## Database changes

Schema changes are append-only numbered SQL migrations with checksums embedded
in `database.rs`. Once a release is tagged, its migrations and fixture are
immutable. A new release that changes the schema adds a migration and a fixture
for the prior released schema; it does not edit old migrations.

## Honest validation

Reports must distinguish commands actually executed, CI results, manual UI paths
actually exercised, skipped checks, and remaining limitations. Static inspection
is not a native desktop test.
