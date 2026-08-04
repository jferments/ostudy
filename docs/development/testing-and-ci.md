# Testing and CI

ostudy uses the Multipass-hosted Linux runner for pull-request, main-branch, and release validation. Repository workflow jobs do not use GitHub-hosted operating-system runners.

## Required runner labels

Every workflow job targets the labels already configured on the Multipass runner:

```yaml
runs-on: [self-hosted, linux, x64, ostudy]
```

The runner must satisfy `scripts/doctor.sh --native-smoke`, including the pinned Node, pnpm, Rust, WebKit, SQLite, OpenSSL, Tauri-driver, and display dependencies.

## Fast validation tier

Run:

```bash
bash ./scripts/check-fast.sh
```

This tier prioritizes inexpensive checks that catch most implementation errors:

1. environment and repository-manifest verification;
2. Stage 2 and Stage 3 structural verification;
3. test-policy and repository-script syntax verification;
4. frozen JavaScript installation and exact dependency verification;
5. frontend tests;
6. TypeScript checking;
7. rustfmt; and
8. locked Rust compile checking for every target and feature.

Draft pull requests run this tier only. This provides quick feedback while a branch is still receiving frequent commits.

## Full qualification tier

Run:

```bash
./scripts/check.sh
```

Unless `OSTUDY_FAST_CHECK_PASSED=1` is supplied by the same CI job, the full command first runs the fast tier. It then adds:

1. native environment qualification;
2. frontend production build;
3. Clippy with warnings denied;
4. Rust unit, integration, SQLite, and migration-fixture tests;
5. standalone Tauri build;
6. Rust dependency verification; and
7. the native create → review → restart → persistence workflow.

The full tier runs for:

- pull requests marked ready for review;
- pushes to `main`;
- manually dispatched checks; and
- release qualification.

Local acceptance before merging always uses `./scripts/check.sh`.

## Pull-request workflow

`.github/workflows/check.yml` retains one `check` job so existing branch protection can continue requiring the same status name.

1. Open a pull request as a draft.
2. Use the fast self-hosted check while implementing.
3. Run `./scripts/check.sh` locally when the implementation is complete.
4. Mark the pull request ready for review.
5. The ready transition runs the fast tier followed by full native qualification.
6. Review the final green result and squash-merge into `main`.

Concurrency cancellation stops obsolete runs for the same pull request or ref. Failure artifacts include the fast log, the full log when that step ran, and the bounded repository diagnostics archive.

## Release workflow

`.github/workflows/release.yml` also targets the Multipass self-hosted runner. A version tag invokes `scripts/release.sh`, which performs release qualification and writes the Linux artifacts uploaded by the workflow.

The workflow does not install toolchains dynamically. The controlled runner image is the authoritative release environment and must be updated deliberately when pinned versions change.
