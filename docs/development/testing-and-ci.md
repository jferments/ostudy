# Testing and CI

ostudy uses tiered local validation and one isolated self-hosted Linux/Android runner for pull-request qualification. The tiers provide fast feedback without pretending a narrow result proves every boundary.

Two pull-request workflows are intentionally separate:

- **PR Contract** validates the machine-readable acceptance snapshot and review-readiness metadata.
- **Check** validates the exact published product source SHA.

A malformed PR body does not block product qualification. A green metadata check does not mean the application compiled or ran.

## Unified runner

Automated product qualification runs on the repository-scoped unified runner with labels:

```text
self-hosted
linux
x64
ostudy-unified
```

The runner is an isolated Ubuntu 24.04 libvirt/QEMU/KVM VM. Linux/Tauri qualification runs directly; the official Android Emulator uses nested KVM. The VM has no personal ostudy library, forwarded SSH agent, host Docker/libvirt control socket, or Android release-signing credentials.

See [Unified Linux/Android runner](unified-linux-android-runner.md) and [runner operations](unified-runner-setup.md). The older split-runner note is retained only as migration context.

## Local validation tiers

```bash
./scripts/check-changed.sh
./scripts/check-static.sh
./scripts/check-fast.sh
./scripts/check.sh
```

### `check-changed.sh`

An impact-aware iteration aid. It inspects the current diff and chooses a conservative focused check when safe. It is not final merge qualification.

### `check-static.sh`

Verifies source/repository contracts that do not require the full runtime, including documentation organization/links, manifests/lockfiles/version mirrors, migration/repair policy, TypeScript and Rust formatting/static rules, typed IPC ownership, frontend/platform/shared-UI boundaries, route/destination inventories, AI/document ownership boundaries, external-file streaming architecture, Android static/generated-project contracts, workflow source-read-only policy, and verifier fixture tests.

This is the minimum maintained gate for documentation-only work.

### `check-fast.sh`

Adds high-return deterministic tests such as the complete frontend/controller/component suite, locked Rust compilation/lint-oriented checks, and focused native/document/platform contracts selected by the wrapper. This is the automatic draft-PR product gate.

### `check.sh`

The authoritative Linux/shared qualification. It includes the fast/static foundations plus the maintained complete native/frontend build and integration suite: native dependency verification, production frontend build, warning-free Rust lint/test coverage, migrations/repairs and public-schema compatibility, package/import/backup/restore behavior, scheduling/practice/analytics/deletion invariants, AI/document/Study Guide/sync contracts, standalone Tauri build, built-binary startup/persistence/adoption smoke, and native WebDriver workflows.

The current public migration registry is defined by `src-tauri/src/database.rs` (public schema version 8 at the time of this guide). Older pre-public v26 terminology belongs to the historical adoption input, not to the current public schema.

### Exact-tree reuse

Successful tier stamps are keyed by relevant source tree, tracked/untracked inputs, deletions, tool versions, and platform. Force a fresh run only when evidence requires it:

```bash
OSTUDY_FORCE_CHECK=1 ./scripts/check.sh
```

Do not use broad forced reruns instead of diagnosing a focused deterministic failure.

## Worktree stability

Qualification must not mutate Git-visible maintained source. Static/fast/full/Android wrappers compare `git status --porcelain` around important boundaries. Tauri Android materialization has known generated-file behavior handled by the maintained Android wrapper; the worktree guard catches new unexplained source rewrites.

Diagnostics belong in ignored artifact roots, not arbitrary repository files.

## Focused tests

Frontend file:

```bash
pnpm exec vitest run path/to/example.test.tsx
```

Complete frontend suite:

```bash
pnpm test
```

Focused native test:

```bash
cargo test --manifest-path src-tauri/Cargo.toml --locked <filter> -- --nocapture
```

Complete native tests:

```bash
cargo test --manifest-path src-tauri/Cargo.toml --locked
```

Use isolated temporary databases/directories. Never point automated recovery/import/restore tests at the learner's live application data.

## Android qualification

### Toolchain/emulator doctor

```bash
pnpm android:doctor:emulator
```

### Compile/package

```bash
pnpm android:test
# maintained wrapper:
./scripts/android-test.sh
```

### API 36 emulator smoke

```bash
pnpm android:smoke
# maintained wrapper:
./scripts/android-emulator-smoke.sh
```

The emulator suite covers representative launch/migration/study/persistence/restart/Back/compact-navigation and system file-picker behavior, plus affected Android-native bridges. It uses isolated application/AVD data.

Emulator success does not prove real-camera QR behavior, OEM storage providers, battery/thermal/low-memory behavior, physical-device screen-reader use, or release signing/store distribution. Record those as explicit manual boundaries when affected.

## `Check`: exact-source qualification

`.github/workflows/check.yml` is the product qualification workflow. It checks out and verifies the exact selected SHA with read-only repository credentials and never authors source.

Automatic behavior:

| Event/state | Gate |
| --- | --- |
| Draft PR source/lifecycle event | `fast` |
| Ready-for-review/non-draft source event | `all-full` |
| Push to `main` | full policy selected by the gate script |
| PR body edited only | no product `Check` |
| PR closed | cancel/skip obsolete automatic work |

