# AGENTS.md

Repository-wide instructions for automated contributors to **ostudy**. More specific `AGENTS.md` files in subdirectories add constraints for their area. Nothing here grants permission to merge, change infrastructure, or widen the requested product scope.

## How work flows

- Start every change from current `main` on a short-lived branch and open a **draft** pull request early. Never commit to `main` directly.
- Work in a real checkout. Build, test, and commit locally, then push the branch. Keep one writer per branch; when resuming, read the branch head, the PR, and the latest `Check` result before editing.
- Run `./scripts/check-static.sh` before every push and the focused tests for what changed. Run `./scripts/check-fast.sh` before marking a PR ready. CI confirms work; it is not the place to discover formatting or lint errors.
- Draft PRs run the fast gate. Marking a PR ready for review runs the full Linux and Android gate on the single self-hosted runner, and every later push to a ready PR runs it again. Mark a PR ready only when the change is complete and its fast gate is green; move it back to draft for substantial rework.
- Keep the PR description current using the [pull request template](.github/pull_request_template.md): what a learner sees before and after, how it works, what was tested and on which commit, and what was not tested.
- The maintainer reviews and squash-merges. Agents never merge, tag, push to `main`, force-push a branch someone else is using, or delete branches they did not create.

When CI fails, read the failure summary at the end of the `check` job log (also shown as annotations and in the job summary), reproduce the smallest failing command locally, fix it, and push once the local check passes.

The current request from the maintainer takes precedence over older plans. Accepted ADRs in `docs/decisions/` record durable decisions, `docs/architecture/` describes current ownership, and `docs/development/` describes how to build and test. Historical documents are evidence, not instructions. Report a genuine conflict with an invariant rather than silently reinterpreting it.

## Product principles

**Local first and user controlled.** Core study must remain useful without an account, network, paid API, telemetry service, or hosted database. Optional AI connects only to a deliberately selected provider. Never silently replace a local provider with cloud processing. Distinguish implemented adapters from catalog placeholders and roadmap ambitions.

**Deterministic authority.** Rust and ordinary SQL own durable rules, scheduling, validation, and analytics. AI can propose learning material; it cannot execute code, write arbitrary storage, or become the hidden source of truth.

**Focused changes.** Prefer cohesive, reviewable changes at an existing boundary over framework expansion, speculative abstractions, or whole-project rewrites. New dependencies need explicit need, license/platform review, exact pins, and qualification. Do not mix unrelated dependency or formatting churn into a feature or documentation task.

**Calm, capable UI.** Keep common actions shallow and advanced detail contextual. Do not restore empty-state clutter, obsolete navigation tabs, duplicate controls, or implementation diagnostics to ordinary user screens. Simplicity must preserve accessibility and functionality rather than hide necessary actions.

## Architecture invariants

These are the short form of the [architecture guardrails](docs/architecture/guardrails.md), [AI provider guardrails](docs/architecture/ai-provider-guardrails.md), and [code standards](docs/development/code-standards.md). Read those and the relevant architecture documents before changing a boundary.

- React renders data and intent. Typed native command gateways live under `src/ipc/`; direct `invoke()` elsewhere is forbidden. There is no monolithic `src/ipc.ts` ownership convention.
- Controllers coordinate reads, mutations, navigation, and reconciliation. Some feature-local controllers are colocated with their workspace; ownership matters more than forcing every hook into one directory.
- Shared UI under `src/ui/` must not import IPC, feature controllers, or Tauri APIs. Operating-system capability adapters live under `src/platform/` and native platform ports. Do not fork product workflows by user-agent or operating-system name.
- Use shared Dialog, Sheet, confirmation, Menu, and overflow primitives. Do not create ad-hoc backdrops, focus traps, ordinary `window.confirm()` calls, or another overlay stack.
- `src/main.tsx` has one global foundation CSS entry point. Migrated presentation uses complete `tw:` Tailwind utilities and oStudy semantic tokens; unmigrated CSS Modules and specialized content/platform CSS retain their explicit owners. Base UI stays under `src/ui/vendor/shadcn`; features import oStudy facades. Do not add a second reset, palette, theme provider, global stage stylesheet or interpolated utility name. Follow the [current migration boundary](docs/development/ui-overhaul-current-implementation.md).
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
- Keep the stable **release** application identifier `io.github.jferments.ostudy`; changing the release identifier or production data location requires a migration plan. Maintained desktop development/debug builds and Android debug installation use `io.github.jferments.ostudy.dev` with separate application data and credential namespaces. Tests use explicit isolated storage.

## Current UI contracts to preserve

