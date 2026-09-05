# Code standards for agents

Write explicit, inspectable, testable code with one clear owner for each rule. Follow nearby conventions unless they violate an accepted invariant. Read [architecture guardrails](ARCHITECTURE-GUARDRAILS.md) and the owning architecture/ADR before changing a cross-module contract.

## General

Prefer cohesive functions and modules, descriptive domain names, and straightforward control flow. Introduce an abstraction only for a demonstrated boundary or repeated policy; splitting by arbitrary line count is not improved architecture. Do not mix unrelated renames, formatting, dependency upgrades, or speculative frameworks into the requested task.

Comments explain why: trust boundaries, ordering, lifecycle traps, compatibility, retry identity, or a non-obvious invariant. Update stale comments with the behavior that invalidates them. Surface recoverable errors with useful context and stable user-facing codes. Do not silently swallow failures that affect durable state.

Do not use `#[allow(dead_code)]` to retain removed product behavior. A target-specific allowance needs a concrete platform/compiler reason. A revised internal request DTO should reject unknown fields when accepting removed or misspelled input would hide a caller defect.

## TypeScript and React

Preserve strict typing; do not use `any` to hide a contract mismatch. Direct native command invocation belongs only in **domain gateway modules under `src/ipc/`**. Platform-specific capabilities belong in `src/platform/`. There is no monolithic IPC-file convention.

Components render views and interaction state; controllers coordinate reads/mutations and reconciliation. Feature-local controller hooks may be colocated with their workspace when that is the current ownership pattern. Derive view-only values rather than duplicating authoritative state. Independent reads need independent stale-result lanes; obsolete successes and errors must not overwrite newer state.

Keep effects narrow, declare dependencies accurately, and clean up listeners, timers, and asynchronous subscriptions. Capture event values before deferred callbacks. Guard pending mutations against repeated submission, preserve operation identity across ambiguous retries, and distinguish a committed write from a later failed refresh.

Handle empty/loading/error/success states deliberately. Reuse semantic HTML, accessible labels, shared overlays, focus restoration, keyboard access, and visible focus. Check compact touch layouts, themes, text scaling, safe areas, and reduced motion. Do not shrink interactive targets merely to remove visual clutter. Shared UI must not import feature controllers, IPC domains, or Tauri APIs.

Use CSS Modules and semantic tokens under the existing ownership system. The application has one foundation CSS entry point; do not add global feature or stage stylesheets. Reuse the canonical editor adapter/configuration and rich-content serialization; do not create a parallel storage format in a screen.

Remove obsolete internal behavior end-to-end: UI state, gateways, native commands, request fields, exclusive styles, tests, verifiers, and documentation. Compatibility must name a supported durable-data or external-format consumer and preserve its regression coverage. Bundled frontend/native IPC is not automatically a public external API.

## Rust and domain services

Use application-owned types at boundaries. External scheduler, parser, renderer, OCR, and HTTP-library types must not define durable schemas or leak throughout unrelated modules. `scheduler.rs` remains the `rs-fsrs` adapter.

Validate IPC/imported content, identifiers, rich-text structure, file handles, limits, and paths natively. Return `Result` and stable errors for recoverable input; avoid panics, unchecked indexing, `unwrap`, `expect`, and lossy conversions in production unless a documented invariant makes failure impossible.

Keep use-case transactions visible. Helpers receive a connection/transaction rather than secretly committing. Validate the logical unit before changing it. Do not hold broad database locks across provider HTTP, document parsing, file pickers, or other unbounded external work. Preserve revision/conflict checks, operation identity, and response-after-commit semantics.

Format with rustfmt and pass applicable Clippy warnings-as-errors checks. When changing a shared struct/enum, inspect production constructors, tests, fixtures, serialization, compatibility readers, and verifier inventories together. Successful frontend tests do not prove native test code compiles.

## SQL, history, and managed files

Use explicit columns, constraints, indexes, bound values, and transactions. Rust-owned query construction may choose SQL structure; untrusted text must not become executable SQL.

Append numbered migrations and registered checksums coherently. Never rewrite a released migration or released fixture. Test fresh and supported upgrade paths, failure rollback, uniqueness, foreign keys, and data preservation. Keep versioned repairs distinct from schema migrations.

Retained review history is append-only evidence; do not rewrite it to simplify analytics. Finalized deletion/purge follows its own explicit ownership contract. Managed images/documents use controlled identifiers and relative locations, not arbitrary remote URLs or local filesystem paths in rich content.

Database and filesystem changes need staging/reconciliation rather than a false claim that a SQL transaction rolls back arbitrary file I/O. Tests must use isolated temporary storage, never the learner's live library.

## Scripts and dependencies

Use Bash with `set -Eeuo pipefail` where appropriate, quote expansions, resolve repository paths explicitly, and fail with actionable messages. Verification must be deterministic and non-interactive. Prefer maintained wrappers over invented Cargo/Tauri/Gradle/ADB alternatives.

GitHub Actions are source-read-only. Formatter/generator verification uses check-only modes; fixes are published by the branch writer. Do not add task-specific applicators, finalizers, encoded source payloads, or source-authoring workflows.

Prefer existing dependencies and standard libraries. New direct dependencies require a written need, compatible license review, exact pins, platform review, and tests. Do not add paid/hosted services as requirements for core local study.

## Documentation and handoff

Use **ostudy** for prose and preserve literal compatibility identifiers, including `io.github.jferments.ostudy`, package names, format/schema identifiers, and URI schemes. Update owning architecture/docs when boundaries change; add or supersede an ADR for a durable cross-module decision.

Follow [documentation maintenance](DOCUMENTATION-MAINTENANCE.md), [the command runbook](COMMAND-RUNBOOK.md), and [GitHub-direct development](GITHUB-DIRECT-DEVELOPMENT.md). State what was inspected, published, executed, or manually verified; never convert inference or a supplied command into a passing-test claim.
