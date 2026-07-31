# Dependency audit

ostudy requires no paid service, hosted database, account system, or cloud
runtime. Direct application and development dependencies are pinned exactly.

## Desktop and frontend

- React / React DOM `19.2.8`
- Tauri JavaScript API `2.11.1` and CLI `2.11.4`
- Tauri dialog/opener plugins `2.7.2` / `2.5.4`
- Tiptap editor packages `3.28.0`
- Vite `7.3.6`, TypeScript `5.8.3`, and esbuild `0.28.1`
- Vitest `4.1.10`
- Testing Library React `16.3.2` and DOM `10.4.1`
- jsdom `29.1.1`

The React testing dependencies run only during development and CI. The native
smoke test uses Python's standard library with `tauri-driver`; it does not add a
second JavaScript end-to-end framework.

pnpm is configured to reject unreviewed dependency lifecycle scripts. Only the
exact esbuild installation script is allowed. Tiptap packages are overridden to
one patch version so the editor cannot resolve a mixed internal graph.

## Rust and local data

- Tauri `2.11.5` and build helper `2.6.3`
- `tauri-driver` `2.0.6` as a development tool
- rusqlite `0.40.1` with bundled SQLite `3.53.2`
- serde `1.0.229`, serde_json `1.0.145`, chrono `0.4.45`
- rs-fsrs `1.2.1`

`rs-fsrs` remains isolated behind `src-tauri/src/scheduler.rs`; durable rows and
IPC objects use ostudy-owned types.

## Lockfile policy

`pnpm-lock.yaml` and `src-tauri/Cargo.lock` are committed source-of-truth files.
Normal setup, checks, and releases use frozen/locked dependency resolution.
Dependency upgrades should be reviewed separately from feature behavior.
