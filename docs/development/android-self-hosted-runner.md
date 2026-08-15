# Android self-hosted runner notes

The former split Android runner topology is retired by the unified runner work.

Current CI uses the repository-scoped `ostudy-ci-unified` runner described in [Unified Linux and Android self-hosted runner](unified-linux-android-runner.md). Linux/Tauri and Android qualification execute serially in one isolated Ubuntu 24.04 libvirt VM, and the official Android Emulator uses nested KVM inside that VM.

## Why this file remains

Older ostudy revisions used:

- an ordinary Linux runner selected with the custom `ostudy` label; and
- a separate host-level Android runner selected with `ostudy`, `android`, and `kvm`.

That arrangement caused Android jobs to remain queued whenever no runner matched the exact split label set and duplicated setup/work across separate jobs. It is retained here only as migration context; do not configure new runners with those old label combinations.

## Current runner labels

The unified runner has the default Linux x64 labels plus one custom label:

```text
self-hosted
linux
x64
ostudy-unified
```

GitHub labels are routing assertions, not capability tests. `scripts/runner/guest-preflight.sh`, `scripts/android-doctor.sh`, and the Android emulator tooling verify the actual SDK/JDK/Rust/KVM state at runtime.

## Current Android qualification

Use:

```bash
./scripts/android-test.sh
./scripts/android-emulator-smoke.sh
```

The automatic `Check` workflow runs both Android gates after the authoritative Linux gate for ready pull requests and pushes to `main`. Manual Android packaging also targets `ostudy-unified`.

See [Testing and CI](testing-and-ci.md) for workflow behavior.

## Nested KVM qualification

The unified configuration is project-qualified on the specific `ML-tower` host and guest configuration:

- host AMD KVM with nested virtualization enabled;
- libvirt `host-passthrough` CPU model;
- usable `/dev/kvm` inside the guest for `ostudy-runner`;
- Android Emulator acceleration check passing inside the guest;
- pinned API 36 Google APIs x86_64 AVD;
- authoritative emulator smoke suite passing from the registered Actions service.

This is an empirical ostudy qualification, not a claim that every nested-virtualization environment is interchangeable. If a future kernel/QEMU/Emulator update breaks nested acceleration or Android behavior, use the pre-registration snapshot/rollback process in the unified runner documentation rather than weakening Android tests.

## Old runner retirement

Do not delete an old runner merely because the unified runner is online. Retire old registrations only after:

1. shadow `all-full` passes on the unified runner;
2. production workflows are routed to `ostudy-unified`;
3. the cutover PR passes exact-head ready-for-review qualification;
4. the cutover is merged; and
5. the `main` push completes `all-full` successfully on `ostudy-ci-unified`.

After that proof, unregister the old GitHub runners, stop their services, remove the old runner installation/VM after confirming it contains no unique data, and keep the host Android SDK if it is still useful for local development.
