# Full repository code review — 2026-08-03

## Status and scope

This is the completed review ledger for **sub-task 1** of the post-core-feature cleanup effort. The review baseline is `main` immediately after PR #18, commit `429de0c302468186d525f65fcaeff46c09d187b4`.

The review covered the complete active repository:

- frontend application shell, screens, components, editors, helpers, styles, and tests;
- Rust command registration, persistence, migrations, scheduling, analytics, authoring, transfer, and tests;
- SQLite schemas, indexes, triggers, migration checksums, and released-schema fixtures;
- generated JSON and `.studydeck` import/export paths;
- build, setup, diagnostics, release, source-verification, CI, and native smoke scripts;
- architecture, data-model, workflow, roadmap, validation, dependency, and ADR documentation.

This sub-task intentionally does **not** implement the proposed refactors, add the larger test suite, change a database schema, change an IPC contract, alter runtime behavior, or rewrite the main project documentation. The source changes in this branch are explanatory comments only. The recommendations below are inputs to the later testing, refactoring, and documentation sub-tasks.

## Executive assessment

No confirmed **P0** defect was found that requires an emergency release block. The codebase has a sound local-first foundation and several unusually strong data-safety boundaries for an application at this stage.

The highest-priority risks are concentrated in six themes:

1. **Atomicity across layered workflows.** Core deck import, optional transfer metadata, filesystem assets, and post-write hydration do not always share one success boundary.
2. **Retry and partial-failure semantics.** Idempotency identifiers protect exact transport replay, but the current frontend does not retain them across ambiguous retries and the backend does not bind an identifier to an equivalent request payload.
3. **Query amplification.** Workload selection, weak-card detection, search rendering, practice hydration, deck detail, and template listing contain repeated per-card or per-note queries.
4. **Historical analytics semantics.** Review events are immutable, but several dashboard dimensions are reconstructed through mutable current notes, decks, tags, and card state.
5. **Module and UI boundary erosion.** The authoring backend is one lexical namespace, IPC contracts are manually mirrored, and large React screens coordinate persistence, navigation, transient state, and rendering.
6. **Drift between implementation, tests, and documentation.** The build gate is broad, but some verifiers are source-shape checks, several important failure modes are untested, and multiple canonical documents now describe an older architecture.

The recommended approach is not a whole-project rewrite. Add the missing behavior-preserving tests first, then repair transaction and retry boundaries, then optimize measured query paths, and only then split modules and components.

## Priority definitions

- **P0 — confirmed correctness or data-loss defect:** should block unrelated work.
- **P1 — high-value reliability or structural risk:** should be addressed early in the testing/refactoring sequence.
- **P2 — moderate maintainability, performance, or consistency issue:** important cleanup after P1 behavior is protected.
- **P3 — localized polish:** worthwhile but low risk.

## Architectural strengths to preserve

The later refactor should retain these existing decisions:

- **Rust is authoritative.** Validation, scheduling, import safety, deterministic template rendering, durable writes, and multi-step transactions live behind Tauri commands.
- **Rich content is structured data.** Cards use allowlisted, versioned Tiptap/ProseMirror JSON rather than imported HTML or executable templates.
- **Migrations fail closed.** Released migrations are checksummed and released-schema fixtures are upgraded forward instead of rewriting history.
- **Notes and cards have separate identities.** Editable source content is distinct from independently scheduled presentations.
- **Generated-card identities are stable.** Disabling and re-enabling a reverse presentation reuses its card ID, schedule, and review history.
- **Cloze uses one canonical schedule.** The selected blank is snapshotted in a practice session so viewing or resuming cannot change the question.
- **The review ledger is append-only.** `card_schedules` is the current mutable projection while `review_events` preserves accepted historical evidence.
- **Imports receive fresh schedules.** Shared packages do not inherit another learner's review history, settings, limits, or analytics.
- **Search does not accept raw FTS syntax.** Rust derives bounded quoted tokens and binds values separately.
- **Duplicate hashes are only candidate indexes.** Complete normalized text is compared before a duplicate is declared.
- **Templates are data-only authoring shortcuts.** Generated cards do not remain owned by a reusable template.
- **The build is reproducible.** Direct dependencies and toolchains are pinned, lockfiles are treated as source, and one command runs the main quality gate.

## Review coverage

