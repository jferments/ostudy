# Testing and CI

ostudy uses tiered validation locally and on self-hosted runners. Each tier answers a different question; inexpensive tiers must not be described as complete application qualification.

The repository does not use GitHub-hosted operating-system runners. Shared/Linux jobs target the ordinary `ostudy` runner class, while Android build/package work targets the dedicated bare-metal `ostudy-android` runner class. Emulator-capable Android work additionally requires `kvm`:

```yaml
runs-on: [self-hosted, linux, x64, ostudy]
runs-on: [self-hosted, linux, x64, ostudy-android]
runs-on: [self-hosted, linux, x64, ostudy-android, kvm]
```

The two custom runner classes are intentionally disjoint. The `ostudy` runner may live inside a VM for shared/Linux qualification; `ostudy-android` runs on the host with the Android SDK/NDK and direct `/dev/kvm` access. Custom labels are maintainer assertions, so runtime doctor scripts verify Android/KVM capabilities rather than trusting labels alone. Third-party actions are pinned to immutable commit SHAs with human-readable version comments.

## Validation model

Shared authoritative qualification progresses from cheap structural guarantees to full native behavior:

```text
changed → static → fast → full
```

Android adds a target-specific path:

```text
project contracts
  → smoke-frontend preflight
  → arm64 package
  → x86_64 Tauri smoke build
  → resolved Gradle/Kotlin contracts
  → JVM tests + Android Lint + instrumentation package
  → APK policy/cleanliness
  → official-emulator smoke using those same APKs
```

The fail-fast authoritative gates answer whether a commit is qualified. The optional diagnostic mode answers a different development question: **what independent failures can be learned from one runner pass before the next edit?** Diagnostic and fail-fast orchestration share the same gate implementations instead of copying commands.

An exact-tree stamp lets later shared tiers reuse earlier successful results only when source, relevant untracked files, deletions, tool versions, and platform still match. `OSTUDY_FORCE_CHECK=1` disables reuse while investigating the gate itself or a suspicious cache result.

## Changed-file iteration

```bash
./scripts/check-changed.sh
```

This is a development aid, not merge qualification. It compares the current change with `origin/main` when available and conservatively routes documentation, frontend, Rust, document, migration, workflow, native, and Android changes to an appropriate tier.

## Static qualification

```bash
./scripts/check-static.sh
```

The static tier verifies:

- environment, manifest, migration, and lockfile relationships;
- deterministic generated UI token artifacts and semantic token usage;
- frontend IPC names against the native command registry;
- repository, platform, external-file, UI, workflow, test, and documentation policies;
- Bash/Node/Python script syntax and script behavior tests;
- frozen JavaScript installation and exact dependency versions;
- TypeScript project checking;
- canonical Rust formatting.

The commands live in `scripts/qualification-gates.sh`, which is also consumed by diagnostic mode. This prevents a new static contract from being added to `check-static.sh` while silently remaining absent from diagnostics.

## Fast qualification

```bash
./scripts/check-fast.sh
```

The fast tier reuses a matching static result, provisions the pinned PDFium runtime, then runs the complete frontend Vitest suite, locked Rust compile checking, Clippy with warnings denied, and focused external-file/document/PDF/migration contracts. Draft pull requests use this tier unless the PR explicitly opts into core diagnostic mode.

## Full shared/Linux qualification

```bash
./scripts/check.sh
```

The full tier reuses a matching fast result and adds native environment qualification, production frontend build, all Rust tests, a standalone Tauri build with locked dependencies, resolved Rust dependency verification, and the native WebDriver create → practice → restart → persistence workflow.

This remains the authoritative shared merge gate. A green diagnostic result does not replace it.

## Multi-failure development diagnostics

```bash
pnpm check:diagnostic
pnpm android:diagnostic
```

See [Agent-driven diagnostics](agent-diagnostics.md) for the full operating procedure. The important distinction is that diagnostic mode continues independent gates after a failure and records dependent work as `BLOCKED`; merge qualification remains fail-fast.

A draft PR opts in through one of these body markers:

```text
<!-- agent-diagnostics:core -->
<!-- agent-diagnostics:android -->
<!-- agent-diagnostics:all -->
```

