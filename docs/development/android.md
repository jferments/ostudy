# Android development architecture

The Android application is a native Tauri 2 shell around ostudy's shared React and Rust application. It is not a Kotlin rewrite.

## Dependency direction

```text
Android Activity / WebView
        ↓
private ostudy-platform Tauri plugin
        ↓
Android implementations of narrow platform ports
        ↓
shared Rust application services and SQLite transactions
        ↓
shared TypeScript controllers and React screens
```

Kotlin may interact with Android's document provider, Sharesheet, Keystore, lifecycle, Back dispatcher, connectivity manager, window insets, battery/thermal services, PDF renderer, local Tesseract OCR, local ZXing QR scanner, USB accessory APIs, and foreground services. Kotlin must not open or migrate the ostudy database, implement scheduling, validate deck/sync packages, decide merge semantics, or modify learner data.

The generated Android project lives under `src-tauri/gen/android`. Custom native behavior lives in `plugins/ostudy-platform`; keep `MainActivity` minimal so regenerating the Tauri shell remains manageable.

Android values that Tauri exposes as configuration belong in `src-tauri/tauri.android.conf.json`, not in hand-edited generated Gradle state. Package verification confirms the resulting debug APK uses `io.github.jferments.ostudy.dev` and the release APK uses `io.github.jferments.ostudy`.

## Current capability boundary

Supported Android capabilities include:

- the five destinations (Home, Library, Tutor, Progress, Settings) in a bottom bar;
- subject, deck, card, tag, template, search, and bulk authoring;
- all native interactive question types, including manual image occlusion;
- FSRS practice and scheduling controls;
- Progress analytics;
- managed images and durable drafts;
- interrupted-practice restoration;
- Storage Access Framework import/export;
- Android Sharesheet send and `ACTION_SEND` receive;
- complete backup/restore;
- Android Keystore credentials;
- cloud and LAN AI-provider profile tests;
- native PDF page rasterization through Android `PdfRenderer`;
- selective on-device PDF OCR through bundled open-source Tesseract;
- foreground document processing with notification/wake-lock ownership while a user-started job is active;
- encrypted same-LAN trusted-device synchronization with explicit conflict handling;
- direct encrypted Android Open Accessory USB synchronization;
- Android QR pairing through the local open-source ZXing Embedded scanner; and
- optional encrypted AI-provider/credential transfer into the destination secure store when the sender opts in.

Android `PdfRenderer` has no independent second text-extraction API, so the `native_pdf_text_rescue` capability remains unavailable. Shared Rust first keeps usable PDF text extracted by the document pipeline and invokes Android OCR only for pages that require visual recovery.

## Platform plugin ownership

`plugins/ostudy-platform/android/src/main/java/OstudyPlatformPlugin.kt` owns general Android API interactions. `OstudyDocumentPlugin.kt` owns bounded PDF rendering, Tesseract text recognition, and foreground document-execution requests. `OstudyQrPlugin.kt` owns local ZXing scanner activity integration. `OstudyUsbPlugin.kt` owns Android Open Accessory attachment/descriptor handoff. The Rust wrappers in `plugins/ostudy-platform/src` expose typed methods and `src-tauri/src/platform/android.rs` adapts them to shared platform contracts.

### External files

Rust creates app-private staging/prepared-export paths and byte limits. Kotlin opens system file UI and streams the selected URI to or from those paths. Shared Rust validates size, hashes, content-derived type, package format, ledger state, and transaction semantics.

React and durable Rust entities do not store Android `content://` URIs.

### Incoming shares

Kotlin copies temporary `ACTION_SEND` URI grants into private cache and exposes only a random token and safe metadata. Rust then adopts the cached file through the ordinary staged-import validators.

### Secrets

Kotlin stores AI credentials behind a non-exportable Android Keystore AES-GCM key. Rust retains shared secure-store policy and write-only UI behavior. Android Auto Backup remains disabled for those records.

The stable device-sync identity key uses the same secure-store boundary. Each synchronization offer uses fresh pairing-secret and ephemeral key-agreement material; there is no long-lived paired-device transport secret that gets reused across QR sessions. Synchronization packages never contain API credential bytes. Saved credentials can travel only when the **sending device** explicitly opts in for that encrypted offer, and received credentials are committed to the destination secure store rather than ordinary SQLite settings.

### PDF rendering and OCR

Rust owns document validation, extraction planning, page selection, cancellation, durable checkpoints, AI generation, and SQLite transactions. Android receives only app-private PDF/image paths and bounded operation parameters.

