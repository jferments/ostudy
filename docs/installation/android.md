# Build and install ostudy for Android

Android runs the same React interface, Rust application/domain logic, SQLite schema, scheduling, analytics, document-learning pipeline, synchronization merge, import formats, and backup format as desktop. Kotlin is limited to Android APIs such as document providers, Sharesheet, lifecycle/system Back/insets, Keystore, native PDF rendering/OCR, QR acquisition, direct USB accessory acquisition, and foreground execution.

Use the [canonical command runbook](../agents/COMMAND-RUNBOOK.md) and maintained wrappers rather than reconstructing Tauri, Gradle, or ADB commands from memory.

## Supported development baseline

The repository pins the Android development environment in [`../../versions.env`](../../versions.env), including JDK 17, Android SDK/API 36, Build Tools 36.0.0, NDK 28.2.13676358, Android 9/API 28 minimum, the maintained Rust ABIs, and the official API 36 Google APIs emulator image.

Do not select arbitrary newer SDK/NDK versions merely because Android Studio offers them. Update repository pins and qualification together.

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

The wrappers derive Android/JDK configuration and create untracked `src-tauri/gen/android/local.properties` when needed. Do not commit machine-specific SDK paths.

Android Studio remains useful for inspection, Logcat, Layout Inspector, and device management, but repository wrappers are the canonical build/test/install path.

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

## 3. Physical Android device for development

Developer installation still uses ADB. Enable Developer options and USB debugging, authorize the development computer, then confirm the device:

```bash
adb devices -l
```

With exactly one authorized ARM64 device:

```bash
pnpm android:install
```

This builds, verifies, installs with replacement, and launches the normal debug application.

When multiple devices are connected:

```bash
bash ./scripts/android-install.sh --debug --target aarch64 --serial '<adb-serial>'
```

To reuse an existing APK:

```bash
bash ./scripts/android-install.sh --debug --no-build --serial '<adb-serial>'
```

Do not add a standalone `--` before `--serial`; `android-install.sh` does not accept that sentinel. The no-build path selects the newest matching APK by modification time and does not prove Git SHA/ABI provenance. Prefer a fresh build/install for PR acceptance.

The debug ID is `io.github.jferments.ostudy.dev`; release is `io.github.jferments.ostudy`. Replacement installation preserves app data. Uninstall/clear-data is destructive and is not routine troubleshooting without backup/explicit consent.

**Developer options and USB debugging are needed only for installing/inspecting a development APK. They are not part of product USB Sync.** Once ostudy is installed, direct USB synchronization uses Android Open Accessory and does not require ADB authorization.

## 4. Optional hot reload

```bash
pnpm android:dev
```

Use this only when live Tauri/Vite development is useful. It introduces development-server networking/device-selection requirements that a built debug APK does not have.

For a local AI server, addresses remain device-relative: the emulator normally reaches the development host at `10.0.2.2`, while a physical phone needs a reachable LAN address. `127.0.0.1` refers to Android itself.

## 5. Automated Android qualification

Compile/package:

```bash
pnpm android:test
```

API 36 emulator smoke:

```bash
pnpm android:smoke
```

Linux developers should use the Ratatui console for maintained desktop gates:

```bash
./scripts/ostudy-dev check-fast
```

Use the full `./scripts/ostudy-dev check` only when the change actually warrants complete Linux qualification. For pull requests, the exact GitHub PR head and its `Check` result are authoritative; `PR Contract` is independent metadata validation.

The emulator suite covers representative launch/migration/study/persistence/restart, compact navigation/system Back, file-picker/import/export, and affected native integrations. A green emulator cannot qualify a physical USB controller/cable path.

## 6. Documents, parsing, PDF rendering, and OCR

Android uses the shared managed document pipeline. Supported sources include PDF, Word, PowerPoint, OpenDocument, EPUB, RTF, Excel/ODS, and CSV. Current non-PDF sources use the in-process Xberg adapter and normalize into the same ostudy-owned canonical source model as desktop.

For PDFs, embedded text/structure is inspected first; pages needing visual recovery are rendered with Android `PdfRenderer`; selected rendered pages use bundled ML Kit Text Recognition v2 (Latin) for OCR. Shared Rust owns selection, limits, cancellation, checkpoints, learning plans, provider tasks, validation, and final database publication.

Android does not require desktop PDFium, Tesseract, Tesseract4Android, or separately installed `tessdata`.

A user-started long document task can acquire a foreground service with a visible notification and bounded wake lock. Process death/reboot can still terminate execution; durable checkpoints/startup reconciliation make it retryable without silently resending provider requests.

## 7. Trusted-device synchronization

Open **Settings → Backup/Sync → Device synchronization**. The sender chooses among Flashcards, Documents, Study Guides, API provider settings, and Saved API keys. Flashcards/Documents/Study Guides default on; provider settings/keys default off. Study Guides require Documents and Saved API keys require API provider settings. Native validation repeats those dependency rules.

Selecting Documents transfers the managed source/page/source-asset files and durable processing graph, including canonical/semantic/OCR state, outlines/checkpoints, durable LLM generation state, learning plans/topics, and document artifact metadata. Study Guides remain a separate selectable graph.

Unselected domains are removed from the copied synchronization snapshot before transmission. Ordinary app/appearance settings and transient AI probe evidence stay device-local. Saved credential bytes travel only through the explicitly authorized encrypted live session and never enter the SQLite snapshot or complete backup.

### Direct USB from Linux desktop to Android

The maintained product cable workflow is intentionally simple:

1. install/open ostudy on both devices;
2. plug one Android phone into the Linux desktop with a data-capable USB cable;
3. select what to share on the desktop;
4. click **USB Sync**;
5. keep the cable attached until completion.

