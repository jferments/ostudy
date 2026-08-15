# Agent-driven diagnostics

ostudy has an opt-in multi-failure development mode for Connector-backed agents. It is integrated into the normal `Check` and `Android check` workflows so a diagnostic pass replaces the corresponding draft-PR development gate instead of launching a duplicate third workflow on the same self-hosted runner.

Diagnostics are a **development aid**, not merge qualification. Ready-for-review pull requests ignore diagnostic markers and run the ordinary authoritative fail-fast qualification.

## Why this exists

The authoritative gates stop when a prerequisite fails. That is correct for merge qualification, but inefficient during repair: a TypeScript failure can hide an unrelated Rustfmt error, then a later Vitest failure, then Clippy. `scripts/check-diagnostic.sh` records independent gates as `PASS`, `FAIL`, or `BLOCKED` and continues wherever execution is meaningful.

The diagnostic runner does not maintain a second copy of the commands it is testing. Shared checks live in `scripts/qualification-gates.sh`; Android-specific stages live in `scripts/android-qualification-gates.sh`. Fail-fast and collect-all orchestration call those same gate implementations so the two modes cannot silently drift apart.

## Local commands and evidence

```bash
pnpm check:diagnostic
pnpm android:diagnostic
./scripts/check-diagnostic.sh --profile all --output-dir .diagnostics/all
```

Each sweep writes `results.tsv`, `diagnostics.json`, `summary.md`, and one complete log per gate. Machine-readable results include dependencies, exit codes, elapsed time, and bounded failure highlights. Android diagnostics also preserve emulator evidence and the structured Android smoke result file when the emulator stage runs.

A failing prerequisite causes only genuinely dependent work to become `BLOCKED`. For example, a failed x86_64 Tauri Android build blocks Gradle postbuild and emulator stages, but it does not prevent an independent arm64 build or repository-policy check from reporting its own result.

## Pull-request opt in

A draft pull request may include one marker:

```text
<!-- agent-diagnostics:core -->
<!-- agent-diagnostics:android -->
<!-- agent-diagnostics:all -->
```

`core` makes the standard `Check` workflow run the core diagnostic sweep instead of the normal draft fast tier. `android` makes the standard `Android check` workflow run the Android diagnostic job instead of the ordinary compile → emulator chain. `all` enables both substitutions.

Editing a draft PR to add or change a marker triggers the relevant workflow. Subsequent pushes rerun the selected diagnostic mode. Marking the PR ready for review disables the substitution automatically: `Check` returns to full shared qualification and `Android check` returns to its authoritative build/package and emulator jobs even if the marker remains in the body.

The workflows keep `contents: read`; diagnostics never edit source, commit, push, or write to `main`.

## Core diagnostic profile

The core profile collects the post-UI-overhaul contract set, including deterministic UI token generation and token-usage policy, manifests, IPC, repository and platform boundaries, documentation, test policy, script tests, frozen JavaScript dependencies, TypeScript, Rustfmt, PDFium provisioning, locked Cargo metadata, Vitest, Cargo compile, Clippy, Rust tests, and the frontend production build.

This profile is intentionally broader than a single fail-fast pass. Its purpose is to expose a useful repair batch, not to provide an alternate definition of correctness.

## Android diagnostic profile

Android diagnostics follow the same ownership rules as authoritative Android qualification:

1. qualify/install the pinned SDK, NDK, JDK, Rust targets, and JavaScript dependencies;
2. verify Android project and streaming contracts;
3. preflight the smoke-only frontend bridge before native compilation;
4. compile and package arm64 so the physical-device architecture is exercised;
5. build the x86_64 smoke application **through Tauri** so transient `tauri.settings.gradle`, the private plugin graph, and JNI output exist;
6. run Gradle dependency/Kotlin contracts only after that Tauri build;
7. run platform/app JVM tests, Android Lint, and instrumentation-package assembly while excluding generated Tauri `rustBuild` tasks;
8. verify APK policy and tracked-source cleanliness;
9. run the official emulator against the already-built application and instrumentation APKs.

Direct Gradle qualification before Tauri initialization is forbidden. The diagnostic runner calls the maintained gate wrappers instead of hand-writing Gradle commands.

## Android errors caught earlier

Several checks exist specifically because earlier Android work revealed bad assumptions that should never require a human to rediscover them:

- the smoke bridge is verified in Vite output both when it must exist and when it must not exist; APK byte-grepping is not used as a JavaScript observability boundary;
- the build wrapper restores tracked generated sources, and a post-build cleanliness check fails if any tracked file remains modified;
- Kotlin/coroutines compatibility is checked from the resolved Gradle graph after Tauri initialization;
- Android Lint runs before emulator qualification;
- arm64 compile/package is checked in addition to the x86_64 emulator architecture;
- WebView attachment, bridge initialization, Android Back interception, and overlay teardown are represented by observable readiness conditions rather than fixed sleeps;
- the emulator shell reuses compile-job APK artifacts instead of rebuilding them;
- instrumentation continues independent smoke cases after one case fails and writes `android-smoke-results.json` with case status, duration, prerequisite, device API/page size, and artifact names;
- an instrumentation-only `DocumentsProvider` lets the real Android system picker successfully stream generated JSON, PNG, and PDF fixtures into ostudy and receive a real deck export; cancellation paths remain covered separately;
- stable Flashcards route test IDs are maintained for native automation instead of making future smoke coverage depend on display copy.

## Minimum-SDK compatibility

The normal functional emulator remains the pinned API 36 Google APIs x86_64 image. `versions.env` also pins an API 28 Google APIs image matching the declared minimum SDK. `android-check.yml` runs a scheduled minimum-SDK compatibility smoke once per week and exposes the same run through `workflow_dispatch` mode `min-sdk-smoke`.

Locally, the maintained wrapper is:

```bash
pnpm android:smoke:min
```

This is deliberately periodic rather than part of every PR because booting another emulator for every edit would add substantial runner time. Android Lint carries the cheap minimum-API checks on ordinary Android qualification.

## Connector-owned repair loop

When the self-hosted runner can execute the check, a Connector-backed agent owns the routine feedback loop:

1. publish and remotely verify a coherent branch head;
2. opt the draft PR into the smallest useful diagnostic profile;
3. inspect exact-head workflow jobs through the GitHub Connector;
4. retrieve decoded logs and uploaded artifacts directly;
5. collect the complete actionable independent failure set before editing;
6. repair those failures as one coherent batch on the same PR branch;
7. publish and verify the new head;
8. inspect the new exact-head results and repeat;
9. remove/ignore diagnostic mode by making the PR ready only after development convergence, then require the authoritative merge gates.

Do not ask the human to run `pnpm check:static`, `pnpm check`, `pnpm android:smoke`, or another CI-capable repository command merely to relay output. Human work remains appropriate for visual judgment, physical-device/OEM behavior, protected credentials, host administration, or another boundary GitHub cannot observe.

GitHub Actions is asynchronous. `queued` and `running` are states, not successes; report them accurately rather than inventing completion or using aggressive polling.

## Result semantics

- **PASS** — the gate executed successfully for the recorded head.
- **FAIL** — the gate executed and returned non-zero or timed out.
- **BLOCKED** — the gate did not execute because a declared prerequisite was not `PASS`.

A green diagnostic sweep still does not qualify a PR for merge. The exact published head must pass the ordinary full shared gate and the authoritative Android build/emulator chain when applicable.
