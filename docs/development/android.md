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

Kotlin may interact with Android's document provider, Sharesheet, Keystore, lifecycle, Back dispatcher, connectivity manager, window insets, battery/thermal services, and incoming intents. Kotlin must not open or migrate the ostudy database, implement scheduling, validate deck packages, decide import semantics, or modify learner data.

The generated Android project lives under `src-tauri/gen/android`. Custom native behavior lives in `plugins/ostudy-platform`; keep `MainActivity` minimal so regenerating the Tauri shell remains manageable.

Android values that Tauri exposes as configuration belong in `src-tauri/tauri.android.conf.json`, not in hand-edited generated Gradle state. In particular, the debug package suffix is owned by `bundle.android.debugApplicationIdSuffix = ".dev"`. Tauri may refresh `src-tauri/gen/android/app/build.gradle.kts`; package verification confirms the resulting debug APK still uses `io.github.jferments.ostudy.dev`.

## A1–A3 capability boundary

Supported Android capabilities:

- deck, card, tag, template, search, and bulk authoring;
- FSRS practice and scheduling controls;
- analytics;
- managed images;
- durable authoring drafts;
- interrupted-practice restoration;
- Storage Access Framework import and export;
- Android Sharesheet send and `ACTION_SEND` receive;
- complete backup/restore;
- Android Keystore credentials;
- cloud and LAN AI-provider profile tests.

Explicit development stubs:

- PDF rendering;
- native PDF text rescue;
- OCR;
- PDF/document AI generation;
- camera and QR transfer;
- background document execution.

Unsupported commands remain registered and fail with stable capability errors. Existing document rows and source files restored from a desktop backup are preserved.

## Platform plugin modules

`plugins/ostudy-platform/android/src/main/java/OstudyPlatformPlugin.kt` currently owns the Android API bridge. Keep additions grouped by capability and extract separate Kotlin classes when a group becomes independently testable or large enough to obscure lifecycle ownership.

The Rust wrapper in `plugins/ostudy-platform/src/mobile.rs` exposes typed methods. `src-tauri/src/platform/android.rs` adapts those methods to the shared `PlatformRuntime` contracts.

### External files

Rust creates an app-private staging or prepared-export path and supplies a byte limit. Kotlin opens the system picker and streams the selected URI to or from that path. Rust then verifies the adapter-reported byte count, actual size, hash, content-derived type, package format, ledger state, and transaction semantics.

The registered frontend commands route every image, PDF, study-content, backup, and prepared-export operation through `ExternalFilePort.acquire_import` or `ExternalFilePort.publish_export`. Desktop and Android therefore exercise the same shared transfer ledger and validators; legacy path-selection methods are not registered as product commands. `scripts/verify-external-file-streaming.mjs` enforces that route before static qualification and before every Android build.

Neither React nor durable Rust entities store Android `content://` URIs.

### Incoming shares

Kotlin copies the temporary incoming URI grant into private cache and emits only a random token and safe metadata. React asks the user for confirmation. Rust adopts the cached file into the normal staged-import ledger and runs the ordinary `.studydeck`, generated JSON, or backup validator. Rejection and dismissal clear the cache.

### Secrets

Kotlin generates a non-exportable AES-GCM key under Android Keystore. Rust's shared AI credential vault continues to enforce write-only UI behavior and secure versus session-only policy. Android Auto Backup is disabled so encrypted records are not restored without their key.

### Back and lifecycle

The frontend maintains a LIFO Back-handler stack. It enables Android interception only while at least one overlay, editor, active practice flow, or non-root navigation destination can consume Back. At the root, Android owns Back and predictive back-to-home remains available.

Kotlin publishes lifecycle observations, but durability does not depend on receiving a final callback. Draft autosave, the device-local UI checkpoint, durable practice sessions, and idempotent operation identities handle abrupt process death.

## Device-local UI checkpoint

The UI checkpoint is stored as a bounded, versioned, atomically replaced JSON file under app-private data. It contains navigation and practice identities, not card content, credentials, or arbitrary HTML. It is excluded from complete backup and future content synchronization.

On launch, the frontend validates referenced deck/session identities. Practice position, revealed state, counts, and card windows are re-derived from SQLite through `resume_practice_session` rather than trusting the checkpoint.

## Testing layers

### Shared tests