The markers alter the existing `Check` and `Android check` workflows rather than launching another competing workflow. `core` substitutes the core diagnostic sweep for the normal draft fast job. `android` substitutes Android diagnostic execution for the ordinary draft Android compile/emulator chain. `all` selects both. Once a PR is ready for review, markers no longer substitute the authoritative gates.

Diagnostic evidence includes per-gate logs, `summary.md`, and schema-versioned `diagnostics.json` with dependencies, result, exit code, duration, and bounded error highlights.

## Android qualification

### Shared host and project contracts

Android qualification begins with the same frozen JavaScript and shared TypeScript/Rust behavior used by the application, then verifies the committed Android project, platform plugin, external-file streaming boundary, package identity, permissions, Back ownership, and toolchain pins.

`versions.env` intentionally keeps generated Tauri Kotlin on 1.9.25 and coroutines on 1.8.1; resolved Gradle dependencies are checked after Tauri initializes the private plugin graph so Kotlin metadata incompatibility is caught before emulator work.

### Smoke-frontend preflight

```bash
./scripts/android-smoke-frontend-preflight.sh
```

The test-only Android JavaScript bridge is verified at the Vite distribution boundary before expensive native compilation. A smoke build must contain exactly one marked `androidTestBridge-*` chunk; a normal build must contain none. APK byte-grepping is not used to prove bridge presence because Tauri may compress or embed web assets.

### Tauri-first Android build ownership

`tauri.settings.gradle` and private-plugin inclusion are transient state created by the Tauri CLI. Direct Gradle qualification before a Tauri Android build is therefore invalid.

The maintained order is:

1. build through `scripts/android-build.sh`;
2. confirm the generated settings and x86_64 JNI library exist;
3. run `scripts/android-gradle-contracts.sh`;
4. run `scripts/android-gradle-postbuild.sh` while excluding generated Tauri `rustBuild` tasks.

The build wrapper restores tracked generated source such as the committed Android manifest even when a build fails. `scripts/verify-build-cleanliness.sh` then fails if build/test execution leaves any tracked source modified.

### arm64 package coverage

`android-test.sh` builds and verifies an arm64 debug package before the x86_64 smoke package. This catches target-specific compile/package problems for the architecture used by ordinary physical Android devices instead of relying solely on the x86_64 emulator architecture.

### Gradle JVM tests and Android Lint

After Tauri initialization, the postbuild tier runs the private platform plugin JVM tests, app JVM tests, plugin `lintDebug`, app `lintUniversalDebug`, and instrumentation-package assembly. Lint provides inexpensive API-level, manifest, resource, accessibility, and Android correctness coverage before a device is booted.

### APK policy

`scripts/verify-android-package.sh` verifies observable package properties: application ID, minimum/target SDK, forbidden broad-storage/install permissions, forbidden desktop document-processing payloads, 16 KiB ZIP alignment, and native ELF LOAD alignment. Release builds additionally reject the emulator-only marker as defense in depth.

### Build once, run emulator later

The authoritative `android-compile` job uploads the already-qualified application and instrumentation APKs. The dependent emulator job downloads those exact artifacts and invokes:

```bash
./scripts/android-emulator-smoke.sh --app-apk <app.apk> --test-apk <test.apk>
```

It does **not** compile the application again. This shortens the expensive loop and guarantees the emulator tests the same build products produced by the compile/package job.

### Official emulator functional smoke

The primary smoke image is the pinned API 36 Google APIs x86_64 AVD. Instrumentation verifies:

- application launch and test-bridge readiness after real WebView attachment;
- deck/card creation, practice review, and analytics;
- persistence after force-stop/restart;
- Documents capability behavior;
- system Back through top-level and nested workflows;
- interrupted-practice restoration after WebView reload and process restart;
- system picker cancellation and native-operation-slot release;
- successful generated-JSON, PNG, and PDF selection through an instrumentation-only Android `DocumentsProvider`;
- a real ACTION_CREATE_DOCUMENT deck export written back through that provider.

Activity launch, WebView attachment, document-root availability, bridge readiness, Back-listener registration, Back interception, and overlay teardown are separate observable states. Tests wait for those conditions rather than using fixed sleeps. AndroidJUnitRunner is told not to finish Tauri activities; shell-owned process teardown avoids racing Wry/native destruction.

