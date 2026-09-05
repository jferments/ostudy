# Development guide

This guide maps the current ostudy repository and ordinary development loop. [Architecture](../architecture/overview.md) owns the system design, [Git workflow](workflow.md) owns publication/review, [Testing and CI](testing-and-ci.md) owns qualification, and [local testing/handoff](local-testing-and-handoff.md) owns safe review-checkout and install instructions.

## Current implementation and platform status

ostudy is a React/TypeScript frontend in a Tauri Rust application with a local SQLite library. Linux and Android are the maintained build/test paths. Android includes a native shell, Kotlin platform plugin, packaging, physical-device debug install, document/AI/synchronization integrations, and emulator instrumentation; it is not merely a planned mobile project. Windows/macOS need regular target qualification and packaging. iOS remains planned.

Source-build support, automated qualification, physical-device acceptance, signed packaging, and public distribution are distinct facts. Read current manifests for exact version/tool pins rather than duplicating transient values across prose.

## Repository map

```text
src/
  screens/                 feature workspaces and colocated feature logic
  controllers/             shared workflow/read/reconciliation controllers
  ipc/                     typed native command gateways
  navigation/              application-owned destination/workflow requests
  platform/                frontend OS/capability adapters
  editor/                  application editor contracts and Tiptap implementation
  ui/                      shared interaction, tokens, styles, guidance, effects

src-tauri/
  src/                     Rust commands, services, persistence, and startup
  migrations/              ordered versioned SQLite migrations
  tests/                   integration and compatibility fixtures
  capabilities/            Tauri permission configuration
  gen/android/             reviewed Android shell/project and instrumentation

plugins/ostudy-platform/    narrow native plugin, including Android Kotlin adapters
scripts/                    setup/build/check/package/diagnostic/policy wrappers
tests/                      script contracts, native WebDriver, fixtures/baselines
docs/                       user/install/architecture/development/reference/
                            agent/decision/history documentation
.github/workflows/          source-read-only qualification and packaging workflows
```

A filename left from an earlier implementation is not proof of its current role. Trace imports, command registration, durable readers, platform selection, and tests before removing an apparent leftover. Feature-local deck-library controllers are deliberately colocated; current native document code still has focused facades plus active root `document_*` owners.

## Sources of truth

`versions.env` owns pinned tool/direct-dependency versions, mirrored where standard tools need conventional files. `package.json`/pnpm lockfiles own the JavaScript graph; Cargo manifests/lockfiles own Rust; `database.rs` registers migrations/checksums; `lib.rs` registers the Tauri command surface. Current source establishes implementation; maintained architecture and accepted ADRs explain durable intent.

GitHub is authoritative for published source, PRs, reviews, and exact-head Actions evidence. Normal ChatGPT work uses the GitHub Connector as sole source publisher. Custom developer MCPs, Codex, workspaces, observers, and orchestrators require an explicit current-task request and are not prerequisites. See [GitHub-direct development](../agents/GITHUB-DIRECT-DEVELOPMENT.md).

## Setup and local execution

First-time Ubuntu/Debian setup:

```bash
./scripts/setup-ubuntu.sh
./run-app.sh
```

On an already configured checkout after source changes:

```bash
./scripts/build.sh
./run-app.sh
```

The build wrapper stages a standalone debug application without release bundles. `run-app.sh` verifies source/build provenance and launches that existing artifact; it does not compile or start Vite. Do not bypass a stale-build refusal by launching Cargo's mutable debug executable.

`pnpm desktop` starts native hot reload. `pnpm dev` is Vite and `pnpm build` builds frontend assets. Android setup/install/testing is documented in [Android installation](../installation/android.md). `pnpm android:install` is the normal physical-device debug build/install; `pnpm android:dev` is deliberate hot reload.

Before switching code, inspect local changes and verify the intended SHA. A separate source worktree does not isolate the learner library. Use [local-testing data isolation](local-testing-and-handoff.md) and disposable Android data for destructive/migration/restore/fresh-install tests.

## Iteration and qualification

```bash
pnpm verify:docs
./scripts/check-changed.sh
./scripts/check-static.sh
./scripts/check-fast.sh
./scripts/check.sh
```

Use focused tests and the cheapest useful tier while editing, then required final-head qualification. `check-changed` depends on the diff and is not final certification. Exact-tree stamps can reuse valid previous tiers only when their source/tool/platform identity matches; `OSTUDY_FORCE_CHECK=1` is an explicit evidence/diagnostic override, not a substitute for understanding what was tested.

GitHub Actions qualifies published source at an exact SHA. Draft automatic `Check` uses the fast gate. Ready PRs and explicit full dispatches provide full/platform qualification. `PR Contract` validates acceptance metadata separately and is not a prerequisite for product `Check`. Every Actions workflow is source-read-only.

When a gate fails, inspect the exact tested SHA, bounded logs/evidence, and smallest failing boundary. Fix that boundary before another broad matrix. Native compilation, frontend mocks, WebDriver persistence, Android package verification, emulator instrumentation, physical-device behavior, and live-provider calls establish different evidence.

