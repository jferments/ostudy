# Testing and CI

ostudy uses one tiered validation system locally and on the self-hosted Linux runner. Each tier answers a different question; none of the inexpensive tiers should be described as complete application qualification.

The repository does not use GitHub-hosted operating-system runners. Every workflow job targets:

```yaml
runs-on: [self-hosted, linux, x64, ostudy]
```

Third-party actions are pinned to immutable commit SHAs with a human-readable version comment.

## Why the tiers exist

The complete Tauri build, Rust suite, and native WebDriver workflow are valuable but expensive. Running them after every small edit wastes time and obscures the first useful failure. The tiers therefore progress from cheap structural guarantees to full native behavior:

```text
changed → static → fast → full
```

An exact-tree stamp allows a later tier to reuse an earlier successful result only when source, relevant untracked files, deletions, tool versions, and platform still match. `OSTUDY_FORCE_CHECK=1` disables reuse when testing the gate itself or investigating a suspicious cache result.

## Changed-file iteration

```bash
./scripts/check-changed.sh
```

This is a development aid, not merge qualification. It compares the branch and working tree with `origin/main` when available and routes the change:

- documentation-only changes run documentation and static repository contracts;
- frontend-only changes run static checks and changed Vitest selection;
- Rust-only changes run static checks, locked compile checking, and focused library tests;
- document-related Rust changes add focused document contract filters;
- migrations, manifests, workflows, scripts, native dependencies, or mixed changes escalate to the fast tier.

The routing is conservative. When the script cannot prove that a narrow selection is sufficient, it chooses the broader tier.

## Static qualification

```bash
./scripts/check-static.sh
```

The static tier verifies:

- pinned environment and manifest relationships;
- committed JavaScript and Cargo lockfiles;
- migration registration and checksums;
- frontend IPC names against the native command registry;
- direct Tauri invocation limited to typed domain gateways;
- repository module, provider-network, credential, UI, workflow, and test policies;
- documentation links, stale canonical paths, and top-level organization;
- Bash and Node syntax plus Python script behavior tests;
- frozen JavaScript installation and exact direct dependency versions;
- TypeScript project checking;
- canonical Rust formatting.

This tier is the minimum check for a documentation reorganization because a moved file can break agent instructions, issue forms, scripts, or reader navigation even when no executable source changed.

## Fast qualification

```bash
./scripts/check-fast.sh
```

The fast tier reuses a matching static result, provisions or verifies the pinned PDFium runtime, then runs:

- the complete frontend Vitest suite;
- locked Rust compile checking for the supported target and feature set;
- focused document contracts that protect source-format, semantic-window, provider, OCR, rendering, and checkpoint boundaries.

Draft pull requests use the fast tier. Controller tests deliberately resolve independent reads out of order and verify that obsolete successes and errors cannot replace newer screen state. Mutation tests preserve committed success when a later reconciliation read fails.

## Full qualification

```bash
./scripts/check.sh
```

The full tier reuses a matching fast result and adds:

- native environment qualification, including WebKit/WebDriver and native runtime checks;
- production frontend build;
- Clippy with warnings denied;
- all Rust unit, integration, migration, import/export, backup, scheduling, analytics, AI, and document tests;
- standalone Tauri build with locked Cargo dependencies;
- resolved Rust dependency verification;
- a native WebDriver create → practice → restart → persistence workflow.

This is the authoritative local merge gate. A pull request is not ready because files compile in isolation or because an older commit passed.

The native smoke test can be skipped temporarily with `OSTUDY_SKIP_NATIVE_SMOKE=1`, but the pull request must state that it was skipped and why. CI normally runs it.

## Important test layers

### Repository contracts

Static scripts protect stable boundaries such as exact versions, immutable migrations, IPC parity, self-hosted workflow policy, dependency isolation, documentation organization, and absence of skipped/focused/ignored tests. They should verify behaviorally meaningful ownership rules rather than brittle private helper names.

