# Testing and CI

ostudy uses tiered validation locally and on self-hosted runners. Each tier answers a different question; inexpensive tiers must not be described as complete application qualification.

The repository does not use GitHub-hosted operating-system runners. Shared/Linux and Android compile jobs currently target:

```yaml
runs-on: [self-hosted, linux, x64, ostudy]
```

The official Android Emulator additionally requires a KVM-capable self-hosted runner. Third-party actions are pinned to immutable commit SHAs with a human-readable version comment.

## Why the tiers exist

The complete Tauri build, Rust suite, native WebDriver workflow, Android cross-build, Gradle tests, and emulator boot are valuable but expensive. Running all of them after every small edit wastes time and obscures the first useful failure. Shared tiers progress from cheap structural guarantees to full native behavior:

```text
changed → static → fast → full
```

Android adds a target-specific path:

```text
Android project contracts → cross-build/package → official-emulator smoke
```

An exact-tree stamp allows a later shared tier to reuse an earlier successful result only when source, relevant untracked files, deletions, tool versions, and platform still match. `OSTUDY_FORCE_CHECK=1` disables reuse when testing the gate itself or investigating a suspicious cache result.

## Changed-file iteration

```bash
./scripts/check-changed.sh
```

This is a development aid, not merge qualification. It compares the branch and working tree with `origin/main` when available and routes the change:

- documentation-only changes run documentation and static repository contracts;
- frontend-only changes run static checks and changed Vitest selection;
- Rust-only changes run static checks, locked compile checking, and focused library tests;
- document-related Rust changes add focused document contract filters;
- migrations, manifests, workflows, scripts, native dependencies, Android project files, or mixed changes escalate to the fast or target-specific tier.

The routing is conservative. When the script cannot prove that a narrow selection is sufficient, it chooses the broader tier.

## Static qualification

```bash
./scripts/check-static.sh
```

The static tier verifies:

- pinned environment and manifest relationships;
- committed JavaScript and Cargo lockfiles;
- migration registration and checksums;
- frontend IPC names against the native command registry;
- direct Tauri invocation limited to typed domain gateways;
- repository module, provider-network, credential, UI, workflow, and test policies;
- documentation links, stale canonical paths, and top-level organization;
- Bash and Node syntax plus Python script behavior tests;
- frozen JavaScript installation and exact direct dependency versions;
- TypeScript project checking;
- canonical Rust formatting.

This tier is the minimum check for a documentation reorganization because a moved file can break agent instructions, issue forms, scripts, or reader navigation even when no executable source changed.

## Fast qualification

```bash
./scripts/check-fast.sh
```

The fast tier reuses a matching static result, provisions or verifies the pinned PDFium runtime, then runs:

- the complete frontend Vitest suite;
- locked Rust compile checking for the supported target and feature set;
- focused document contracts that protect source-format, semantic-window, provider, OCR, rendering, and checkpoint boundaries.

Draft pull requests use the fast tier. Controller tests deliberately resolve independent reads out of order and verify that obsolete successes and errors cannot replace newer screen state. Mutation tests preserve committed success when a later reconciliation read fails.

## Full shared/Linux qualification

```bash
./scripts/check.sh
```

The full tier reuses a matching fast result and adds:

- native environment qualification, including WebKit/WebDriver and native runtime checks;
- production frontend build;
- Clippy with warnings denied;
- all Rust unit, integration, migration, import/export, backup, scheduling, analytics, AI, and document tests;
- standalone Tauri build with locked Cargo dependencies;
- resolved Rust dependency verification;
- a native WebDriver create → practice → restart → persistence workflow.

This remains the authoritative shared/local merge gate. Android changes must pass it because Android reuses the same application and domain code.

The native Linux smoke test can be skipped temporarily with `OSTUDY_SKIP_NATIVE_SMOKE=1`, but the pull request must state that it was skipped and why. CI normally runs it.

## Android qualification

### Compile and package tier

```bash
./scripts/android-test.sh
```

This tier verifies the pinned SDK/NDK/JDK environment and Android project, then runs shared TypeScript/Rust tests, Kotlin/JVM tests, Rust Android cross-compilation, x86_64 APK generation, instrumentation-package compilation, manifest/permission checks, and 16 KiB ZIP/ELF alignment checks.

`android-check.yml` runs this tier for Android pull requests and pushes to `main`. APKs, instrumentation packages, and Gradle reports are uploaded as short-lived diagnostic artifacts.

### Official Android Emulator smoke

```bash
./scripts/android-emulator-smoke.sh
```

The emulator tier uses the pinned API 36 Google APIs image. It installs an explicitly test-enabled x86_64 APK and instrumentation package, then verifies:

- cold boot and SQLite migration;
- deck/card creation;
- practice review and analytics;
- persistence after force-stop/restart;
- Android Documents development stub;
- system Back behavior;
- revealed practice restoration after WebView reload and process restart.

