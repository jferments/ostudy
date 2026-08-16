# Build and install ostudy for Android

The Android project is generated and maintained by Tauri under
`src-tauri/gen/android`. Use the repository wrappers so local and CI builds use
the same command sequence.

## Prerequisites

Install the repository's pinned Android toolchain when needed:

```bash
pnpm android:install-deps
```

Verify the Android environment and confirm that `adb` can see the phone or
emulator:

```bash
pnpm android:doctor
adb devices
```

Enable Developer options and USB debugging on a physical phone before
installing a development APK. Accept the computer's USB-debugging authorization
prompt on the phone the first time it connects.

## Debug APK

Build the normal ARM64 debug APK used by current Android phones:

```bash
pnpm android:build:debug
```

Build, verify, install, and launch it on the single connected phone:

```bash
pnpm android:install
```

If more than one device is connected, select one explicitly:

```bash
bash ./scripts/android-install.sh --debug --target aarch64 --serial <adb-serial>
```

Install the newest already-built debug APK without rebuilding:

```bash
pnpm android:install:existing
```

The lower-level packaging wrapper can also install a specific APK that was
downloaded from CI:

```bash
node scripts/android-package.mjs install path/to/ostudy.apk
```

The normal debug application ID is `io.github.jferments.ostudy.dev`. The
release application ID is `io.github.jferments.ostudy`.

Generated APKs and bundles remain under:

```text
src-tauri/gen/android/app/build/outputs/
```

List the available local artifacts with:

```bash
node scripts/android-package.mjs artifacts
```

## Release artifacts

Build a release APK for direct distribution:

```bash
pnpm android:build:release
```

Build an Android App Bundle for an application store:

```bash
pnpm android:build:aab
```

An AAB is an application-store input, not a directly installable phone package.
For direct installation use an APK. Release signing material must remain outside
the repository; a release build intended for distribution must be signed through
the Android signing configuration supplied by the release environment.

## CI artifact workflow

The **Android artifact** workflow runs only on the unified self-hosted runner.
It can be dispatched manually for a debug APK, release APK, or release AAB.
Version tags build the release AAB. The workflow includes the exact source SHA in
the artifact name.

## Qualification before installation

For a quick local source gate:

```bash
pnpm check:fast
```

For Android compile/package qualification and the API 36 emulator smoke suite:

```bash
pnpm android:test
pnpm android:smoke
```

For the complete shared Linux/Tauri qualification as well:

```bash
pnpm check
```

A ready-for-review pull request runs the authoritative shared and Android gates
on the unified self-hosted runner. Do not treat an APK as a release candidate
until those exact-head checks are green.
