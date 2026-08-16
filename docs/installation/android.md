# Build and install ostudy for Android

The Android application uses the same React interface, Rust application logic, SQLite schema, scheduling engine, analytics, document pipeline, synchronization merge engine, import formats, and backup format as the desktop application. Kotlin is limited to Android APIs such as the document picker, Sharesheet, lifecycle, system Back, window/IME insets, connectivity, Android Keystore, PDF rasterization, ML Kit OCR, QR acquisition, and foreground services.

The current Android build supports ordinary study/authoring features plus PDF import/generation, bundled ML Kit document OCR, encrypted same-LAN device synchronization, QR pairing, and optional secure AI-credential transfer.

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

The official Android Emulator is the qualification environment. Waydroid can be useful for a quick informal installation check on Linux, but it does not replace the API-level, clean-data, process-lifecycle, native-inset, OCR, and native-library checks performed by the emulator suite.

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

## 5. Build and install a debug APK

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

The maintained install wrapper builds, verifies, installs, and launches the normal debug application:

```bash
pnpm android:install
```

If more than one device is connected:

```bash
bash ./scripts/android-install.sh --debug --target aarch64 --serial <adb-serial>
```

Install an already-built debug APK without rebuilding:

```bash
pnpm android:install:existing
```

The install wrapper deliberately excludes instrumentation APKs from artifact discovery. The debug build uses application ID `io.github.jferments.ostudy.dev`, so it can coexist with a future signed release build. Its app-private data is separate from the release package.

## 6. Run automated Android qualification

Compile the Android app, Kotlin tests, instrumentation package, shared TypeScript tests, and Rust tests:

```bash
pnpm android:test
```

Run the complete official-emulator smoke suite:

```bash
pnpm android:smoke
```

The smoke suite starts a clean API 36 AVD, builds an x86_64 APK with an explicitly opt-in test bridge, installs the application and instrumentation APK, and verifies representative application behavior. Native instrumentation also verifies system/cutout inset ownership, `adjustResize` keyboard behavior, and a real offline ML Kit recognition pass against a synthetic high-contrast text image.

The UI smoke creates a deck/card, performs a review, restarts the app, verifies persistence and analytics, checks Android Back behavior, restores an interrupted practice session, exercises document-picker cancellation, and verifies native picker ownership is released between operations.

Diagnostics are written to:

```text
artifacts/android-smoke/
```

The test bridge is not present in normal debug builds unless `--smoke-bridge` is requested, and the package verifier rejects it in a release APK.

## 7. PDF processing and OCR

Android PDF processing uses the shared durable document pipeline with Android-specific native adapters:

- embedded PDF text/structure is inspected first;
- pages that need visual recovery are rasterized with Android `PdfRenderer`;
- selected rendered pages are recognized with bundled ML Kit Text Recognition v2;
- shared Rust owns page selection, byte/dimension limits, cancellation, checkpoints, source windows, provider requests, and final SQLite commits.

Android does **not** require Tesseract, Tesseract4Android, a Tesseract sidecar, or separately installed `tessdata`.

A user-started long-running PDF generation task acquires an Android foreground service with a visible processing notification and a partial wake lock. Activity backgrounding therefore does not make the WebView the lifetime owner of the operation. Full process death or a device restart still ends the process; durable checkpoints let ostudy recover and retry/resume when reopened.

## 8. Trusted-device synchronization

Open **Settings → Device Sync** on both installations. Devices must be able to reach one another on the same private network.

The host creates a short-lived QR invitation containing the session endpoint and authenticated key-agreement material. Android scans the invitation with the embedded QR-only scanner. Shared Rust authenticates the devices, encrypts synchronization frames, validates the versioned package, performs common-base merging, and records explicit conflicts rather than silently discarding overlapping edits.

Protocol version 1 sends one snapshot per QR session. For two-way convergence, run a session in one direction, resolve conflicts, then create a fresh QR session in the reverse direction.

File fallback uses the same snapshot/merge rules but excludes API credentials. Optional credential transfer is disabled by default and is available only inside the authenticated encrypted LAN session; accepted keys are written through Android Keystore rather than SQLite.

