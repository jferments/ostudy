# Testing and CI

ostudy uses tiered validation locally and one isolated self-hosted Linux runner for automated Linux/Tauri and Android qualification. Inexpensive tiers support iteration; only the authoritative exact-head gates qualify a merge.

Automated qualification validates published source. It never authors the pull-request product tree, pushes commits, or changes PR readiness.

## Runner architecture

Automated qualification runs on the repository-scoped `ostudy-ci-unified` runner with:

```text
self-hosted
linux
x64
ostudy-unified
```

The runner is an Ubuntu 24.04 libvirt/QEMU/KVM virtual machine on `ML-tower`. It runs Linux/Tauri checks directly and the official Android Emulator through nested KVM. The VM has no host filesystem share, forwarded SSH agent, Docker/libvirt host-control socket, signing credentials, or personal ostudy data.

See [Unified Linux and Android self-hosted runner](unified-linux-android-runner.md) for the architecture and [Unified runner setup](unified-runner-setup.md) for provisioning and recovery.

## Local validation tiers

Use the repository tiers in increasing order:

```bash
./scripts/check-changed.sh
./scripts/check-static.sh
./scripts/check-fast.sh
./scripts/check.sh
```

`check-changed.sh` is a conservative diff-aware iteration aid, not merge qualification.

`check-static.sh` verifies repository and workflow policy, manifests/lockfiles, migrations, typed IPC, dependency/platform boundaries, documentation links, script syntax/contracts, Python script regressions, TypeScript, and Rust formatting.

`check-fast.sh` adds the complete frontend test suite, locked Rust compilation, warning-free Clippy, and high-return document/platform/database contracts.

`check.sh` is the authoritative Linux/shared merge gate. It adds native dependency qualification, production frontend build, all Rust tests, standalone Tauri build, dependency verification, the built-binary pre-public adoption smoke, and native WebDriver persistence/UI tests.

Successful check stamps are keyed by the exact source tree, relevant untracked files, tool versions, and platform. Use `OSTUDY_FORCE_CHECK=1` only when deliberately bypassing exact-tree reuse. A failed run with retained artifacts writes a step-specific `reproduce.sh`; run that boundary before repeating a broader tier.

## Pull-request execution contract

Every substantial PR contains one machine-readable JSON ledger between the `ostudy-agent-state` markers. [`scripts/verify-pr-contract.mjs`](../../scripts/verify-pr-contract.mjs) validates it before qualification.

Draft PRs may contain incomplete acceptance criteria and pending evidence. A synchronize event can briefly make the recorded draft head stale; this is reported as a warning so implementation is not blocked by a PR-body update race.

Before review readiness, the ledger must exactly match the GitHub base, branch, and head, and must establish:

- `currentState: readyForReview`;
- terminal evidence for every `AC-##` criterion;
- static, fast, and full passed on the current exact head;
- Android passed or explicitly `notRequired` for a non-Android change;
- manual acceptance passed, `notRequired`, or explicitly `humanRequired`;
- no unresolved known failure.

`notRun`, inspection, or an older green commit is not readiness evidence. The schema and completion predicate are documented in [Agent execution state](../agents/EXECUTION-STATE.md).

## Worktree stability

Qualification must not mutate Git-visible repository state. Static, fast, full, and Android qualification snapshot `git status --porcelain` and compare it at test boundaries. Pre-existing local changes are tolerated as the baseline; new tracked/untracked mutations fail immediately with a before/after diff.

Tauri's Android materializer is known to rewrite selected generated Android sources while constructing a build. `android-build.sh` saves/restores the reviewed versions on every exit. The mutation guard catches any additional generated source drift.

Local diagnostics use `.artifacts/diagnostics/`, which is ignored by Git and excluded from release-source copies. Qualification may not create arbitrary root-level evidence files.

## Android qualification

Android shares the React/Rust application and domain code, so Android-affecting ready changes pass both Linux full and Android gates.