The test-only JavaScript bridge is opt-in at build time and forbidden in release APKs. Waydroid may be used for informal manual testing but does not replace official-emulator qualification.

The emulator job is manually dispatchable because it requires KVM and is substantially more expensive than compilation. A merge that changes Android runtime, lifecycle, files, sharing, secrets, networking, or navigation should record an emulator run or explain why it was unavailable and complete the equivalent manual physical-device checklist.

## Important test layers

### Repository contracts

Static scripts protect stable boundaries such as exact versions, immutable migrations, IPC parity, self-hosted workflow policy, dependency isolation, documentation organization, Android permissions/package identity, and absence of skipped/focused/ignored tests. They should verify behaviorally meaningful ownership rules rather than brittle private helper names.

### Frontend and controller tests

Vitest and Testing Library cover user interaction, accessible relationships, error/empty/loading states, responsive primitives, editor behavior, controller request ordering, mobile Back integration, checkpoints, and process-restoration state machines. Independent request lanes are resolved in deliberately surprising orders to prove that stale reads cannot update current state.

### Pure Rust tests

Pure tests cover rich-content validation, cloze parsing and presentation, template substitution, duplicate normalization, scheduling adapters, local-calendar semantics, provider schema conversion, endpoint policy, semantic partitioning, resource planning, safe error normalization, and practice-session restoration.

### SQLite integration and migration tests

Integration tests cover transactions, CRUD, FTS triggers, card identity, scheduling/history preservation, move/copy, archive/delete semantics, analytics attribution, import/export, backups, managed assets, startup repair, and idempotent retry behavior.

Released schema fixtures load representative historical data, apply current migrations and repairs, verify preserved behavior, and run SQLite integrity checks. Released migration files and fixtures are never edited to make a new test pass.

### Generated-import conformance

The `.ostudy.json` harness materializes real files under unique temporary roots and exercises valid Markdown, Basic/Reverse/Cloze behavior, Unicode, limits, unknown fields, unsafe syntax, duplicates, exact-name create/append behavior, fresh schedules, transaction atomicity, and cleanup. It never opens the user's live library.

### Provider and document contracts

Provider fixtures use deterministic local servers rather than paid internet credentials. Document tests cover extraction quality, malformed text, semantic windows, outline assembly, rendering, OCR selection, cancellation, checkpoints, source locators, provider budgets, structured output, generated-card validation, backup, and deletion lifecycle. Android A1–A3 must preserve these records while showing an explicit unsupported document-processing capability.

### Native smoke tests

The Linux native smoke launches the built Tauri binary through WebDriver, performs a real create and review workflow, restarts the application, and confirms persistence. The Android native smoke performs the corresponding workflow through the official emulator and additionally tests Back, system lifecycle, and mobile restoration. Mocked frontend tests cannot replace either native shell boundary.

## Manual acceptance

Automation protects known contracts. A human still evaluates the changed workflow in the built application. Test what actually changed rather than copying an unrelated historical checklist.

For user-visible work, consider:

- normal, empty, loading, validation, provider, cancellation, and recovery states;
- persistence after restart and process death;
- Light, Dark, Console, and Synthwave themes;
- compact phone, tablet, landscape, split-window, and wide layouts;
- keyboard-only and coarse-pointer/touch interaction;
- reduced motion and high interface/font scale;
- import/export, backup, sharing, migration, and deletion effects;
- Android document providers, Sharesheet, incoming intents, Keystore, Back, and IME behavior;
- close/exit behavior during active practice or document work;
- privacy of diagnostics and errors.

Record the exact paths exercised and any unavailable platform test.

## CI workflows

- `check.yml` selects static, fast, or full shared/Linux qualification.
- `android-check.yml` cross-builds and package-checks Android; manual dispatch can add the official emulator smoke suite.
- `android-package.yml` creates unsigned QA APK/AAB artifacts through explicit dispatch.
- `release.yml` currently creates tagged Linux artifacts.

Concurrency cancellation stops obsolete runs for the same pull request or ref. Successful runs must belong to the current head SHA. CI uses persistent build caches outside the checkout, while Git and toolchain fingerprints continue to control correctness.

## Failure artifacts and diagnostics

On failure, workflows write bounded summaries and upload relevant logs. Android failures may additionally include Gradle reports, APKs, Logcat, screenshots, and UI hierarchy dumps.

Diagnostic export excludes the SQLite library, cards, tags, templates, managed images, source PDFs, OCR/source text, provider prompts or responses, credentials, Keystore plaintext, and Authorization headers.

## Release qualification

```bash
./scripts/release-check.sh
```

The release check repeats the complete shared gate from a clean source copy and verifies version agreement and artifact assumptions. The current tagged release workflow builds Linux artifacts. Android can produce unsigned QA APK/AAB packages; public distribution still requires the permanent signing key and Play App Signing configuration. Windows, macOS, and iOS require their own qualified runners and packaging before release documentation can present them as supported artifacts.