| Repository area | Status | Result |
| --- | --- | --- |
| Manifests, dependency pins, toolchain, Tauri/Vite/TypeScript configuration | Complete | Strong pinning and strict compilation; duplication and workflow drift recorded below. |
| CI, setup, build, release, diagnostics, and repository verifiers | Complete | Broad gate; source-shape verifiers and environment duplication are refactor targets. |
| Tauri composition root and shared infrastructure | Complete | Startup fails closed; one serialized connection and cross-resource operations require care. |
| Migrations, indexes, triggers, and released-schema preservation | Complete | Checksummed append-only model is strong; recurring normalization is outside the migration ledger. |
| Decks, cards, assets, tags, search, duplicates, and bulk actions | Complete | Correct boundaries are present; several N+1 and post-commit ambiguity paths exist. |
| Reverse, cloze, reusable templates, and generated JSON import | Complete | Safety and identity design are strong; implementation coupling and compatibility complexity are high. |
| Workload, FSRS scheduling, practice, manual schedule controls, and analytics | Complete | Core schedule authority is sound; retries, repeated aggregates, and historical semantics need tests/refactoring. |
| `.studydeck` core, tag, and card-type transfer | Complete | Backwards compatibility is deliberate; extension phases are not one structured-data transaction. |
| React shell, screens, components, editors, and styles | Complete | Functional decomposition exists, but coordinator state, asynchronous races, modal behavior, and global CSS coupling remain. |
| Frontend, Rust, migration, transfer, and native smoke tests | Complete | Good foundation with important negative, race, rollback, and end-to-end gaps. |
| Architecture, roadmap, workflow, validation, dependency, and ADR documents | Complete | Several contradictions and duplicate ADR identifiers are recorded for sub-task 4. |

## Findings summary

| ID | Priority | Area | Finding |
| --- | --- | --- | --- |
| CR-001 | P1 | Rust authoring | `include!` creates one oversized lexical namespace rather than enforceable modules. |
| CR-002 | P1 | IPC | Rust/TypeScript contracts and command names are manually duplicated across several direct invoke modules. |
| CR-003 | P1 | React | Root and feature screens have become broad workflow coordinators. |
| CR-004 | P1 | Transfer | Core and optional package metadata use separate snapshots and commit boundaries. |
| CR-005 | P1 | Assets | SQLite and filesystem lifecycles are not crash-atomic; editor cancellation can create orphans. |
| CR-006 | P1 | Startup | Compatibility normalization performs recurring global scans and some recurring writes. |
| CR-007 | P1 | Data access | Workload, search, practice, cards, and templates contain substantial query amplification. |
| CR-008 | P1 | Practice | Session creation commits before complete DTO hydration and can leave a durable session after a returned error. |
| CR-009 | P1 | Scheduling controls | Multi-card scheduling is frontend-sequential and can partially apply. |
| CR-010 | P1 | Retry safety | Idempotency identifiers are not bound end-to-end to one equivalent user operation. |
| CR-011 | P1 | Analytics | Immutable events are attributed through mutable current library state. |
| CR-012 | P1 | Mutation UX | A successful write followed by a failed refresh/hydration is often reported as a failed operation. |
| CR-013 | P1 | Async UI | Practice and analytics requests lack stale-response protection present in Search/Filter. |
| CR-014 | P1 | Workload semantics | Deck due counts and authoritative workload eligibility disagree about buried cards. |
| CR-015 | P2 | Database concurrency | One mutex-protected SQLite connection serializes long reads, writes, imports, and file operations. |
| CR-016 | P2 | Templates | The update wrapper duplicates CRUD behavior and uses temporary child rows to satisfy validation. |
| CR-017 | P2 | Derived logic | Weak-card, cloze, template-preview, defaults, and validation rules are repeated across layers. |
| CR-018 | P2 | Generated JSON import | A large hand-written Markdown parser and HTML heuristic are difficult to extend and verify. |
| CR-019 | P2 | Accessibility | The shared modal does not own a complete dialog focus/stack lifecycle. |
| CR-020 | P2 | CSS | Feature-named stylesheets contain global rules and hidden cross-feature coupling. |
| CR-021 | P2 | Quality gate | Source-string verifiers are brittle and current behavioral tests emphasize happy-path wiring. |
| CR-022 | P2 | CI/release | Action tags, Ubuntu versions, and setup/version declarations are duplicated across workflows. |
| CR-023 | P2 | Contracts | Defaults, string states, error codes, and redundant plain-text inputs are manually repeated. |
| CR-024 | P2 | Lists/aggregates | Deck detail, card hydration, tag counts, and template lists have scaling or naming ambiguity. |
| CR-025 | P2 | Diagnostics | Runtime log rotation and some failure-report bounds are startup- or message-size dependent. |
| CR-026 | P2 | Legacy/compatibility | Deliberate compatibility code is mixed with historical stage naming and residual schema surface. |
| CR-027 | P2 | Documentation | Canonical plans, IPC rules, validation claims, and ADR numbering have drifted. |
| CR-028 | P3 | Localized UI | Several small semantics and rendering details deserve cleanup after behavior is protected. |

