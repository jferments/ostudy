# ostudy

**ostudy** is a free, local-first desktop flashcard application built with
Tauri, React, TypeScript, Rust, and embedded SQLite. It requires no account or
server. Decks, rich-text cards, review history, settings, and analytics remain
on the user's computer.

The first release series starts at **0.1.0**. Old archive revision labels are not
part of the versioning scheme; Git commits describe development history and
release tags describe published versions.

## Current functionality

- Create, rename, and delete decks.
- Create, edit, search, and delete rich-text flashcards.
- Import and export complete `.studydeck` packages with managed images.
- Run single-deck or multi-deck FSRS-scheduled practice sessions.
- Record review history and display deterministic analytics.
- Configure themes, typography, scale, motion, and review preferences.

## Ubuntu setup

From the repository root, run the setup once:

```bash
./scripts/setup-ubuntu.sh
```

The setup script installs the pinned Ubuntu, Node, pnpm, Rust, and WebDriver
requirements and creates a standalone debug build. Cargo uses all detected CPU
cores by default; override the number when needed:

```bash
CARGO_BUILD_JOBS=24 ./scripts/setup-ubuntu.sh
```

## Fast startup and rebuilding

Start the already-compiled application:

```bash
./run-app.sh
```

`run-app.sh` does not run Vite, Cargo, or the test suite. It verifies that the
existing binary matches the current source and then starts it. After editing
source code, rebuild explicitly:

```bash
./scripts/build.sh
```

Both `build.sh` and `check.sh` keep Cargo's incremental cache. If the repository
is moved or renamed, they detect the old absolute target path and clear
`src-tauri/target` once rather than repeatedly failing with stale build-script
artifacts.

## The normal development loop

1. Make a focused change on a branch.
2. Run the complete automated gate:

   ```bash
   ./scripts/check.sh
   ```

3. Start the current build with `./run-app.sh` and manually exercise the changed
   UI path.
4. Commit, push, and merge the pull request when both checks pass.

The single gate installs from lockfiles, verifies exact dependencies, runs React
component tests, checks Rust formatting and Clippy, runs Rust and migration
fixture tests, creates the standalone Tauri build, and launches a native
WebDriver smoke test. Set `OSTUDY_SKIP_NATIVE_SMOKE=1` only when diagnosing a
machine-specific WebDriver problem, and record that the native test was skipped.

## GitHub Actions

`.github/workflows/check.yml` repeats `./scripts/check.sh` on a clean Ubuntu
runner for every pull request to `main` and every push to `main`. This catches
missing local dependencies and uncommitted generated files before a change is
merged.

## Local diagnostics

`run-app.sh` preserves terminal output in a small JSON Lines log at:

```text
${XDG_STATE_HOME:-$HOME/.local/state}/ostudy/ostudy.jsonl
```

The log rotates at 2 MiB and retains one previous file. Create a support archive
without including the SQLite library, cards, or managed images:

```bash
./scripts/export-diagnostics.sh
```

## Release artifacts

A version tag such as `v0.1.0` triggers `.github/workflows/release.yml`. The
workflow runs the clean-source gate and uses `scripts/release.sh` to generate the
same Linux outputs every time:

- a Debian package;
- an AppImage;
- a source archive;
- build metadata;
- SHA-256 checksums.

Artifacts are uploaded to the workflow run. Phase 1 intentionally does not add
code signing, auto-update publishing, or a multi-platform release matrix.

## Application identifier and data location

The Tauri identifier is `io.github.jferments.ostudy`. Changing from the old
example identifier changes the operating-system application-data directory.
This is an intentional pre-0.1 compatibility break; old development data is not
migrated automatically.

The Settings screen displays the active database and managed-image locations.
`.studydeck` export is for sharing deck content and intentionally excludes
personal review history and application settings.

## Architecture and contributor documentation

- [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md)
- [`docs/CARD-FORMAT-AND-DATA-MODEL.md`](docs/CARD-FORMAT-AND-DATA-MODEL.md)
- [`docs/DEVELOPMENT-AND-QUALITY.md`](docs/DEVELOPMENT-AND-QUALITY.md)
- [`docs/GIT-WORKFLOW.md`](docs/GIT-WORKFLOW.md)
- [`AGENTS.md`](AGENTS.md)
- [`docs/agents/`](docs/agents/)

The project is MIT licensed. Direct dependencies are pinned and free/open
source. The Linux application uses the operating system's WebKitGTK/GTK runtime
required by Tauri.
