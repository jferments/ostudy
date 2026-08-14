# Build and install ostudy for Android

The Android application uses the same React interface, Rust application logic, SQLite schema, scheduling engine, analytics, import formats, and backup format as the desktop application. Kotlin is limited to Android APIs such as the document picker, Sharesheet, lifecycle, system Back, window insets, connectivity, and Android Keystore.

Phases A1–A3 include deck/card authoring, practice, analytics, managed images, deck and JSON import, deck/backup export, incoming and outgoing system sharing, complete backup/restore, durable editor recovery, interrupted-practice restoration, and secure AI-provider credentials. PDF processing, OCR, and AI document generation intentionally remain a development stub on Android.

## Supported development baseline

- Ubuntu or another Linux development host;
- JDK 17;
- Android SDK platform 36;
- Android Build Tools 36.0.0;
- Android NDK 28.2.13676358;
- Android 9/API 28 minimum application level;
- Android 16/API 36 target level;
- official Android Emulator or a physical arm64 Android device;
- Rust targets for arm64, armv7, x86, and x86_64 Android.

The exact project pins are in [`../../versions.env`](../../versions.env). Do not select an arbitrary newer NDK or SDK merely because Android Studio offers one; update the repository pins and qualification together.

## 1. Install host prerequisites

Install the ordinary Linux ostudy prerequisites first. Then install a complete JDK 17. On Ubuntu, a typical package is:

```bash
sudo apt update
sudo apt install openjdk-17-jdk unzip qemu-kvm
```

For accelerated emulator use, add your account to the KVM group and sign out/in:

```bash
sudo usermod -aG kvm "$USER"
```

Confirm that `/dev/kvm` is readable and writable after signing back in.

Android Studio is recommended for inspecting the generated project, Layout Inspector, Logcat, and device management, but the command-line build does not depend on the IDE. The repository downloads the pinned Android command-line tools into an ostudy-owned cache unless `ANDROID_HOME` already points to another SDK.

## 2. Install the pinned Android SDK and Rust targets

From the repository root:

```bash
pnpm install --frozen-lockfile
pnpm android:install-deps

rustup target add \
  aarch64-linux-android \
  armv7-linux-androideabi \
  i686-linux-android \
  x86_64-linux-android
```

`android:install-deps` installs the pinned platform, Build Tools, NDK, emulator, and API 36 Google APIs system image. The script verifies the downloaded command-line tools archive against the SHA-256 value in `versions.env`.

Run the environment doctor:

```bash
pnpm android:doctor
```

For emulator development, use the stricter check:

```bash
pnpm android:doctor:emulator
```

The scripts derive and export `ANDROID_HOME`, `ANDROID_SDK_ROOT`, `NDK_HOME`, and `JAVA_HOME`, then create the untracked `src-tauri/gen/android/local.properties` file when needed. Do not commit `local.properties`; it contains a machine-specific SDK path.

## 3. Create and boot the official emulator

Create the pinned API 36 virtual device once:

```bash
pnpm android:avd create
```

Start it with a visible emulator window:

```bash
pnpm android:avd start
```

For CI or a remote machine:

```bash
pnpm android:avd start --headless --wipe-data
```

Check status or stop it:

```bash
pnpm android:avd status
pnpm android:avd stop
```

The official Android Emulator is the qualification environment. Waydroid can be useful for a quick informal installation check on Linux, but it does not replace the API-level, clean-data, process-lifecycle, and 16 KiB native-library checks performed by the emulator suite.

## 4. Run ostudy in Android development mode

With an emulator or USB-debugging device connected:

```bash
pnpm android:dev
```

Select a particular device when more than one is attached:

```bash
bash ./scripts/android-dev.sh --device emulator-5554
```

For a physical phone, enable Developer Options and USB debugging, authorize the computer, and confirm it appears under:

```bash
adb devices -l
```

Hot development requires the phone to reach the Vite server on the development computer. Supply the computer's LAN address when automatic selection is insufficient:

```bash
bash ./scripts/android-dev.sh --device <serial> --host 192.168.1.25
```

Allow the development port through the local firewall. `localhost` inside Android means the Android device itself, not the development computer. The standard Android Emulator can reach the host through `10.0.2.2` when testing a local AI server.

## 5. Build an installable debug APK

For a physical arm64 phone:

```bash
pnpm android:build:debug
```

For the x86_64 emulator:

```bash
pnpm android:build:emulator
```

Generated APKs are under:

```text
src-tauri/gen/android/app/build/outputs/apk/
```

Install the newest debug APK:

