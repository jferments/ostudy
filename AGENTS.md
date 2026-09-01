# AGENTS.md

This file is the repository-wide entry point for AI coding agents, LLMs, and automated maintenance tools working on **ostudy**. It applies to the entire repository unless a more specific `AGENTS.md` in a subdirectory adds stricter rules.

## Instruction priority

When instructions conflict, follow this order:

1. the current human request and its explicit acceptance criteria;
2. accepted Architecture Decision Records in `docs/decisions/`;
3. `docs/architecture/overview.md`, `docs/architecture/ui.md`, `docs/architecture/card-format-and-data-model.md`, and `docs/architecture/ai-and-document-processing.md`;
4. `docs/development/guide.md`, `docs/development/workflow.md`, and `docs/development/testing-and-ci.md`;
5. the guidance in `docs/agents/`;
6. existing local code conventions.

Do not silently reinterpret a requirement to make implementation easier. Report a genuine conflict before changing an accepted invariant.

## Execution hot path

For every substantial code task:

1. Turn the human request into stable `AC-##` acceptance criteria and explicit non-goals.
2. Select one execution mode: local checkout, authenticated GitHub Connector, or Connector with a separate execution workspace.
3. Read and record the exact base SHA.
4. Create one short-lived `agent/<description>` branch, publish a meaningful first commit, and open one draft pull request.
5. Keep the PR's machine-readable acceptance snapshot current for compatibility. It records evidence but does not own source, job, or runner state.
6. Publish the actual reviewed source directly. CI must not be the only place where product source exists and must not author the PR branch.
7. Implement in coherent vertical commits; update acceptance evidence and tests as each criterion becomes real.
8. Consume exact failure evidence and use focused reproduction before another broad qualification run.
9. Escalate through `check-changed`, static, fast, full, platform, and manual acceptance. A green result qualifies only the exact SHA it tested.
10. Report publication, automated validation, and manual acceptance as separate states. Never say complete until the completion predicate in [`docs/agents/EXECUTION-STATE.md`](docs/agents/EXECUTION-STATE.md) holds.

Exactly one source writer owns the task branch at a time. Before every publication, re-read the remote head, require it as the expected parent, and update without force. A mismatch is a stop-and-handoff boundary, not permission to overwrite or start a competing repair.

When work resumes after interruption or in another conversation, first read the open PR acceptance snapshot, verify the actual remote head, inspect exact-head CI, enumerate incomplete criteria, and continue from that evidence. Do not reconstruct task state from conversational memory.

## Required reading

Before editing code, read:

- `docs/README.md`;
- `docs/agents/README.md`;
- `docs/agents/EXECUTION-STATE.md`;
- `docs/agents/FAILURE-TRIAGE.md`;
- `docs/agents/ARCHITECTURE-GUARDRAILS.md`;
- `docs/agents/CODE-STANDARDS.md`;
- `docs/agents/WORKFLOW-AND-VALIDATION.md`;
- `docs/agents/GITHUB-CONNECTOR.md` when operating through an authenticated GitHub Connector;
- the active architecture, development, reference, testing, and ADR documents relevant to the requested change.

Documents under `docs/history/` are project archaeology, not current instructions.

## Core product principles

- **Local first:** decks, cards, documents, review history, settings, analytics, and managed assets remain on the learner's device.
- **No required account or hosted service:** core study behavior must not depend on a paid API, proprietary cloud, telemetry service, or network connection.
- **Free/open-source foundation:** direct dependencies require explicit justification, compatible licensing, exact version pins, and separate review.
- **User control and inspectability:** deterministic code and ordinary SQL own authoritative application behavior. AI may assist or interpret; it must not become the hidden source of truth for scheduling, analytics, or stored data.
- **Explicit provider choice:** local work never silently falls back to a cloud model, and only currently implemented adapters may be presented as available.
- **Incremental development:** prefer focused, reviewable vertical changes over whole-project rewrites or speculative frameworks.
- **Truthful documentation:** distinguish shipped behavior, source-build work, known limitations, and roadmap items.

## Non-negotiable architecture

