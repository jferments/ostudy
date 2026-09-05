# Build and install ostudy for Android

The Android project is generated/maintained by Tauri under `src-tauri/gen/android`. Use repository wrappers so local and CI builds share the same pinned environment and package checks. The short command authority is [`../agents/COMMAND-RUNBOOK.md`](../agents/COMMAND-RUNBOOK.md).

## Prerequisites

Install/verify the pinned Android toolchain when needed:

```bash
pnpm android:install-deps
pnpm android:doctor
```

For emulator qualification:

```bash
pnpm android:doctor:emulator
```

For a physical phone/tablet, enable Developer options and USB debugging, authorize the development computer, then inspect devices:

```bash
adb devices -l
```

## Physical-device debug install

With exactly one authorized compatible device connected:

```bash
pnpm android:install
```

This builds/verifies the normal ARM64 debug APK, installs it with non-streaming replacement installation, and launches `io.github.jferments.ostudy.dev`.

When multiple devices are attached, call the maintained Bash wrapper directly because its argument parser accepts `--serial` but does **not** accept an extra standalone `--` sentinel:

```bash
bash ./scripts/android-install.sh --debug --target aarch64 --serial '<adb-serial>'
```

To install an existing debug APK without rebuilding:

```bash
bash ./scripts/android-install.sh --debug --no-build --serial '<adb-serial>'
```

The no-build path selects the newest matching APK by modification time. It does not attest the Git SHA or target provenance of that artifact, so use a fresh normal build/install for PR acceptance unless provenance is independently known.

The wrapper deliberately uses:

```text
adb install --no-streaming -r
```

Do not replace it with streamed installation or an ad-hoc Tauri device command merely because that command is shorter. Diagnose/fix the maintained wrapper or package/device problem.

Debug and release application IDs are separate:

```text
io.github.jferments.ostudy.dev
io.github.jferments.ostudy
```

A replacement install preserves the application's current data. It is not a fresh-profile test. Uninstalling/clearing data can destroy that installation's library and requires explicit consent/backup.

Generated Android artifacts live under:

```text
src-tauri/gen/android/app/build/outputs/
```

## Android hot reload

Use live Tauri/Vite Android development only when hot reload is specifically useful:

```bash
pnpm android:dev
```

This is not the ordinary physical-device acceptance path. A built debug APK does not require the phone to reach the host Vite server.

## Automated qualification

```bash
pnpm android:test
pnpm android:smoke
```

The first is compile/package qualification. The second runs the maintained API 36 emulator smoke suite. They prove different boundaries.

The shared Linux/Tauri gate is:

```bash
./scripts/check.sh
```

For pull requests, GitHub's exact PR head and `Check` evidence are authoritative. A local APK or pass cannot qualify another SHA. `PR Contract` metadata validation is independent from product qualification.

## Release artifacts

```bash
pnpm android:build:release
pnpm android:build:aab
```

An AAB is store input, not a directly installable phone package. Release signing material remains outside the repository. The manual Android package workflow can produce unsigned QA artifacts; public Android distribution still needs the separately reviewed signing/Play process.

## Diagnostics

```bash
pnpm android:diagnostics
```

When multiple devices are connected, follow that wrapper's own supported target/serial options rather than reconstructing its `adb run-as` calls.

## Manual acceptance

Use the maintained install wrapper and test only the behavior relevant to the change. Common manual-only boundaries include physical camera/Google Code Scanner QR acquisition, OEM file providers, real touch/IME/safe-area behavior, battery/thermal pressure, screen readers, and live provider/network conditions. Emulator qualification must not be reported as evidence for those unexercised paths.