`OstudyDocumentPlugin` uses `PdfRenderer` to rasterize requested PDF pages to app-private PNG files. For a rendered page that needs OCR, the plugin uses Tesseract4Android 4.9.0, which packages Tesseract 5.5.1 and Leptonica 1.85.0. The first Android release bundles the English `tessdata_fast` model only. The model is fetched at build time from immutable upstream commit `87416418657359cb625c412a48b6e1d6d41c29bd`, verified as Git blob `bbef4675053b5b468cdb477053e28b1c698ba08e` with exact size 4,113,088 bytes, then packaged in the APK at `assets/tessdata/eng.traineddata`.

Recognition itself performs no model download and requires no hosted OCR service. On first use, the packaged model is copied into app-private storage before Tesseract initializes. Source bytes, image dimensions, recognition time, and output metadata remain bounded. Requests for an unbundled OCR language fail explicitly instead of silently using a network service.

Desktop platforms continue using their independently qualified Tesseract CLI adapter behind the same Rust OCR contract.

### Background document execution

A user-started long-running document generation task acquires an Android foreground-service lease. The service presents a visible progress notification and holds a partial wake lock while work is active. Rust remains the owner of the durable job and checkpoints.

Backgrounding the Activity does not make the WebView the lifetime owner of the job. If Android terminates the entire application process or the device restarts, the foreground service ends with that process; reopening ostudy recovers from the document pipeline's persisted state rather than pretending arbitrary process death is transparent.

### Device synchronization

Shared Rust owns snapshot construction, authenticated encryption, common-base merge planning, conflict persistence, typed-question graph admission, asset validation, and secure credential policy. Android contributes local QR scanning, Open Accessory I/O and Keystore implementation. The sync protocol never copies a live SQLite database over another installation.

Protocol v3 uses explicit **one-way offers**. The device that creates an invitation is the sender. Scanning that invitation on Android is the acceptance gesture and immediately starts receiving the sender-selected study-library snapshot; the receiver does not upload its library or create a second invitation. A future reverse transfer is a separate offer initiated by the other device. Direct USB uses the same authenticated transfer semantics under a different transport adapter rather than tunneling through TCP, tethering, ADB or a second merge implementation.

`app_settings` is excluded from both the outgoing package and the synchronization merge allowlist, so theme, fonts, UI scale, motion/layout preferences, and other device-specific settings remain local. AI provider configuration and saved credentials are the only settings-related synchronization scope currently supported, and that scope is off by default and controlled by the sender per offer.

See [Device synchronization architecture](../architecture/device-sync.md) for the merge and transport contract.

### Back, insets, keyboard, and lifecycle

The frontend owns the logical Back-handler stack and enables Android Back interception only while application navigation can consume it.

The native content boundary applies status-bar, navigation-bar, display-cutout, and IME insets so the shared application shell is not obscured by Android system UI. `SOFT_INPUT_ADJUST_RESIZE` and IME visibility allow form/dialog content to stay reachable while the keyboard is open.

Lifecycle observations are useful but are not the sole durability mechanism. Draft autosave, UI checkpoints, durable practice sessions, document checkpoints, and idempotent operation identities protect abrupt process death.

Android system picker and local scanner Activity results are delivered to the Tauri invoke only after the host Activity has returned to a valid result boundary. This prevents a system Activity transition from being mistaken for an application-level completion.

## Device-local UI checkpoint

The UI checkpoint is a bounded, versioned, atomically replaced app-private JSON file containing navigation/practice identity state, not card content or credentials. Practice state is revalidated and re-derived from SQLite on launch.

## Android test layers

### Project verification

`node scripts/verify-android-project.mjs` verifies generated/native files, SDK/NDK/API pins, package identity, Kotlin compatibility, parallelism controls, permissions, backup exclusions, FileProvider scope, Tauri overlays, and package scripts.

`node scripts/verify-external-file-streaming.mjs` protects the streaming import/export boundary and the required Android smoke coverage.

`node scripts/verify-android-qr-scanner.mjs` enforces the open-source Android imaging boundary: pinned ZXing Embedded/core and Tesseract4Android versions, immutable English Tesseract model provenance, local QR-only scanning, no retained scan images, camera permission, bounded OCR timeout, and absence of Google Code Scanner/ML Kit text-recognition runtime dependencies.

The device-sync contract verifier additionally checks local QR routing, automatic receive after scanning, direct USB routing, sender-owned API scope, absence of `app_settings` from the merge allowlist/outgoing package, typed-question graph transfer, and first-contact merge behavior.

