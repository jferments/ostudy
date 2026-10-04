# Testing and CI

ostudy uses tiered local validation and one isolated self-hosted Linux/Android runner for pull-request qualification. The tiers provide fast feedback without pretending a narrow result proves every boundary.

**Check** (`.github/workflows/check.yml`) is the only pull-request workflow. It validates the exact source commit of the PR head; nothing in the PR description affects it.

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

Contributor machines use the repository-owned commands below directly. They do not need the repository owner's Actions runner registration, runner username, VM sizing/KVM policy, GitHub workspace variables, or release-signing credentials. GitHub Actions adds runner-specific isolation/resource preflights and then calls these same maintained wrappers.

```bash
./scripts/ostudy-dev doctor
./scripts/check-changed.sh
./scripts/check-static.sh
./scripts/check-fast.sh
./scripts/check.sh
```

Use `./scripts/ostudy-dev doctor --native-smoke` to verify WebDriver/native-smoke prerequisites installed by the full Ubuntu/Debian contributor setup.

### `check-changed.sh`

An impact-aware iteration aid. It inspects the current diff and chooses a conservative focused check when safe. It is not final merge qualification.

### `check-static.sh`

Verifies source/repository contracts that do not require the full runtime, including documentation organization/links, manifests/lockfiles/version mirrors, migration/repair policy, TypeScript and Rust formatting/static rules, typed IPC ownership, frontend/platform/shared-UI boundaries, route/destination inventories, AI/document ownership boundaries, external-file streaming architecture, Android static/generated-project contracts, workflow source-read-only policy, Actions runner/storage policy, and verifier fixture tests.

This is the minimum maintained gate for documentation-only work.

### `check-fast.sh`

Adds high-return deterministic tests such as the complete frontend/controller/component suite, locked Rust compilation/lint-oriented checks, and focused native/document/platform contracts selected by the wrapper. This is the automatic draft-PR product gate.

### `check.sh`

The authoritative Linux/shared qualification. It includes the fast/static foundations plus the maintained complete native/frontend build and integration suite: native dependency verification, production frontend build, warning-free Rust lint/test coverage, migrations/repairs and public-schema compatibility, package/import/backup/restore behavior, scheduling/practice/analytics/deletion invariants, AI/document/Study Guide/sync contracts, standalone Tauri build, built-binary startup/persistence/adoption smoke, and native WebDriver workflows.

The current runtime lineage is `ostudy-release-v1`, schema 2. Its immutable schema-1 baseline and reviewed forward release migrations live under `src-tauri/schema/`. The immutable `ostudy-public-v1` migration registry in `src-tauri/src/database.rs` remains a compatibility input through schema 30 for existing libraries and supported historical backups. Legacy admission validates the exact known migration/repair evidence before any conversion write, normalizes through schema 30, proves preservation on an isolated SQLite copy, and only then atomically switches lineage metadata. Older pre-public v26 terminology belongs to the explicit historical recovery input, not to the current release schema.

### Exact-tree reuse

Successful tier stamps are keyed by relevant source tree, tracked/untracked inputs, deletions, tool versions, and platform. The script linters (ruff, ShellCheck, ty) are pinned with checksums in `versions.env`, so hashing that file covers them; the fingerprint does not probe them on `PATH`, which would make a stamp depend on whether the caller had loaded them. Force a fresh run only when evidence requires it:

```bash
OSTUDY_FORCE_CHECK=1 ./scripts/check.sh
```

Do not use broad forced reruns instead of diagnosing a focused deterministic failure.

Static and fast results are keyed by the exact source tree. Full Linux and Android results use a product fingerprint that ignores only root-level Markdown and Markdown under `docs/`; the Android fingerprint also covers the Java/Android toolchain. CI may reuse an earlier runner-local full or Android pass only when the current head has passed static, the fingerprint is unchanged, and the workflow sets `OSTUDY_ALLOW_QUALIFICATION_REUSE=1`. Within one CI job, the static step's result is passed to the fast and full steps (`OSTUDY_STATIC_CHECK_PASSED`), so static runs once per job. The run reports any reuse together with the earlier source that produced it. Source, tests, scripts, workflows, configuration, assets, and lockfiles all invalidate reuse.

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

