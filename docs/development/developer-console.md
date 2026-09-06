# Linux developer console

`./scripts/ostudy-dev` is the human-facing terminal dashboard for Linux development. It does **not** replace the maintained build/test/run scripts or create a second qualification system. It starts those same wrappers, watches a small structured stage stream, summarizes useful test output, and writes the child process output to a durable plain-text log.

## Why Ratatui

The console uses [Ratatui](https://ratatui.rs/) with Crossterm rather than a Python/Textual application. Textual has a strong high-level widget system, including progress, logging, and sparkline widgets, and would have been a reasonable UI choice. Ratatui fits ostudy better because the project already pins a Rust toolchain, the console can compile into one small native executable, and it avoids making Python package management part of ordinary interactive development.

The TUI lives in the separate `scripts/developer-console/` Cargo package. It is deliberately **not** a dependency of `src-tauri`: Android/mobile/application builds do not carry terminal UI code or its dependencies.

Direct terminal dependencies are exact-pinned through `versions.env` and the console manifest. The console package is built and tested from a cache/temporary source copy so Cargo's generated developer-tool lock state never mutates maintained source during a build or qualification run.

## Starting it

From the repository root on an already configured Linux checkout:

```bash
./scripts/ostudy-dev
```

The menu currently exposes:

```text
run
build
check-changed
check-static
check-fast
check
desktop
setup
```

A workflow can also be launched directly:

```bash
./scripts/ostudy-dev build
./scripts/ostudy-dev check-fast
./scripts/ostudy-dev run
```

`--plain` bypasses the dashboard and runs the same underlying wrapper normally:

```bash
./scripts/ostudy-dev check-fast --plain
```

Non-interactive invocations automatically use plain mode when a workflow was supplied. This keeps pipes, CI, redirected output, and terminal-less automation predictable. GitHub Actions continues to call the maintained scripts directly.

## First-time setup

The first `./scripts/ostudy-dev setup` has a bootstrapping constraint: Ratatui is a Rust program, while the setup workflow is responsible for installing Rust. If Cargo is not available yet, the launcher therefore runs `scripts/setup-ubuntu.sh` in the ordinary terminal. Once setup completes, the console can compile into the user's XDG cache and future setup/build/test sessions can use the dashboard.

When Rust is already present, the launcher performs `sudo -v` **before** entering the alternate terminal screen. Password prompts therefore remain normal shell prompts instead of corrupting the dashboard.

## What the dashboard shows

The top of the console uses ostudy's violet `#8b5cf6` identity color, a terminal wordmark, and a raccoon ASCII panel. During a workflow the screen is stable rather than becoming a scrollback of hundreds of test lines.

The running view contains:

- a **session** tile with the exact maintained shell command, elapsed time, child PID, and raw log path;
- a **workflow progress** tile driven by explicit stage boundaries from the existing scripts;
- a **test signal** tile with passing/failing/skipped counts, a proportional pass/fail bar, and a small list of recently completed Rust/Vitest/Python tests;
- a bounded **recent output** tile for high-value process output;
- an **errors** view containing ANSI-free copy/paste-ready lines and a few lines of context after each detected error.

The TUI intentionally does not try to reimplement every test runner's protocol. It recognizes stable high-value output shapes and treats the complete log as the source for anything it cannot summarize. A test runner that does not expose a current test name cannot be made to expose one by the dashboard; in that case the console shows the current repository stage and the latest completed tests instead.

Press `e` while running or after completion to toggle recent output and captured errors. `q`, `Esc`, or `Ctrl-C` terminates the workflow process group rather than leaving a compiler/test child detached in the background.

## Logs

The dashboard does not discard output. Every TUI workflow gets a raw ANSI-free log under:

```text
${XDG_STATE_HOME:-$HOME/.local/state}/ostudy/dev-console/
```

Set `OSTUDY_DEV_LOG_DIR` to redirect these developer-console logs. The existing application launcher continues to maintain its independent structured application log at `~/.local/state/ostudy/ostudy.jsonl` (or the equivalent XDG state path).

After the TUI restores the normal terminal, failed workflows print the log path plus a short error tail so the important diagnostics remain easy to select and paste into an issue or chat.

## Relationship to the maintained scripts

The TUI is an orchestration/presentation layer only. `scripts/build.sh`, `scripts/check-static.sh`, `scripts/check-fast.sh`, `scripts/check.sh`, `scripts/check-changed.sh`, `scripts/dev.sh`, `scripts/setup-ubuntu.sh`, and `run-app.sh` retain their existing responsibilities and exit codes.

When `OSTUDY_CONSOLE_EVENTS=1`, those wrappers emit private lines in this form:

```text
@@OSTUDY:TUI@@\tstage\t<scope>\t<stage name>
```

`scripts/lib/console-events.bash` owns that tiny protocol. With the environment variable absent, it emits nothing, so normal shell output and existing CI behavior remain unchanged. The TUI consumes these lines as control information and excludes them from the human raw log.

This is preferable to scraping blue `==>` headings forever: presentation can change without silently breaking progress accounting, and the scripts remain usable without the console.

## Console qualification

`check-fast.sh` contains a dedicated **Developer console tests** stage. `scripts/check-developer-console.sh` copies the console crate into a temporary directory, then runs its unit tests and warnings-as-errors Clippy gate. This design exercises dependency resolution and compilation without allowing Cargo to add a lockfile to maintained source during a supposedly read-only qualification.

Parser tests cover ANSI removal, structured stage events, Rust test results, Vitest file/count summaries, and copyable compiler errors. The normal repository script syntax/worktree guards continue to cover the shell integration.

The dashboard is currently a Linux developer experience. The underlying scripts remain the portable contract; Windows/macOS/mobile distribution is expected to use prebuilt packages rather than this source-build console.
