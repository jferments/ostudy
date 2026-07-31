# Code standards for agents

The goal is code that is explicit, inspectable, testable, and difficult to
misuse. Match nearby conventions unless doing so would violate an accepted
invariant.

## General standards

- Prefer small cohesive functions and modules with one clear responsibility.
- Use descriptive domain names instead of vague names such as `data`, `item`,
  `thing`, or `handler` when a more precise term exists.
- Prefer explicit control flow over compressed or clever expressions.
- Do not introduce an abstraction until the current change demonstrates a real
  repeated boundary or policy.
- Do not perform unrelated renames, formatting passes, or file moves.
- Surface errors with useful context while preserving stable user-facing error
  codes.
- Never silently ignore an error that can affect durable state.
- Comments explain **why**: invariants, ordering, trust boundaries, lifecycle
  traps, compatibility, and retry behavior. Do not narrate obvious syntax.
- Delete or update stale comments in the same change that invalidates them.

## TypeScript and React

- Preserve strict typing. Do not use `any` to bypass a type problem.
- Keep Tauri calls inside typed wrappers in `src/ipc.ts`.
- Components own rendering and interaction state, not durable domain logic.
- Prefer derived values over duplicated state that can become inconsistent.
- Keep effects narrowly scoped, declare dependencies honestly, and clean up
  subscriptions, timers, and asynchronous work.
- Capture values from React events before entering deferred callbacks or state
  updaters.
- Guard pending mutations so repeated clicks cannot issue duplicate writes.
- Handle empty, loading, error, and success states intentionally.
- Preserve semantic HTML, labels, focus order, keyboard access, and visible
  focus.
- Verify behavior under Light, Dark, Console, and Synthwave themes when UI or
  styling changes.
- Reuse the canonical editor configuration and serialization helpers; do not
  create alternate rich-content formats inside components.

## Rust

- Format with `rustfmt` and pass Clippy with warnings denied.
- Represent domain concepts with explicit application-owned types.
- Validate all IPC and imported data at the Rust boundary.
- Return `Result` values and stable application errors; do not panic on
  recoverable runtime input.
- Avoid `unwrap`, `expect`, unchecked indexing, and lossy conversions in
  production paths unless an invariant makes failure impossible and the reason
  is documented.
- Keep transaction boundaries visible. A multi-step durable operation either
  commits completely or leaves no partial structured state.
- Preserve idempotency for operations that can be retried.
- Keep scheduler-crate types inside the scheduler adapter.
- Keep SQL and filesystem side effects out of presentation-oriented command
  plumbing when a domain module already owns the use case.

## SQL and migrations

- Use explicit columns, constraints, and indexes that encode durable invariants.
- Parameterize values; never construct SQL from untrusted string fragments.
- Add append-only numbered migrations. Never modify a released migration.
- Update the migration checksum registry and migration tests together.
- Test fresh creation, upgrade behavior, rollback on failure, and relevant
  uniqueness or foreign-key constraints.
- Treat the immutable review ledger as append-only; do not rewrite history to
  simplify analytics.

## Shell and verification scripts

- Use `#!/usr/bin/env bash` and `set -Eeuo pipefail` for Bash scripts.
- Quote expansions unless intentional word splitting is required and explained.
- Resolve paths relative to the script rather than assuming the current working
  directory.
- Fail loudly with actionable messages.
- Keep verification deterministic and non-interactive.

## Dependencies

- Prefer the standard library and existing dependencies.
- Any new direct dependency requires a written need, free/open-source license
  review, exact version pin, platform review, and tests.
- Keep dependency upgrades separate from feature changes whenever practical.
- Do not introduce a hosted or paid service as a requirement for core behavior.

## Documentation

- Use the product name **ostudy** for human-facing project references.
- Preserve literal technical identifiers when compatibility depends on them,
  including `com.example.studyapp`, package names, schema identifiers, and URI
  schemes, unless the task includes an explicit migration.
- Update architecture documentation when boundaries change.
- Add or supersede an ADR when a durable cross-module decision changes.
- State what was actually tested. Do not convert inference into a validation
  claim.