The test `DocumentsProvider` exists only in the instrumentation APK. It supplies deterministic system-picker fixtures and a writable root, so successful Storage Access Framework streaming can be tested without granting the production application special storage access.

Every emulator run writes `android-smoke-results.json` with per-case `PASS`/`FAIL`/`SKIP`, elapsed time, prerequisite information, device API/page size, and artifact filenames. Failure evidence still includes per-case instrumentation output, Logcat, a screenshot, and a UI hierarchy dump.

### Minimum-SDK compatibility

The normal functional smoke stays on API 36. `versions.env` also pins an API 28 Google APIs x86_64 image matching `ANDROID_MIN_SDK`.

```bash
pnpm android:smoke:min
```

`android-check.yml` runs that compatibility smoke weekly and also exposes it through `workflow_dispatch` mode `min-sdk-smoke`. It is intentionally not run for every PR because a second emulator boot is expensive; ordinary Android Lint protects inexpensive minimum-API mistakes on every Android qualification.

## Important test layers

### Repository contracts

Static scripts protect stable boundaries such as exact versions, immutable migrations, IPC parity, self-hosted workflow policy, UI tokens, dependency isolation, Android permissions/package identity, Tauri-first Gradle ownership, smoke-bridge observability, Back ownership, test-only provider isolation, and absence of skipped/focused/ignored tests. Prefer behaviorally meaningful ownership rules over checks for incidental comment prose or private helper names.

### Frontend and controller tests

Vitest and Testing Library cover user interaction, accessible relationships, error/empty/loading states, responsive primitives, editors, controller request ordering, mobile Back integration, checkpoints, restoration state machines, and stable automation hooks. Native automation should prefer structural/test IDs over visible product copy.

### Rust and persistence tests

Pure and SQLite-backed tests cover rich content, templates, scheduling, local-calendar semantics, providers, transfers, migrations, FTS, card identity, archive/delete behavior, analytics attribution, import/export, backups, managed assets, document processing, and recovery. Released migrations and released-schema fixtures are immutable.

### Native smoke tests

The Linux WebDriver smoke and Android instrumentation smoke exercise real shell boundaries. Mocked frontend tests cannot replace process lifecycle, WebView, system Back, Storage Access Framework, or persistence behavior.

## Manual acceptance

Automation protects known contracts; a human still evaluates user-visible quality and hardware/OEM-specific behavior. For relevant changes, manually consider themes, compact/tablet/wide layouts, touch and keyboard interaction, IME behavior, reduced motion, physical-device behavior, Sharesheet/incoming intents, Keystore, provider UX, and visual quality.

Routine CI command execution and log relaying are not manual acceptance. A Connector-backed coding agent should retrieve self-hosted CI logs/artifacts directly when GitHub exposes them.

## CI workflows

- `check.yml` selects static, fast, diagnostic, or full shared/Linux qualification on the `ostudy` runner class. Diagnostic substitution applies only to draft PRs.
- `android-check.yml` runs on the dedicated `ostudy-android` host and performs either draft Android diagnostics or the authoritative build-once → artifact-reuse emulator chain; it also owns scheduled/manual API-28 compatibility smoke.
- `android-package.yml` creates explicitly requested unsigned QA APK/AAB artifacts on the dedicated Android runner.
- `release.yml` currently creates tagged Linux artifacts.

Concurrency cancellation stops obsolete work for the same pull request/ref. Successful runs qualify only the exact head they tested.

## Failure artifacts and privacy

Failure workflows upload bounded summaries and relevant logs. Android failures may include Gradle reports, APKs, Logcat, screenshots, UI hierarchy dumps, diagnostic JSON, and smoke-case JSON.

Diagnostic export excludes the live SQLite library, cards, tags, templates, managed images, source PDFs, OCR/source text, provider prompts/responses, credentials, Keystore plaintext, and Authorization headers.

## Release qualification

```bash
./scripts/release-check.sh
```

Release qualification repeats the complete shared gate from a clean source copy and verifies version agreement and artifact assumptions. Public Android distribution still requires permanent signing-key and Play App Signing configuration; Windows, macOS, and iOS require their own qualified packaging paths before release documentation can present them as supported artifacts.
