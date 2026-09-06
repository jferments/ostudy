# Build and install ostudy for Android

Android runs the same React interface, Rust application/domain logic, SQLite schema, scheduling, analytics, document-learning pipeline, synchronization merge, import formats, and backup format as desktop. Kotlin is limited to Android APIs such as document providers, Sharesheet, lifecycle/system Back/insets, Keystore, native PDF rendering/OCR, QR acquisition, and foreground execution.

Use the [canonical command runbook](../agents/COMMAND-RUNBOOK.md) and maintained wrappers rather than reconstructing Tauri, Gradle, or ADB commands from memory.

## Supported development baseline

The repository pins the Android development environment in [`../../versions.env`](../../versions.env), including:

- JDK 17;
- Android SDK platform 36 / API 36 target;
- Android Build Tools 36.0.0;
- Android NDK 28.2.13676358;
- Android 9 / API 28 minimum;
- arm64, armv7, x86, and x86_64 Rust targets;
- the official API 36 Google APIs emulator image used for automated smoke testing.

Do not select an arbitrary newer SDK/NDK merely because Android Studio offers one. Update the repository pins and qualification together.

## 1. Install/verify the Android toolchain

From the repository root:

```bash
corepack enable
pnpm install --frozen-lockfile
pnpm android:install-deps
pnpm android:doctor
```

For emulator work:

```bash
pnpm android:doctor:emulator
```

The wrappers derive the Android/JDK environment and create untracked `src-tauri/gen/android/local.properties` when needed. Do not commit machine-specific SDK paths.

Android Studio is useful for inspection, Logcat, Layout Inspector, and device management, but the repository wrappers are the canonical build/test/install path.

## 2. Official emulator

Create the pinned virtual device once:

```bash
pnpm android:avd create
```

Start it visibly:

```bash
pnpm android:avd start
```

For CI/remote use:

```bash
pnpm android:avd start --headless --wipe-data
```

Inspect/stop it with:

```bash
pnpm android:avd status
pnpm android:avd stop
```

The official emulator is the automated Android runtime qualification environment. Other emulators can be useful experimentally but do not replace the maintained API/lifecycle/native-integration smoke suite.

## 3. Physical Android device

Enable Developer options and USB debugging, authorize the development computer, then confirm the device:

```bash
adb devices -l
```

With exactly one authorized ARM64 device:

```bash
pnpm android:install
```

This builds, verifies, non-streaming installs with replacement, and launches the normal debug application.

When multiple devices are connected, use the wrapper directly:

```bash
bash ./scripts/android-install.sh --debug --target aarch64 --serial '<adb-serial>'
```

To reuse an existing APK:

```bash
bash ./scripts/android-install.sh --debug --no-build --serial '<adb-serial>'
```

Do not add a standalone `--` before `--serial`; `android-install.sh` does not accept that sentinel. The no-build path selects the newest matching APK by modification time and does not prove its Git SHA/ABI provenance. Prefer a fresh build/install for PR acceptance.

The debug ID is `io.github.jferments.ostudy.dev`; release is `io.github.jferments.ostudy`. Replacement installation preserves app data. Uninstall/clear-data is destructive and not routine troubleshooting without backup/explicit consent.

Developer options/USB debugging are needed here only to install and inspect a development APK with ADB. They are **not** required by ostudy's product USB synchronization workflow after the app is installed.

## 4. Optional hot reload

```bash
pnpm android:dev
```

Use this only when live Tauri/Vite development is useful. It introduces development-server networking/device-selection requirements that a built debug APK does not have.

For a local AI server, remember addresses are device-relative: the emulator normally reaches the development host at `10.0.2.2`, while a physical phone needs a reachable LAN address. `127.0.0.1` refers to the Android device itself.

## 5. Automated Android qualification

Compile/package:

```bash
pnpm android:test
```

API 36 emulator smoke:

```bash
pnpm android:smoke
```

Shared Linux/Tauri qualification:

