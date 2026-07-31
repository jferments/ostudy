# Phase 1 validation record

This file describes the validation expected for the repository-stabilization
patch. It is not a claim that a command passed until its output is recorded in
the pull request and GitHub Actions run.

## Automated gate

```bash
./scripts/check.sh
```

The gate covers exact manifests and lockfiles, React tests, Rust formatting,
Clippy, Rust tests, release-schema fixtures, a standalone Tauri build, resolved
Rust dependencies, and a native WebDriver launch/navigation smoke test.

## Manual acceptance check

After the automated gate:

```bash
./run-app.sh
```

Confirm that the existing binary starts promptly, the UI is branded `ostudy`,
Decks and Settings open, and the displayed application version is `0.1.0`.
Then edit a harmless source file or use an uncommitted test change to confirm the
launcher refuses a stale build with the instruction to run `scripts/build.sh`.

## Deliberate compatibility break

The Tauri identifier changes to `io.github.jferments.ostudy`. The old example
identifier's application-data directory is not migrated. This is intentional
before the first 0.1 release.