### Frontend and controller tests

Vitest and Testing Library cover user interaction, accessible relationships, error/empty/loading states, responsive primitives, editor behavior, and controller request ordering. Independent request lanes are resolved in deliberately surprising orders to prove that stale reads cannot update current state.

### Pure Rust tests

Pure tests cover rich-content validation, cloze parsing and presentation, template substitution, duplicate normalization, scheduling adapters, local-calendar semantics, provider schema conversion, endpoint policy, semantic partitioning, resource planning, and safe error normalization.

### SQLite integration and migration tests

Integration tests cover transactions, CRUD, FTS triggers, card identity, scheduling/history preservation, move/copy, archive/delete semantics, analytics attribution, import/export, backups, managed assets, startup repair, and idempotent retry behavior.

Released schema fixtures load representative historical data, apply current migrations and repairs, verify preserved behavior, and run SQLite integrity checks. Released migration files and fixtures are never edited to make a new test pass.

### Generated-import conformance

The `.ostudy.json` harness materializes real files under unique temporary roots and exercises valid Markdown, Basic/Reverse/Cloze behavior, Unicode, limits, unknown fields, unsafe syntax, duplicates, exact-name create/append behavior, fresh schedules, transaction atomicity, and cleanup. It never opens the user's live library.

### Provider and document contracts

Provider fixtures use deterministic local servers rather than paid internet credentials. Document tests cover extraction quality, malformed text, semantic windows, outline assembly, rendering, OCR selection, cancellation, checkpoints, source locators, provider budgets, structured output, generated-card validation, backup, and deletion lifecycle.

Focused acceptance plans under this directory describe the regression matrices for high-risk document, scheduling, transfer, backup, and managed-asset changes.

### Native smoke test

The native smoke test launches the built Linux Tauri binary through WebDriver, performs a real create and review workflow, restarts the application, and confirms persistence. It validates the application shell, IPC, native storage, and restart boundary together; a mocked frontend test cannot replace it.

## Manual acceptance

Automation protects known contracts. A human still evaluates the changed workflow in the built application. Test what actually changed rather than copying an unrelated historical checklist.

For user-visible work, consider:

- normal, empty, loading, validation, provider, cancellation, and recovery states;
- persistence after restart;
- Light, Dark, Console, and Synthwave themes;
- compact, tablet, and wide layouts;
- keyboard-only and coarse-pointer/touch interaction;
- reduced motion and high interface scale;
- import/export, backup, migration, and deletion effects;
- close/exit behavior during active practice or document work;
- privacy of diagnostics and errors.

Record the exact paths exercised and any unavailable platform test.

## CI workflow

Draft pull requests run fast qualification. Ready pull requests run the full gate. Pushes to `main` and tagged releases use the same scripts rather than a parallel CI-only implementation.

Concurrency cancellation stops obsolete runs for the same pull request or ref. Manual workflow dispatch can request static, fast, or full qualification. The successful run must belong to the current head SHA.

CI uses a persistent Cargo target cache outside the checkout, while Git and toolchain fingerprints continue to control correctness. The workflow may expose a candidate Cargo lockfile or rustfmt artifact when normalization is required, but normal qualification is read-only and still fails until intentional source changes are committed.

## Failure artifacts and diagnostics

On failure, the workflow writes a bounded summary of the phase and relevant excerpts. Complete check logs and the privacy-conscious diagnostic archive may be uploaded as artifacts.

Diagnostic export includes environment, pinned versions, Git state, and bounded recent log records. It excludes the SQLite library, cards, tags, templates, managed images, source PDFs, OCR/source text, provider prompts or responses, credentials, and Authorization headers.

## Release qualification

```bash
./scripts/release-check.sh
```

The release check repeats the complete gate from a clean source copy and verifies version agreement and artifact assumptions. The current release workflow builds Linux artifacts from a version tag. Other platforms require their own qualified runners and packaging before they can be added to release documentation.
