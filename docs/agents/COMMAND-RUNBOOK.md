# Canonical build, test, and install runbook

This is the short command reference for AI agents and maintainers. Prefer these
maintained repository wrappers over ad-hoc `cargo`, `tauri`, Gradle, ADB, or Vite
command lines. If a wrapper fails, diagnose and fix the wrapper or underlying
defect; do not silently invent a parallel workflow.

Read [`DIRECT-WORKSPACE.md`](DIRECT-WORKSPACE.md) for normal ChatGPT execution
and [`EXECUTOR-SELECTION.md`](EXECUTOR-SELECTION.md) before selecting Codex or
the full orchestrator.

## Direct-workspace command mapping

For normal ChatGPT development, select only configured symbolic commands:

```text
workspace_job(action="start", kind="operation", name="format")
workspace_job(action="start", kind="validation", name="changed")
workspace_job(action="start", kind="validation", name="static")
workspace_job(action="start", kind="validation", name="fast")
workspace_job(action="start", kind="validation", name="build")
```

The start call is asynchronous. Use `workspace_job` with `action="status"` or
`action="log"` for bounded progress and `action="cancel"` to terminate the
complete process group. Do not reconstruct arbitrary argv and do not use
chat-side sleep loops.

`format` may mutate only its configured source roots and invalidates previous
validation evidence. Validations must leave the worktree unchanged. Run `static`
and `fast` against the exact current head and diff before publication.

The corresponding human/local shell commands are below.

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

For pull requests, the exact-head `Check` workflow is authoritative for the
complete required Linux/Tauri and Android qualification.

## Android physical-device acceptance

See connected/authorized devices:

```bash
adb devices -l
```

With exactly one connected device, build, verify, install, and launch the normal
debug APK:

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

The maintained install wrapper uses non-streaming ADB installation because it is
reliable on the physical Android devices used for acceptance and provides a
simpler package-manager boundary than streamed install.

Use `pnpm android:dev` only when live Android hot reload is specifically needed.
Ordinary acceptance should use the built debug APK above; do not substitute a
custom Tauri device-selection or LAN-host command line.

Retrieve app-private debug diagnostics when needed:

```bash
pnpm android:diagnostics
```

## CI / pull-request workflow

The GitHub remote head and exact-head qualification evidence are authoritative.
The PR JSON is a compatibility acceptance snapshot, not an independent state
owner.

- Keep the PR draft while implementation or acceptance criteria are incomplete.
- Automatic `Check` is read-only; it must not author, format, or push product
  source.
- Run formatting, static, and fast locally through direct workspace before normal
  publication.
- Read exact-head artifacts/logs before changing source after a CI failure.
- Use focused reproduction for the reported boundary before another broad run.
- A green result qualifies only the exact SHA it tested.
- Do not mark work complete merely because source was published or one gate
  passed.

## Executor rule

Direct workspace is the default even when Codex credits are available. Advisory
Codex does not own source. Implementation Codex owns a separate bounded branch
exclusively. Do not bounce one branch between Codex, direct workspace, and
low-level Connector writes.

## Rule for agents

Before giving build, test, or install instructions, read this file and the
relevant platform document. Use the shortest maintained wrapper or symbolic job
that matches the requested task. If the situation is not covered, inspect the
existing scripts and docs first rather than constructing a new command from
memory.
