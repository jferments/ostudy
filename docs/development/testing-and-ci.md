# Testing and CI

ostudy uses tiered validation locally and one isolated self-hosted Linux runner for automated Linux/Tauri and Android qualification. Inexpensive tiers help iteration; only the authoritative full gates qualify a merge.

## Runner architecture

Automated qualification runs on the repository-scoped `ostudy-ci-unified` runner with:

```text
self-hosted
linux
x64
ostudy-unified
```

The runner is an Ubuntu 24.04 libvirt/QEMU/KVM virtual machine on `ML-tower`. It runs Linux/Tauri checks directly and the official Android Emulator through nested KVM. The VM has no host filesystem share, no forwarded SSH agent, no Docker/libvirt host-control socket, no signing credentials, and no personal ostudy data.

See [Unified Linux and Android self-hosted runner](unified-linux-android-runner.md) for the architecture and [Unified runner setup](unified-runner-setup.md) for provisioning and recovery.

## Local validation tiers

Use the repository tiers in increasing order:

```bash
./scripts/check-changed.sh
./scripts/check-static.sh
./scripts/check-fast.sh
./scripts/check.sh
```

`check-changed.sh` is an iteration aid. It inspects the current diff and chooses a conservative focused check when one is safe.

`check-static.sh` verifies repository policy, manifests and lockfiles, migrations, typed IPC, dependency boundaries, documentation links, workflow policy, TypeScript checking, Rust formatting, startup-boundary source contracts, and script syntax/contracts.

`check-fast.sh` adds the complete frontend test suite, locked Rust compilation, warning-free Rust linting, and high-return document/platform contracts.

`check.sh` is the authoritative Linux/shared merge gate. It adds native dependency qualification, production frontend build, Clippy, all Rust tests, standalone Tauri build, dependency verification, a **built-binary pre-public adoption smoke**, and the native WebDriver persistence/UI smoke tests.

The adoption smoke is intentionally different from the ordinary WebDriver tests. WebDriver scenarios use fresh isolated app-data so they can verify normal first-run and persistence behavior. The adoption smoke instead creates a real on-disk database with the exact final development lineage: all 26 immutable migration identities/checksums, all 3 repair identities/checksums, the terminal v26 schema, the historical SQLite `user_version = 0` marker, and representative deck/card/review/analytics rows. The old migration runner never assigned `PRAGMA user_version`; the immutable ledgers are therefore the authoritative version record. A separate unit test also accepts an explicitly stamped `user_version = 26` copy when every other exact-lineage check matches. The smoke launches the exact built binary and requires ostudy to:

1. create a consistent SQLite backup;
2. adopt `ostudy-public-v1` transactionally;
3. preserve every application-table row count and the seeded deck/card/review/attribution rows;
4. emit `pre_public_database_adoption_completed` and `application_started`;
5. flush those startup/adoption events to the persistent JSONL log;
6. remain alive as a usable application;
7. avoid `application_startup_blocked`, `panic_captured`, and Tauri setup failure.

This directly covers the state that fresh WebDriver homes cannot exercise. Separate unit tests verify that unsupported pre-public databases are not modified or backed up automatically.

Native WebDriver element lookup is a read-only polling operation and tolerates a transient driver HTTP disconnect within its existing deadline. Mutating commands such as session creation, clicks, form submission, and actions are never automatically retried because the operation may already have been applied before its response was lost. Multi-stage smoke scenarios should assert already-established navigation state instead of issuing redundant clicks between adjacent layout checks. Responsive deck-toolbar checks therefore re-resolve the selected Flashcards route and Decks workspace after each resize rather than retaining element IDs across React layout replacement.

Successful check stamps are keyed by the exact source tree, relevant untracked files, tool versions, and platform. Set `OSTUDY_FORCE_CHECK=1` when deliberately bypassing exact-tree reuse. A failed run with `--keep-artifacts` writes a step-specific `reproduce.sh`; use that focused replay before repeating a broader tier.

### Worktree stability

Qualification must not mutate source-control-visible repository state. Static, fast, full, and Android qualification take a `git status --porcelain` snapshot before work begins and compare it at test boundaries. Pre-existing local changes are tolerated as part of the baseline, but a gate that creates a new untracked file or changes a tracked file fails immediately and prints the before/after Git status diff. This is specifically intended to catch generated-source rewrites and unexplained artifacts at the gate that created them instead of discovering them later.

Tauri's Android materializer is known to rewrite committed generated Android sources while constructing a build. `android-build.sh` therefore saves and restores the reviewed `AndroidManifest.xml` and `BuildTask.kt` versions on every exit. The worktree mutation guard is the backstop for any additional generated source that Tauri or Gradle begins changing in the future.

Local diagnostic captures use `.artifacts/diagnostics/`, which is ignored by Git and excluded from release-source copies. Diagnostic tooling must not create arbitrary root-level files as an alternative artifact path.

