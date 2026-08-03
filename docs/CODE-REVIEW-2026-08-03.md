# Full repository code review — 2026-08-03

## Purpose

This document is the durable review ledger for **sub-task 1** of the post-core-feature cleanup effort. The review baseline is the `main` branch immediately after PR #18, commit `429de0c302468186d525f65fcaeff46c09d187b4`.

This sub-task is intentionally limited to:

- inspecting the complete repository;
- documenting the current architecture and its invariants;
- adding explanatory comments where code has non-obvious safety, compatibility, or transactional requirements;
- identifying stale code, complexity, duplication, performance risks, weak boundaries, and future refactoring opportunities.

It does **not** change runtime behavior, database schemas, command contracts, build steps, test coverage, or user-visible functionality. Recommendations in this report are candidates for the later refactoring and testing sub-tasks, not authorization to modify those systems in this PR.

## Review method

The review treats the merged `main` tree as the sole source of truth. Historical pull-request implementations are useful for understanding why compatibility code exists, but a file is not considered active merely because it appeared in an old PR.

The review is organized by architectural layer:

1. repository configuration, dependency pinning, and release scripts;
2. Tauri composition root and IPC registration;
3. SQLite lifecycle, migrations, normalization, and shared infrastructure;
4. decks, notes, generated cards, assets, tags, and reusable templates;
5. practice selection, FSRS scheduling, schedule controls, and analytics;
6. deck interchange and backwards-compatibility adapters;
7. React navigation, screens, editors, IPC wrappers, and styles;
8. current unit, integration, migration, and native smoke tests;
9. project documentation and architecture decision records.

Findings use these priorities:

- **P0 — correctness/data-loss risk:** should block unrelated structural work until understood.
- **P1 — high-value structural risk:** materially affects maintainability, extensibility, or reliability.
- **P2 — moderate improvement:** worthwhile cleanup or performance work without an immediate correctness threat.
- **P3 — low-risk polish:** readability, naming, localized duplication, or documentation cleanup.

## Architectural strengths to preserve

The current codebase already has several strong boundaries that future refactoring should not weaken:

- Rust is authoritative for validation, scheduling, durable writes, import safety, and multi-step transactions.
- SQLite migrations are immutable and checksummed; a historical migration mismatch stops startup rather than silently rewriting user data.
- Notes and generated cards have distinct identities, allowing reverse and cloze presentations to retain independent scheduling semantics where intended.
- Review history is append-only while `card_schedules` acts as the mutable projection used for fast workload selection.
- Imported packages do not carry local schedules, review history, settings, or arbitrary executable content.
- Search uses bounded, quoted tokens and bound SQL parameters instead of accepting raw FTS syntax.
- Reusable templates are data-only authoring tools and do not own generated notes after creation.
- Optimistic revisions are used for user-edited entities so stale editors fail visibly rather than overwriting newer data.
- The repository pins toolchain and application dependencies and runs a frozen, multi-language build gate.

These are architectural invariants, not incidental implementation details.

## Findings ledger

The ledger will be expanded and re-prioritized as each repository area is completed. No recommendation below is implemented by this sub-task.

### CR-001 — `include!` composition creates one oversized authoring namespace

- **Priority:** P1
- **Area:** `src-tauri/src/authoring.rs`, `src-tauri/src/authoring/stage3_v2.rs`
- **Observation:** The authoring implementation is physically split into files but lexically compiled into one Rust module through nested `include!` calls. This gives every included item broad access to the same private namespace, makes imports and ownership difficult to see, increases collision risk, and prevents the compiler from enforcing intended submodule boundaries.
- **Why it matters:** The files look modular in the repository, but their coupling is substantially higher than ordinary `mod`-based modules. A future contributor can unintentionally depend on any private helper defined in another included file.
- **Later direction:** Convert the authoring tree to ordinary modules with explicit `pub(super)`/`pub(crate)` APIs, organized around cohesive domains such as card content, search, template rendering, generated-card lifecycle, and transfer normalization. Preserve command names and transactional behavior during that conversion.

### CR-002 — frontend IPC contracts are manually duplicated and concentrated in one file

- **Priority:** P1
- **Area:** `src/ipc.ts`, `src/analytics-ipc.ts`, Rust command DTOs
- **Observation:** Most Rust DTOs and command inputs have hand-maintained TypeScript counterparts. `src/ipc.ts` now combines contracts and wrappers for nearly every feature.
- **Why it matters:** Rust/TypeScript drift is caught only when a test exercises the affected path. The file is also becoming a broad dependency hub, so unrelated features import from and modify the same module.
- **Later direction:** Split IPC by domain and add a mechanical contract-verification strategy. Code generation is one option, but generated output should remain reviewable and should not replace Rust-side validation.

### CR-003 — `App.tsx` is becoming a manual application controller

- **Priority:** P2
- **Area:** `src/App.tsx`
- **Observation:** The root component owns navigation, settings bootstrap, theme application, deck refresh, dirty-state coordination, cross-screen practice launch requests, analytics launch requests, and top-level error/toast state.
- **Why it matters:** Every new screen or cross-screen workflow adds another branch and another sequence counter to the same component. Navigation behavior and unsaved-change behavior are therefore easy to couple accidentally.
- **Later direction:** Introduce a small application-state reducer or navigation coordinator with explicit transition events. Keep screen-specific state in the screens and preserve the current no-network/local-first startup behavior.

### CR-004 — several React screens combine data access, workflow orchestration, and rendering

- **Priority:** P1
- **Area:** deck, Search/Filter, card editor, template, practice, and analytics screens
- **Observation:** Feature screens own many independent asynchronous operations, modal states, derived selections, and presentation branches.
- **Why it matters:** Large state surfaces make cancellation, stale-response handling, dirty-state reporting, and error recovery difficult to reason about. They also encourage tests to mock a broad IPC surface for a small interaction.
- **Later direction:** Extract domain-focused hooks/controllers and smaller presentational components. Do not move authoritative validation or scheduling logic into React.