For non-preflight gates, static qualification is the first substantive source boundary. `all-full` then performs runner/KVM/dependency preflight, authoritative Linux `check.sh`, Android compile/package, and API 36 emulator smoke. Within one successful exact-SHA `all-full` job, Android compile reuses the shared TypeScript/Vitest/host-Rust coverage already established by Linux full, and emulator smoke reuses the x86_64 smoke-bridge APK/instrumentation package produced by Android compile. When Android compile passed by reusing an earlier qualification, the clean checkout has no APKs, so emulator smoke builds them itself. Standalone Android gates remain self-contained.

Every run records the tested SHA and selected gate and produces machine-readable/bounded qualification/failure evidence. That evidence is persisted on the self-hosted runner rather than uploaded to GitHub Actions storage. A run qualifies only the SHA it actually checked out. A later source push requires new product qualification; a PR-body-only edit does not.

## Source-read-only Actions

All workflows are source-read-only, including manual maintenance/packaging/validation. They may check, build, package, and retain runner-local evidence/deliverables. They must not stage, format-rewrite, create commits/trees, or push refs. `scripts/verify-repository-policy.mjs` rejects `pull_request_target`, write permissions, persisted checkout credentials, and `git push` in workflows.

The retired Orchestrator PR Ledger, the branch-specific frontend source-checkpoint workflow, and the PR Contract description ledger are not part of the current development system. Source changes are committed on the branch; Actions qualifies them.

## Actions compute and storage policy

Routine CI executes on the repository's self-hosted runner. GitHub-hosted operating-system runners are forbidden by repository policy. Reusable Cargo, Gradle, package-manager, and toolchain caches live on the runner; `actions/cache` and action-managed cloud caches are forbidden because a self-hosted job using those features still stores cache data in GitHub's Actions service.

GitHub Actions artifact storage is also forbidden for oStudy workflows. Qualification JSON, failure summaries, logs, Android smoke evidence, failure bundles, Android packages, and tagged Linux release output are copied under the runner's persistent oStudy Actions state directory. The workflow summary records the runner-local path. Runner-local evidence defaults to seven-day retention and a 20 GiB total ceiling; the active run is never pruned.

`.github/workflows/actions-storage-maintenance.yml` runs hourly on the self-hosted runner and deletes **all** residual GitHub Actions artifacts plus all GitHub-hosted Actions caches for the repository. Cleanup reads the current GitHub REST rate-limit budget, reserves headroom for other repository operations, and checkpoints successfully when the installation token is exhausted so the next scheduled pass resumes instead of failing or idling the runner until reset. This intentionally catches historical objects and artifacts produced from stale branches. `scripts/verify-actions-storage-policy.mjs`, executed by static qualification, rejects GitHub-hosted runners, cloud caches, or any use of `actions/upload-artifact`.

GitHub still retains workflow-run logs and job summaries according to GitHub's workflow retention behavior; those are control-plane records rather than Actions artifacts and are not replaced by this runner-local evidence policy.

## Failure evidence and triage

A failed `Check` ends with a **Report qualification failure** step: it prints the failure summary from `scripts/summarize-check-failure.mjs` and raises an error annotation naming the failing boundary and its focused local reproduction. The last hundred or so lines of the job log are therefore enough to start, and the annotation also appears on the check run. The job's step summary carries the same Markdown.

Qualification commands run through `scripts/run-ci-captured.sh`. The complete stdout/stderr of each tier goes to its `ostudy-*-check.log` file; the live job log receives only a liveness heartbeat (`Qualification still running`, every `OSTUDY_CI_HEARTBEAT_SECONDS`, default 60) and a bounded tail (4 KiB on success, 32 KiB on failure). Zero byte growth in the heartbeat is a clue, not proof of a hang. Do not restore `2>&1 | tee` for qualification in `check.yml`; full raw logs stay in the runner-local evidence.

`scripts/write-qualification-result.mjs` writes `ostudy-qualification-result.json`. Its `qf-…` failure fingerprint excludes commit SHAs, durations, and incidental numbers so the same underlying failure is recognizable across repair commits, and independent failures keep separate records. Its `classificationHint` (`githubControlPlane`, `runnerEnvironment`, `productOrTest`, or `unclassified`) is advisory and never overrides the logs.

