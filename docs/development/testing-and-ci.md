# Testing and CI

ostudy uses the Multipass-hosted Linux runner for pull-request, main-branch, and release validation. Repository workflow jobs do not use GitHub-hosted operating-system runners.

## Required runner labels

Every workflow job targets the labels already configured on the Multipass runner:

```yaml
runs-on: [self-hosted, linux, x64, ostudy]
```

The runner must satisfy `scripts/doctor.sh --native-smoke`, including the pinned Node, pnpm, Rust, WebKit, SQLite, OpenSSL, Tauri-driver, and display dependencies.

## Development check tiers

The repository deliberately separates fast iteration from merge qualification. A narrower tier may accelerate local development, but only the full tier qualifies a ready pull request for merge.

### Changed-file developer check

Run:

```bash
./scripts/check-changed.sh
```

This is an iteration aid, not a qualification gate. It compares the current branch and working tree with `origin/main` when available, then chooses a conservative path:

- docs-only changes perform no executable checks;
- frontend-only changes run the static tier plus Vitest's changed-test selection;
- Rust-only source changes run the static tier, a locked all-target Rust compile check, and Rust library tests;
- mixed, migration, manifest, workflow, or script changes escalate to the complete fast tier.

Pass an explicit base ref when needed:

```bash
./scripts/check-changed.sh main
```

### Static tier

Run:

```bash
./scripts/check-static.sh
```

The static tier is intended to fail quickly on structural mistakes before expensive compilation. It performs:

1. environment and repository-manifest verification;
2. Stage 2, Stage 3, managed-asset, backup, and repair structural verification;
3. test-policy and repository-script syntax verification;
4. frozen JavaScript installation and exact dependency verification;
5. TypeScript checking; and
6. `cargo fmt --check`.

### Fast tier

Run:

```bash
./scripts/check-fast.sh
```

The fast tier reuses a current static result for the exact same source tree when possible, then adds:

1. the complete frontend test suite; and
2. locked Rust compile checking for every target and feature.

Draft pull requests run this tier by default. This provides broad feedback while a branch is still receiving frequent commits.

### Full qualification tier

Run:

```bash
./scripts/check.sh
```

If the exact current source tree already passed the fast tier, `check.sh` reuses that result instead of running it again. Otherwise it runs `check-fast.sh` first. It then adds:

1. native environment qualification;
2. frontend production build;
3. Clippy with warnings denied;
4. Rust unit, integration, SQLite, and migration-fixture tests;
5. standalone Tauri build;
6. Rust dependency verification; and
7. the native create → review → restart → persistence workflow.

The full tier is the local acceptance command before merging.

## Exact-tree qualification stamps

`scripts/check-state.sh` fingerprints tracked files, non-ignored untracked files, and the active Node/pnpm/Rust/platform toolchain. Successful static, fast, and full checks write stamps under the user's cache directory rather than into the repository checkout.

A stamp is reusable only when the fingerprint still matches. Editing, adding, or deleting source/test/configuration files invalidates it automatically. Missing JavaScript dependencies also prevent static/fast reuse. Set `OSTUDY_FORCE_CHECK=1` to force a tier to execute even when a current stamp exists.

This allows the normal local sequence:

```bash
./scripts/check-fast.sh
./scripts/check.sh
```

to avoid running the complete fast gate twice when nothing changed between the commands.

## Persistent Cargo artifacts on the self-hosted runner

Local development continues to use `src-tauri/target` by default. Under `CI=true`, `scripts/cargo-target.sh` instead uses a persistent Cargo target under the runner's cache directory. The GitHub checkout can therefore clean the workspace without deleting compiled dependency and build-script artifacts after every pull-request synchronization.

Cargo remains responsible for invalidating artifacts when source, features, dependencies, or the compiler change. The persistent target is a build cache, not a qualification cache.

Override the location explicitly with:

```bash
OSTUDY_CARGO_TARGET_DIR=/path/to/target ./scripts/check.sh
```

## Pull-request workflow

`.github/workflows/check.yml` retains one `check` job so existing branch protection can continue requiring the same status name.

1. Open a pull request as a draft.
2. Use `check-changed.sh`, `check-static.sh`, or `check-fast.sh` while implementing.
3. Freeze a meaningful SHA while its CI run completes; do not invalidate useful diagnostics with speculative follow-up commits.
4. Fix the complete failure batch from that exact run.
5. Run `./scripts/check.sh` locally when the implementation is complete.
6. Mark the pull request ready for review.
7. The ready transition runs the full self-hosted qualification gate.
8. Review the exact-head green result and squash-merge into `main`.

Concurrency cancellation stops obsolete runs for the same pull request or ref.

## Manual GitHub Actions gates

The `Check` workflow supports `workflow_dispatch` with one of three gate choices:

- `static` — structural/type/format checks only;
- `fast` — static + frontend tests + Rust compile check;
- `full` — complete merge qualification.

Normal event behavior remains conservative:

- draft pull request synchronization → fast;
- ready pull request synchronization → full;
- push to `main` → full.

## Failure diagnostics

On failure, `scripts/summarize-check-failure.mjs` extracts the failed phase and bounded highlights for TypeScript, Vitest, rustfmt, Rust/Clippy/test panics, verifier errors, and native smoke failures into the GitHub job summary.

The workflow still uploads the complete fast/full logs and the repository diagnostics archive. The summary is a debugging index, not a replacement for the source logs.

## Release workflow

`.github/workflows/release.yml` also targets the Multipass self-hosted runner. A version tag invokes `scripts/release.sh`, which performs release qualification and writes the Linux artifacts uploaded by the workflow.

The workflow does not install toolchains dynamically. The controlled runner image is the authoritative release environment and must be updated deliberately when pinned versions change.
