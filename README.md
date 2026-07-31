# ostudy

**ostudy** is a free, local-first desktop flashcard application built with
Tauri, React, TypeScript, Rust, and embedded SQLite. It requires no account or
server. Decks, card content, review history, settings, images, search indexes,
and analytics remain on the learner's computer.

The first release series starts at **0.1.0**. Git commits describe development
history and release tags describe published versions.

## Standalone flashcard core

The Phase 2 core supports:

- basic and automatically generated reverse cards;
- **Fill in the blank** notes using literal `[[answer]]` markers;
- reusable, field-based card templates, including a built-in Pharmacology
  template;
- tags, SQLite FTS5 text search, card browsing, sorting, and status filters;
- question-and-answer bulk entry without a separate CSV/TSV file format;
- autosaved editor drafts and recovery after leaving the editor;
- duplicate detection with an explicit **Save anyway** path;
- move or copy between decks;
- suspend/resume, bury/unbury, reset-to-New, and explicit rescheduling;
- daily new-card and review limits;
- direct review ratings from `0` through `3`: **Wrong**, **Hard**, **OK**, and
  **Easy**;
- interval previews, undo of the most recent review, related-card burying, and
  **Struggling cards** detection;
- recent-deck practice summaries with the Wrong/Hard/OK/Easy distribution;
- managed local images with small/medium/large sizing and left/center/right
  alignment;
- keyboard navigation, focus-managed dialogs, live-region announcements, and
  visible keyboard focus.

Math-specific cards are intentionally deferred to a dedicated later phase.
Deck interchange remains a single SQLite-based `.studydeck` file; Phase 2 does
not add CSV/TSV import.

## Safe card and template content

Card content is not stored or rendered as arbitrary HTML. The editor persists
versioned Tiptap/ProseMirror JSON, and Rust validates every node, mark,
attribute, link, image reference, and size/alignment value before it enters the
library. JavaScript URLs, remote image URLs, unknown nodes, arbitrary CSS, and
unsupported attributes fail closed.

The template language is deliberately non-programmable. It supports literal
text plus exact placeholders such as `{{medication name}}`; it does not support
expressions, loops, property access, scripts, SQL, HTML execution, or CSS.
Imported `.studydeck` databases are opened read-only with SQLite
`trusted_schema=OFF`, integrity checked, schema checked against an exact
allowlist, size limited, and fully revalidated before one transactional import.
Imported cards receive fresh local schedules and never import another learner's
review history.

## Ubuntu setup

From the repository root, run setup once:

```bash
./scripts/setup-ubuntu.sh
```

The script installs the pinned Ubuntu, Node, pnpm, Rust, and WebDriver
requirements and creates a standalone debug build. Cargo uses all detected CPU
cores by default; override the number when needed:

```bash
CARGO_BUILD_JOBS=24 ./scripts/setup-ubuntu.sh
```

## Start, rebuild, and test

Start the already compiled application:

```bash
./run-app.sh
```

`run-app.sh` does not run Vite, Cargo, or the test suite. After editing source,
rebuild explicitly:

```bash
./scripts/build.sh
```

Run the complete development gate:

```bash
./scripts/check.sh
```

The gate performs frozen dependency installation, exact manifest checks, React
component tests, Rust formatting, warning-free Clippy, Rust and migration tests,
a standalone Tauri build, resolved dependency checks, and a native
Tauri/WebDriver smoke test.

Builds run through `scripts/with-immutable-cargo-lock.mjs`. It first verifies
that `src-tauri/Cargo.toml` agrees with the committed `Cargo.lock`, then restores
and rejects any build that attempts to change or delete the lockfile. This
prevents the build-script problem where an incidental local dependency
resolution left `Cargo.lock` modified and blocked branch switching.

## Normal development loop

1. Make a focused change on a branch.
2. Run `./scripts/check.sh`.
3. Start the build with `./run-app.sh` and exercise the changed UI path.
4. Commit, push, and merge only after the automated and manual checks pass.

See [`docs/PHASE-2-ACCESSIBILITY-AUDIT.md`](docs/PHASE-2-ACCESSIBILITY-AUDIT.md)
for the keyboard and accessibility acceptance matrix for this phase.

## GitHub Actions and diagnostics

`.github/workflows/check.yml` repeats `./scripts/check.sh` on a clean Ubuntu
runner for pull requests to `main` and pushes to `main`.

`run-app.sh` writes a small rotating JSON Lines log to:

```text
${XDG_STATE_HOME:-$HOME/.local/state}/ostudy/ostudy.jsonl
```

Create a privacy-conscious support archive that excludes the card database and
managed images with:

```bash
./scripts/export-diagnostics.sh
```

## Application data and deck packages

The Tauri identifier is `io.github.jferments.ostudy`. The Settings screen shows
the active database and managed-image paths.

A `.studydeck` export is one standalone SQLite database containing deck content,
generated-card definitions, templates used by the deck, tags, and image bytes.
It intentionally excludes application settings, review events, current
schedules, struggling/suspended state, and other personal learning data. Legacy
version-1 SQLite packages remain readable through a compatibility importer;
new exports use package version 2.

## Architecture and contributor documentation

- [`ARCHITECTURE-AND-PLAN.md`](ARCHITECTURE-AND-PLAN.md)
- [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md)
- [`docs/CARD-FORMAT-AND-DATA-MODEL.md`](docs/CARD-FORMAT-AND-DATA-MODEL.md)
- [`docs/DEVELOPMENT-AND-QUALITY.md`](docs/DEVELOPMENT-AND-QUALITY.md)
- [`docs/GIT-WORKFLOW.md`](docs/GIT-WORKFLOW.md)
- [`docs/decisions/`](docs/decisions/)
- [`AGENTS.md`](AGENTS.md)

The project is MIT licensed. Direct dependencies are pinned and free/open
source. The Linux application uses the operating system's WebKitGTK/GTK runtime
required by Tauri.