Compile/package qualification:

```bash
./scripts/android-test.sh
```

This verifies pinned SDK/NDK/JDK configuration, generated Android project policy, shared contracts, Android Rust targets, Kotlin/Gradle build, debug APK/instrumentation packages, package/permission/alignment constraints, and worktree stability.

Official emulator qualification:

```bash
./scripts/android-emulator-smoke.sh
```

The smoke boots the pinned API 36 Google APIs x86_64 emulator through KVM and verifies native launch, SQLite migration, study workflows, persistence/restart, Android Back, process restoration, and system document-picker/import/export behavior. The smoke-only JavaScript bridge is restricted to test APKs and rejected by release verification.

A real Google Code Scanner camera scan and physical-device LAN transfer remain manual boundaries.

## Automatic GitHub Actions behavior

`.github/workflows/check.yml` is the single automatic qualification workflow. It uses one serialized job for the one-runner topology and minimum permission:

```yaml
permissions:
  contents: read
```

It checks out and verifies the exact PR head. For pull requests it validates the PR state ledger, then runs **static qualification first** before runner/KVM checks, dependency graph validation, fast, Linux full, or Android work.

Automatic selection:

- draft PR: `fast`;
- ready/non-draft PR: `all-full`;
- push to `main`: `all-full`;
- manual dispatch: explicit `preflight`, `static`, `fast`, `linux-full`, `android-full`, or `all-full`.

An `all-full` run is ordered:

```text
PR execution contract
→ check-static.sh
→ unified runner/KVM preflight
→ locked Rust dependency graph
→ check.sh
   → fast qualification (reusing static evidence)
   → production build and full Rust suite
   → pre-public adoption smoke
   → native WebDriver persistence/UI smoke
→ android-test.sh
→ android-emulator-smoke.sh
```

Concurrency cancellation replaces obsolete runs for the same PR/ref. A green result qualifies only its recorded `QUALIFIED_SHA`.

## Read-only qualification and explicit maintenance

Automatic PR qualification must not:

- request `contents: write` or `pull-requests: write`;
- persist checkout credentials;
- stage a generated tree;
- create commits or Git trees;
- push refs;
- mutate draft/readiness state;
- run task-specific source applicators/finalizers.

These invariants are checked by `scripts/verify-agent-workflow-policy.mjs`.

The separate manual `Maintain generated Rust candidates` workflow is an explicit maintenance boundary. It requires confirmation, refuses tags and `main`, and restricts its write to canonical Rust source/lockfile paths. It is not normal implementation and does not replace direct source publication.

## Machine-readable qualification evidence

Every `Check` job, including an early failure, writes and uploads `ostudy-qualification-result.json`. The schema records:

- exact head SHA, gate, workflow/run/runner identity;
- normalized step outcomes;
- stable failure fingerprint;
- failing boundary and failed tests;
- bounded summary and classification hint;
- recommended diagnostic filter;
- focused reproduction command;
- whether transient evidence was retained.

Fingerprints ignore commit SHAs, durations, and incidental numbers so the same underlying defect can be recognized across repair commits. The classification hint is evidence to inspect, not an automatic authority.

After a failure:

1. verify the tested SHA and current PR head;
2. fetch job steps and decoded logs;
3. inspect the machine-readable result and bounded diagnostics;
4. identify/classify all independent failures;
5. run the focused reproduction;
6. repair a coherent batch;
7. publish/verify a new head;
8. rerun the narrow boundary before escalating.

An unchanged head may be rerun once only when evidence supports a transient external cause. The same fingerprint twice requires a strategy change before another broad run. See [Agent failure triage](../agents/FAILURE-TRIAGE.md).

## Failure evidence and diagnostics

Every scripted gate starts with the low-noise diagnostic baseline:

```text
warn,ostudy=info
```