There is **no USB tethering, USB-network interface, IP address, firewall setup, QR scan, ADB, Developer options, or USB debugging requirement** for the synchronization itself.

Linux is the Android Open Accessory host. It discovers the attached Android USB device with libusb, switches it to the `oStudy USB Sync` accessory identity, and communicates over AOA bulk endpoints. Android's private `ostudy-usb` plugin receives the accessory attachment, calls `UsbManager.openAccessory()`, and hands the raw file descriptor to shared Rust. Shared Rust then runs the same authenticated/encrypted synchronization protocol and merge engine used by LAN.

Android can show a first-use system chooser/permission prompt asking which app should handle the `oStudy USB Sync` accessory. Choose ostudy. That accessory association is an Android OS trust prompt, not ADB/developer authorization.

The Linux installer/developer setup owns USB permissions. oStudy installs its own udev rules for the Android OEM vendor IDs supported by the native adapter and for Google's AOA re-enumerated accessory IDs. The Debian bundle depends only on the libusb runtime for this feature; USB Sync does not acquire an Android SDK/ADB runtime dependency. These are installation concerns rather than sync-screen instructions.

Current direct-USB hosting is Linux desktop → Android. Unsupported desktop builds expose no fake manual USB fallback; use local-network sync there until a native adapter is implemented.

### Local-network synchronization

LAN sync remains available over a trusted private Wi-Fi/Ethernet network. The sender creates **Create network sync offer**; Android can accept with Google Code Scanner through Play services or pasted invitation text.

A successful scan provides decoded invitation data to native protocol validation. The receiver does not get a second independent scope selector; the authenticated sender selection defines the package.

LAN uses private TCP ports 49251–49258. **Advanced LAN address override** is a LAN-only fallback and is never used for direct USB.

Receiving never uploads the receiver's library back. A later reverse transfer is a separate explicit offer from the other installation and may use a different supported transport.

Protocol v3 binds the transport into the pairing transcript: LAN invitations carry a private socket endpoint; direct USB invitations use the symbolic `usb` endpoint and are delivered only through the accessory stream.

See the [device sync user guide](../user-guide/device-sync.md) and [sync architecture](../architecture/device-sync.md).

## 8. Release artifacts

Direct-distribution release APK:

```bash
pnpm android:build:release
```

Multi-ABI Android App Bundle:

```bash
pnpm android:build:aab
```

An AAB is store input, not directly installable. Release signing material stays outside the repository. Public distribution still requires permanent signing custody, Play App Signing, store metadata/privacy declarations, and signed-release qualification.

## Android storage and permissions

ostudy does not request broad storage access. Android document providers supply user-selected bytes; Kotlin streams them into app-private staging and shared Rust validates size/hash/type/package/domain graph before commit.

Outgoing files use the system Create Document/Share flow and a narrowly scoped `FileProvider` where appropriate. Incoming shares are cached privately and require explicit confirmation before import.

Android Auto Backup/device-transfer backup is disabled for the application sandbox. Use ostudy's complete `.ostudybackup`, which captures coherent learner data/managed files while excluding Keystore secrets.

The USB-accessory declaration does not grant broad storage or arbitrary USB-host access to Android. On the phone, ostudy is the accessory-side application selected by Android for the exact `oStudy USB Sync` accessory identity.

## Secure provider credentials

Saved provider credentials use the Android Keystore-backed secure store; session-only credentials disappear with process lifetime. Credential bytes are excluded from learner SQLite, drafts, deck exports, complete backups, routine diagnostics, and ordinary file shares.

Device synchronization can transfer supported saved credentials only after explicit sender opt-in for **Saved API keys**, with **API provider settings** included as its required parent scope.

## Manual acceptance for Android-affecting changes

Use the maintained install wrapper and test only relevant real-device paths. Human-only checks may include system insets/keyboard, persistence/restart, Practice/Analytics, Android system import/export/share, saved credentials/provider connectivity, representative document processing, foreground execution/cancel/retry, Google Code Scanner, and direct USB.

For direct USB acceptance specifically verify:

1. one data-capable cable and one attached Android phone;
2. **USB Sync** succeeds without tethering or address entry;
3. first accessory association prompt behaves correctly when present;
4. selected/unselected synchronization categories are respected;
5. cancellation interrupts active USB I/O;
6. disconnecting the cable produces a bounded actionable failure;
7. reconnecting and starting a fresh USB Sync works;
8. Developer options/USB debugging can be turned off after the debug APK is installed without changing product USB behavior.

Emulator success must not be reported as physical-camera/OEM-provider/battery/screen-reader/USB-cable acceptance.

## Troubleshooting development installation

### Install failure

```bash
adb devices -l
bash ./scripts/android-install.sh --debug --target aarch64 --serial '<adb-serial>'
```

The wrapper already uses `--no-streaming -r`. Preserve the package-manager error rather than switching install mechanisms.

### Emulator/KVM problem

```bash
pnpm android:doctor:emulator
```

Regular automated qualification requires usable hardware acceleration.

### Local AI server unreachable

- emulator → development host: `10.0.2.2`;
- physical phone → reachable host LAN address;
- `127.0.0.1` only when the model server runs on Android;
- confirm LAN routing/firewall/AP isolation for LAN services.

These network details do not apply to direct USB Sync, which carries synchronization bytes over the accessory endpoints rather than IP.

### Retrieve diagnostics

```bash
pnpm android:diagnostics
```

The bounded private diagnostic path excludes credentials, raw document/source text, prompts/responses/reasoning, and other intentionally redacted content.
