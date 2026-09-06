# AGENTS.md

Repository-wide instructions for automated maintainers of **ostudy**. More specific `AGENTS.md` files may add constraints. These instructions do not grant permission to merge, change infrastructure, or expand the human's requested product scope.

## Start here: the workflow is GitHub-direct

Read [GitHub-direct development](docs/agents/GITHUB-DIRECT-DEVELOPMENT.md) and [the command runbook](docs/agents/COMMAND-RUNBOOK.md). The authenticated GitHub Connector is the normal reader and **sole normal source publisher**. GitHub is authoritative for source, branches, PRs, reviews, and Actions state. Actions qualify published source and are never source writers.

Do not use Codex, `dev-workspace-v1`, the orchestrator, observer, exact-head MCP, Secure MCP Tunnel, or any custom developer MCP unless the human explicitly requests it for the current task. Do not infer authorization from an old conversation, available tool, or historical deployment. If an explicitly requested MCP fails or disappears, stop that path, verify GitHub, and continue GitHub-direct rather than debugging plugins.

### Required execution sequence

1. Read the exact current base or PR head through GitHub. Inspect the relevant source and tests at that SHA.
2. Define stable `AC-##` acceptance criteria and explicit non-goals. Conversation establishes intent, not current implementation or test success.
3. Use one short-lived `agent/<description>` branch and one draft PR. Publish a meaningful first increment, not an ownership marker or no-op access test.
4. Keep exactly one source writer per branch. For each publication use `create_blob` → `create_tree` based on the expected parent's tree → `create_commit` naming that parent → immediate remote-head check → non-force `update_ref` → remote-head verification.
5. Stop on head drift. Inspect the competing commit and reconcile writer ownership; never force or silently replace the tree. A plain ref update is not an atomic compare-and-swap API.
6. Publish ordinary source and documentation, not task-specific applicators, patch payloads, finalizers, or runner-only product trees. No workflow may author or push source, even when manually dispatched.
7. Validate the smallest affected boundary while implementation is active. Before terminal qualification, perform the full implementation/documentation/handoff audit, resolve stale maintained prose and cleanup, then freeze that exact candidate.
8. Run the required terminal repository/platform gates against the frozen candidate. Once terminal qualification starts, do not continue broad audits, opportunistic documentation edits, refactors, formatting, or cleanup while the expensive gates run.
9. After qualification, reconcile evidence and hand off rather than reopening scope. A non-blocking late finding goes to follow-up work. A blocking Markdown-only correction may use the documented qualification-reuse path; any qualification-bearing change requires the applicable expensive gates again.
10. Reconcile the PR acceptance snapshot, report publication/qualification/manual acceptance separately, provide local test instructions, and release writer ownership at handoff.

On resumption, read the actual PR, remote head, unresolved review state, and exact-head runs before editing. Do not reconstruct current task state from memory. A snapshot is progress metadata, not a branch lock, job scheduler, or source authority.

## Authority and required reading

Within repository policy, the current human request and explicit acceptance criteria take precedence over older plans. Accepted ADRs describe durable decisions; maintained architecture describes current ownership; development and agent guides describe execution. Historical documents are evidence, not active instructions. Report a genuine invariant conflict rather than silently reinterpreting it.

Read [the documentation index](docs/README.md), [agent index](docs/agents/README.md), [execution state](docs/agents/EXECUTION-STATE.md), [failure triage](docs/agents/FAILURE-TRIAGE.md), [architecture guardrails](docs/agents/ARCHITECTURE-GUARDRAILS.md), [code standards](docs/agents/CODE-STANDARDS.md), and the current architecture/ADRs relevant to the change. Use [documentation maintenance](docs/agents/DOCUMENTATION-MAINTENANCE.md) for documentation or cleanup work.

## Product principles

**Local first and user controlled.** Core study must remain useful without an account, network, paid API, telemetry service, or hosted database. Optional AI connects only to a deliberately selected provider. Never silently replace a local provider with cloud processing. Distinguish implemented adapters from catalog placeholders and roadmap ambitions.

**Deterministic authority.** Rust and ordinary SQL own durable rules, scheduling, validation, and analytics. AI can propose learning material; it cannot execute code, write arbitrary storage, or become the hidden source of truth.