The main navigation names five destinations: **Home**, **Library**, **Tutor**, **Progress**, and **Settings**. On wider screens they are a side rail beside the content, with Search and Settings at its foot; on phones they are a bottom bar and Search stays in the top bar. There is no More menu and no Review destination: Review lives on Home, whose Today strip starts today's review or the chosen subjects' review, and the legacy `review` section shows Home. The app opens on Home, which shows the Today strip, lessons to pick up, suggestions from real study state, the week at a glance and the learner's subjects; empty Home panels are hidden, not shown as placeholders, and Android Back returns to Home from any other destination once the Workbench has stepped back to its tree or to all of the subject's cards; during Practice it returns to where the session started. **Library** organizes every material by subject: its subject list (Everything, each subject, No subject and New subject) sits beside the page on wide screens and opens from the subject's name in the header on narrower ones, and the chosen subject's name, counts, Practice a mix, Review button and its own menu (rename, reorder, delete) head every Library page. That list is Library's only list of subjects: no page repeats it. Library's pages are tabs under that header, each showing the chosen subject: **Overview** (the subject's home: due now, remembered, days studied and coming up, what to do next, and how each deck is going; it does not list the materials the other tabs list), **Lessons**, **Study guides**, **Documents** (the subject's documents as one list; Everything groups them under their subjects), **Notes** (the subject's notebooks, written with the PDF annotator's tools; Everything groups them under their subjects) and **Flashcards** (the **Workbench**: all of the subject's cards, its decks and its tags with no subject level, the chosen cards, and the open card's editor, with Import deck beside the tabs). There are no document collections in the interface: a subject is the record the native side calls a collection, and its study materials, reading ranges and filing live with the subject. The subject stays chosen across Library's pages, and the tabs never rearrange the page. There are no standalone Templates or Import/Export destinations; the header's Search opens the Workbench on All cards with its search focused, and a subject opened from Home opens its Library Overview (No subject has one too); a lesson resumed from Home opens on its subject's Lessons page. Card templates are a normal Settings tab. Deck import/export is contextual. Practice is an internal contextual workflow that runs inside Home, which stays highlighted during a session; it is not a destination of its own.

A truly empty library opens on Home's welcome with Create a subject as its one main action, and a truly empty Workbench has one useful create-deck state, not empty subject tiles or meaningless selection/collapse controls. Subjects are created with New subject in Library's subject list or on Home; collapse/expand and select controls appear only for meaningful scopes. Archived-only libraries are not empty libraries. Choosing the Study guides page returns to the guide list rather than trapping the learner in an open viewer.

Do not delete legacy route/checkpoint compatibility simply because its old label disappeared. Trace production callers and durable serialized readers before declaring code dead. See [the current architecture](docs/architecture/overview.md) and [cleanup audit](docs/development/documentation-and-cleanup-audit.md).

## Validation

```bash
./scripts/check-changed.sh   # iteration aid: picks focused checks for the current diff
./scripts/check-static.sh    # formatting, lint, typecheck, repository contracts
./scripts/check-fast.sh      # static + frontend suite + Rust lint and focused native tests
./scripts/check.sh           # full Linux gate: Rust suite, builds, native smoke and WebDriver tests
pnpm android:test            # Android compile and package
pnpm android:smoke           # Android emulator smoke (needs KVM)
```

`check-changed` is an aid, not merge qualification, and static checks are not native runtime testing. [Testing and CI](docs/development/testing-and-ci.md) describes each tier, the `Check` workflow, and focused test commands; [the command runbook](docs/development/command-runbook.md) covers setup, builds, and installs.

Expensive Linux and Android tiers may reuse an earlier runner-local pass when only root-level Markdown or Markdown under `docs/` changed; any other path invalidates reuse, and `OSTUDY_FORCE_CHECK=1` forces fresh execution. A green run qualifies only the commit it tested. Performance measurements are advisory, not a substitute for correctness gates.

Fix the cause of a failure. Do not skip, disable, or weaken tests, add sleeps or longer timeouts, push empty commits, or toggle PR state to get a green result. Re-run an unchanged commit at most once, and only with evidence of a transient cause. Do not claim live-provider, physical-device, screenshot, accessibility, or manual acceptance that was not actually exercised.

## Documentation and cleanup

`README.md` is the human-readable product introduction and short start path, not a PR diary or implementation ledger. Task instructions belong in user/installation guides; ownership and rationale in architecture; contributor commands in development; exact external formats in reference; historical proposals in history or clearly marked records. [Documentation maintenance](docs/development/documentation-maintenance.md) has the details.

Update existing canonical sections instead of appending contradictory stage notes. Update indexes and links together. Source-check UI labels, wrappers, provider availability, data formats, and security claims. Preserve valuable historical provenance without presenting it as current behavior. Remove demonstrably dead code with its exclusive styles/tests/contracts; do not remove live compatibility, platform fallbacks, or regression fixtures to make the tree look smaller.

## Safety

Never commit secrets, tokens, personal documents, live `library.sqlite3`, managed libraries, build output, provisioned binaries, or `.env` files. Use temporary isolated data for destructive/import/restore tests. Never instruct the maintainer to clear app data or uninstall as routine setup without warning about data loss and backup.

When a change affects stored data, say so in the PR: which migration runs, whether an older build can still open the library, and how to recover from the pre-upgrade checkpoint. Give the maintainer the short commands to build and try the change locally (see [Local testing](docs/development/local-testing-and-handoff.md); his checkout is `~/code/ostudy`, and he runs `pnpm install --frozen-lockfile` then `pnpm desktop`, never bare `pnpm tauri dev`) and the concrete behavior to check. Android debug installation is `pnpm android:install`.