See [Synchronize trusted devices](../user-guide/device-sync.md) for the user workflow and [Device synchronization architecture](../architecture/device-sync.md) for protocol details.

## 9. Build release artifacts

Build a release APK for direct distribution:

```bash
pnpm android:build:release
```

Build a multi-ABI Android App Bundle:

```bash
pnpm android:build:aab
```

Release outputs are under the generated Gradle `build/outputs` tree. An AAB is an application-store input, not a directly installable phone package.

A release intended for distribution still requires a protected signing keystore and release workflow configuration. Do not commit a keystore, passwords, or `keystore.properties`. Signing material is deliberately unavailable to pull-request jobs.

The manual **Android package** workflow on the unified self-hosted runner can build a debug APK, release APK, or release AAB and uploads the selected artifact with the exact source SHA in its artifact name.

## Android storage and permissions

ostudy does not request broad storage access. Android's Storage Access Framework supplies selected files through the system document picker; Kotlin streams them into ostudy's app-private staging directory, and shared Rust performs size, hash, content-type, package, and transaction validation.

Outgoing files are either saved with the system Create Document interface or shared through a narrowly scoped `FileProvider` and the Android Sharesheet. Files sent to ostudy by another application are cached privately and require explicit confirmation before import.

Android Auto Backup and device-transfer backup are disabled for the application sandbox. Use ostudy's complete `.ostudybackup` format, which creates a coherent SQLite and managed-file snapshot without copying Android Keystore material.

## Secure provider credentials

Secure AI-provider credentials use an AES-256-GCM key generated in Android Keystore. Encrypted records remain in app-private preferences excluded from backup. Session-only credentials still disappear when Android kills the process. Credentials are never included in SQLite learner data, authoring drafts, deck exports, complete backups, diagnostics, or ordinary incoming/outgoing shares.

Device Sync can transfer selected secure credentials only when explicitly enabled for an authenticated encrypted LAN session. File-based synchronization never contains credential bytes.

## Manual acceptance before merging an Android change

1. Confirm the app header is below the Android status/cutout region and the bottom ostudy navigation is above gesture/three-button system navigation.
2. Open Create Deck and other representative editors; show the software keyboard and confirm the focused field and actions remain reachable.
3. Create, edit, archive, and delete decks and cards.
4. Add a managed image to a card and template; restart and verify rendering.
5. Start practice, reveal and rate cards, then verify Analytics.
6. Background and resume during editing and practice.
7. Force-stop and relaunch during an unfinished card/template/bulk draft and during revealed-but-unrated practice.
8. Import a `.studydeck` and generated `.ostudy.json` through the picker.
9. Save and restore an `.ostudybackup` through a document provider.
10. Save and share a deck through Android's system interfaces, and send a supported file to ostudy from Files.
11. Save a provider credential, restart, and test the provider profile.
12. Import a digital-text PDF and confirm generation succeeds without unnecessary page OCR.
13. Import a scanned/image-only PDF and confirm ML Kit OCR recovers usable text and generation can complete.
14. Start a longer PDF job, background the app, and verify the foreground processing notification remains visible.
15. Cancel/retry a PDF job and verify cards are not duplicated or partially committed.
16. Pair Android with a desktop over a private LAN; sync independent changes, then create an intentional same-field conflict and resolve it explicitly.
17. Rotate, test landscape, test the software keyboard, and verify system Back closes overlays/nested screens correctly.

## Troubleshooting

### The emulator is very slow or will not start

Run:

```bash
pnpm android:doctor:emulator
```

The regular test path requires KVM. A nested virtual machine may not expose hardware virtualization even when the outer host supports it; use a properly configured self-hosted runner or physical device in that case.

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

### PDF OCR fails

No Tesseract installation is required on Android. Capture the document-processing phase and redacted error text, verify the page image is readable at normal zoom, and inspect Logcat for the native ML Kit adapter error. Very low-resolution scans, unusual scripts not covered by the bundled Latin recognizer, extreme page dimensions, or malformed PDFs may still need preprocessing or a future script-specific model.
