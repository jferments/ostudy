# Study App

Study App is a free, local-first desktop flashcard application built with
Tauri, React, TypeScript, Rust, and embedded SQLite. It requires no account or
server. Decks, rich-text cards, review history, settings, and analytics remain
on the user's computer.

## Current functionality

- Create, rename, and delete decks.
- Create, edit, search, and delete rich-text flashcards.
- Format text with headings, lists, indentation, alignment, fonts, sizes,
  colors, highlights, symbols, links, code, and local images.
- Import and export complete `.studydeck` packages.
- Start a scheduled session directly from one deck or select several decks from
  the Practice screen.
- Grade reviews as Wrong or Right with three confidence levels.
- Schedule subsequent reviews with FSRS.
- View due counts, accuracy, response time, rating distribution, per-deck
  analytics, and recent reviews.
- Choose Light, Dark, Console, or Synthwave themes and typography settings.

## Architecture in one sentence

**React displays state and user intent; Rust owns validation and application
behavior; SQLite owns durable structured data; the filesystem owns managed
images.**

The frontend never executes SQL. All writes go through typed Tauri commands and
Rust-side transactions. Rich content is stored as validated, versioned Tiptap
JSON. Current FSRS state is materialized in `card_schedules`, while every review
is appended to an immutable `review_events` ledger.

Read the design documents:

- [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md)
- [`docs/CARD-FORMAT-AND-DATA-MODEL.md`](docs/CARD-FORMAT-AND-DATA-MODEL.md)
- [`docs/DEVELOPMENT-AND-QUALITY.md`](docs/DEVELOPMENT-AND-QUALITY.md)
- [`docs/GIT-WORKFLOW.md`](docs/GIT-WORKFLOW.md)
- [`docs/decisions/`](docs/decisions/)

## Ubuntu development setup

The supplied setup script installs the pinned Node, pnpm, and Rust toolchains and
the Linux libraries required by Tauri:

```bash
./scripts/setup-ubuntu.sh
```

The script adds `$HOME/.local/bin` and `$HOME/.cargo/bin` to future shell
sessions. Project scripts also set those PATH entries themselves, so a manual
`export PATH=...` command is not required.

## Run the application

```bash
./run-app.sh
```

On the first run, exact direct dependency pins are resolved and lockfiles are
generated. Commit both lockfiles before establishing the Git baseline:

```text
pnpm-lock.yaml
src-tauri/Cargo.lock
```

The application uses Tauri's stable identifier `com.example.studyapp`, so a new
source checkout continues to open the same local application-data directory and
SQLite library.

## Strict quality gate

```bash
./scripts/check.sh
```

This runs formatting, exact dependency verification, the production frontend
build, Clippy with warnings denied, all Rust tests, and a locked Rust build.
For release qualification from a clean source copy:

```bash
./scripts/release-check.sh
```

## Git baseline

After the strict check and a manual smoke test pass:

```bash
git init -b main
git add .
git commit -m "baseline: functional Study App v0.3.0"
git tag -a v0.3.0 -m "Known-good Study App v0.3.0 baseline"
```

Future work should be developed on short-lived branches and merged through
reviewed pull requests. See [`docs/GIT-WORKFLOW.md`](docs/GIT-WORKFLOW.md).

## Data locations and backups

The SQLite database and managed images live in the operating system's Tauri
application-data directory. The Settings screen displays the exact paths.
`.studydeck` export is for sharing content; it intentionally excludes personal
review history and application settings.

## License and dependency policy

The project is MIT licensed. Direct dependencies are pinned exactly and are
free/open-source. The Linux build dynamically uses the operating system's
WebKitGTK/GTK libraries required by Tauri.