Every Android change continues to run the Linux full gate. Vitest and Rust tests remain the fastest and broadest validation of shared behavior.

### Android project verification

`node scripts/verify-android-project.mjs` verifies:

- required generated/native files;
- SDK/NDK/API pins;
- source-of-truth debug package identity;
- Kotlin/coroutines compatibility pins;
- Android build parallelism controls;
- manifest permissions;
- backup exclusions;
- FileProvider authority;
- Android/Linux Tauri overlays;
- package scripts and plugin dependency.

`node scripts/verify-external-file-streaming.mjs` additionally verifies that:

- registered frontend commands use streaming platform ports rather than desktop path selection;
- Android implements both bounded import and export methods;
- the emulator bridge exposes asynchronous file-operation failures;
- every supported import purpose has a smoke case;
- picker cancellation waits for the native result instead of sleeping for a fixed interval;
- the smoke runner aggregates independent failures from one APK build.

### APK verification

`verify-android-package.sh` checks:

- debug/release package ID;
- min and target SDK;
- absence of broad storage/install permissions;
- absence of PDFium/Tesseract during A1–A3;
- 16 KiB ZIP alignment;
- 16 KiB ELF LOAD-segment alignment for every native library;
- absence of the emulator test bridge from release packages.

### Kotlin and instrumentation compilation

`android-test.sh` first resolves the Kotlin dependency graph and compile-checks the private Android plugin, then runs shared TypeScript/Rust qualification, builds the instrumentation APK, and verifies the x86_64 debug APK.

By default Android qualification uses all logical CPUs reported by the host. `OSTUDY_ANDROID_JOBS` can lower that ceiling on memory-constrained machines. Vitest files, Cargo compilation/tests, independent Gradle subprojects, and JVM test forks use the shared worker budget.

Emulator methods still execute one at a time because restart and restoration checks intentionally cross process boundaries. The shell runner no longer stops at the first assertion: it records independent failures, skips only cases whose explicit prerequisite failed, and prints one aggregate result after all viable cases finish. This prevents one expensive APK build from revealing only the first of several unrelated platform defects.

The application test source is under:

```text
src-tauri/gen/android/app/src/androidTest/
```

### Official emulator smoke

`android-emulator-smoke.sh` is the authoritative mobile smoke path. It uses the pinned API 36 Google APIs x86_64 image and verifies:

- cold boot and SQLite migration;
- deck/card creation;
- practice review and analytics;
- persistence after force-stop/restart;
- Documents development stub;
- Android Back handling;
- interrupted revealed-practice restoration after WebView reload and process restart;
- Storage Access Framework launch and cancellation for study-content, image, PDF, and library-backup imports;
- prepared deck export through `ACTION_CREATE_DOCUMENT`;
- release of the native pending-operation slot across sequential picker cancellations.

The smoke-only JavaScript bridge is enabled at build time with `VITE_OSTUDY_ANDROID_TEST_BRIDGE=1`; normal builds do not expose it and release verification rejects it. The bridge reports `pending`, `settled`, or `error` for each file operation so an IPC or native-adapter error is surfaced directly instead of being misreported as a missing system picker.

Waydroid may be used for informal manual tests but is not accepted as the release qualification environment.

## GitHub Actions

- `check.yml` remains the shared/Linux authority.
- `android-check.yml` compiles the app and instrumentation package on each ready Android PR and can run the official emulator smoke suite through manual dispatch on a KVM-capable self-hosted runner.
- `android-package.yml` builds unsigned QA APK/AAB artifacts through explicit manual dispatch.
- protected release signing will be added only when the permanent Android signing key and Play App Signing process are established.

Do not expose signing material to pull-request jobs.

## Updating the generated project

When the pinned Tauri CLI requires regeneration:

1. create a clean branch from `main`;
2. run `pnpm tauri android init --ci`;
3. compare generated Gradle, manifest, and Kotlin files rather than accepting them blindly;
4. keep supported Android identity/build options in `tauri.android.conf.json` instead of reapplying them as generated-file edits;
5. reapply only documented generated-project customizations that Tauri does not expose as configuration;
6. run `verify-android-project.mjs`;
7. run `android-test.sh` and the official emulator smoke suite;
8. verify Linux remains green.

The generated directory is committed because reproducibility and review are more important than minimizing generated files.