Manual dispatch supports:

```text
preflight
static
fast
linux-full
android-full
all-full
```

Automatic and manual runs have separate contexts/concurrency so a diagnostic dispatch does not replace the required automatic PR check.

For non-preflight gates, static qualification is the first substantive source boundary. `all-full` then performs runner/KVM/dependency preflight, authoritative Linux `check.sh`, Android compile/package, and API 36 emulator smoke.

Every run records the tested SHA and selected gate and produces machine-readable/bounded qualification/failure evidence. A run qualifies only the SHA it actually checked out. A later source push requires new product qualification; a PR-body-only edit does not.

## `PR Contract`: metadata only

`.github/workflows/pr-contract.yml` validates the acceptance snapshot independently on PR open/edit/synchronize/reopen/draft/readiness transitions.

It validates schema/canonical fields/evidence requirements and, for non-draft PRs, base/head/branch identity. It does **not** compile/test product source and does not authorize, suppress, or abort `Check`.

Review readiness can be the transition that triggers full product qualification. Therefore validation entries may honestly be pending at readiness; they must never be marked passed in advance merely to obtain the run.

After full `Check` passes, updating the PR body with exact evidence reruns `PR Contract` only. It does not require a redundant product run because source did not change.

## Source-read-only Actions

All workflows are source-read-only, including manual maintenance/packaging/validation. They may check, build, package, upload artifacts, or validate PR metadata. They must not stage, format-rewrite, create commits/trees, push refs, or finalize an agent's product implementation.

The retired Orchestrator PR Ledger and branch-specific frontend source-checkpoint workflow are not part of the current development system. Source publication belongs to the branch writer; Actions qualifies published source.

## Failure evidence and triage

On a current-head failure:

1. confirm the run's SHA equals the current PR head;
2. read the selected gate, failed step, machine-readable result, and bounded logs/artifacts;
3. classify product, test-contract, runner/environment, GitHub/Connector, or external/manual cause;
4. reproduce the smallest failed boundary;
5. repair every independently actionable finding in one coherent batch;
6. rerun the narrow boundary before another broad gate.

One unchanged-head rerun is acceptable only with evidence of a transient external cause. The same fingerprint twice requires a changed strategy. Do not create no-op commits, toggle draft/ready repeatedly, widen permissions, add sleeps/timeouts blindly, skip/weaken tests, or introduce source-writing CI to manufacture green results.

See [Failure triage](../agents/FAILURE-TRIAGE.md).

## Native startup/persistence and WebDriver smoke

The full gate uses isolated app data and built binaries. It verifies representative first-run/public migration, supported pre-public adoption, startup reconciliation, persistence, and application workflows rather than relying only on in-memory tests.

The pre-public adoption smoke seeds the exact supported final development lineage and verifies data-preserving adoption into `ostudy-public-v1`; this historical v26 input must not be confused with the current public migration count/version.

Native WebDriver uses fresh isolated XDG/application data and exercises representative built-app behavior: current visible destinations/internal Practice, fresh library/authoring, deck-context Practice return and review completion, Analytics/restart persistence, responsive shell geometry, current Study Guides behavior, dialogs/focus/scroll containment, and native window behavior where applicable. Mutating interactions are not blindly retried after an ambiguous transport disconnect because the operation may already have committed.

## Performance and visual evidence

Wall-clock WebKit/UI performance measurements are advisory/manual rather than required correctness gates. Historical Stage 0/Stage 4 baseline records are retained as provenance for comparable measurements, not as current UI specifications. A timing-capture problem must not prevent Linux/Android correctness qualification.

When performance is an acceptance criterion, collect new exact-source candidate evidence and compare against the intended stable baseline without rewriting historical data to manufacture success.

## GitHub incidents

GitHub control-plane failure is neither product pass nor product fail. When broad API/Actions/archive/webhook operations show repeated service failures or an official incident is active, preserve the exact branch head, determine whether ambiguous writes landed using bounded reads, keep unavailable qualification explicitly pending, and rerun the normal workflow after recovery. Do not change source, permissions, triggers, or PR state merely to force activity.

## Packaging and releases

`android-package.yml` can build unsigned QA APK/AAB artifacts; it does not hold public release-signing credentials. `release.yml` produces current tagged Linux artifacts. Public Android signing/Play distribution and continuously qualified Windows/macOS/iOS release pipelines remain separately reviewed work.

Release candidates additionally run:

```bash
./scripts/release-check.sh
```

## Privacy of evidence

Tests/diagnostics must not contain the live learner SQLite database, real card/source content, managed personal images/documents, raw OCR/source text, provider prompts/responses/reasoning, API keys, authorization headers, or Android Keystore plaintext. Use generated fixtures and temporary storage; review diagnostic archives before sharing them.

## Reporting evidence

For every claimed pass, name the command/workflow, selected gate, exact SHA/tree, and result. Keep physical-device/live-provider/manual boundaries separate. Follow [Local testing and handoff](local-testing-and-handoff.md) for exact checkout/build/install instructions.
