# Testing and CI

ostudy uses one Multipass-hosted Linux runner for pull-request, main-branch, and release validation. GitHub-hosted runners are not used.

## Runner labels

Every workflow job targets this exact label set:

```yaml
runs-on: [self-hosted, linux, x64, ostudy, tauri]
```

The runner image is expected to satisfy `scripts/doctor.sh --native-smoke`, including the pinned Node, pnpm, Rust, Tauri, WebKit, SQLite, OpenSSL, WebDriver, and display dependencies.

## Validation tiers

### Fast gate

```bash
bash ./scripts/check-fast.sh
```

The fast gate prioritizes failures that are inexpensive and likely to be actionable:

1. environment and repository-manifest verification;
2. stage-specific and test-policy verification;
3. script syntax checks;
4. frozen JavaScript dependency verification;
5. frontend tests;
6. TypeScript checking;
7. rustfmt; and
8. a locked Rust compile check for all targets and features.

Draft pull requests run this gate only. It provides rapid feedback while a branch is still changing frequently.

### Full qualification gate

```bash
./scripts/check.sh
```

The full gate runs the fast gate unless `OSTUDY_FAST_CHECK_PASSED=1`, then adds:

1. native environment qualification;
2. frontend production build;
3. warning-free Clippy;
4. Rust unit, integration, and migration-fixture tests;
5. standalone Tauri build;
6. Rust dependency verification; and
7. the native create → review → restart → persistence workflow.

A pull request runs the full gate when it is marked ready for review. Pushes to `main` and manually dispatched checks also run the full gate. Local acceptance before merge always uses `./scripts/check.sh`.

## Workflow behavior

`.github/workflows/check.yml` uses one `check` job so existing branch protection can continue requiring the same status name. The job always runs the fast gate. Its full step is conditional on a non-draft pull request or a non-pull-request event.

Concurrency cancellation stops obsolete runs for the same pull request or ref. Failure diagnostics include the fast log, full log when present, and the repository diagnostics archive.

`.github/workflows/release.yml` also uses the self-hosted runner. A version tag runs `scripts/release.sh`, which performs release qualification and writes the Linux artifacts uploaded by the workflow.

## Pull-request sequence

1. Open the pull request as a draft and use the fast check while implementing.
2. Run `./scripts/check.sh` locally before asking for merge approval.
3. Mark the pull request ready so CI performs the full self-hosted qualification.
4. Review the final diff and green check.
5. Squash and merge into `main`.