## Detailed findings

### CR-001 — `include!` composition creates one oversized authoring namespace

- **Priority:** P1
- **Area:** `src-tauri/src/authoring.rs`, `src-tauri/src/authoring/stage3_v2.rs`, included authoring files
- **Observation:** Files are physically separated but lexically compiled into one Rust module. Every included file shares imports, private helpers, and the same namespace. The compiler cannot enforce the ownership boundaries implied by directory names.
- **Risk:** A change in search, template rendering, generated-card lifecycle, normalization, or transfer can silently depend on unrelated private implementation details. Name collisions and ordering-dependent comprehension grow as the files grow.
- **Later direction:** Replace `include!` with ordinary modules organized around explicit use cases and narrow `pub(super)` APIs. Preserve Tauri command names, validation order, generated-card identity rules, and transaction boundaries during the conversion.

### CR-002 — frontend IPC contracts are manually duplicated and bypass a single gateway

- **Priority:** P1
- **Area:** `src/ipc.ts`, `src/analytics-ipc.ts`, `src/generatedImport.ts`, Rust command DTOs
- **Observation:** TypeScript manually mirrors most Rust request/response structures and command-name strings. Contrary to several architecture documents, analytics and generated import invoke Tauri directly outside `src/ipc.ts`.
- **Risk:** TypeScript compilation proves only internal browser consistency; it cannot prove that Rust serialization still matches. A backend rename, optionality change, enum change, or default change can survive compilation until a specific runtime path is exercised.
- **Later direction:** Split transport wrappers by domain behind one documented frontend IPC boundary and add a mechanical contract check. Generated types are one option, but Rust must remain authoritative and generated output must remain reviewable.

### CR-003 — application and feature screens are broad workflow coordinators

- **Priority:** P1
- **Area:** `src/App.tsx`, Decks, Search/Filter, Card Editor, Templates, Practice, Analytics
- **Observation:** `App.tsx` owns navigation, settings bootstrap and preview, themes, dirty-state guards, deck refresh, cross-screen launch requests, and notification state. Several screens combine data fetching, modal orchestration, mutation sequencing, transient editor state, and large rendering branches.
- **Risk:** New workflows add more coupled state and effects to the same components. Error recovery, cancellation, dirty-state reporting, and test setup become harder, while mocked tests must reproduce a broad IPC surface to exercise one interaction.
- **Later direction:** Introduce small domain controllers/hooks or reducers with explicit transition events, then split presentational components. Keep persistence, validation, analytics, and scheduling authority in Rust.

### CR-004 — transfer extensions do not share one snapshot or commit boundary

- **Priority:** P1
- **Area:** `transfer.rs`, `tagged_transfer/`, `stage3_transfer.rs`, import wrappers
- **Observation:** Core package content, tag metadata, and Stage 3 metadata are captured/applied through layered wrappers. Export metadata can be read under separate database lock acquisitions. On import, the core deck can commit before tags or card-type metadata are restored in later transactions.
- **Risk:** A concurrent edit between export phases can produce a package whose optional metadata does not describe the same library snapshot as its core content. An extension failure can return an import error after a deck has already been durably inserted, making a retry create another deck or obscure the partial result.
- **Later direction:** Define one versioned package model and one top-level import/export orchestration boundary. Structured database state should commit once, or an explicit compensation result must identify and safely remove a partially imported deck. Preserve support for old Basic packages and fresh local schedules.

### CR-005 — asset storage lacks a complete cross-resource lifecycle

- **Priority:** P1
- **Area:** `assets.rs`, transfer import, rich editors, template editors, startup integrity handling
- **Observation:** Image bytes are written and renamed in the managed filesystem while metadata and references are stored in SQLite. Ordinary errors attempt cleanup, but a process failure can occur between the two resources. Editors import an image before the card/template save; canceling or deleting the image from an unsaved document does not reclaim the unreferenced asset.
- **Risk:** Orphan rows, orphan files, or missing files can accumulate. SQLite cannot make a filesystem rename part of its transaction, so normal rollback alone cannot close the crash window.
- **Later direction:** Introduce an explicit asset lifecycle: staged/importing state, reference accounting or reachability, and a conservative reconciliation job. Cleanup must operate only inside the managed directory and must never delete an asset still referenced by a note, template, copied card, import, or future draft.