```bash
apk="$(find src-tauri/gen/android/app/build/outputs/apk -type f -name '*.apk' \
  ! -path '*/androidTest/*' -printf '%T@ %p\n' | sort -nr | awk 'NR == 1 { $1=""; sub(/^ /, ""); print }')"
adb install -r -t "$apk"
```

The debug build uses application ID `io.github.jferments.ostudy.dev`, so it can coexist with a future signed release build. Its app-private data is separate from the release package.

## 6. Run automated Android qualification

Compile the Android app, Kotlin tests, instrumentation package, shared TypeScript tests, and Rust tests:

```bash
pnpm android:test
```

Run the complete official-emulator smoke suite:

```bash
pnpm android:smoke
```

The smoke suite starts a clean API 36 AVD, builds an x86_64 APK with an explicitly opt-in test bridge, installs the application and instrumentation APK, creates a deck/card, performs a review, restarts the app, verifies persistence and analytics, checks the Android Documents development stub and system Back behavior, and restores an interrupted revealed practice session after reload and process restart.

Diagnostics are written to:

```text
artifacts/android-smoke/
```

The test bridge is not present in normal debug builds unless `--smoke-bridge` is requested, and the package verifier rejects it in a release APK.

## 7. Build a release bundle

Build an unsigned multi-ABI Android App Bundle:

```bash
pnpm android:build:aab
```

The `.aab` is placed under:

```text
src-tauri/gen/android/app/build/outputs/bundle/
```

A release intended for Google Play still requires a protected signing keystore and release workflow configuration. Do not commit a keystore, passwords, or `keystore.properties`. The current workflow deliberately produces unsigned QA artifacts until the project is ready to establish its permanent release key and Play App Signing configuration.

## Android storage and permissions

ostudy does not request broad storage access. Android's Storage Access Framework supplies selected files through the system document picker; Kotlin streams them into ostudy's app-private staging directory, and shared Rust performs size, hash, content-type, package, and transaction validation.

Outgoing files are either saved with the system Create Document interface or shared through a narrowly scoped `FileProvider` and the Android Sharesheet. Files sent to ostudy by another application are cached privately and require explicit confirmation before import.

Android Auto Backup and device-transfer backup are disabled for the application sandbox. Use ostudy's complete `.ostudybackup` format, which creates a coherent SQLite and managed-file snapshot without copying Android Keystore material.

## Secure provider credentials

Secure AI-provider credentials use an AES-256-GCM key generated in Android Keystore. Encrypted records remain in app-private preferences excluded from backup. Session-only credentials still disappear when Android kills the process. Credentials are never included in SQLite learner data, authoring drafts, deck exports, complete backups, diagnostics, or incoming/outgoing shares.

## Manual acceptance before merging an Android change

1. Create, edit, archive, and delete decks and cards.
2. Add a managed image to a card and template; restart and verify rendering.
3. Start practice, reveal and rate cards, then verify Analytics.
4. Background and resume during editing and practice.
5. Force-stop and relaunch during an unfinished card/template/bulk draft.
6. Force-stop and relaunch during revealed-but-unrated practice.
7. Import a `.studydeck` and generated `.ostudy.json` through the picker.
8. Save and restore an `.ostudybackup` through a document provider.
9. Save and share a deck through Android's system interfaces.
10. Send a supported file to ostudy from Files and confirm before import.
11. Save a provider credential, restart, and test the provider profile.
12. Confirm that Documents shows the Android development placeholder and performs no PDF/OCR work.
13. Rotate, resize, test landscape, and test the software keyboard.
14. Verify system Back closes overlays/nested screens and returns home from the root.

## Troubleshooting

### The emulator is very slow or will not start

Run:

```bash
pnpm android:doctor:emulator
```

The regular test path requires KVM. A nested virtual machine may not expose hardware virtualization even when the outer host supports it; use a host-level self-hosted runner or physical device in that case.

### Android Studio cannot find the SDK

Run any repository Android script once. It writes `src-tauri/gen/android/local.properties` from the selected `ANDROID_HOME`. You can also open `src-tauri/gen/android` directly in Android Studio after that file exists.

### A local AI server is unreachable

- emulator to development host: use `10.0.2.2`;
- physical phone: use the computer's LAN address;
- do not use `127.0.0.1` unless the server runs on Android itself;
- confirm phone and host are on the same network;
- LAN HTTP still requires ostudy's explicit insecure-local-network acknowledgement;
- cloud providers remain HTTPS-only.

### A document provider import fails

Retry with a local Downloads copy and inspect Logcat. ostudy deliberately rejects empty, oversized, changing, malformed, or unsupported content even when the provider supplied a plausible filename or MIME type.