## Frontend changes

Screens render data and intent. Controllers coordinate workflow identity, stale-read policy, mutations, and reconciliation. Typed native calls stay under `src/ipc/`; shared `src/ui/` does not import feature IPC/controllers/Tauri APIs. `src/platform/` owns presentation-facing OS integration.

Use shared overlays, focus/Back handling, keyboard alternatives, CSS Modules, semantic tokens, and responsive patterns. Preserve compact touch targets, enlarged text, themes, safe areas, reduced motion, and dirty-state guards. Independent async reads use independent lanes. A committed mutation must remain distinguishable from a following failed refresh.

Current main information architecture is Flashcards, Study Guides, Documents, Analytics, and Settings. Flashcards has Decks and Search/Filter. Card templates live in Settings. Transfer is contextual. Practice is an internal contextual workflow. Do not restore retired permanent tabs or empty-state clutter because an old stage document still contains them.

## Native data and learning changes

Rust owns bounded command validation, application services, transaction scope, and safe errors. SQLite is the structured authority; managed files use staged/reconciled lifecycle protocols. A SQL transaction does not make external filesystem/secret/network work atomic.

Append migrations and register checksums coherently. Preserve released migration/repair identities, supported readers, and upgrade fixtures. Define rollback, deletion, backup/import/export, sync allowlist, and compatibility effects explicitly. Avoid broad database locks across provider requests, OCR, or file pickers.

Notes own editable source; cards own independent study presentations. `scheduler.rs` isolates `rs-fsrs`; current schedules and retained review evidence are different entities. Scheduling controls must not fabricate reviews. Calendar semantics, preview revisions, operation identity, historical attribution, and finalized deletion need native regression coverage.

## AI and document changes

Gateway transport, profile aggregates, capability probes, provider dialects, ingestion adapters, canonical source reasoning, and authoring publication have separate owners. Current non-PDF ingestion uses Xberg; PDF has native text/render/OCR paths with desktop and Android adapters. Convert third-party types into ostudy-owned contracts before crossing subsystem boundaries.

A model can propose bounded structured material, not execute native commands or SQL. Saved credentials stay behind the native secret store; a newly entered key is transient write input, not a saved-secret readback API. Raw prompts/responses/reasoning stay out of routine logs; validated learning content is intentionally stored in the library.

Study Guides are current first-class artifacts. They share stable learning topics with generated cards while preserving generated revisions, learner notes/section overrides, and conflicts independently. Whole-library backup covers a broader document/guide graph than current device sync. Extend external formats and sync scope deliberately rather than assuming every table transfers automatically.

## Platform changes

Native/platform adapters own file selection/publication, secure storage, lifecycle, clock/time-zone facts, resource observations, Back, sharing, PDF/OCR, QR acquisition, and other OS behavior. Android Kotlin/plugin code must not implement scheduling, analytics, SQLite migrations, package validation, provider fallback, or sync merge policy.

Current Android QR acquisition uses Google Code Scanner through Play services; document OCR uses ML Kit and is a separate capability. A real unavailable-capability fallback is not dead code simply because the primary development machine does not display it.

## Documentation and cleanup

Write for the reader:

- root README — product purpose/current features/platform status/short start;
- user guides — tasks and concepts;
- installation — prerequisites and honest qualification/distribution status;
- architecture — ownership, data flow, trust, invariants, tradeoffs;
- development/agents — commands, publication, evidence, diagnostics;
- reference — exact external/versioned formats;
- decisions/history — durable rationale and completed/superseded stages.

Update canonical paragraphs instead of appending contradictory PR notes. Keep indexes/links current. Move completed stage/checkpoint records to history or leave only a clear pointer when a stable old path is worth preserving.

Trace production, test, dynamic, platform, and durable-compatibility consumers before deleting code. Remove exclusive styles/tests/scripts when their owner is proven unused, but keep released readers/migrations and real platform fallbacks. See [Documentation maintenance](../agents/DOCUMENTATION-MAINTENANCE.md) and [cleanup audit](documentation-and-cleanup-audit.md).

## Dependencies, diagnostics, and release

Direct dependencies are pinned and need demonstrated need, license/trust/platform review, and qualification. Initial provisioning can use the network even though installed core study is local-first. See [Dependency audit](dependency-audit.md).

Diagnostics preserve bounded operational context without the learner database, source bytes, raw provider payloads, reasoning, or credentials. Review exported metadata before sharing. See [Diagnostics](diagnostics.md).

A handoff names exact base/branch/head/PR, changed areas, actual checks and tested SHAs, unrun/manual boundaries, known failures, compatibility/rollback risk, and writer release. It supplies a short exact checkout/build/run/Android install path and concrete observations. A normal feature merge does not automatically tag/release/version-bump. See [Git workflow](workflow.md), [Testing and CI](testing-and-ci.md), and [local testing](local-testing-and-handoff.md).