### Compile and package

```bash
pnpm android:test
```

This is the authoritative Android compile/package gate. It runs the shared prerequisites required by Android, builds through the Tauri-owned Android path, resolves and verifies the Kotlin/coroutines/ZXing/Tesseract dependency graph, compiles the private Kotlin plugin and instrumentation package, and verifies the debug APK. Package verification requires camera permission for the local scanner, rejects broad storage/install permissions, requires the pinned Tesseract model in the APK, and retains 16 KiB native-library alignment checks.

`OSTUDY_ANDROID_JOBS` may lower parallelism on a constrained machine. The canonical CI runner currently targets 32 vCPUs and ample RAM.

### Official emulator smoke

```bash
pnpm android:smoke
```

The authoritative mobile smoke uses the pinned API 36 Google APIs x86_64 AVD and verifies:

- cold boot and SQLite migration;
- deck/card creation and practice/analytics;
- persistence after force-stop/restart;
- Android Back behavior;
- system-bar/IME inset ownership;
- interrupted practice restoration;
- Storage Access Framework launch/cancellation for supported import purposes;
- prepared export through `ACTION_CREATE_DOCUMENT`;
- release of native picker-operation ownership across sequential cancellations;
- bundled Tesseract OCR through a real synthetic-text recognition case; and
- production native-plugin routing for document, QR and USB capabilities.

The smoke-only JavaScript bridge is opt-in at build time and forbidden in release packages. A headless emulator cannot reproduce a physical phone camera aimed at a desktop QR code, real USB accessory re-enumeration, or physical touch/occlusion ergonomics, so those remain part of the explicit physical-device acceptance pass.

Waydroid may be useful for informal checks but does not replace the official emulator gate.

## CI runner

Android CI runs on the same isolated self-hosted runner as Linux/Tauri:

```text
ostudy-ci-unified
self-hosted / linux / x64 / ostudy-unified
```

The runner VM exposes nested `/dev/kvm` to the unprivileged `ostudy-runner` account and has passed the repository guest preflight plus authoritative emulator smoke.

See [Unified Linux and Android self-hosted runner](unified-linux-android-runner.md) for the containment/security model. The previous split host-level Android runner is documented only as migration history in [Android self-hosted runner notes](android-self-hosted-runner.md).

## GitHub Actions

`.github/workflows/check.yml` remains the **single automatic pull-request qualification workflow** introduced by the unified-runner refactor. Draft pull requests use the fast shared gate. Ready pull requests, pushes to `main` and drafts labelled `qualification:full` run the complete serialized sequence:

```text
check-static.sh
→ check.sh
→ android-test.sh
→ android-emulator-smoke.sh
```

`.github/workflows/android-package.yml` is the explicit manual packaging workflow on the same unified runner and can produce a debug APK, release APK, or release AAB. Android qualification is intentionally not duplicated in a second automatic workflow.

`.github/workflows/unified-runner-qualification.yml` is an opt-in infrastructure/runner-health workflow. The former automatic split `android-check.yml` workflow remains retired.

Signing material is never exposed to pull-request jobs.

## Updating the generated project

When the pinned Tauri CLI requires regeneration:

1. create a clean branch from `main`;
2. run `pnpm tauri android init --ci`;
3. compare generated Gradle, manifest, and Kotlin files rather than accepting them blindly;
4. keep supported Android identity/build options in `tauri.android.conf.json` where possible;
5. reapply only documented generated-project customizations Tauri does not expose as configuration;
6. run `pnpm verify:android`;
7. run `pnpm android:test` and `pnpm android:smoke`;
8. verify `pnpm check` remains green.

The generated directory is committed because reproducibility and review are more important than minimizing generated files.

## QR scanner verification

The Android pairing scanner is fully local. JourneyApps ZXing Embedded 4.3.0 owns the scanner Activity and ZXing core 3.5.4 performs decoding. oStudy requests only QR format, disables barcode-image retention and bounds returned content to 16 KiB. `android.permission.CAMERA` is required for this local scanner; camera hardware remains optional so non-camera devices can still install oStudy and use other transports.

`node scripts/verify-android-qr-scanner.mjs` and `./scripts/android-test.sh` enforce the dependency/version/runtime boundary and reject reintroduction of Google Code Scanner or ML Kit text recognition. `./scripts/android-test.sh --debug=qr --keep-artifacts` retains bounded oStudy QR Logcat evidence on failure without recording decoded payloads.
