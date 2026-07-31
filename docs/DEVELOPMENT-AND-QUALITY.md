# ostudy development and quality process

The application must remain runnable after every merged change. Features should
be delivered as small vertical slices that include UI, IPC, Rust behavior,
database changes, tests, and documentation only when all are necessary for one
coherent behavior.

## Local commands

```bash
./run-app.sh          # build and launch the development application
./scripts/check.sh    # strict formatting, lint, build, and test gate
./scripts/release-check.sh  # repeat the strict gate in a clean source copy
```

`run-app.sh` is intentionally not a substitute for CI. It checks manifests,
installs from lockfiles, builds TypeScript and Rust, and launches. Formatting,
Clippy, and the full test suite live in `scripts/check.sh` so a maintenance-test
or formatting mistake cannot make the normal development launcher unusable.

## Required checks before merge

1. `cargo fmt --check`
2. frozen pnpm installation and exact direct-version verification
3. strict TypeScript/Vite production build
4. Clippy with warnings denied
5. all Rust unit/integration tests
6. locked Rust/Tauri build
7. exact resolved Rust dependency verification
8. manual smoke test of the affected UI path

Both `pnpm-lock.yaml` and `src-tauri/Cargo.lock` must be committed once generated.
No release should be created from an uncommitted working tree or from dependency
graphs resolved differently from CI.

## Testing layers

- **Pure Rust tests:** content validation, URL rules, scheduler mapping, settings,
  and other deterministic domain rules.
- **SQLite integration tests:** migrations, transactions, CRUD, idempotency,
  import/export, and review-ledger invariants using temporary databases.
- **React tests (next infrastructure task):** dialogs, pending states, unsaved
  changes, selection, practice grading, and error rendering with mocked IPC.
- **Native Tauri end-to-end tests (next infrastructure task):** launch, create a
  deck and card, restart, review, verify analytics, export, and import.

## Change discipline

Before implementation, write acceptance criteria. Keep unrelated refactors and
dependency upgrades out of feature changes. Any new dependency needs a written
reason, license review, exact version pin, supported-platform review, and tests
that exercise it.

## Database changes

Every schema change is an append-only numbered SQL migration with a SHA-256
checksum embedded in `database.rs`. Migrations run in transactions and must be
idempotent at the migration-ledger level. Existing migration files are never
edited after release.

## Comment standard

Add comments for:

- why an ordering requirement exists;
- why a transaction includes particular operations;
- security and trust boundaries;
- retry/idempotency behavior;
- browser/WebView or React lifecycle traps;
- compatibility constraints that are not obvious from types.

Do not add comments that merely restate a function or assignment. Stale comments
are defects and should be updated in the same change as the code.

## Honest validation reports

A validation report must distinguish:

- checks executed in the current environment;
- checks deferred to the target Ubuntu/Tauri machine;
- manual UI behavior actually exercised;
- known limitations.

Static inspection must never be described as a successful native build.

## Agent-maintained changes

Automated maintainers must follow `AGENTS.md` and `docs/agents/`. Their final
handoff must identify the exact base commit, list changed files, report commands
actually executed, distinguish unrun checks from passing checks, and describe
remaining risks. An agent must not merge its own work or push directly to
`main`.
