# Build and install ostudy for Android

The Android project is generated and maintained by Tauri under `src-tauri/gen/android`. Use the repository wrappers so local and CI builds use the same command sequence. The short canonical command reference is [`../agents/COMMAND-RUNBOOK.md`](../agents/COMMAND-RUNBOOK.md).

## Prerequisites

Install the repository's pinned Android toolchain when needed:

```bash
pnpm android:install-deps
```

Verify the Android environment:

```bash
pnpm android:doctor
```

For emulator qualification, use the stricter environment check:

```bash
pnpm android:doctor:emulator
```

For a physical phone, enable Developer options and USB debugging, authorize the development computer, then confirm the device is visible:

```bash
adb devices -l
```

## Physical-device debug install

The normal physical-device acceptance path is the maintained install wrapper, not `tauri android dev`.

With exactly one authorized device connected, build, verify, install, and launch the ARM64 debug APK:

```bash
pnpm android:install
```

If more than one device is connected, select one through the same wrapper:

```bash
pnpm android:install -- --serial <adb-serial>
```

Install the newest already-built debug APK without rebuilding:

```bash
pnpm android:install:existing -- --serial <adb-serial>
```

The install wrapper deliberately uses non-streaming replacement installation (`adb install --no-streaming -r`) because that is the reliable physical-device package-manager path established during development. Do not replace it with streamed `adb install -r` or an ad-hoc Tauri device-selection command when diagnosing an install problem; fix the maintained wrapper or the underlying device/package issue instead.

The normal debug application ID is `io.github.jferments.ostudy.dev`. The release application ID is `io.github.jferments.ostudy`.

Generated APKs and bundles remain under:

```text
src-tauri/gen/android/app/build/outputs/
```

List available local artifacts with:

```bash
node scripts/android-package.mjs artifacts
```

## Android hot reload

Use live Tauri/Vite Android development only when hot reload is specifically useful:

```bash
pnpm android:dev
```

This is an optional development path, not the physical-device acceptance/install procedure. Device selection and host networking are Tauri development concerns and can vary with the connected emulator/phone and local network. If `android:dev` fails, diagnose the maintained wrapper and Tauri development boundary rather than inventing a replacement command sequence. A successfully built and installed debug APK does not require the phone to reach the host Vite server.

## Automated qualification

Run the repository's Android compile/package qualification:

```bash
pnpm android:test
```

Run the official API-36 emulator smoke suite:

```bash
pnpm android:smoke
```

For the normal Linux/Tauri gate:

```bash
./scripts/check.sh
```

For pull requests, the PR #60 execution contract and exact-head CI evidence are authoritative. A ready-for-review PR runs the required Linux/Tauri and Android qualification on the unified self-hosted runner. Do not treat an APK or a local pass as qualifying a different commit SHA.

## Release artifacts

Build a release APK for direct distribution:

```bash
pnpm android:build:release
```

Build an Android App Bundle for an application store:

```bash
pnpm android:build:aab
```

An AAB is an application-store input, not a directly installable phone package. For direct installation use an APK. Release signing material must remain outside the repository; a release build intended for distribution must be signed through the Android signing configuration supplied by the release environment.

## Diagnostics

Retrieve the debug application's bounded private diagnostics with:

```bash
pnpm android:diagnostics
```

When multiple devices are attached and the diagnostics wrapper requires a target, use the documented serial option for that wrapper rather than manually reconstructing its `adb run-as` commands.

## CI package workflow

The manual **Android package** workflow runs only on the unified self-hosted runner. It can build a debug APK, release APK, or release AAB. Uploaded artifact names include the selected artifact kind and exact source SHA.

Pull-request qualification is deliberately not duplicated in a second Android workflow: `.github/workflows/check.yml` remains the single automatic PR gate and owns the evidence used for review readiness.
