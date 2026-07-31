# ostudy

**ostudy** is a free, local-first desktop flashcard application. It runs on the
user's computer without an account or hosted service, and stores decks, cards,
managed images, review history, settings, and analytics locally.

The application is built with Tauri, React, TypeScript, Rust, and embedded
SQLite. Release versioning begins at **0.1.0**.

## What ostudy can do

- Create, rename, and delete decks.
- Create **Basic**, **Reverse**, **Fill in the blank**, and field-template notes.
- Generate several study cards from one note without duplicating its source
  content.
- Organize notes with tags and find them with full-text search, filters, sorting,
  and pagination.
- Warn before saving a duplicate note.
- Move or copy notes and all of their generated review cards between decks.
- Use the built-in Pharmacology template or create reusable templates with
  simple `{{field name}}` placeholders.
- Import and export complete `.studydeck` SQLite packages, including tags,
  templates, generated cards, and managed images.
- Run single-deck or multi-deck FSRS-scheduled practice sessions.
- Record review history and display deterministic analytics.
- Configure themes, typography, scale, motion, and review preferences.

Math-specific cards, bulk entry, autosaved drafts, expanded study controls,
and the direct 0–3 confidence flow are planned in later Phase 2 stages. See
[`docs/PHASE-2-ROADMAP.md`](docs/PHASE-2-ROADMAP.md).

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

## Start the application

Launch the already-built application:

```bash
./run-app.sh
```

`run-app.sh` starts the existing binary without running Vite, Cargo, or the test
suite. After changing source code, rebuild explicitly:

```bash
./scripts/build.sh
```

Build commands protect the committed dependency lockfiles and provide recovery
instructions rather than leaving a dirty working tree. Dependency and build
troubleshooting details are kept in
[`docs/DEVELOPMENT-AND-QUALITY.md`](docs/DEVELOPMENT-AND-QUALITY.md).

Both `build.sh` and `check.sh` keep Cargo's incremental cache. If the repository
is moved or renamed, they detect the old absolute target path and clear
`src-tauri/target` once rather than repeatedly failing with stale build-script
artifacts.

## Development and testing

The normal development loop is:

1. Make a focused change on a branch.
2. Run the complete automated gate:

   ```bash
   ./scripts/check.sh
   ```

3. Start the current build with `./run-app.sh` and manually exercise the changed
   UI path.
4. Commit, push, and merge the pull request after both checks pass.

The automated gate installs from lockfiles, verifies exact dependencies, runs
React tests, checks Rust formatting and Clippy, runs Rust and migration-fixture
tests, creates the standalone Tauri build, and launches a native WebDriver smoke
test. Set `OSTUDY_SKIP_NATIVE_SMOKE=1` only while diagnosing a machine-specific
WebDriver problem, and record that the native test was skipped.

GitHub Actions repeats the same gate on a clean Ubuntu runner for pull requests
to `main` and pushes to `main`.

## Deck sharing and safety

A `.studydeck` file is a versioned SQLite package for one deck. It contains deck
content and managed images, but excludes application settings and personal
review history. Imported cards receive fresh local schedules.

Shared decks and card text are treated as untrusted data. ostudy stores rich
content as validated JSON rather than executable HTML, uses literal template and
Fill in the blank grammars, binds SQL parameters, validates package schemas and
relationships, and opens imported SQLite packages read-only with SQLite trusted
schema features disabled.

## Local data and diagnostics

The Settings screen displays the active database and managed-image locations.
To create a support archive without including the library database, cards, or
managed images, run:

```bash
./scripts/export-diagnostics.sh
```

Application output is also written to a small rotating JSON Lines log at:

```text
${XDG_STATE_HOME:-$HOME/.local/state}/ostudy/ostudy.jsonl
```

## Releases

A version tag such as `v0.1.0` triggers the Linux release workflow. It runs the
clean-source gate and produces a Debian package, AppImage, source archive, build
metadata, and SHA-256 checksums.

## Project documentation

- [`docs/PHASE-2-ROADMAP.md`](docs/PHASE-2-ROADMAP.md)
- [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md)
- [`docs/CARD-FORMAT-AND-DATA-MODEL.md`](docs/CARD-FORMAT-AND-DATA-MODEL.md)
- [`docs/DEVELOPMENT-AND-QUALITY.md`](docs/DEVELOPMENT-AND-QUALITY.md)
- [`docs/GIT-WORKFLOW.md`](docs/GIT-WORKFLOW.md)
- [`docs/decisions/`](docs/decisions/)
- [`AGENTS.md`](AGENTS.md) and [`docs/agents/`](docs/agents/)

The project is MIT licensed. Direct dependencies are pinned and free/open
source. The Linux application uses the operating system's WebKitGTK/GTK runtime
required by Tauri.