**Focused changes.** Prefer cohesive, reviewable changes at an existing boundary over framework expansion, speculative abstractions, or whole-project rewrites. New dependencies need explicit need, license/platform review, exact pins, and qualification. Do not mix unrelated dependency or formatting churn into a feature or documentation task.

**Calm, capable UI.** Keep common actions shallow and advanced detail contextual. Do not restore empty-state clutter, obsolete navigation tabs, duplicate controls, or implementation diagnostics to ordinary user screens. Simplicity must preserve accessibility and functionality rather than hide necessary actions.

## Architecture invariants

- React renders data and intent. Typed native command gateways live under `src/ipc/`; direct `invoke()` elsewhere is forbidden. There is no monolithic `src/ipc.ts` ownership convention.
- Controllers coordinate reads, mutations, navigation, and reconciliation. Some feature-local controllers are colocated with their workspace; ownership matters more than forcing every hook into one directory.
- Shared UI under `src/ui/` must not import IPC, feature controllers, or Tauri APIs. Operating-system capability adapters live under `src/platform/` and native platform ports. Do not fork product workflows by user-agent or operating-system name.
- Use shared Dialog, Sheet, confirmation, Menu, and overflow primitives. Do not create ad-hoc backdrops, focus traps, ordinary `window.confirm()` calls, or another overlay stack.
- `src/main.tsx` has one global foundation CSS entry point. Feature/component styles use owned CSS Modules and semantic tokens, not new global or stage-specific stylesheets.
- Preserve keyboard, touch/coarse-pointer, reduced-motion, theme, focus, safe-area, and compact-screen behavior. A smaller visual control is not permission to shrink its interactive target or clip its label.
- Optional visual/audio/haptic effects consume semantic feedback; durable operations do not depend on effects succeeding.
- Independent reads use independent latest-request lanes. Obsolete results and errors must not overwrite newer state. Do not apply stale-read cancellation semantics to durable mutations.
- A committed mutation remains successful when a later refresh fails. Report reconciliation trouble separately; do not invite a second write or allocate a new retry identity to repeat it.
- `CardNote` is editable source; `CardPresentation` is a rendered scheduled/search identity. Do not collapse them into a generic card DTO. Notes own source and tags; cards own independently studyable presentations.
- Rust validates IPC, imports, rich content, identifiers, and external file handles. The frontend does not execute SQL, read saved credentials, contact providers, or calculate authoritative FSRS schedules.
- SQLite owns durable structured data; controlled managed files own document/image bytes. Rich content is validated, versioned Tiptap/ProseMirror JSON, never arbitrary executable HTML.
- `scheduler.rs` isolates `rs-fsrs`. `card_schedules` is mutable state; `review_events` is append-only review evidence while its owning material is retained. Explicit finalized deletion/purge is a separate policy, not a license to rewrite retained history.
- Review submission and multi-step mutations remain transactional and retry-safe. Preserve preview revisions, operation/submission identities, stable session-card snapshots, and calendar semantics.
- `.studydeck` transfers content with fresh local schedules and no donor review history. `.ostudy.json` is strict whole-file validated authoring input; do not restore an import-preview step that the product removed. Whole-library backup and trusted-device synchronization are distinct contracts.
- Released migrations and released schema fixtures are immutable. New migrations and registered checksums are one coherent change. Keep versioned data repairs and supported legacy readers explicit.
- Provider HTTP stays in the native AI gateway; credentials stay behind the native secret boundary. Parser, renderer, OCR, provider, and authoring implementations exchange application-owned contracts.
- Keep the stable application identifier `io.github.jferments.ostudy`; changing identifiers or data locations requires a migration plan. Android debug installation uses a separate `.dev` application ID.

## Current UI contracts to preserve

Flashcards exposes **Decks** and **Search/Filter**, not standalone Templates or Import/Export tabs. Card templates are a normal Settings tab. Deck import/export is contextual. Compact navigation exposes Flashcards, Study Guides, Documents, and Analytics directly, with Settings in the upper-right gear. Practice is an internal contextual workflow, not a standing navigation destination.

