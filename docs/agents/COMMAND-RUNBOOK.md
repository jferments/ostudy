# Canonical build, test, and install runbook

Read this before giving shell instructions. Commands below are maintained repository entry points, not evidence that they have already run. Execute them from the repository root after checking out the intended source. [Local testing and handoff](../development/local-testing-and-handoff.md) provides safe checkout, SHA verification, isolated data, and acceptance-report guidance.

## First-time Linux setup

```bash
./scripts/setup-ubuntu.sh
./run-app.sh
```

The setup wrapper installs/provisions the pinned development environment and makes an initial standalone debug build. It is not a routine command to repeat for every PR. Use [Linux installation](../installation/linux.md) for system prerequisites and optional OCR dependencies.

## Existing Linux checkout: build and test-drive

```bash
./scripts/build.sh
./run-app.sh
```

`pnpm build:app` is an alias for `scripts/build.sh`. It makes a **standalone debug application, without bundles**, through Tauri's build path. It is not a release-package command.

`run-app.sh` does not build or start a Vite server. It verifies the source fingerprint, checkout, and staged binary checksum, then launches the existing standalone artifact. Do not bypass a stale-build error by launching Cargo's mutable `debug/ostudy` output. Rebuild with the maintained wrapper.

Use `pnpm desktop` only for deliberate Tauri hot-reload development. `pnpm dev` is the frontend Vite server; `pnpm build` produces frontend assets. Neither is a complete native install or native runtime acceptance test.

## Validation

```bash
pnpm verify:docs
node ./scripts/verify-agent-workflow-policy.mjs
./scripts/check-changed.sh
./scripts/check-static.sh
./scripts/check-fast.sh
./scripts/check.sh
```

Choose the smallest relevant command during iteration. The first two are focused documentation/workflow checks; they do not replace broader gates when required. `check-changed` selects by the current diff, so a clean checkout with no relevant diff can do little. Use explicit static/fast/full wrappers when certifying a particular tier.

For a specific frontend failure, use `pnpm exec vitest run` followed by the exact existing test path identified in the failure. Do not invent test filenames. For native failures, use the focused command supplied by the current runbook or qualification evidence before rerunning the full matrix.

## Android environment and automated qualification

```bash
pnpm android:doctor
pnpm android:doctor:emulator
pnpm android:test
pnpm android:smoke
```

First-time SDK/NDK/JDK and emulator setup is in [Android installation](../installation/android.md). Compile/package tests and emulator smoke are different gates. Neither proves physical-device camera, OEM, or live-provider acceptance.

## Android physical-device test-drive

```bash
adb devices -l
pnpm android:install
```

With exactly one authorized compatible device, the install wrapper builds a normal ARM64 debug APK, verifies it, installs with non-streaming `adb install -r`, and launches `io.github.jferments.ostudy.dev`. It does not use the dev-server/hot-reload workflow.

For multiple devices, use the wrapper's explicit serial argument:

```bash
bash ./scripts/android-install.sh --serial "REPLACE_WITH_ADB_SERIAL"
```

To reuse an already-built debug APK:

```bash
bash ./scripts/android-install.sh --no-build --serial "REPLACE_WITH_ADB_SERIAL"
```

The no-build path chooses the newest matching build-mode APK by modification time. It does **not** prove the APK belongs to the current Git SHA or intended ABI. Prefer a fresh normal build/install for PR acceptance; use no-build only with independently known artifact provenance. Do not add an extra `--` sentinel to the direct Bash wrapper; its parser accepts the named options shown above.

Debug and release package IDs/data are separate. Reinstalling with `-r` is not a fresh-data test. Uninstalling or clearing app data can destroy that installation's library and must never be a routine troubleshooting instruction without backup and explicit consent.

```bash
pnpm android:diagnostics
```

Use `pnpm android:dev` only when live hot reload is the actual goal. Use the existing Android build scripts for emulator/ABI or release packaging rather than custom Tauri/Gradle commands reconstructed from memory.

## Packages and releases

Read [Android build/install](../development/android-build-and-install.md), [Linux installation](../installation/linux.md), and [the development workflow](../development/workflow.md) for release packaging. A debug executable, debug APK, release bundle, signed artifact, and public distribution are distinct deliverables. Do not infer release availability from a script's existence.

## Agent handoff rule

Give the exact branch and verified head, checkout commands, the shortest applicable build/run/install path, actual qualification results, and concrete manual observations. Do not require the human to run every tier merely to test-drive the app, and do not claim those tiers passed because the commands were supplied. GitHub-direct publication and read-only Actions qualification remain the normal workflow.