## Android qualification

Android uses the same shared application and domain code, so ready changes must pass both the Linux full gate and the Android gates.

Compile/package qualification:

```bash
./scripts/android-test.sh
```

This verifies the pinned SDK/NDK/JDK environment and generated Android project, runs the shared test contracts required by the Android build, compiles the Android Rust targets and Kotlin/Gradle project, builds the debug APK and instrumentation package, verifies package/permission/alignment constraints, and fails if the qualification changes Git-visible source state.

Official emulator qualification:

```bash
./scripts/android-emulator-smoke.sh
```

The smoke suite boots the pinned API 36 Google APIs x86_64 emulator using KVM acceleration and verifies native launch, SQLite migration, study workflows, persistence/restart behavior, Android Back, process restoration, and Android system document picker/import/export behavior. It uses a fresh AVD data image by default.

The smoke-only JavaScript bridge is enabled only for test APKs and is rejected by release package verification.

## Automatic GitHub Actions behavior

`.github/workflows/check.yml` is the single automatic qualification workflow. A single serialized job matches the one-runner topology and avoids having Linux and Android jobs queue behind each other while repeating checkout/setup work.

Automatic gates are intentionally conservative:

- draft pull request: `fast`;
- ready-for-review pull request: `all-full`;
- non-draft reopened/synchronized pull request: `all-full`;
- push to `main`: `all-full`.

Every non-`preflight` run executes **static qualification first** as the first substantive qualification boundary, before KVM/runner preflight, dependency-graph validation, fast, Linux-full, or Android work. The later tier reuses the exact-tree static stamp instead of repeating successful static work. A static failure therefore stops expensive product gates immediately and is attributed directly to the static step rather than appearing later as a nested initialization failure.

`all-full` runs, in order:

```text
check-static.sh
→ unified runner/KVM preflight
→ locked Rust dependency-graph validation
→ check.sh
   → fast qualification (reusing the static stamp)
   → built-binary final-v26 data-preserving adoption smoke
   → native WebDriver persistence/UI smoke
→ android-test.sh
→ android-emulator-smoke.sh
```

The same workflow supports manual dispatch with explicit `preflight`, `static`, `fast`, `linux-full`, `android-full`, and `all-full` gates.

The workflow uses the minimum normal permission:

```yaml
permissions:
  contents: read
```

The ordinary GitHub Actions check run, its exact checked-out SHA, step result, summary, and uploaded failure evidence are the authoritative remote qualification record. The workflow does not mirror Actions state through additional legacy commit-status API calls.

### GitHub service incidents

GitHub service incidents must be treated as control-plane failures, not as evidence that oStudy source code passed or failed.

Before changing workflow permissions, triggers, PR state, or unrelated source merely because a run is missing or an API read fails, classify the evidence:

- A completed local command or workflow log with a deterministic assertion is a repository or environment failure.
- A stable `403 Resource not accessible by integration` limited to an API family while repository reads/writes work is normally a connector permission or capability boundary.
- Repeated `5xx`, `502`, timeout, archive/raw download, Actions, webhook, pull-request, or broad API failures across unrelated operations require an official GitHub Status check.

During a confirmed or suspected GitHub incident:

1. preserve the exact branch head SHA;
2. record the first failing operation, timestamp, HTTP class, and affected GitHub surface;
3. check the official GitHub Status incident and affected components;
4. use only bounded idempotent reads to determine whether a requested write actually landed;
5. do not create no-op commits, edit unrelated files, close/reopen the PR, toggle draft state, widen token permissions, add status mirrors, or change triggers merely to force a workflow or webhook;
6. do not infer an Actions result from a missing check-run, webhook, or empty legacy status response;
7. keep the PR draft and describe the exact head as unqualified;
8. after GitHub reports recovery, run the normal workflow on that exact head without incident-specific shims.

The [August 17, 2026 GitHub outage audit](../history/github-outage-audit-2026-08-17.md) records the incident that established this policy and distinguishes legitimate ostudy fixes from outage-driven workflow changes that were removed.

Every scripted CI gate automatically starts with the low-noise diagnostic filter:

```text
warn,ostudy=info
```

This captures the original internal operation timeline without an additional qualification run. A failure summary then combines wrapper/compiler output with bounded structured events and emits the smallest useful reproduction command. CI does not automatically rerun the failed gate at debug level: doing so consumes substantial runner time, may erase isolated state, and can convert nondeterministic defects into false greens.

Concurrency cancellation replaces obsolete runs for the same pull request/ref. A green result qualifies only the exact head SHA it executed.

The former automatic `android-check.yml` split workflow is retired. On a single runner, separate automatic Linux/Android jobs only create additional queueing and repeated setup. Android qualification is now part of the authoritative serialized `Check` job.

