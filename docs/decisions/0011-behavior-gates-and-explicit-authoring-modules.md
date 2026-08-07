# ADR 0011: Behavior gates and explicit authoring modules

- **Status:** Accepted
- **Date:** 2026-08-06
- **Scope:** Stages 3.9 and 3.10

## Context

Early feature stages used JavaScript verifiers that searched fixed Rust files, React components, CSS classes, helper names, and user-interface wording. Those checks were useful scaffolding while the features were being assembled, but they became a second implementation specification: behavior-preserving refactors could fail because code moved or private names changed.

The native authoring subsystem had the same historical shape. `include!` split one lexical module across stage-numbered files, so imports, private helpers, constants, and transaction assumptions were implicit. The compiler could not enforce meaningful boundaries between basic authoring, generated card types, template behavior, transfer, and generated-file import.

## Decision

The static tier enforces only durable repository contracts:

- exact toolchain and dependency mirrors;
- immutable migration registration and checksums;
- frontend/Tauri IPC command parity;
- no obsolete unbounded command surface;
- no skipped, focused, conditional, or ignored tests;
- immutable GitHub Action SHAs on the controlled self-hosted runner;
- no lexical Rust `include!` composition, stage-numbered production modules, stale-code suppressions, one-off workflows, or executable template behavior.

Feature behavior belongs in Rust, Vitest, SQLite migration/round-trip, and native WebDriver tests. A private helper rename, CSS rename, or source-file move is not a product regression.

The authoring implementation uses ordinary Rust modules. The public Tauri command names and serde contracts remain unchanged, while internal visibility follows the narrowest useful scope. Transaction-owning command/service functions remain responsible for committing one logical mutation; subordinate helpers accept `&Connection` or `&Transaction` and do not secretly commit.

Diagnostics are bounded at both the producer and file-writer boundary. WebView crash reports contain bounded event/message/stack fields, continuous JSONL logging rotates during a running process, and diagnostic export still excludes the SQLite library, flashcard contents, and managed images.

## Consequences

- Refactors can reorganize private source without editing historical feature-stage verifiers.
- The compiler now exposes accidental authoring dependencies.
- The quality gate is smaller but stronger because executable tests own behavior.
- Workflow supply-chain references are immutable and human-readable through version comments.
- Diagnostic failures cannot grow one record or one running log without bound.

## Verification

The acceptance sequence remains:

```bash
./scripts/check-static.sh
./scripts/check-fast.sh
./scripts/check.sh
./run-app.sh
```

A valid behavior-preserving authoring refactor must pass without changing frontend IPC wrappers, migrations, card identities, scheduling/history semantics, package formats, or UI behavior tests.