### CR-005 — deck interchange is intentionally compatible but structurally fragmented

- **Priority:** P1
- **Area:** `transfer.rs`, `tagged_transfer/`, `stage3_transfer.rs`, authoring import normalization
- **Observation:** Import/export behavior is layered across the original package implementation and later optional metadata adapters for tags and card types.
- **Why it matters:** The compatibility requirement is valid and must be preserved, but the active import path is difficult to trace end-to-end. Adding another package feature could require edits in several wrappers and increase the chance that one export/import combination omits metadata.
- **Later direction:** Define an explicit versioned package model and a single top-level import/export pipeline with version adapters. Keep old Basic packages readable and continue assigning fresh local schedules.

### CR-006 — recurring startup normalization sits outside the migration ledger

- **Priority:** P2
- **Area:** `database.rs`, authoring normalization helpers
- **Observation:** After checksummed migrations run, startup always invokes duplicate-hash backfill and Stage 3 normalization.
- **Why it matters:** Idempotent repair is useful, especially for imported or historically inconsistent data, but repeated startup scans blur the distinction between schema migration and data repair. The cost can grow with the library, and it is harder to know whether a repair has already completed.
- **Later direction:** Separate schema migration from versioned repair/normalization jobs, record completion where safe, and retain an explicit integrity-repair path for imported legacy data.

### CR-007 — one mutex-protected SQLite connection serializes every command

- **Priority:** P2
- **Area:** `database.rs`
- **Observation:** `Database` exposes a single `Mutex<Connection>` to all command handlers.
- **Why it matters:** This is simple and protects a desktop application from accidental concurrent use of one connection. It also means a long analytics query, import, normalization, or filesystem-associated operation can delay unrelated reads and writes.
- **Later direction:** Measure before changing. If contention becomes visible, consider a dedicated write connection plus bounded read connections or background command execution. Preserve SQLite transaction ordering and Tauri state safety.

### CR-008 — image import spans SQLite and the filesystem without crash reconciliation

- **Priority:** P1
- **Area:** `assets.rs`, application startup/integrity handling
- **Observation:** Image import copies and fsyncs a temporary file, renames it into the managed directory, and then inserts the database row. Ordinary database failures remove the final file, but a process or machine failure between filesystem and database steps can leave an orphan. The inverse problem can occur if a managed file is removed externally after the row exists.
- **Why it matters:** SQLite cannot atomically commit a filesystem rename. The current code handles recoverable errors well but has no startup reconciliation policy for interrupted cross-resource operations.
- **Later direction:** Add a small integrity/reconciliation mechanism using temporary markers, an import journal, or a managed-directory scan. Never delete unknown user files outside the managed directory.

### CR-009 — deck summary queries duplicate correlated aggregate SQL

- **Priority:** P2
- **Area:** `decks.rs`
- **Observation:** `list_decks` and `get_deck` repeat correlated card-count and due-count subqueries.
- **Why it matters:** The duplication invites semantic drift. Correlated aggregates may also become expensive as deck and card counts grow, especially when additional scheduling states are added.
- **Later direction:** Centralize the projection and benchmark a grouped aggregate/CTE form against realistic libraries. Confirm that “due” consistently accounts for suspension, burial, and other scheduling controls.

### CR-010 — defaults and string-valued contracts are repeated across layers

- **Priority:** P2
- **Area:** `src/App.tsx`, `settings.rs`, IPC error/status/filter strings
- **Observation:** Default settings are repeated in React and Rust. Many command names, error codes, card states, filter names, and setting keys are represented as independent strings.
- **Why it matters:** Repetition is currently manageable but grows with every feature. A changed backend default or renamed string can leave a frontend fallback or test fixture stale.
- **Later direction:** Centralize backend defaults, narrow TypeScript unions by domain, and add explicit contract checks. Keep human-readable error messages separate from stable machine codes.

## Review coverage ledger

| Area | Status | Notes |
| --- | --- | --- |
| Repository manifests and dependency pins | In progress | Package and Rust manifests reviewed; lockfiles and version guards remain. |
| CI, build, release, setup, and diagnostics scripts | In progress | Main check gate reviewed; supporting scripts remain. |
| Tauri composition root and shared infrastructure | In progress | `lib.rs`, database, errors, settings, decks, and assets reviewed. |
| Migrations and released-schema preservation | In progress | Migration runner reviewed; SQL files and all fixtures remain. |
| Card authoring/search/templates | In progress | Composition structure reviewed; implementation files remain. |
| Practice, scheduling, workload, and analytics | Not started | Full logic and tests remain. |
| Transfer/import/export | Not started | Compatibility paths remain. |
| React application shell and IPC | In progress | Root controller and initial IPC contracts reviewed. |
| React feature screens/editors/styles | Not started | Full screen and style review remains. |
| Unit/integration/smoke tests | Not started | Test architecture review remains; new tests belong to sub-task 2. |
| Written project documentation | Not started | Full documentation refresh belongs to sub-task 4; contradictions found here will still be logged. |

## Commenting policy for this review

Comments added by this PR should explain one of the following:

- a data-loss or backwards-compatibility invariant;
- why an operation must be transactional or idempotent;
- why a seemingly redundant layer exists;
- the trust boundary for untrusted card/import/template content;
- an ordering dependency that future edits could violate;
- a non-obvious concurrency, scheduling, or identity rule.

The review should not add comments that merely restate syntax. When a block cannot be explained clearly without a long warning, the report should record it as a refactoring candidate rather than hiding the complexity behind excessive prose.