### CR-006 — recurring normalization is global and can rewrite on every startup

- **Priority:** P1
- **Area:** `database.rs`, authoring duplicate backfill, Stage 3 normalization
- **Observation:** Checksummed schema migration is followed by recurring compatibility normalization. The normalization scans broad library tables; the cloze canonicalization path can update timestamps even when the logical representation is already canonical. Import also invokes global Stage 3 normalization rather than limiting work to imported entities.
- **Risk:** Startup cost and write-ahead-log churn grow with the library. Repeated writes make backups and modification timestamps noisier and blur the distinction between an immutable migration, a one-time data repair, and a recurring integrity check.
- **Later direction:** Separate schema migration, versioned one-time data repair, and explicit integrity reconciliation. Make no-op normalization truly read-only, record completed repair versions where safe, and scope import normalization to the imported graph.

### CR-007 — repeated per-card/per-note queries amplify several normal workflows

- **Priority:** P1
- **Area:** `workload.rs`, `learning_signals.rs`, search, card hydration, practice selection, templates, decks
- **Observation:** Representative paths load a list and then query each row for weak-card status, assets, rendered presentation, deck name, template children, or related counts. `count_extra_available` hydrates all candidates, and `fair_interleave` repeatedly removes index zero from vectors, producing quadratic movement.
- **Risk:** Performance degrades nonlinearly on the exact libraries the application is intended to grow into. Because one SQLite mutex is held, an expensive read also delays unrelated commands.
- **Later direction:** Batch related state with CTEs/grouped queries, hydrate maps once, use `VecDeque` or iterators for round-robin ordering, and measure with representative card counts before changing the connection model.

### CR-008 — practice session creation commits before complete response hydration

- **Priority:** P1
- **Area:** `practice_selection.rs`
- **Observation:** Session rows and selected card snapshots are committed, then the command hydrates the returned session DTO. Hydration performs additional per-card work and can fail after the transaction has succeeded.
- **Risk:** The frontend receives an error even though a durable practice session exists. A retry can create another session, and abandoned rows can accumulate. This is the backend form of a post-commit ambiguity.
- **Later direction:** Build and validate all response data that can fail before commit, or return a stable committed session identifier and make post-commit hydration separately retryable. Batch deck names, content, assets, and variants during the same planned selection.

### CR-009 — multi-card scheduling can partially apply

- **Priority:** P1
- **Area:** `schedule_controls.rs`, `CardsScreen.tsx`, `ScheduleActionModal.tsx`
- **Observation:** The backend input contains `cardIds` but currently enforces a one-card operation. Search/Filter loops those commands sequentially for a selected set. A failure after several calls leaves earlier cards changed and later cards untouched. No-op suspend/resume/bury/unbury operations can still create audit events and increment revisions. Optional session validation establishes session existence but should also establish the card/session relationship.
- **Risk:** The UI presents one bulk intent that is not atomic. After a partial error, the displayed list can remain stale and a retry can apply a different subset.
- **Later direction:** Define a bounded transactional multi-card command with per-card validation completed before mutation. Specify whether already-in-state actions are no-ops, successes without events, or errors, and validate any session association explicitly.

### CR-010 — idempotency identifiers do not represent one stable user operation end to end

- **Priority:** P1
- **Area:** review submission, schedule mutation, frontend retry logic
- **Observation:** Unique submission/mutation IDs correctly prevent exact duplicate rows. The backend replay path returns the existing result without proving that a repeated ID carries the same session, card, outcome, revision, or action. The frontend generates a new ID when the user retries after an error rather than retaining the original operation ID until the result is known.
- **Risk:** Exact transport replay is safe, but an ambiguous response loss followed by a user retry can record a second review or action. Conversely, accidental reuse of an ID for a different payload can return an unrelated prior result.
- **Later direction:** Bind each identifier to a canonical request fingerprint and reject mismatched reuse. Generate the ID when the user operation begins and retain it across transport retries until success, authoritative rejection, or explicit abandonment.

### CR-011 — analytics reconstructs history through mutable current state

