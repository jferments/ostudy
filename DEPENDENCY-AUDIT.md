# Dependency audit — revision 5.4

ostudy requires no paid service, hosted database, account system, or cloud
runtime. Every direct application dependency is pinned to an exact version.

## Desktop and frontend

- React / React DOM `19.2.8`
- Tauri JavaScript API `2.11.1`
- Tauri CLI `2.11.4`
- Tauri dialog plugin `2.7.2`
- Tauri opener plugin `2.5.4`
- Tiptap editor packages `3.28.0`
- Vite `7.3.6`
- Vite React plugin `4.7.0`
- TypeScript `5.8.3`
- esbuild `0.28.1`

The editor uses the MIT-licensed, headless Tiptap/ProseMirror stack. ostudy
owns the toolbar, visual design, validation rules, and persistence format. It
does not use Tiptap Cloud, collaboration, AI, or paid extensions.

pnpm is configured fail-closed for dependency lifecycle scripts. Only the exact
`esbuild@0.28.1` installation script is approved. Tiptap packages are overridden
to one patch version so the editor cannot resolve a mixed internal graph.

## Rust and local data

- Tauri `2.11.5` with the local asset protocol
- Tauri build helper `2.6.3`
- Tauri dialog/opener plugins `2.7.2` / `2.5.4`
- rusqlite `0.40.1` with bundled SQLite
- serde `1.0.229`
- serde_json `1.0.145`
- chrono `0.4.45`
- rs-fsrs `1.2.1`

`rs-fsrs` is isolated behind `src-tauri/src/scheduler.rs`; database rows and
Tauri DTOs use ostudy-owned types. This prevents a scheduler crate update
from silently redefining the durable database format.

The Linux desktop build dynamically uses the operating system's free/open-source
WebKitGTK/GTK stack required by Tauri. It does not integrate a proprietary
runtime or hosted service.

## Lockfile policy

The full source archive contains exact direct pins. The first successful local
run creates:

- `pnpm-lock.yaml`
- `src-tauri/Cargo.lock`

Both lockfiles must be committed when the Git baseline is established. Future
feature work must use frozen/locked installs, and dependency upgrades should be
reviewed separately from unrelated behavior changes.