This captures the original internal timeline without an expensive debug rerun. Nested wrappers share one **failure-capture token**, so the innermost precise boundary owns evidence and outer `initialization` traps do not duplicate it.

A capture includes:

- failed boundary and exit status;
- exact commit and active filter;
- recommended subsystem filter;
- focused executable `reproduce.sh`;
- bounded launcher/application JSONL tails;
- bounded isolated native-smoke logs where available.

Native WebDriver tests use temporary XDG homes. Final failures copy application logs before cleanup. A recovered **transient WebDriver** transport failure is retained even when the one clean-state retry succeeds; a green result does not erase flake evidence.

The Actions summary combines compiler/test output with bounded structured application events. Vitest failures are narrowed to named files; Rust failures are narrowed to an exact test when possible.

Small diagnostic/qualification artifacts use 7-day retention. Full failure wrappers, Gradle reports, emulator Logcat, screenshots, and UI hierarchy evidence use 14-day retention.

Diagnostics must never contain the learner SQLite library, cards/tags/templates, managed images, source PDFs, OCR/source text, provider prompts/responses, credentials, Keystore plaintext, or Authorization headers.

See [Diagnostics and debug filters](diagnostics.md).

## GitHub service incidents

**GitHub service incidents** are control-plane failures, not evidence that source passed or failed.

Classify before changing workflows or source:

- deterministic command/workflow assertion: repository or environment failure;
- stable endpoint-family `403` while repository reads/writes work: Connector capability boundary;
- broad repeated `5xx`, timeout, webhook, Actions, pull-request, archive/raw failures: suspected GitHub incident requiring official Status review.

During an incident:

1. preserve the exact head;
2. record first failing operation/timestamp/HTTP class/surface;
3. check official GitHub Status;
4. use only bounded idempotent reads to determine whether a write landed;
5. do not make no-op commits, toggle PR state, widen permissions, add status mirrors, or edit triggers merely to force acknowledgement;
6. do not infer Actions state from a missing check/webhook or empty legacy status list;
7. keep the PR draft and exact head unqualified;
8. after recovery, run the normal exact-head workflow without incident shims.

The [August 17, 2026 outage audit](../history/github-outage-audit-2026-08-17.md) records the incident that established this policy.

## Self-hosted runner security

Pull-request code executes inside the isolated VM with read-only repository contents permission.

Do not install Codex or another autonomous source-writing process on the runner. Do not provide host filesystem mounts, host SSH keys/agent forwarding, Docker/libvirt host-control sockets, release signing keys, or personal data.

The VM keeps a powered-off `pre-github-runner` snapshot from before runner registration. Restoring it removes registration state; register again with a new one-time token.

## Manual acceptance

Automation protects known contracts, not subjective visual/experiential quality or unavailable external systems. Manually exercise every changed user workflow on relevant platforms after automated qualification, including persistence, cancellation/error/restart, input methods, widths, themes, and assistive behavior.

Record what was actually exercised. Use `humanRequired` for a physical device, camera, live provider, private credential, or hardware boundary the agent cannot perform. Do not infer it from unit, mocked, emulator, or static coverage.

For the public-baseline cutover, preserve/restore an exact final-v26 development library and start the built desktop application with database diagnostics. Verify backup/adoption, existing decks/analytics, normal close/reopen, and absence of reset/panic/partial startup.

## Shadow, packaging, and releases

`.github/workflows/unified-runner-qualification.yml` remains opt-in runner-health/shadow qualification. It does not replace authoritative `Check` evidence.

`.github/workflows/android-package.yml` is manual-only and produces unsigned QA APK/AAB artifacts. `.github/workflows/release.yml` builds tagged Linux artifacts on the isolated runner without Android signing material.

For a release candidate, also run:

```bash
./scripts/release-check.sh
```

The release check repeats complete shared qualification from a clean source copy and verifies version/artifact assumptions. Local `.artifacts/` evidence is excluded from release source.