On a current-head failure:

1. confirm the run's SHA equals the current PR head;
2. read the failure summary at the end of the log, then the failed step's bounded tail;
3. classify product, test-contract, runner/environment, GitHub, or external/manual cause;
4. reproduce the smallest failed boundary locally;
5. repair every independently actionable finding in one coherent batch;
6. rerun the narrow boundary before pushing and before another broad gate.

One unchanged-head rerun is acceptable only with evidence of a transient external cause. The same fingerprint twice requires a changed strategy. Do not create no-op commits, toggle draft/ready repeatedly, widen permissions, add sleeps/timeouts blindly, skip/weaken tests, or introduce source-writing CI to manufacture green results.

## Native startup/persistence and WebDriver smoke

The full gate uses isolated app data and built binaries. It verifies representative first-run/public migration, supported pre-public adoption, startup reconciliation, persistence, and application workflows rather than relying only on in-memory tests.

The pre-public adoption smoke seeds the exact supported final development lineage and verifies data-preserving recovery into the historical `ostudy-public-v1` compatibility lineage. Current runtime startup subsequently converges supported public-v1 data to `ostudy-release-v1` / schema 1; neither historical v26 nor public-v1 schema 30 should be described as the fresh-release schema.

Native WebDriver uses fresh isolated XDG/application data and exercises representative built-app behavior: current visible destinations/internal Practice, fresh library/authoring, deck-context Practice return and review completion, Analytics/restart persistence, responsive shell geometry, current Study Guides behavior, dialogs/focus/scroll containment, and native window behavior where applicable. Mutating interactions are not blindly retried after an ambiguous transport disconnect because the operation may already have committed.

## Performance and visual evidence

Wall-clock WebKit/UI performance measurements are advisory/manual rather than required correctness gates. Historical Stage 0/Stage 4 baseline records are retained as provenance for comparable measurements, not as current UI specifications. A timing-capture problem must not prevent Linux/Android correctness qualification.

When performance is an acceptance criterion, collect new exact-source candidate evidence and compare against the intended stable baseline without rewriting historical data to manufacture success.

## GitHub incidents

GitHub control-plane failure is neither product pass nor product fail. When broad API/Actions/archive/webhook operations show repeated service failures or an official incident is active, preserve the exact branch head, determine whether ambiguous writes landed using bounded reads, keep unavailable qualification explicitly pending, and rerun the normal workflow after recovery. Do not change source, permissions, triggers, or PR state merely to force activity.

## Packaging and releases

Terminal `linux-full` and `all-full` qualification now has two distinct Linux artifacts: `./scripts/check.sh` qualifies the development-profile standalone/native behavior, then `./scripts/check-linux-package.sh` builds and verifies the production `io.github.jferments.ostudy` Debian package from the same exact SHA. The package candidate, build metadata, and checksums remain runner-local. Contributors can exercise the same production-package boundary with `./scripts/ostudy-dev package-deb`.


`android-package.yml` can build unsigned QA APK/AAB artifacts; it does not hold public release-signing credentials. Packages remain on the self-hosted runner rather than being uploaded as Actions artifacts. `release.yml` produces current tagged Linux artifacts and likewise retains them on the runner. The optional generated-source inspection bundle is persisted runner-locally when requested. If browser-downloadable public release distribution is needed later, design it as an explicit release-distribution boundary rather than reusing Actions artifact storage as a general file store. Public Android signing/Play distribution and continuously qualified Windows/macOS/iOS release pipelines remain separately reviewed work.

Release candidates additionally run:

```bash
./scripts/release-check.sh
```

## Privacy of evidence

Tests/diagnostics must not contain the live learner SQLite database, real card/source content, managed personal images/documents, raw OCR/source text, provider prompts/responses/reasoning, API keys, authorization headers, or Android Keystore plaintext. Use generated fixtures and temporary storage; review diagnostic archives before sharing them.

## Reporting evidence

For every claimed pass, name the command/workflow, selected gate, exact SHA/tree, and result. Keep physical-device/live-provider/manual boundaries separate. Follow [Local testing and handoff](local-testing-and-handoff.md) for exact checkout/build/install instructions.
