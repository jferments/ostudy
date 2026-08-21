# Build and install ostudy for Android

The Android application uses the same React interface, Rust application logic, SQLite schema, scheduling engine, analytics, document pipeline, synchronization merge engine, import formats, and backup format as the desktop application. Kotlin is limited to Android APIs such as the document picker, Sharesheet, lifecycle, system Back, window/IME insets, connectivity, Android Keystore, PDF rasterization, ML Kit OCR, QR acquisition, and foreground services.

The short build/test/install command reference is [`../agents/COMMAND-RUNBOOK.md`](../agents/COMMAND-RUNBOOK.md). Prefer those maintained wrappers over hand-written Tauri, Gradle, or ADB command sequences.

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

## 1. Install the Android toolchain

Install the ordinary Linux ostudy prerequisites first, including JDK 17 and KVM support when you will use the emulator. From the repository root:

```bash
corepack enable
pnpm install --frozen-lockfile
pnpm android:install-deps
```

The repository wrapper installs/verifies the pinned Android platform, Build Tools, NDK, emulator, and API 36 Google APIs image. It also verifies the downloaded command-line-tools archive against the checksum in `versions.env`.

Run the environment doctor:

```bash
pnpm android:doctor
```

For emulator development and qualification:

```bash
pnpm android:doctor:emulator
```

The wrappers derive `ANDROID_HOME`, `ANDROID_SDK_ROOT`, `NDK_HOME`, and `JAVA_HOME`, then create the untracked `src-tauri/gen/android/local.properties` when needed. Do not commit `local.properties`; it contains a machine-specific SDK path.

Android Studio is useful for inspecting the generated project, Layout Inspector, Logcat, and device management, but it is not the canonical command-line build/install entrypoint.

## 2. Official emulator

Create the pinned API 36 virtual device once:

```bash
pnpm android:avd create
```

Start it visibly:

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

The official Android Emulator is the automated qualification environment. Informal containers/emulators can be useful for experimentation but do not replace the clean API-level, lifecycle, native-inset, OCR, and native-library checks in the maintained smoke suite.

## 3. Physical Android device

Enable Developer options and USB debugging on the phone/tablet, authorize the development computer, then confirm the device is visible:

```bash
adb devices -l
```

With exactly one authorized physical device connected, build, verify, install, and launch the normal ARM64 debug application:

```bash
pnpm android:install
```

When more than one device is connected, select one through the same wrapper:

```bash
pnpm android:install -- --serial <adb-serial>
```

If the debug APK has already been built and you only need to reinstall it:

```bash
pnpm android:install:existing -- --serial <adb-serial>
```

The maintained install wrapper deliberately uses non-streaming replacement installation (`adb install --no-streaming -r`). This is the canonical physical-device package-manager path because it proved reliable on the physical devices used during development. Do not fall back to streamed `adb install -r` merely because an ad-hoc command is shorter.

The debug application ID is `io.github.jferments.ostudy.dev`, so it can coexist with a future signed release package `io.github.jferments.ostudy`.

Generated APKs remain under:

```text
src-tauri/gen/android/app/build/outputs/apk/
```

## 4. Optional Android hot reload

Use Tauri/Vite development mode only when you specifically need live hot reload:

```bash
pnpm android:dev
```

This is not the normal physical-device acceptance path. Hot reload adds Tauri device-selection and host-networking requirements because the Android WebView must reach the development Vite server. A built debug APK installed with `pnpm android:install` does not have that dependency.

If hot reload fails, diagnose the maintained `android:dev` wrapper/Tauri boundary. Do not reconstruct a new sequence of `tauri android dev`, device arguments, host flags, Android Studio commands, or Vite bind addresses from memory.

For a local AI server, remember that network addresses are device-relative: the standard emulator reaches the development host through `10.0.2.2`; a physical phone normally needs the computer's reachable LAN address; `127.0.0.1` refers to the Android device itself.

## 5. Automated Android qualification

Compile/package the Android application and its Android-facing tests with:

```bash
pnpm android:test
```

Run the complete official API-36 emulator smoke suite with:

```bash
pnpm android:smoke
```

The smoke suite builds a clean x86_64 debug application with the explicitly opt-in emulator test bridge, installs the application/instrumentation packages, and verifies representative product behavior. It covers native inset/IME ownership, system Back, persistence, practice restoration, file-picker ownership/cancellation, and a real offline ML Kit recognition path.

The test bridge is excluded from normal debug builds unless the smoke build explicitly requests it, and package verification rejects that bridge in release artifacts.

For the shared desktop/Tauri gate:

```bash
./scripts/check.sh
```

For pull requests, the PR #60 state ledger and exact-head CI evidence are authoritative. A green run qualifies only the SHA it tested.

## 6. PDF processing and OCR

Android PDF processing uses the shared durable document pipeline with Android-specific native adapters:

- embedded PDF text/structure is inspected first;
- pages that need visual recovery are rasterized with Android `PdfRenderer`;
- selected rendered pages are recognized with bundled ML Kit Text Recognition v2;
- shared Rust owns page selection, byte/dimension limits, cancellation, checkpoints, source windows, provider requests, and final SQLite commits.

Android does **not** require Tesseract, Tesseract4Android, a Tesseract sidecar, or separately installed `tessdata`.

A user-started long-running PDF generation task acquires an Android foreground service with a visible processing notification and a partial wake lock. Activity backgrounding therefore does not make the WebView the lifetime owner of the operation. Full process death or a device restart still ends the process; durable checkpoints let ostudy recover and retry/resume when reopened.

## 7. Trusted-device synchronization

Open **Settings → Backup/Sync → Device synchronization** on both installations. Devices must be able to reach one another on the same private network.