- React displays state and communicates user intent.
- Typed Tauri gateways live under `src/ipc/`, split by domain. Direct `@tauri-apps/api/core` `invoke()` calls outside that directory are forbidden and mechanically checked.
- Desktop/mobile capability adapters live under `src/platform/`. Feature screens and shared UI must not import Tauri dialog/opener plugins directly.
- Workflow-heavy screens delegate native reads/mutations and stale-read policy to focused controllers under `src/controllers/`; presentational components do not own persistence orchestration.
- Shared interaction and accessibility primitives live under `src/ui/`. They may not import IPC domains, feature controllers, or Tauri APIs.
- Feature code must use the shared Dialog, ConfirmDialog/confirmation service, Sheet, Menu, and other overlay primitives. Do not hand-write backdrops, focus traps, `role="dialog"`, or ordinary `window.confirm()` calls.
- Only `src/ui/foundations/index.css` is imported globally by `src/main.tsx`. New component and feature styling uses CSS Modules and semantic design tokens; do not add another feature-global or stage-named stylesheet.
- Every UI workflow must remain usable with keyboard, touch/coarse pointers, reduced motion, and compact layouts around 360–430 CSS pixels. Respect dynamic viewport height and safe-area insets for fixed or sticky UI.
- Optional animations, sounds, and haptics subscribe to semantic feedback events. Durable controllers must not call a specific effect or wait for an effect to complete.
- Obsolete async reads may finish in Rust but must not overwrite newer frontend state. Use independent request lanes for independent read concerns; do not use stale-read cancellation semantics for durable mutations.
- A committed mutation remains successful even if later view reconciliation fails. Surface reconciliation failure separately rather than inviting a retry of an already committed write.
- Editable source-note transport is `CardNote`; rendered scheduled/search study transport is `CardPresentation`. Do not collapse them into one generic card DTO.
- Rust validates all data crossing the IPC boundary and owns durable behavior, domain rules, AI task boundaries, and multi-step transactions.
- The frontend must not execute SQL, contact a provider directly, read credentials, or calculate the authoritative FSRS schedule.
- SQLite owns durable structured data; the managed filesystem owns controlled image and document bytes.
- Rich card content is validated, versioned Tiptap/ProseMirror JSON, not arbitrary HTML.
- Notes own editable content; cards own studyable presentations.
- `src-tauri/src/scheduler.rs` remains the adapter boundary around `rs-fsrs`.
- `card_schedules` is mutable current state; `review_events` is the append-only historical ledger.
- Review submission must remain transactional and idempotent.
- Imported decks receive fresh local schedules and do not import another learner's personal review history.
- Analytics remain deterministic and reproducible from stored data.
- Released migrations and release-schema fixtures are append-only and must never be edited after their release is tagged.
- Provider HTTP is confined to the native AI gateway; operating-system credentials are confined to the native secret boundary.
- Document parser, renderer, OCR, provider, and authoring implementations exchange ostudy-owned data rather than exposing third-party types across boundaries.
- The Tauri identifier is `io.github.jferments.ostudy`. A future identifier change is a storage-compatibility decision and requires an explicit migration plan.

## Change discipline

- Never develop directly on `main`.
- Start from a named current base commit and use one short-lived `agent/<description>` branch and one pull request for the task.
- Publish a meaningful first commit and open a draft pull request before substantial implementation.
- Write stable acceptance IDs, concrete acceptance criteria, non-goals, expected tests, and manual boundaries.
- Keep the PR's `currentState`, per-criterion status, known failures, and validation evidence truthful as the branch changes.
- A planning ADR, scaffold, generated patch, or source-rewriting script is not the product implementation.
- Product source must be committed directly to the task branch. Do not create task-specific applicators, finalizers, source-generating Actions workflows, or transient runner-only product trees.
- Automatic PR qualification is read-only. It may not persist write credentials, push, create commits, stage a generated tree, or change PR draft/readiness state.
- Do not mix feature work, dependency upgrades, broad refactors, documentation churn, and formatting churn unless the task explicitly requires that coherent combination.
- Inspect existing implementations, callers, tests, migrations, transfer formats, and documentation before adding a new abstraction.
- Prefer an existing UI primitive, controller pattern, domain service, or platform adapter before adding a parallel path.
- Prefer straightforward code over clever compression or premature generalization.
- Preserve accessibility, keyboard behavior, themes, responsive behavior, provider choice, import compatibility, and stable application-data identities unless the task explicitly changes them.
- A new or changed overlay must test initial focus, topmost dismissal, focus containment/restoration, and busy/non-dismissible behavior where applicable.
- A new or materially changed layout must be checked at compact phone, tablet, and desktop widths and must not introduce document-level horizontal overflow.
- Add a regression test for every defect when a practical test layer exists.
- Update affected documentation and add an ADR for durable cross-module decisions.
- Generated-import, backup, and document tests must use isolated temporary storage and must never point at the user's live ostudy data directory.
- Do not describe planned providers, platforms, sync, tutoring, quizzes, or study guides as shipped.

## Failure discipline

Follow [`docs/agents/FAILURE-TRIAGE.md`](docs/agents/FAILURE-TRIAGE.md).

- Classify product, test-contract, runner/environment, Connector, GitHub-incident, and external-manual failures separately.
- Read the exact-head machine-readable qualification result and bounded logs before changing source.
- Use the focused reproduction and repair all independent actionable failures exposed by the run in one coherent batch.
- An unchanged head may be rerun once only when evidence supports a transient external cause.
- The same failure fingerprint twice requires a strategy change before another broad run.
- Do not create no-op commits, toggle PR state, expand permissions, increase timeouts, add sleeps, skip tests, or weaken assertions merely to obtain another run or a green result.

## GitHub Connector discipline

When operating through an authenticated GitHub Connector on this private repository:

