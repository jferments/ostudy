# Testing and CI

ostudy uses the configured Multipass-hosted Linux runner for pull requests, `main`, and release artifacts. GitHub-hosted build runners are not used.

## Runner labels

Every workflow job targets the same runner label set:

```yaml
runs-on: [self-hosted, linux, x64, ostudy]
```

The runner image is expected to satisfy `scripts/doctor.sh --native-smoke`, including the pinned Node, pnpm, Rust, SQLite, WebKit, Tauri, WebDriver, and display dependencies.

## Validation tiers

### Fast gate

```bash
bash ./scripts/check-fast.sh
```

The fast gate prioritizes inexpensive checks with a high probability of finding an actionable problem:

1. environment diagnostics;
2. project, Stage 2, and Stage 3 structural verification;
3. test-policy enforcement;
4. Bash, Node, and Python syntax checks;
5. frozen JavaScript installation and exact dependency verification;
6. frontend tests;
7. strict TypeScript checking;
8. rustfmt; and
9. locked Rust compile checking for all targets and features.

Draft pull requests run this tier only. Obsolete runs for the same pull request are canceled by the workflow concurrency group.

### Full qualification gate

```bash
./scripts/check.sh
```

The full gate runs the fast gate unless `OSTUDY_FAST_CHECK_PASSED=1`, then adds:

1. native environment qualification;
2. frontend production build;
3. warning-free Clippy;
4. Rust unit, integration, SQLite, and migration-fixture tests;
5. standalone Tauri build;
6. resolved Rust dependency verification; and
7. the native create → review → restart → persistence workflow.

A ready-for-review pull request runs the full gate. Pushes to `main`, manual workflow dispatches, and tagged releases also use the full qualification path. Local acceptance before merge always uses `./scripts/check.sh`.

## Pull-request workflow

`.github/workflows/check.yml` retains one `check` job so existing branch protection can continue requiring the same status name.

- Draft pull request: fast gate.
- Ready pull request: fast gate, then full gate in the same checked-out workspace.
- New commit on a ready pull request: fast and full gates.
- Converted back to draft: fast gate only.
- Push to `main`: fast and full gates.
- Manual dispatch: fast and full gates.

The workflow sets `OSTUDY_FAST_CHECK_PASSED=1` before invoking the full gate, preventing the fast tier from being repeated in the same job.

Failure artifacts contain the fast log, the full log when that step ran, and the bounded repository diagnostics archive.

## Release workflow

`.github/workflows/release.yml` uses the same self-hosted runner. A version tag invokes `scripts/release.sh`, which performs release qualification before producing Linux artifacts.

The workflow does not install a second toolchain or a second set of Ubuntu packages. The Multipass runner is the controlled build environment, and `versions.env`, `rust-toolchain.toml`, manifest checks, and the repository gate verify its pinned tools.

## Local sequence

During implementation:

```bash
bash ./scripts/check-fast.sh
```

Before asking for merge approval:

```bash
./scripts/check.sh
./run-app.sh
```

After local acceptance, mark the draft pull request ready so the self-hosted runner performs the same full qualification against the remote branch head.
