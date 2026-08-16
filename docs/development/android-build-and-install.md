# Build and install ostudy for Android

The Android project is generated and maintained by Tauri under
`src-tauri/gen/android`. Use the repository wrapper so local and CI builds use
the same command sequence.

## Prerequisites

Run the normal Android environment setup and verify that `adb` can see the
phone or emulator:

```bash
adb devices
```

Enable USB debugging on a physical phone before installing a development APK.

## Debug APK

Build an installable debug APK:

```bash
node scripts/android-package.mjs build-debug
```

Build and install it on the connected device in one command:

```bash
node scripts/android-package.mjs install-debug
```

Install an APK that was already built or downloaded from CI:

```bash
node scripts/android-package.mjs install path/to/ostudy.apk
```

The wrapper prints the exact artifact path after a successful build. Generated
APKs and bundles remain under:

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
node scripts/android-package.mjs build-release
```

Build an Android App Bundle for an application store:

```bash
node scripts/android-package.mjs bundle-release
```

Release signing material must remain outside the repository. A release build
intended for distribution must be signed through the Android signing
configuration supplied by the release environment.

## CI artifact workflow

The **Android artifact** workflow runs only on the unified self-hosted runner.
It can be dispatched manually for a debug APK, release APK, or release AAB.
Version tags build the release AAB. The workflow uploads the exact source SHA in
the artifact name.

## Qualification before installation

Run the repository gates before treating an APK as ready:

```bash
./scripts/check-fast.sh
./scripts/android-test.sh
./scripts/android-smoke.sh
```

The authoritative pull-request gate also builds the Android package and runs the
API 36 emulator smoke suite on the unified runner.
