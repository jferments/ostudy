# Development guide

This guide explains how to work in the ostudy repository without having to reconstruct the project from completed stage plans. It covers the active source layout, ordinary local commands, and the boundaries a change is expected to preserve. Pull-request publication is described in [Workflow](workflow.md); validation mechanics are described in [Testing and CI](testing-and-ci.md).

## Development status

Ubuntu/Debian Linux is the primary qualified environment. The repository uses a self-hosted Linux runner for pull-request and release validation. Windows and macOS source-build paths exist but are not yet continuously qualified, and mobile projects are planned rather than present.

The application version is currently `0.1.0`. The project is under active development, so a successful source build is not the same as a broadly supported platform release.

## Repository map

```text
src/                         React/TypeScript application
  screens/                   feature workspaces
  controllers/               workflow orchestration and stale-read policy
  ipc/                       typed native command gateways
  platform/                  operating-system/WebView capability adapters
  ui/                        shared interaction and visual foundations

src-tauri/
  src/                       Rust domain, services, commands, and startup
  migrations/                append-only SQLite schema migrations
  tests/                     integration and release-schema fixtures
  capabilities/              Tauri permissions
  resources/                 staged native runtime resources (mostly ignored)

scripts/                     setup, build, checks, release, diagnostics, policy
 tests/                      cross-language fixtures and script tests
 docs/                       user, installation, architecture, development,
                             reference, decision, agent, and historical material
.github/                     pull-request, issue, CI, and release configuration
```

Root-level files are limited to conventional project entry points and files that build tools discover there: README, license, contribution/agent guidance, package and toolchain manifests, frontend configuration, the launcher, and authoritative version pins. Subject-specific reports and guides belong under `docs/`.

## Sources of truth

Several values are intentionally mirrored for tool compatibility. The verifier prevents them from drifting.

- `versions.env` is authoritative for pinned tool and direct dependency versions.
- `package.json`, `pnpm-lock.yaml`, and `pnpm-workspace.yaml` own the JavaScript graph.
- `src-tauri/Cargo.toml` and `src-tauri/Cargo.lock` own the Rust graph.
- `rust-toolchain.toml`, `.node-version`, and `.nvmrc` mirror the pinned language versions for standard tooling.
- `src-tauri/src/database.rs` registers every migration checksum.
- `src-tauri/src/lib.rs` registers the native command surface.
- `docs/architecture/` and accepted ADRs describe durable ownership rules.

Do not “simplify” a mirrored version file merely because another file contains the same number. It may exist because a different tool discovers that conventional filename, and the manifest verifier documents the relationship.

## One-time Linux setup

```bash
./scripts/setup-ubuntu.sh
./scripts/install-native-deps.sh --with-tesseract
```

The first command installs the qualified build environment and creates an initial standalone build. The second ensures OCR fallback is available for corrupted or scanned PDFs. See [Linux installation](../installation/linux.md) for prerequisites and troubleshooting.

## Day-to-day commands

```bash
./run-app.sh                     # start the existing standalone build
./scripts/build.sh               # rebuild after source changes
pnpm desktop                     # live Tauri/Vite development session
./scripts/export-diagnostics.sh  # create a privacy-conscious support archive
```

`run-app.sh` does not compile. It verifies that the standalone build matches the source tree and refuses to launch a stale binary as though it were current.

Cargo uses its incremental cache across builds. `CARGO_BUILD_JOBS` defaults to the machine's logical processor count and may be capped for memory-constrained or shared systems.

## Validation while editing

Use the smallest useful tier during iteration:

```bash
./scripts/check-changed.sh
./scripts/check-static.sh
./scripts/check-fast.sh
./scripts/check.sh
```

The complete gate is required before merge. Exact-tree stamps allow a later tier to reuse a successful earlier tier only when tracked and relevant untracked source, deletions, tools, and platform still match. Set `OSTUDY_FORCE_CHECK=1` when diagnosing the gate itself or when an exact-tree cache must be bypassed.

The static tier includes documentation links and top-level organization. Moving a guide is therefore a code-reviewable change: references must be updated in the same commit rather than left for a future cleanup.

See [Testing and CI](testing-and-ci.md) for every command and what it does.

## Working on the frontend

Screens render state and express user intent. Workflow-heavy logic belongs in a focused controller, and native calls belong in typed domain gateways under `src/ipc/`.