A truly empty deck library has one useful create-deck state, not empty folder/unsorted tiles or meaningless selection/collapse controls. Folders use a `+` action; collapse/expand and select/deselect controls appear only for meaningful scopes. Archived-only libraries are not empty libraries. Study Guides navigation returns to its library rather than trapping the learner in an open viewer.

Do not delete legacy route/checkpoint compatibility simply because its old label disappeared. Trace production callers and durable serialized readers before declaring code dead. See [the current architecture](docs/architecture/overview.md) and [cleanup audit](docs/development/documentation-and-cleanup-audit.md).

## Validation and failure discipline

Use maintained wrappers:

```bash
./scripts/check-changed.sh
./scripts/check-static.sh
./scripts/check-fast.sh
./scripts/check.sh
```

`check-changed` is an iteration aid, not merge qualification. Static is not native runtime testing. Draft `Check` uses fast qualification; ready PRs or explicit full dispatches run the full required platform gates. `PR Contract` validates metadata independently; it does not block product qualification. Review readiness and merge qualification are distinct states. Follow [Execution state](docs/agents/EXECUTION-STATE.md), not an old claim that pending CI must be marked passed to trigger CI.

**Terminal qualification is a freeze boundary.** Finish the broad source/diff review, documentation consistency audit, stale-claim search, and handoff preparation before `all-full` begins. Once the expensive Linux/Android gates are running, stop searching for incidental cleanup. Diagnose failures if they occur; otherwise finish the handoff. Do not invalidate a green candidate for a non-blocking wording, formatting, refactor, or cleanup opportunity that can be handled separately.

Static and fast qualification remain exact-source checks. Full Linux, Android compile/package, and Android emulator qualification may reuse a previous runner-local pass only when the current exact head has passed static qualification and the conservative product/toolchain fingerprint is unchanged. The neutral set is intentionally tiny: root-level Markdown and Markdown under `docs/` only. Source, tests, scripts, workflows, configuration, assets, lockfiles, and every other path invalidate expensive reuse. The current head still receives its own `Check` run, and any reuse must be reported explicitly with the earlier source that produced the cached pass. `OSTUDY_FORCE_CHECK=1` forces fresh execution.

A green workflow result qualifies the current SHA according to the evidence recorded by that run: either fresh execution or an explicitly reported safe reuse. An older green run by itself remains historical evidence only. Performance measurements are advisory/manual, not a substitute for correctness gates. Do not claim live-provider, physical-device, screenshot, accessibility, or manual acceptance that was not actually exercised.

Classify product, test-contract, runner/environment, Connector, GitHub-incident, and human-only failures separately. One unchanged-head rerun is permitted only with transient-cause evidence. The same fingerprint twice requires a strategy change. No no-op commits, permission expansion, PR-state toggles, timeouts, sleeps, skips, or weakened assertions merely to obtain green CI.

## Documentation and cleanup

`README.md` is the human-readable product introduction and short start path, not a PR diary or implementation ledger. Task instructions belong in user/installation guides; ownership and rationale in architecture; contributor commands in development; exact external formats in reference; historical proposals in history or clearly marked records.

Update existing canonical sections instead of appending contradictory stage notes. Update indexes and links together. Source-check UI labels, wrappers, provider availability, data formats, and security claims. Preserve valuable historical provenance without presenting it as current behavior. Remove demonstrably dead code with its exclusive styles/tests/contracts; do not remove live compatibility, platform fallbacks, or regression fixtures to make the tree look smaller.

## Safety and final handoff

Never commit secrets, tokens, personal documents, live `library.sqlite3`, managed libraries, build output, provisioned binaries, or `.env` files. Use temporary isolated data for destructive/import/restore tests. Never instruct the human to clear app data or uninstall as routine setup without warning about data loss and backup.

Every handoff includes exact base, branch, remote head, PR state, last expected parent, changed areas, acceptance/non-goals, actual checks and tested SHAs, unrun checks with reasons, manual boundaries, compatibility/rollback risks, and writer release. Report no workspace when none was used; do not invent clean/dirty state or an active process.

Provide copyable local checkout/SHA verification, build/run or Android install instructions, and the concrete behavior to inspect. Read [Local testing and handoff](docs/development/local-testing-and-handoff.md) before composing commands. A published implementation is a proposal until a human reviews and merges it.
