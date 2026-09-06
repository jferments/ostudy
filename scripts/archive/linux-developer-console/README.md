# Archived Linux Ratatui developer console

**Status:** retired from active ostudy development on 2026-09-06.

This directory preserves the Linux Ratatui developer-console prototype that was introduced in PR #89. It is historical source only. Normal setup, build, run, install, and qualification must use the plain-text scripts in `scripts/` and `run-app.sh`.

## Why it was retired

The console used Crossterm/Ratatui raw terminal mode and an alternate screen while child setup/build/test processes also interacted with the controlling terminal. That made interactive boundaries such as sudo/package configuration more complicated than the underlying shell workflow. During manual use, exiting a setup session was also observed to terminate the graphical login session and return the workstation to the login screen. That severe behavior was not root-caused because the decision was made to stop spending engineering time on a nonessential presentation layer.

The plain shell installer/build/test workflow existed before the TUI and remained the authoritative implementation underneath it, so ostudy returned to that simpler model.

## Preserved contents

- `developer-console/` — the separate Ratatui/Crossterm Rust crate.
- `ostudy-dev` — the original launcher that compiled and entered the TUI.
- `check-developer-console.sh` — the prototype's dedicated test/Clippy wrapper.
- `console-events.bash` — the private stage-event helper consumed by the dashboard.
- `DESIGN.md` — the former active developer-console documentation.

PR #91 attempted to add explicit terminal handoff for interactive children, but it was closed without merge once the TUI was retired. This archive intentionally preserves the merged PR #89 implementation rather than treating the abandoned PR #91 experiment as current code.

## If this is revisited later

Do not wire this directory back into normal setup/build/test scripts as part of unrelated work. Revival should happen on a dedicated branch and should start with real-terminal acceptance covering raw-mode cleanup, alternate-screen cleanup, process-group termination, sudo/debconf/curses handoff, and desktop-session safety. Until then, files under this directory are excluded from the maintained developer workflow and should not be used as qualification evidence.