```bash
./scripts/check.sh
```

The emulator suite covers representative launch/migration/study/persistence/restart, compact navigation/system Back, file-picker/import/export, and affected native integrations. Its explicit test bridge is restricted to smoke builds and rejected by release-package verification.

For pull requests, the exact GitHub PR head and its `Check` result are authoritative. `PR Contract` is independent metadata validation. A green run qualifies only the SHA it actually tested.

## 6. Documents, parsing, PDF rendering, and OCR

Android uses the shared managed document pipeline. Supported sources include PDF, Word, PowerPoint, OpenDocument, EPUB, RTF, Excel/ODS, and CSV. Current non-PDF sources use the in-process Xberg adapter and normalize into the same ostudy-owned canonical source model as desktop.

For PDFs:

- embedded text/structure is inspected first;
- pages needing visual recovery are rendered with Android `PdfRenderer`;
- selected rendered pages use bundled ML Kit Text Recognition v2 (Latin) for OCR;
- shared Rust owns page selection, dimensions/byte limits, cancellation, checkpoints, learning plans, provider tasks, validation, and final database publication.

Android does **not** require desktop PDFium, Tesseract, Tesseract4Android, or separately installed `tessdata`.

A user-started long document task can acquire a foreground service with a visible notification and bounded wake lock. Activity backgrounding therefore does not make the WebView the task owner. Process death/reboot can still terminate execution; durable checkpoints/startup reconciliation make it retryable without silently resending provider requests.

## 7. Trusted-device synchronization

Open **Settings → Backup/Sync → Device synchronization** on both installations. For normal LAN sync, devices must be mutually reachable on the same trusted private network. For USB sync, the current implementation uses an operating-system-provided point-to-point USB network interface and the same authenticated protocol over that private link.

An invitation is a one-way offer from the device that created it. The sender chooses the data categories for that offer with five checkboxes:

- **Flashcards**;
- **Documents**;
- **Study Guides**;
- **API provider settings**;
- **Saved API keys**.

Flashcards, Documents, and Study Guides are selected by default. API provider settings and Saved API keys are off by default for each new offer. Study Guides require Documents. Saved API keys require API provider settings. The native backend validates these dependencies again rather than trusting only the checkbox state.

Selecting Documents transfers the managed source/page/source-asset files and the durable document-processing graph needed on the other device, including canonical source/semantic state, OCR-derived data, outlines/checkpoints, durable LLM generation jobs/units/windows/checkpoints, learning plans/topics, and document artifact metadata. Study Guides are a separate selectable domain covering generated guides, sections, overlays, and guide checkpoints.

The receiver accepts by pasted invitation or, on Android, **Google Code Scanner through Play services**. A successful scan supplies decoded invitation data to ostudy; native protocol/identity/expiry/cryptographic validation still decides whether the offer is accepted. The receiver does not get a second independent scope selector for the same offer: the authenticated sender selection defines the package being sent.

Receiving does not upload the receiver's library back to the sender. To transfer Android-side changes later, create a new offer from Android and accept it on the other device. That reverse offer can select a different set of categories or use a different transport.

Shared Rust authenticates/encrypts the session, validates the versioned allowlisted package, merges from saved common-base state, and records explicit conflicts. Unselected domains are removed from the copied synchronization snapshot before transmission. Ordinary app/appearance settings stay device-local. Transient AI probe evidence is not synchronized.

API provider settings and saved credentials are distinct permissions. Selecting provider settings transfers eligible profile/default metadata without key bytes. Saved key bytes travel only when **Saved API keys** is separately selected, the sender can read them from secure storage, and the receiver has usable secure storage. Credential bytes travel through the authenticated encrypted live channel rather than being inserted into the SQLite snapshot or complete backup.

### USB-network synchronization

Product USB synchronization does not require ADB, Developer options, or USB debugging. Those are development-install/debug tools only.

For the current USB-network path:

