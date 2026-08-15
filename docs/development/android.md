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

Android values that Tauri exposes as configuration belong in `src-tauri/tauri.android.conf.json`, not in hand-edited generated Gradle state. Package verification confirms the resulting debug APK still uses `io.github.jferments.ostudy.dev`.

## Current capability boundary

Supported Android capabilities include:

- deck, card, tag, template, search, and bulk authoring;
- FSRS practice and scheduling controls;
- analytics;
- managed images and durable drafts;
- interrupted-practice restoration;
- Storage Access Framework import/export;
- Android Sharesheet send and `ACTION_SEND` receive;
- complete backup/restore;
- Android Keystore credentials;
- cloud and LAN AI-provider profile tests.

Explicit development stubs remain for native PDF rendering/text rescue/OCR and PDF generation, camera/QR transfer, and background document execution. Unsupported commands fail with stable capability errors while existing restored document records remain preserved.

## Platform plugin ownership

`plugins/ostudy-platform/android/src/main/java/OstudyPlatformPlugin.kt` owns Android API interactions. The Rust wrapper in `plugins/ostudy-platform/src/mobile.rs` exposes typed methods and `src-tauri/src/platform/android.rs` adapts them to shared platform contracts.

### External files

Rust creates app-private staging/prepared-export paths and byte limits. Kotlin opens system file UI and streams the selected URI to or from those paths. Shared Rust validates size, hashes, content-derived type, package format, ledger state, and transaction semantics.

React and durable Rust entities do not store Android `content://` URIs.

### Incoming shares

Kotlin copies temporary `ACTION_SEND` URI grants into private cache and exposes only a random token and safe metadata. Rust then adopts the cached file through the ordinary staged-import validators.

### Secrets

Kotlin stores AI credentials behind a non-exportable Android Keystore AES-GCM key. Rust retains shared secure-store policy and write-only UI behavior. Android Auto Backup remains disabled for those records.

### Back and lifecycle

The frontend owns the logical Back-handler stack and enables Android Back interception only while application navigation can consume it.

Lifecycle observations are useful but are not the sole durability mechanism. Draft autosave, UI checkpoints, durable practice sessions, and idempotent operation identities protect abrupt process death.

Android system picker Activity results are delivered to the Tauri invoke only after the host Activity has returned to `RESUMED`. This prevents a result from being lost while the WebView is still returning from system file UI.

## Device-local UI checkpoint

The UI checkpoint is a bounded, versioned, atomically replaced app-private JSON file containing navigation/practice identity state, not card content or credentials. Practice state is revalidated and re-derived from SQLite on launch.

## Android test layers

### Project verification

`node scripts/verify-android-project.mjs` verifies generated/native files, SDK/NDK/API pins, package identity, Kotlin compatibility, parallelism controls, permissions, backup exclusions, FileProvider scope, Tauri overlays, and package scripts.

`node scripts/verify-external-file-streaming.mjs` protects the streaming import/export boundary and the required Android smoke coverage.

### Compile and package

```bash
./scripts/android-test.sh
```

This is the authoritative Android compile/package gate. It runs the shared prerequisites required by Android, builds through the Tauri-owned Android path, compiles the private Kotlin plugin and instrumentation package, and verifies the debug package.

`OSTUDY_ANDROID_JOBS` may lower parallelism on a constrained machine. The canonical CI runner currently has 16 vCPUs and ample RAM.

### Official emulator smoke

```bash
./scripts/android-emulator-smoke.sh
```

The authoritative mobile smoke uses the pinned API 36 Google APIs x86_64 AVD and verifies:

- cold boot and SQLite migration;
- deck/card creation and practice/analytics;
- persistence after force-stop/restart;
- Android Back behavior;
- interrupted practice restoration;
- Storage Access Framework launch/cancellation for supported import purposes;
- prepared export through `ACTION_CREATE_DOCUMENT`;
- release of native picker-operation ownership across sequential cancellations.

The smoke-only JavaScript bridge is opt-in at build time and forbidden in release packages.

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

`.github/workflows/check.yml` is the single automatic qualification workflow.

Draft pull requests use the fast shared gate. Ready pull requests and pushes to `main` run the complete serialized sequence:

```text
check.sh
→ android-test.sh
→ android-emulator-smoke.sh
```

`.github/workflows/android-package.yml` remains an explicit manual unsigned QA packaging workflow on `ostudy-unified`.

`.github/workflows/unified-runner-qualification.yml` is an opt-in infrastructure/runner-health workflow. The former automatic split `android-check.yml` workflow is retired.

Signing material is never exposed to pull-request jobs.

## Updating the generated project

When the pinned Tauri CLI requires regeneration:

1. create a clean branch from `main`;
2. run `pnpm tauri android init --ci`;
3. compare generated Gradle, manifest, and Kotlin files rather than accepting them blindly;
4. keep supported Android identity/build options in `tauri.android.conf.json` where possible;
5. reapply only documented generated-project customizations Tauri does not expose as configuration;
6. run `verify-android-project.mjs`;
7. run `android-test.sh` and the official emulator smoke suite;
8. verify the Linux full gate remains green.

The generated directory is committed because reproducibility and review are more important than minimizing generated files.