- **Priority:** P1
- **Area:** `analytics.rs`
- **Observation:** Immutable review events are joined to current cards, notes, decks, and tags. Current deletion/archive state can remove historical activity from reports. Moving a card, renaming a deck, changing tags, or editing source text can reattribute or relabel past reviews. Recent-review text is current note text and may not reproduce the exact reverse or cloze presentation reviewed at the time.
- **Risk:** Dashboard totals remain deterministic, but their historical meaning changes as the library is reorganized. This may be acceptable for some “current organization” views and unacceptable for audit/history views; the product contract is not explicit enough.
- **Later direction:** Decide which dimensions are event-time facts and which intentionally use current organization. Snapshot only the minimum immutable facts required for historical meaning, or label current-state attribution clearly. Preserve the append-only review ledger.

### CR-012 — durable success and UI refresh success are often treated as one operation

- **Priority:** P1
- **Area:** deck/card/template mutations, imports, bulk actions, `onSaved`/refresh callbacks
- **Observation:** Many frontend `try` blocks await a successful Rust mutation and then await detail reloads, deck refreshes, or parent callbacks. If the refresh fails, the catch path often reports that the save/import/action failed even though durable data already changed. Several backend commands also commit and then reacquire the connection to hydrate the return DTO.
- **Risk:** Users may retry a successful create/import/copy because the UI reported failure, producing duplicates or confusing revision conflicts. Error messages do not distinguish “write failed” from “write succeeded; display refresh failed.”
- **Later direction:** Separate mutation outcome from reconciliation outcome. Return stable identifiers and committed revisions, update local state when safe, and show a distinct recoverable refresh error with an explicit reload path.

### CR-013 — some asynchronous screens allow stale responses to overwrite newer state

- **Priority:** P1
- **Area:** Practice workload/setup, Analytics filters, related launch effects
- **Observation:** Search/Filter already uses a request sequence guard, but Practice workload and Analytics loading do not consistently cancel or ignore obsolete requests. Rapid deck, tag, or range changes can complete out of order.
- **Risk:** The screen can display results for an older selection under newer controls, creating misleading workload or analytics decisions.
- **Later direction:** Standardize one stale-response pattern for read effects: sequence token, abortable wrapper, or reducer state keyed by the complete request. Add race tests that resolve requests in reverse order.

### CR-014 — deck due counts and workload eligibility disagree about burial

- **Priority:** P1
- **Area:** `decks.rs`, `workload.rs`, deck list/detail UI
- **Observation:** Authoritative workload candidate selection excludes cards whose `buried_until` is still in the future. Deck due-count queries exclude suspended cards but do not apply the same burial predicate.
- **Risk:** A deck can display cards as due that cannot enter the scheduled workload, undermining trust in the summary count.
- **Later direction:** Centralize active/due eligibility SQL or a shared query builder and define whether deck badges represent raw overdue schedules or currently studyable cards. Use the same product term everywhere.

### CR-015 — one SQLite connection serializes every command and some long file operations

- **Priority:** P2
- **Area:** `database.rs`, analytics, import/export, assets
- **Observation:** A single `Mutex<Connection>` is simple and safe, but long analytics queries, package construction, large import validation/write phases, startup normalization, and filesystem I/O can hold or repeatedly acquire the only connection.
- **Risk:** UI commands may appear frozen behind unrelated work as libraries and packages grow.
- **Later direction:** Measure contention after query amplification is fixed. If needed, use a dedicated writer plus bounded read connections or background tasks. Any change must preserve transaction ordering, schedule revisions, and SQLite configuration on every connection.

### CR-016 — template update uses a duplicated wrapper and temporary child records

- **Priority:** P2
- **Area:** `authoring/stage3_v2/template_update_wrapper.rs`, template CRUD
- **Observation:** Update behavior is split from the main CRUD implementation and creates temporary/pending child rows to satisfy validation before deleting and rebuilding children.
- **Risk:** The path is difficult to explain, duplicates lifecycle behavior, and increases the chance that create/update validation diverges or a future constraint makes the workaround unsafe.
- **Later direction:** Separate pure template-graph validation from persistence, assign stable child IDs in memory, and persist the validated replacement graph once inside the optimistic-revision transaction.

### CR-017 — authoritative and preview/derived rules are repeated in several representations

