# ADR 0001: Do not run a project generator during normal startup

## Status

Accepted.

## Decision

The source repository contains the complete Tauri, React, TypeScript, and Vite
project skeleton. `run-app.sh` installs and verifies dependencies; it does not
run `create-tauri-app`, `pnpm add`, or `cargo add`.

## Rationale

A runtime scaffolder introduces an unnecessary moving part before the app can
start. It can disappear from a registry, change its template, add new
transitive dependencies, or prompt for input. Keeping the project files under
version control makes changes reviewable and reproducible.