- use Connector reads as the source of truth for the repository, intended base, branch, pull request, and remote head state;
- do not use `gh`, anonymous HTTP, scraping, temporary transfer repositories, an unrelated local checkout, or CI runner availability to infer Connector access;
- treat repository visibility, repository-level permission, exposure of a particular connector action, authorization for a concrete mutation, remote publication, and CI validation as separate states;
- do not infer that writes are unavailable merely because an approval prompt has not appeared or a write action has not yet been invoked;
- do not change the Connector to permanent or unrestricted write access merely to bypass per-change authorization;
- at the first intended mutation, invoke the actual Connector write operation and let its authorization boundary handle any required approval;
- only an actual rejected Connector mutation establishes a Connector write failure; report the exact operation and error;
- treat the selected task execution mode as the branch's only source writer until explicit handoff;
- immediately before publication, verify the expected remote head and use a non-force update; stop if it changed;
- after every meaningful publication step, verify the remote branch/head through GitHub before claiming that a commit or file change exists;
- keep publication state separate from CI state: a missing or mismatched runner may block validation but does not establish that branch, commit, or PR writes are unavailable.

Good status language:

```text
Authenticated repository reads succeed and GitHub reports push permission.
No mutation has been attempted in this session, so concrete write authorization is untested.
```

Or, after a failure:

```text
create_file on agent/example was rejected with <exact connector error>.
```

Bad language:

```text
I do not have GitHub access.
```

unless the relevant operation was actually attempted and the exact failure supports that narrow statement.

A local patch, generated diff, or proposed commit message is not remote publication. Follow `docs/agents/GITHUB-CONNECTOR.md` for the complete connector-mode workflow.

## Documentation discipline

- `README.md` is a product overview and quick start, not an implementation ledger.
- Put task-oriented material under `docs/user-guide/` and platform setup under `docs/installation/`.
- Put ownership, data flow, trust boundaries, and implementation tradeoffs under `docs/architecture/`.
- Put contributor commands, CI, dependencies, and acceptance plans under `docs/development/`.
- Put exact external formats under `docs/reference/`.
- Move completed stage plans and dated audits to `docs/history/` rather than leaving stale files at the repository root.
- Keep `docs/README.md` and all relative links current. Run the documentation verifier after moves or renames.
- Root files are reserved for conventional repository entry points and tool-discovered configuration. Do not add a new top-level report because it is convenient for one pull request.

## Repository safety

Never commit:

- secrets, credentials, tokens, or `.env` files;
- `node_modules/`, `dist/`, or `src-tauri/target/`;
- the live `library.sqlite3`;
- personal imported documents or managed image libraries;
- provisioned native binaries except intentional release assets;
- generated artifacts that are not intentional lockfiles, fixtures, or release evidence.

Do not use destructive Git commands, force-push, rewrite shared history, merge a pull request, or push directly to `main` unless the human explicitly requests that exact operation.

## Required validation

Use the repository's tiered model instead of repeatedly running the most expensive gate while editing:

```bash
./scripts/check-changed.sh  # iteration aid based on the current diff
./scripts/check-static.sh   # repository, docs, IPC, type, and format contracts
./scripts/check-fast.sh     # static + complete frontend tests + Rust compile
./scripts/check.sh          # authoritative full merge qualification
```

Draft pull requests run the fast tier; ready pull requests run the full tier on the self-hosted runner. PR-body edits do not trigger product qualification, and wall-clock performance measurements remain manual/advisory rather than required correctness gates. A green result qualifies only the exact commit SHA it ran against. Use `OSTUDY_FORCE_CHECK=1` when an exact-tree cache must be bypassed. For release qualification, also run `./scripts/release-check.sh`.

The full Rust tier includes generated `.ostudy.json` conformance, migrations, package/backup behavior, scheduling and analytics invariants, provider contracts, document-processing tests, and native WebDriver persistence. Manually exercise every affected user workflow. Static inspection or a successful frontend build must never be reported as a successful native Tauri test.

The `Check` workflow validates the PR acceptance snapshot before qualification and always uploads a machine-readable exact-head result. Static, fast, and full must pass on the current head before a PR can truthfully record `readyForReview`; Android may be `notRequired` only when the change does not affect Android qualification surfaces.

See `docs/development/testing-and-ci.md` for authoritative mechanics.

## Required handoff

Every completed change must report:

- exact base commit SHA, branch, head SHA, and pull-request number;
- acceptance criteria and non-goals addressed;
- changed files grouped by purpose;
- tests and documentation added or updated;
- exact commands actually run and their results;
- commands not run and why;
- exact-head CI state;
- manual behavior actually exercised or still required;
- known limitations, migration/compatibility effects, rollback concerns, and residual risks;
- the requested merge method.

For Connector-backed work, verify every claimed remote mutation through GitHub and report publication state separately from CI/runner state.

An agent's work is a proposal until a human reviews and merges it.