## Shadow and runner-health qualification

`.github/workflows/unified-runner-qualification.yml` is a manual-only runner-health workflow. It targets only `ostudy-unified` and can execute:

```text
preflight
linux-fast
linux-full
android-full
all-full
```

Choose the branch or exact ref and lane explicitly through `workflow_dispatch`. Pull-request body edits and persistent markers never start this workflow, and its result does not replace the authoritative `Check` result.

## Packaging and releases

`.github/workflows/android-package.yml` is manual-only and builds unsigned QA APK/AAB artifacts on `ostudy-unified`.

`.github/workflows/release.yml` builds tagged Linux artifacts on the same isolated runner. It does not contain Android signing material or release secrets. Public Android distribution still requires a separately reviewed signing/Play App Signing design.

## Failure evidence

Qualification failures preserve bounded diagnostics rather than user study data.

Nested qualification wrappers share one failure-capture token, so the innermost precise failure wins and outer `initialization` traps do not create duplicate evidence. Each failing script writes its authoritative capture to the runner-temporary `OSTUDY_DIAGNOSTIC_ARTIFACT_ROOT`. A capture includes:

- the failed boundary and exit status;
- the exact commit and active filter;
- a recommended subsystem-specific filter;
- an executable focused `reproduce.sh`;
- bounded launcher/application JSONL tails when they exist;
- a bounded command log for isolated native smoke failures.

Native WebDriver smoke tests use temporary XDG homes. Their application logs are copied before cleanup. A retryable transport disconnect is retained as **transient WebDriver** evidence even when the one clean-state retry succeeds, so a green check does not erase the existence of a recovered flake.

The built-binary adoption smoke also uses isolated XDG state, but deliberately seeds an exact final-v26 database rather than a clean library. A failure captures its command/application logs before cleanup so backup, adoption, preservation, startup, and panic regressions are visible in the same diagnostic workflow.

The Actions summary parses application JSONL and structured Android Logcat records, showing the final warning/error timeline next to the ordinary test/compiler excerpt. Vitest failures are narrowed to the identified test file; Rust failures are narrowed to the exact failed test when possible.

Small structured diagnostic captures are uploaded with 7-day retention even when they contain only a recovered transient failure. Full failure wrapper logs, Gradle reports, emulator Logcat, screenshot, and UI hierarchy evidence use 14-day retention. A clean run with no capture does not produce a diagnostic artifact.

Diagnostics must not contain the SQLite learner library, cards, tags, templates, managed images, source PDFs, OCR/source text, provider prompts/responses, credentials, Keystore plaintext, or Authorization headers.

See [Diagnostics and debug filters](diagnostics.md) for local replay and filter escalation.

## Self-hosted runner security

Pull-request qualification uses read-only repository contents permission and executes untrusted repository code inside the isolated VM rather than on `ML-tower`.

Do not install Codex or another autonomous code-writing process on the runner. Do not give the runner host filesystem mounts, the host SSH private key, forwarded agent access, Docker/libvirt host-control sockets, release signing keys, or personal data.

The VM keeps a powered-off `pre-github-runner` snapshot from before runner registration. Restoring that snapshot removes the runner registration state; register it again with a new one-time GitHub token after a restore.

## Manual acceptance

Automation protects known contracts, not visual or experiential quality. For user-visible work, manually exercise the affected workflow on the relevant platform after automated qualification, including error/cancellation/restart behavior when applicable.

For the public-baseline cutover, retain or restore an exact final-v26 development library and start the built desktop application with database diagnostics. The expected result is automatic backup/adoption followed by normal startup—not a blank library, reset dialog, Rust panic, or partially initialized application. Verify existing decks and Analytics, close the app, reopen it, and verify them again.

If the earlier reset helper has already archived the old app-data directory, use:

```bash
python3 ./scripts/restore_pre_public_backup.py \
  --backup-dir "/absolute/path/to/io.github.jferments.ostudy.pre-public-<timestamp>" \
  --data-dir "/absolute/path/to/io.github.jferments.ostudy" \
  --confirm
```

The recovery helper validates the exact final migration/repair ledgers and accepted historical SQLite marker, copies the archive back, archives the currently active directory, and leaves the original timestamped archive untouched. The next startup performs the verified adoption. `reset-pre-public-data.sh --confirm-empty-library` is only for an intentional blank-library test.

A real Google Code Scanner camera scan on a Google Play services-enabled Android device and a live OpenRouter request remain manual acceptance boundaries because deterministic CI has neither a physical camera workflow nor a provider credential.

## Release qualification

For a release candidate, also run:

```bash
./scripts/release-check.sh
```

The release check repeats the complete shared qualification from a clean source copy and verifies version/artifact assumptions. Local `.artifacts/` diagnostics are excluded from that clean source copy.