- **Priority:** P2
- **Area:** weak-card SQL/Rust logic, frontend cloze helpers, template preview, settings defaults, link validation
- **Observation:** “Needs more practice” appears in Rust helper queries, search SQL, and analytics SQL. Browser cloze and template previews mirror Rust rendering rules. Settings defaults and link rules are repeated in frontend and backend.
- **Risk:** A rule change can update one representation but not another. Users may preview one result and save another, or one screen may classify a card differently from another.
- **Later direction:** Keep Rust authoritative, expose small preview commands where exact parity matters, and add cross-layer fixtures for intentionally duplicated immediate-feedback logic. Centralize shared SQL fragments or materialized facts only after semantics are protected.

### CR-018 — generated import owns a large hand-written restricted Markdown parser

- **Priority:** P2
- **Area:** `authoring/generated_import.rs`, generated import schema/docs
- **Observation:** The importer correctly rejects unknown fields and executable content, but it also owns a substantial custom Markdown parser and a broad angle-bracket HTML heuristic. A large file can produce a very large aggregate validation message when thousands of cards fail.
- **Risk:** CommonMark edge cases, ordinary comparison characters, nested blocks, and future formatting additions are difficult to reason about. Parser and documentation behavior can drift.
- **Later direction:** Keep the intentionally small safe subset, but model it as a clearly specified tokenizer/parser with table-driven fixtures and bounded error aggregation. Do not replace it with raw HTML rendering or a permissive parser without an explicit trust review.

### CR-019 — shared modal behavior is incomplete for accessibility and stacking

- **Priority:** P2
- **Area:** `src/components/Modal.tsx`, hand-rolled scheduling modal
- **Observation:** The shared modal uses a fixed `modal-title` ID, closes from a window-level Escape listener, and does not establish initial focus, trap focus, restore focus, or make the background inert. Multiple open dialogs can register multiple Escape listeners. One scheduling modal duplicates dialog behavior instead of reusing the shared component.
- **Risk:** Keyboard and assistive-technology behavior is inconsistent, especially with nested confirmation/duplicate dialogs.
- **Later direction:** Give each dialog unique IDs, own a modal stack, capture and restore focus, trap tab navigation, and centralize backdrop/Escape behavior. Verify destructive dialogs and nested flows.

### CR-020 — feature stylesheets are not actual style boundaries

- **Priority:** P2
- **Area:** `styles.css`, stage/feature CSS files
- **Observation:** Files named for card organization, card types, analytics, or scheduling contain global selectors for checkboxes, modal sizes, buttons, and shared layout. The cascade, import order, and selector specificity—not the filename—determine ownership.
- **Risk:** A feature fix can silently alter another screen. Historical phase/stage file names also describe implementation chronology rather than stable component ownership.
- **Later direction:** Organize styles by shared primitive and component boundary, scope feature selectors under screen roots, and add narrow-layout/theme visual regression coverage before moving rules.

### CR-021 — repository verifiers are coupled to source shape and tests omit important failure modes

- **Priority:** P2
- **Area:** `verify-stage2.mjs`, `verify-stage3.mjs`, Vitest, Rust tests, native smoke
- **Observation:** The repository has valuable migration, validation, scheduler, transfer, and component tests. However, source verifiers often search for literal files/strings and therefore encode implementation layout rather than behavior. Frontend tests mock the complete IPC layer and mostly exercise happy paths. The native smoke test navigates Analytics and Settings but does not create, persist, practice, import, restart, or recover from errors.
- **Risk:** A correct refactor can fail a verifier because code moved, while an incorrect implementation can pass because a string remains. Race, rollback, retry, and post-commit ambiguity defects have little coverage.
- **Later direction:** Replace structural assertions with contract/schema tests and real behavior. Sub-task 2 should add negative and failure-injection coverage before sub-task 3 moves code.

### CR-022 — CI, release, and setup environments duplicate mutable declarations

- **Priority:** P2
- **Area:** GitHub Actions, setup/build/release scripts, `versions.env`
- **Observation:** Workflow action references use moving major tags, check and release workflows use different Ubuntu releases, and OS packages/tool versions are declared in several places.
- **Risk:** The local gate, PR gate, and release qualification can diverge. A third-party action major tag can change without a repository diff.
- **Later direction:** Centralize version declarations where practical, document intentional OS differences, and consider pinning actions by reviewed commit SHA with an update process. Keep dependency upgrades separate from functional changes.

### CR-023 — defaults, string contracts, and redundant input snapshots are repeated

