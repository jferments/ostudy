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

`check-static.sh` verifies repository policy, manifests and lockfiles, migrations, typed IPC, dependency boundaries, documentation links, workflow policy, TypeScript checking, Rust formatting, and script syntax/contracts.

`check-fast.sh` adds the complete frontend test suite, locked Rust compilation, and high-return document/platform contracts.

`check.sh` is the authoritative Linux/shared merge gate. It adds native dependency qualification, production frontend build, Clippy, all Rust tests, standalone Tauri build, dependency verification, and the native WebDriver persistence smoke test.

Native WebDriver element lookup is a read-only polling operation and tolerates a transient driver HTTP disconnect within its existing deadline. Mutating commands such as session creation, clicks, form submission, and actions are never automatically retried because the operation may already have been applied before its response was lost. Multi-stage smoke scenarios should also assert already-established navigation state instead of issuing redundant clicks between adjacent layout checks. Responsive deck-toolbar checks therefore re-resolve the selected Flashcards route and Decks workspace after each resize rather than retaining element IDs across React layout replacement.

Successful check stamps are keyed by the exact source tree, relevant untracked files, tool versions, and platform. Set `OSTUDY_FORCE_CHECK=1` when deliberately bypassing exact-tree reuse.

## Android qualification

Android uses the same shared application and domain code, so ready changes must pass both the Linux full gate and the Android gates.

Compile/package qualification:

```bash
./scripts/android-test.sh
```

This verifies the pinned SDK/NDK/JDK environment and generated Android project, runs the shared test contracts required by the Android build, compiles the Android Rust targets and Kotlin/Gradle project, builds the debug APK and instrumentation package, and verifies package/permission/alignment constraints.

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

`all-full` runs, in order:

```text
unified runner preflight
→ check.sh
→ android-test.sh
→ android-emulator-smoke.sh
```

The same workflow supports manual dispatch with explicit `preflight`, `static`, `fast`, `linux-full`, `android-full`, and `all-full` gates.

Concurrency cancellation replaces obsolete runs for the same pull request/ref. A green result qualifies only the exact head SHA it executed.

The former automatic `android-check.yml` split workflow is retired. On a single runner, separate automatic Linux/Android jobs only create additional queueing and repeated setup. Android qualification is now part of the authoritative serialized `Check` job.

## Shadow and runner-health qualification

`.github/workflows/unified-runner-qualification.yml` remains an opt-in runner-health/shadow workflow. It targets only `ostudy-unified` and can execute:

```text
preflight
linux-fast
linux-full
android-full
all-full
```

During infrastructure work, one explicit PR-body marker may activate a lane. Normal product pull requests do not need a marker and the shadow job does not replace the authoritative `Check` result.

## Packaging and releases

`.github/workflows/android-package.yml` is manual-only and builds unsigned QA APK/AAB artifacts on `ostudy-unified`.

`.github/workflows/release.yml` builds tagged Linux artifacts on the same isolated runner. It does not contain Android signing material or release secrets. Public Android distribution still requires a separately reviewed signing/Play App Signing design.

## Failure evidence

Qualification failures preserve bounded diagnostics rather than user study data.

Linux/shared failures may upload the standard diagnostic archive. Android failures may also retain Gradle reports plus emulator Logcat, screenshot, and UI hierarchy artifacts generated by the existing Android smoke tooling.

Diagnostics must not contain the SQLite learner library, cards, tags, templates, managed images, source PDFs, OCR/source text, provider prompts/responses, credentials, Keystore plaintext, or Authorization headers.

## Self-hosted runner security

Pull-request jobs use minimum GitHub token permissions and execute untrusted repository code inside the isolated VM rather than on `ML-tower`.

Do not install Codex or another autonomous code-writing process on the runner. Do not give the runner host filesystem mounts, the host SSH private key, forwarded agent access, Docker/libvirt host-control sockets, release signing keys, or personal data.

The VM keeps a powered-off `pre-github-runner` snapshot from before runner registration. Restoring that snapshot removes the runner registration state; register it again with a new one-time GitHub token after a restore.

## Manual acceptance

Automation protects known contracts, not visual or experiential quality. For user-visible work, manually exercise the affected workflow on the relevant platform after automated qualification, including error/cancellation/restart behavior when applicable.

## Release qualification

For a release candidate, also run:

```bash
./scripts/release-check.sh
```

The release check repeats the complete shared qualification from a clean source copy and verifies version/artifact assumptions.