1. connect the devices with a data-capable USB cable;
2. enable the operating system's USB networking feature, such as Android **USB tethering** where supported;
3. identify the private IP address assigned to the sending device's USB-network interface;
4. choose **USB cable** in ostudy and enter that address in **USB interface address**;
5. select the desired transfer categories;
6. create the USB sync offer and scan/paste it on the receiver.

The invitation, pairing identity, encryption, package validation, merge rules, conflicts, baselines, and cancellation behavior are the same as for LAN sync. Automatic cross-platform USB-interface discovery is intentionally deferred; explicit private-address entry is the deterministic fallback rather than guessing from platform-specific adapter names.

See [device sync user guide](../user-guide/device-sync.md) and [sync architecture](../architecture/device-sync.md).

## 8. Release artifacts

Direct-distribution release APK:

```bash
pnpm android:build:release
```

Multi-ABI Android App Bundle:

```bash
pnpm android:build:aab
```

An AAB is store input, not directly installable. Release signing material stays outside the repository. The manual Android package workflow can build QA artifacts and records source identity, but public distribution still requires permanent signing custody, Play App Signing, store metadata/privacy declarations, and signed-release qualification.

## Android storage and permissions

ostudy does not request broad storage access. Android document providers supply user-selected bytes; Kotlin streams them into app-private staging and shared Rust validates size/hash/type/package/domain graph before commit.

Outgoing files use the system Create Document/Share flow and a narrowly scoped `FileProvider` where appropriate. Incoming shares are cached privately and require explicit confirmation before import.

Android Auto Backup/device-transfer backup is disabled for the application sandbox. Use ostudy's complete `.ostudybackup`, which captures coherent learner data/managed files while excluding Keystore secrets.

## Secure provider credentials

Saved provider credentials use the Android Keystore-backed secure-store implementation; session-only credentials disappear with process lifetime. Credential bytes are excluded from learner SQLite, drafts, deck exports, complete backups, routine diagnostics, and ordinary file shares.

Device synchronization can transfer supported saved credentials only after explicit sender opt-in for **Saved API keys** on the individual authenticated encrypted offer, with **API provider settings** included as its required parent scope.

## Manual acceptance for Android-affecting changes

Use the maintained install wrapper and test only the relevant real-device paths. Typical human-only checks include:

1. status/cutout and bottom-system-inset containment;
2. editor reachability with software keyboard;
3. create/edit/archive/delete and managed images after restart;
4. Practice/review/Analytics behavior;
5. background/resume and force-stop/relaunch recovery;
6. Android system import/export/backup/share providers;
7. saved credentials and live provider connectivity where in scope;
8. representative text/scanned document processing;
9. long document foreground execution/cancel/retry;
10. Google Code Scanner receive/send direction on a real Play-services device;
11. real USB-network send/receive with a data-capable cable, including selected/unselected sync categories and cancellation;
12. portrait/landscape, system Back, and accessibility behavior.

Emulator success must not be reported as physical-camera/OEM-provider/battery/screen-reader/USB-cable acceptance.

## Troubleshooting

### Install failure

```bash
adb devices -l
bash ./scripts/android-install.sh --debug --target aarch64 --serial '<adb-serial>'
```

The wrapper already uses `--no-streaming -r`. Preserve the complete package-manager error rather than switching install mechanisms.

### Emulator/KVM problem

```bash
pnpm android:doctor:emulator
```

Regular automated qualification requires usable hardware acceleration.

### Local AI server unreachable

- emulator → development host: `10.0.2.2`;
- physical phone → reachable host LAN address;
- `127.0.0.1` only when the model server runs on Android;
- confirm local routing/firewall/AP isolation;
- LAN HTTP still requires the application's explicit acknowledgement;
- public cloud/custom remote providers require the applicable HTTPS policy.

### Retrieve diagnostics

```bash
pnpm android:diagnostics
```

The bounded private diagnostic path excludes credentials, raw document/source text, prompts/responses/reasoning, and other intentionally redacted content.