- **Priority:** P2
- **Area:** settings, IPC types, error/status/filter/action strings, card/template input DTOs
- **Observation:** Default settings appear in Rust, `App.tsx`, screens, and tests. Many states and command/error strings are manually repeated. Some card/template commands accept frontend plain-text snapshots that Rust ignores or recomputes from canonical documents.
- **Risk:** Callers can believe a submitted field is authoritative when it is not, and defaults or enum values can drift silently.
- **Later direction:** Remove redundant internal request fields in one coordinated IPC change, centralize backend defaults, use domain enums internally, and retain stable human-independent error codes at the transport boundary.

### CR-024 — list and aggregate projections need clearer scale and naming contracts

- **Priority:** P2
- **Area:** `decks.rs`, `cards.rs`, tag summaries, template listing, Decks screen
- **Observation:** Deck list/get repeat correlated count SQL; card listing hydrates assets per note; template listing hydrates each template graph; Decks loads all source notes for local detail/search despite the global Search/Filter feature. Tag `cardCount` is based on distinct notes even when a note can generate multiple study cards.
- **Risk:** Queries grow with the full library and names can imply a different unit than the value returned.
- **Later direction:** Define summary DTO units explicitly (`noteCount`, `activeCardCount`, `studyableDueCount`), batch projections, and page deck details rather than loading all rich cards and assets up front.

### CR-025 — diagnostics and error reporting have unbounded or startup-only limits

- **Priority:** P2
- **Area:** `scripts/log-run.py`, generated import errors, native diagnostics
- **Observation:** Log rotation is checked when a run starts; a single long-running process can continue growing the active file. Generated import can aggregate paths/messages for a very large invalid file. Native smoke diagnostics are useful but narrow.
- **Risk:** Failure handling itself can consume excessive disk, memory, or UI space under pathological inputs.
- **Later direction:** Rotate by size during writes, cap displayed validation details while preserving a total count and first representative failures, and keep privacy exclusions intact.

### CR-026 — compatibility code is legitimate but mixed with historical residue

- **Priority:** P2
- **Area:** migrations, transfer adapters, template normalization, file/style names
- **Observation:** Legacy template block placeholders, old Basic packages, historical multi-row cloze packages, and soft-deleted generated cards all have documented data-compatibility reasons and should not be deleted as “stale.” In contrast, phase/stage-named files, duplicated wrapper layers, and the historical `image_occlusion` schema option remain visible despite no current product workflow.
- **Risk:** Blanket cleanup could destroy backwards compatibility, while fear of compatibility can preserve unrelated implementation residue indefinitely.
- **Later direction:** Classify every candidate as durable format, released data, active internal contract, or merely historical organization. Never edit released migrations; use a new migration only if narrowing old schema surface has a demonstrated benefit. Remove internal residue end to end once tests prove no caller remains.

### CR-027 — canonical documentation and ADR indexing have drifted

- **Priority:** P2
- **Area:** root plan, architecture docs, agent docs, validation report, ADR directory
- **Observation:** The root plan still lists reverse cards, cloze, and scheduling controls as future work. Architecture and agent docs say `src/ipc.ts` is the only invoker even though two other modules invoke directly. ADR numbers 0004 and 0005 are each used twice. The root validation report is a Phase 1 artifact, and some testing/idempotency descriptions are stronger than the current implementation proves.
- **Risk:** Contributors can follow conflicting “canonical” guidance, and future decisions cannot cite a unique ADR identifier.
- **Later direction:** Sub-task 4 should establish one current roadmap, correct the IPC description, assign unique ADR identities without erasing history, archive or clearly label phase-specific reports, and align validation claims with tests actually present.

### CR-028 — localized UI and temporal semantics need cleanup

- **Priority:** P3
- **Area:** practice progress, date math, analytics actions, chart rendering, editor effects
- **Observation:** Examples include a zero-based progressbar value alongside a one-based visible position, fixed 24-hour arithmetic in some browser filters despite local-calendar product language, a due-tag practice action that silently caps selected cards, chart color objects recreated on render, and effects that re-register listeners more often than necessary.
- **Risk:** These are not architectural failures, but they create small inconsistencies, accessibility noise, or avoidable work.
- **Later direction:** Address them as focused regressions after the surrounding behavior has direct tests; do not mix them into transaction or module-boundary changes.

## Stale-code and compatibility classification

The review did **not** find another confirmed active compatibility shim equivalent to the removed `skipDuplicates` field that should simply be deleted now. Several old-looking paths are required:

