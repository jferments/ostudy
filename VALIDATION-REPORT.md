# ostudy revision 5.4 validation report

## Scope

Revision 5.4 implements the requested deck-detail cleanup, direct single-deck
practice, multi-deck practice setup, graded FSRS reviews, review analytics,
image-dimension compatibility, import-title preservation, simplified settings,
and a React error boundary.

## Checks executed in the packaging environment

The following checks were actually executed against the final source tree:

- every shell script parsed with `bash -n`;
- every Node verifier parsed with `node --check`;
- the project manifest/command/migration verifier passed;
- all JSON and TOML configuration files parsed;
- all three SQLite migrations executed in order against a fresh in-memory
  database;
- representative deck, note, card, schedule, session, review, and analytics
  operations executed successfully in SQLite;
- SQLite `PRAGMA integrity_check` returned `ok` after those operations;
- migration SHA-256 values matched the constants embedded in `database.rs`;
- all TypeScript/TSX files passed the TypeScript 5.8 parser;
- a strict semantic TypeScript pass with local dependency declarations found no
  unused locals/parameters or source-level type errors;
- the stylesheet parsed without CSS syntax errors;
- a Rust lexical/delimiter scan found no unclosed strings, comments, or
  delimiters;
- frontend Tauri invocation names matched registered Rust commands;
- the requested obsolete UI labels and import suffix were absent;
- the Practice checkbox captures its boolean value before entering a React
  state updater;
- the final source archive was extracted and compared to the packaged tree.

## Regression coverage included in the source

Rust tests cover, among other behavior:

- all three migrations and bundled SQLite version;
- deck and card CRUD;
- rich-content and link validation;
- Tiptap image `width`/`height` attributes;
- unsupported-content versus malformed-document error precedence;
- FSRS outcome mapping and schedule transitions;
- idempotent review submission and schedule update;
- completed-session event rejection;
- settings persistence;
- deck export/import round trips and preserved import titles;
- empty-library analytics.

## Native execution boundary

This artifact environment does not include the pinned Rust 1.97.1 toolchain,
Cargo registry, pnpm dependency graph, or WebKitGTK development runtime. It
therefore could not execute the final native Cargo build, `rustfmt`, Clippy,
Rust test binary, or Tauri window.

The source does not claim that static validation is a native desktop test. On
the configured Ubuntu machine, run:

```bash
./scripts/check.sh
./run-app.sh
```

`run-app.sh` performs the real production frontend build and locked Rust/Tauri
build before opening the window. `scripts/check.sh` additionally enforces
rustfmt, warning-free Clippy, all Rust tests, and exact dependency verification.
After both pass and the manual workflow is exercised, commit the generated
lockfiles and establish the Git baseline.