Protocol version 2 treats every invitation as a one-way offer from the device that created it. On the sending device, choose whether to include API provider settings/saved credentials, then select **Create sync offer**. Android can select **Scan and receive**; once the QR code is detected, the scan itself accepts the offer and receiving starts immediately. Pasted invitation text uses **Accept sync offer**.

The receiver never has to generate a reverse invitation to complete that transfer, and accepting an offer does not upload the receiver's library to the sender. To send Android-side changes back later, intentionally create a new offer on Android and accept that offer on the desktop.

Shared Rust authenticates the devices, encrypts synchronization frames, validates the versioned package, performs common-base merging, and records explicit conflicts rather than silently discarding overlapping edits. General `app_settings` are excluded from both the outgoing package and the merge allowlist, so themes, fonts, UI scale, motion, tutorials, and other device-specific preferences remain local.

API provider profiles/task defaults are also excluded by default. They are included only when the sender enables the API-sharing option for that offer. Saved credential bytes use the authenticated encrypted channel and are written through Android Keystore rather than SQLite when the receiving secure store is available.

See [Synchronize trusted devices](../user-guide/device-sync.md) for the user workflow and [Device synchronization architecture](../architecture/device-sync.md) for protocol details.

## 8. Release artifacts

Build a release APK for direct distribution:

```bash
pnpm android:build:release
```

Build a multi-ABI Android App Bundle:

```bash
pnpm android:build:aab
```

An AAB is an application-store input, not a directly installable phone package. Release signing material must remain outside the repository. Do not commit a keystore, passwords, or `keystore.properties`.

The manual **Android package** workflow on the unified self-hosted runner can build the supported Android artifacts and records the exact source SHA in the uploaded artifact name.

## Android storage and permissions

ostudy does not request broad storage access. Android's Storage Access Framework supplies selected files through the system document picker; Kotlin streams them into ostudy's app-private staging directory, and shared Rust performs size, hash, content-type, package, and transaction validation.

Outgoing files are either saved with the system Create Document interface or shared through a narrowly scoped `FileProvider` and the Android Sharesheet. Files sent to ostudy by another application are cached privately and require explicit confirmation before import.

Android Auto Backup and device-transfer backup are disabled for the application sandbox. Use ostudy's complete `.ostudybackup` format, which creates a coherent SQLite and managed-file snapshot without copying Android Keystore material.

## Secure provider credentials

Secure AI-provider credentials use an AES-256-GCM key generated in Android Keystore. Encrypted records remain in app-private preferences excluded from backup. Session-only credentials disappear when Android kills the process. Credentials are never included in SQLite learner data, authoring drafts, deck exports, complete backups, diagnostics, or ordinary incoming/outgoing shares.

Device Sync can transfer selected secure credentials only when explicitly enabled by the sender for an authenticated encrypted LAN offer. File-based synchronization never contains credential bytes.

## Manual acceptance before merging an Android change

For changes that affect Android behavior, use the maintained install wrapper first, then verify the relevant workflows on the actual physical device being tested. Do not assume the device model from earlier testing.

Representative acceptance checks include:

1. The app header is below the status/cutout region and bottom navigation is above system navigation.
2. Editors remain reachable with the software keyboard visible.
3. Create/edit/archive/delete deck and card paths work.
4. Managed images survive restart.
5. Practice reveal/rating and Analytics work.
6. Background/resume and force-stop/relaunch preserve the intended durable state.
7. Import/export/backup use Android system pickers correctly.
8. Saved AI credentials remain available after restart and provider testing succeeds.
9. Digital-text and scanned PDFs use the intended extraction/OCR pipeline.
10. Longer PDF jobs keep their foreground-processing behavior and do not duplicate cards after cancel/retry.
11. Device-sync QR receive/send behavior works in the intended direction.
12. Portrait/landscape, IME resizing, and system Back behave correctly.

Choose the checks relevant to the change; the emulator suite remains the automated baseline.

## Troubleshooting

### Physical install fails

Start with:

```bash
adb devices -l
pnpm android:install -- --serial <adb-serial>
```

The wrapper already uses `--no-streaming -r`. Capture the complete `adb` package-manager error if installation still fails rather than switching to a different install mechanism.

### Emulator is slow or will not start

Run:

```bash
pnpm android:doctor:emulator
```

The regular qualification path requires KVM. A nested virtual machine may not expose hardware virtualization even when the outer host supports it; use the configured self-hosted runner or an appropriate development host.

### Android Studio cannot find the SDK

Run a repository Android wrapper once. It writes `src-tauri/gen/android/local.properties` from the selected SDK. You can then open `src-tauri/gen/android` in Android Studio for inspection if needed.

### A local AI server is unreachable

- emulator to development host: use `10.0.2.2`;
- physical phone: use the computer's reachable LAN address;
- do not use `127.0.0.1` unless the model server runs on Android itself;
- confirm the devices can communicate on the local network;
- LAN HTTP still requires ostudy's explicit insecure-local-network acknowledgement;
- cloud providers remain HTTPS-only.

### Retrieve Android diagnostics

With the debug device connected:

```bash
pnpm android:diagnostics
```

The diagnostics wrapper reads the debug app's bounded private log without copying credentials, document text, prompts, or other data excluded by the diagnostic redaction boundary.

### PDF OCR fails

No Tesseract installation is required on Android. Capture the document-processing phase and redacted error text, verify the page image is readable at normal zoom, and inspect Logcat for the native ML Kit adapter error. Very low-resolution scans, unusual scripts not covered by the bundled recognizer, extreme page dimensions, or malformed PDFs may still need preprocessing or a future script-specific model.