- historical SQL migrations and released fixtures are immutable evidence;
- old `.studydeck` tables and Basic packages remain supported external formats;
- legacy template placeholder normalization protects saved schema-7 data;
- retired cloze siblings and disabled reverse cards preserve schedule/review history;
- old card IDs are intentionally reactivated instead of replaced.

Likely cleanup candidates after tests exist are:

- the lexical `include!` authoring tree;
- duplicated direct-invoke frontend modules and manually repeated transport types;
- stage/phase-named source and CSS organization;
- source-string verifiers that enforce file layout rather than behavior;
- redundant plain-text request snapshots recomputed by Rust;
- duplicated template update paths and global import normalization;
- stale root documentation and duplicate ADR numbering.

Compatibility must be established from a released data/external-format need, not inferred from age or naming alone.

## Test implications for sub-task 2

The review recommends prioritizing these behavior-preserving tests before refactoring:

1. Extension import failure after core deck insertion must not leave an unreported partial deck.
2. Export must capture core content, tags, and card-type metadata from one coherent snapshot.
3. The same idempotency ID with a different payload must fail; an ambiguous retry with the same operation ID must not double-review.
4. Multi-card schedule actions must be all-or-nothing, including validation and no-op semantics.
5. A post-commit refresh failure must report committed success separately from display reconciliation failure.
6. Practice-session hydration failure must not create an invisible duplicate session on retry.
7. Rapid Practice and Analytics filter changes must ignore responses from older requests.
8. Deck due counts must match the chosen buried/suspended eligibility definition.
9. Startup normalization should perform no writes on an already canonical database.
10. Asset cancellation, deletion, crash-window reconciliation, and shared-reference retention need filesystem/database integration tests.
11. Historical analytics behavior must be specified and tested across deck move/rename, tag changes, note edits, archive, and deletion.
12. Workload/search/template/card-list paths need representative large-library query-count or performance tests.
13. Modal focus entry, tab containment, Escape stacking, and focus restoration need component tests.
14. The native smoke layer should create a deck/card, restart, practice a card, and verify persisted state before it is treated as a core workflow smoke test.

## Recommended sequencing for sub-task 3

After sub-task 2 protects the current behavior, refactoring should proceed in this order:

1. **Atomicity and retry semantics:** transfer phases, schedule bulk actions, operation IDs, post-commit response boundaries, and asset reconciliation.
2. **Semantic consistency:** due eligibility, analytics historical attribution, no-op actions, and local-calendar rules.
3. **Query consolidation:** weak-card calculation, practice hydration, search/card/template batching, deck summaries, and workload ordering.
4. **Backend module boundaries:** convert the authoring lexical namespace to explicit modules and consolidate transfer/template pipelines.
5. **Frontend workflow boundaries:** split IPC by domain, extract screen controllers/hooks, standardize stale-request handling, and separate mutation from refresh.
6. **UI infrastructure:** modal lifecycle, scoped CSS, shared primitives, and localized polish.

Each stage should remain a separate reviewable branch rather than one broad rewrite.

## Explanatory comments added by this review

This branch adds comments only at boundaries where a future edit could otherwise violate a non-obvious invariant:

- `src-tauri/src/lib.rs`: startup ordering, managed-state assumptions, and the registered IPC compatibility surface;
- `src-tauri/src/authoring.rs`: the fact that `include!` is a lexical split rather than a module boundary;
- `src-tauri/src/authoring/stage3_v2.rs`: coupling and ordering of generated-card, template, normalization, and transfer code;
- `src-tauri/src/authoring/stage3_v2/transfer_wrapper.rs`: the layered import/normalization/recount contract;
- `src-tauri/src/authoring/normalization.rs`: duplicate fingerprints as candidate indexes, not identities;
- `src/generatedImport.ts`: Rust as the generated-file trust and transaction boundary;
- `src/editor/links.ts`: browser link checks as immediate feedback rather than authoritative persisted validation.

Comments were not added merely to narrate syntax. Where explanation exposed a design problem, the issue is recorded above rather than hidden behind a long defensive comment.

## Validation statement

This review was performed by direct inspection of the post-PR #18 repository through the authenticated GitHub interface. No runtime behavior, schema, dependency, build script, command contract, or test was intentionally changed.

Local `pnpm`, Cargo, Tauri, WebDriver, and `./scripts/check.sh` execution were not available in the review environment. The branch still requires the normal GitHub Actions gate and the maintainer's local `./scripts/check.sh` before merge. Because the source diff is comments plus this report, any build or test failure should be treated as evidence of an accidental textual change and investigated rather than waived.
