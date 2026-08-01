# ostudy

**ostudy** is a free, local-first desktop flashcard application. It runs without
an account or server and keeps decks, cards, images, review history, settings,
and analytics on the user's computer.

## What it can do

- Create, rename, and delete decks.
- Create, edit, search, and delete rich-text flashcards.
- Add managed local images to cards.
- Import and export complete `.studydeck` deck files.
- Practice one or more decks with spaced-repetition scheduling.
- Rate each revealed answer once on a four-level confidence scale:
  **0 Wrong**, **1 Hard**, **2 Medium**, or **3 Easy**.
- Review confidence, response-time, due-card, and per-deck analytics.
- Configure themes, typography, scale, motion, and review preferences.

## Ubuntu setup

From the repository root, run the setup once:

```bash
./scripts/setup-ubuntu.sh
```

The setup script installs the required Ubuntu packages and pinned development
tools, installs dependencies from the committed lockfiles, and creates the first
standalone debug build. Cargo uses all detected CPU cores by default. To limit
parallel compilation, set `CARGO_BUILD_JOBS`, for example:

```bash
CARGO_BUILD_JOBS=16 ./scripts/setup-ubuntu.sh
```

## Start the application

Launch the already-built application:

```bash
./run-app.sh
```

This is intentionally fast: it does not start a development server or rebuild
the project. After changing source code, rebuild first:

```bash
./scripts/build.sh
./run-app.sh
```

Build and development commands treat `src-tauri/Cargo.lock` as immutable. If the
lockfile no longer matches the Rust manifests, the command fails and explains
the problem instead of silently rewriting the file.

## Test a change

Run the complete automated gate from the repository root:

```bash
./scripts/check.sh
```

The gate installs from the committed lockfiles, runs React tests, checks Rust
formatting and Clippy, runs Rust and database tests, creates a standalone Tauri
build, and performs a native desktop smoke test. After it passes, start the app
with `./run-app.sh` and manually exercise the changed workflow.

## Local data and deck sharing

The Settings screen displays the active database and managed-image locations.
Deck sharing uses `.studydeck` files, which are versioned SQLite packages that
contain deck content and managed images. They intentionally exclude personal
review history and application settings.

Cards are stored as validated structured documents rather than executable HTML.
Shared decks cannot supply JavaScript, SQL, shell commands, arbitrary local file
paths, or remote images for the application to execute or load.

## Documentation

- [`docs/PHASE-2-ROADMAP.md`](docs/PHASE-2-ROADMAP.md)
- [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md)
- [`docs/CARD-FORMAT-AND-DATA-MODEL.md`](docs/CARD-FORMAT-AND-DATA-MODEL.md)
- [`docs/DEVELOPMENT-AND-QUALITY.md`](docs/DEVELOPMENT-AND-QUALITY.md)
- [`docs/GIT-WORKFLOW.md`](docs/GIT-WORKFLOW.md)
- [`docs/decisions/`](docs/decisions/)

The project is MIT licensed. Its direct application dependencies are pinned and
free/open source.
