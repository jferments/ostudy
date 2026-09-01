# Canonical build, test, and install runbook

This is the short command reference for AI agents and maintainers. Prefer these maintained repository wrappers over ad-hoc `cargo`, `tauri`, Gradle, ADB, or Vite command lines. If a wrapper fails, diagnose and fix the wrapper or the underlying defect; do not silently invent a parallel workflow.

## First-time / refreshed checkout

```bash
corepack enable
pnpm install --frozen-lockfile
```

Install native dependencies only when the checkout or machine needs them:

```bash
./scripts/install-native-deps.sh
```

## Linux desktop

Iterate with the cheapest relevant gate first:

```bash
./scripts/check-changed.sh
./scripts/check-static.sh
./scripts/check-fast.sh
```

Run the authoritative merge qualification only after focused checks are green:

```bash
./scripts/check.sh
```

Launch the development application:

```bash
./run-app.sh
```

Build desktop bundles:

```bash
pnpm build:app
```

## Android automated qualification

Check the emulator/toolchain environment:

```bash
pnpm android:doctor:emulator
```

Compile/package the Android application and Android-facing tests:

```bash
pnpm android:test
```

Run the official API-36 emulator smoke suite:

```bash
pnpm android:smoke
```

For pull requests, the PR #60 exact-head workflow is authoritative for the complete required Linux/Tauri + Android qualification.

## Android physical-device acceptance

See connected/authorized devices:

```bash
adb devices -l
```

With exactly one connected device, build, verify, install, and launch the normal debug APK:

```bash
pnpm android:install
```

When more than one device is connected:

```bash
pnpm android:install -- --serial <adb-serial>
```

Install the newest already-built debug APK without rebuilding:

```bash
pnpm android:install:existing -- --serial <adb-serial>
```

The maintained install wrapper uses non-streaming ADB installation because it is reliable on the physical Android devices used for acceptance and provides a simpler package-manager boundary than streamed install.

Use `pnpm android:dev` only when live Android hot reload is specifically needed. Ordinary acceptance should use the built debug APK above; do not substitute a custom Tauri device-selection or LAN-host command line.

Retrieve app-private debug diagnostics when needed:

```bash
pnpm android:diagnostics
```

## CI / pull-request workflow

The GitHub remote head and exact-head qualification evidence are authoritative. The PR JSON is a compatibility acceptance snapshot, not an independent state owner.

- Keep the PR draft while implementation or acceptance criteria are incomplete.
- Automatic `Check` is read-only; it must not author or push product source.
- Read the exact-head qualification artifact/logs before changing source after a failure.
- Use focused reproduction for the reported boundary before another broad run.
- A green run qualifies only the exact SHA it tested.
- Do not mark work complete merely because source was published or one gate passed.

## Rule for agents

Before giving build/test/install instructions, read this file and the relevant platform document. Use the shortest maintained wrapper that matches the requested task. If the requested situation is not covered here, inspect the existing scripts/docs first rather than constructing a new command from memory.
