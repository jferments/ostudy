# Testing and CI

ostudy uses the Multipass-hosted Linux runner for pull-request, main-branch, and release validation. Repository workflow jobs do not use GitHub-hosted operating-system runners. Third-party actions are pinned to immutable commit SHAs with a human-readable release comment.

## Required runner labels

Every workflow job targets:

```yaml
runs-on: [self-hosted, linux, x64, ostudy]
```

The runner must satisfy `scripts/doctor.sh --native-smoke`, including the pinned Node, pnpm, Rust, WebKit, SQLite, OpenSSL, Tauri-driver, and display dependencies. `versions.env` is authoritative; `.node-version`, `.nvmrc`, package engines, `rust-toolchain.toml`, JavaScript manifests, and Cargo manifests are verified mirrors.

## What the gate treats as a contract

Static verification protects durable repository boundaries rather than historical source layout:

- identity, exact versions, lockfiles, and migration checksums;
- frontend `invoke` names matching the Tauri command registry;
- direct Tauri invocation limited to typed domain gateways under `src/ipc/`;
- absence of obsolete unbounded or superseded IPC commands;
- explicit Rust modules and narrow repository policies;
- no skipped/focused/ignored tests;
- script syntax and the bounded-log Python tests;
- immutable self-hosted workflow dependencies.

Feature behavior belongs in Vitest, controller race tests, Rust/SQLite/migration tests, generated-import conformance/package round trips, and the native WebDriver workflow. Renaming a private helper, CSS class, or heading must not fail a structural stage verifier.

## Development check tiers

### Changed-file iteration aid

```bash
./scripts/check-changed.sh
```

This compares the branch and working tree with `origin/main` when available. Documentation-only changes skip executable checks; frontend-only changes run static checks plus changed Vitest selection; Rust-only changes run static checks, compile checking, and Rust library tests; mixed, migration, workflow, manifest, or script changes escalate to the complete fast tier. It is not merge qualification.

### Static tier

```bash
./scripts/check-static.sh
```

The static tier performs environment diagnostics, manifest/migration validation, IPC parity and gateway ownership, repository policy, test policy, Bash/Node/Python syntax, Python behavior tests for diagnostic rotation, frozen JavaScript installation, exact dependency verification, TypeScript checking, and `cargo fmt --check`.

### Fast tier

```bash
./scripts/check-fast.sh
```

The fast tier reuses an exact-tree static result when possible, then runs the complete frontend suite and locked Rust compile checking for every target and feature. Draft pull requests use this tier. Stage 3.12 controller tests deliberately resolve independent Search, Deck, Template, Practice, and Analytics reads out of order and assert that only the latest request may update current state.

### Full qualification tier

```bash
./scripts/check.sh
```

The full tier reuses an exact-tree fast result when possible, then adds native environment qualification, production frontend build, Clippy with warnings denied, all Rust/unit/integration/migration tests, standalone Tauri build, dependency verification, and the native create → review → restart → persistence workflow. This is the local acceptance command before merging.

The full Rust suite includes the generated `.ostudy.json` conformance corpus. The harness writes real input files under a unique temporary root, imports into a disposable test database, verifies supported/rejected syntax, boundaries, duplicates, create/append resolution, fresh schedules and atomicity, explicitly removes its imported test graph, and removes its temporary filesystem root through RAII cleanup. It never opens the user's live library.

## Exact-tree qualification and Cargo artifacts

`scripts/check-state.sh` fingerprints tracked files, non-ignored untracked files, deletions, active Node/pnpm/Rust versions, and platform. Static, fast, and full stamps live under the user cache directory. Editing the tree invalidates reuse; `OSTUDY_FORCE_CHECK=1` forces execution.

Local builds continue using `src-tauri/target`. CI uses `$HOME/.cache/ostudy/cargo-target`, so checkout cleanup does not discard compiled dependencies. Cargo remains responsible for normal source/toolchain invalidation.

## Diagnostics

`log-run.py` bounds every JSONL record, rotates the active log while the application is still running, and retains one bounded predecessor. WebView render, uncaught-error, and unhandled-rejection reports cross a typed Tauri command with bounded event/message/stack fields. Diagnostic export includes only environment/version/git state and bounded log tails; it excludes the SQLite library, flashcard contents, and managed images.

On CI failure, `summarize-check-failure.mjs` writes a bounded index of the failed phase and relevant excerpts to the job summary. Complete check logs and the privacy-conscious diagnostic archive remain available as failure artifacts.

## Pull-request workflow

1. Create one branch from current `main` and open one draft PR.
2. Use changed/static/fast checks while implementing.
3. Freeze a meaningful SHA long enough to consume its complete failure batch.
4. Run `./scripts/check.sh` locally when implementation is complete.
5. Mark the PR ready; the self-hosted workflow runs full qualification.
6. Review the exact-head green result and squash-merge.

Concurrency cancellation stops obsolete runs for the same PR/ref. Manual workflow dispatch supports `static`, `fast`, and `full` gates.