Before adding a component:

1. check whether `src/ui/` already owns the needed accessibility or interaction behavior;
2. keep domain rules out of the shared primitive;
3. use CSS Modules and semantic tokens;
4. test keyboard, touch/coarse pointer, compact layout, reduced motion, and all themes where relevant;
5. preserve stale-read and committed-mutation semantics.

Direct `invoke()`, provider `fetch()`, raw dialogs, ordinary browser confirmations, extra global stylesheets, and feature-local focus traps are rejected by repository policy.

## Working on Rust and SQLite

Rust owns validation, domain rules, task orchestration, and transactions. A command should accept a bounded typed request, validate it, call a domain/service boundary, and return a bounded DTO or normalized error.

When changing storage:

- append a numbered migration rather than editing a released file;
- register the migration and SHA-256 checksum;
- add or update release-schema upgrade coverage;
- use a versioned data repair for one-time canonicalization that is not a schema change;
- define deletion, backup, import/export, and rollback effects;
- keep helpers transaction-neutral unless they are the explicit operation owner.

Warnings are denied in the full Rust gate. Do not use dead-code or unused suppressions to hide an incomplete refactor.

## Working on scheduling and analytics

`src-tauri/src/scheduler.rs` is the adapter around `rs-fsrs`. ostudy-owned types remain the durable and IPC contract.

Review submission updates the current schedule and appends historical evidence transactionally. Schedule controls that are not reviews must not invent review events. Analytics must remain reproducible from stored events/sessions and must honor stable identity, move, rename, archive, and finalized-deletion semantics.

A change in this area needs explicit tests for retry identity, transaction boundaries, local calendar behavior, and historical attribution.

## Working on AI and documents

Provider networking is confined to the native AI gateway, and credential access is confined to the native secret boundary. Features request narrow tasks; they do not receive a generic completion endpoint.

Document work should preserve the separation between:

- source-format ingestion;
- text/semantic extraction;
- page rendering;
- OCR;
- document outline and bounded source windows;
- provider task execution;
- canonical card authoring and persistence.

A parser, renderer, OCR engine, or model response must be converted into ostudy-owned data before crossing its boundary. See [AI and document processing](../architecture/ai-and-document-processing.md) and the focused document test plans in this directory.

## Imports, backups, and compatibility

Treat `.studydeck`, `.ostudy.json`, and `.ostudybackup` as different external contracts. A change must state which format versions it reads and writes, how old content is adapted, what personal data is excluded, and whether a failed import or restore can leave partial state.

Use isolated temporary storage in import/conformance tests. Never point a test at the live application data directory.

## Documentation changes

Write for the intended reader:

- README: product purpose, current features, platform status, quick start, and links;
- user guide: tasks and concepts in ordinary language;
- installation: platform-specific prerequisites and qualification status;
- architecture: ownership, data flow, invariants, and tradeoffs;
- development: commands, tests, diagnostics, and contribution process;
- reference: versioned formats and exact contracts;
- history: completed stage plans, dated audits, and superseded validation records.

Avoid lists of implementation trivia in the product overview. Avoid claiming that a roadmap item is shipped. A moved or renamed file must update every relative link, agent instruction, script reference, and issue template. Run the documentation verifier before treating a reorganization as complete.

## Dependencies

Direct dependencies are exactly pinned and lockfiles are committed. Upgrade dependencies separately from feature behavior when practical. Explain why the dependency is needed, its license and trust boundary, lifecycle scripts, native code or network behavior, and how it will be maintained.

The current graph and PDFium/Tesseract provisioning policy are documented in [Dependency audit](dependency-audit.md).

## Diagnostics and privacy

Application runs emit bounded structured records. Diagnostic export includes environment, versions, Git state, and bounded recent errors, but excludes the SQLite library, card content, managed images, source PDFs, OCR text, provider prompts/responses, and credentials.

A developer should still review an archive before sharing it. Do not solve a diagnostic problem by expanding logs to include private study material or authorization data.

## Releases

A release starts from a clean tagged commit whose version matches `package.json`, `src-tauri/Cargo.toml`, and `src-tauri/tauri.conf.json`. The release gate repeats full qualification from a clean source copy. The current workflow produces Linux artifacts; other platform pipelines remain future work.

Normal feature merges do not change the version automatically. See [Workflow](workflow.md) for tagging, artifacts, and rollback.
