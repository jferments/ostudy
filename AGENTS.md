# AGENTS.md

This file is the repository-wide entry point for AI coding agents, LLMs, and
automated maintenance tools working on **ostudy**. It applies to the entire
repository unless a more specific `AGENTS.md` in a subdirectory adds stricter
rules.

## Instruction priority

When instructions conflict, follow this order:

1. the current human request and its explicit acceptance criteria;
2. accepted Architecture Decision Records in `docs/decisions/`;
3. `docs/ARCHITECTURE.md` and
   `docs/CARD-FORMAT-AND-DATA-MODEL.md`;
4. `docs/DEVELOPMENT-AND-QUALITY.md` and `docs/GIT-WORKFLOW.md`;
5. the guidance in `docs/agents/`;
6. existing local code conventions.

Do not silently reinterpret a requirement to make implementation easier. Report
a conflict before changing an accepted invariant.

## Required reading

Before editing code, read:

- `docs/agents/README.md`
- `docs/agents/ARCHITECTURE-GUARDRAILS.md`
- `docs/agents/CODE-STANDARDS.md`
- `docs/agents/WORKFLOW-AND-VALIDATION.md`
- the domain documentation and ADRs relevant to the requested change

## Core product principles

- **Local first:** decks, cards, review history, settings, analytics, and managed
  assets remain on the user's computer.
- **No required account or hosted service:** core study behavior must not depend
  on a paid API, proprietary cloud, telemetry service, or network connection.
- **Free/open-source dependencies:** direct dependencies require explicit
  justification, compatible licensing, exact version pins, and separate review.
- **User control and inspectability:** deterministic code and ordinary SQL own
  authoritative application behavior. AI may assist or interpret; it must not
  become the hidden source of truth for scheduling, analytics, or stored data.
- **Incremental development:** prefer small, reviewable vertical changes over
  whole-project rewrites or speculative frameworks.

## Non-negotiable architecture

- React displays state and communicates user intent.
- Typed Tauri gateways live under `src/ipc/`, split by domain. Direct
  `@tauri-apps/api/core` `invoke()` calls outside that directory are forbidden
  and mechanically checked.
- Workflow-heavy screens delegate native reads/mutations and stale-read policy to
  focused controllers under `src/controllers/`; presentational components do not
  own persistence orchestration.
- Obsolete async reads may finish in Rust but must not overwrite newer frontend
  state. Use independent request lanes for independent read concerns; do not use
  stale-read cancellation semantics for durable mutations.
- A committed mutation remains successful even if later view reconciliation
  fails. Surface reconciliation failure separately rather than inviting a retry
  of an already committed write.
- Editable source-note transport is `CardNote`; rendered scheduled/search study
  transport is `CardPresentation`. Do not collapse them into one generic card DTO.
- Rust validates all data crossing the IPC boundary and owns durable behavior,
  domain rules, and multi-step transactions.
- The frontend must not execute SQL or calculate the authoritative FSRS schedule.
- SQLite owns durable structured data; the managed filesystem owns image bytes.
- Rich card content is validated, versioned Tiptap/ProseMirror JSON, not
  arbitrary HTML.
- Notes own editable content; cards own studyable presentations.
- `src-tauri/src/scheduler.rs` remains the adapter boundary around `rs-fsrs`.
- `card_schedules` is mutable current state; `review_events` is the append-only
  historical ledger.
- Review submission must remain transactional and idempotent.
- Imported decks receive fresh local schedules and do not import another
  learner's personal review history.
- Analytics remain deterministic and reproducible from stored data.
- Released migrations and release-schema fixtures are append-only and must
  never be edited after their release is tagged.
- The Tauri identifier is `io.github.jferments.ostudy`. A future identifier
  change is a storage-compatibility decision and requires an explicit migration
  plan.

## Change discipline

- Never develop directly on `main`.
- Start from a named, current base commit and use a short-lived branch.
- Write concrete acceptance criteria before implementation.
- Keep one logical behavior per branch or pull request.
- Do not mix feature work, dependency upgrades, broad refactors, and formatting
  churn.
- Inspect existing implementations and tests before adding a new abstraction.
- Prefer straightforward code over clever compression or premature
  generalization.
- Preserve existing accessibility, keyboard behavior, theme behavior, and
  import/export compatibility unless the task explicitly changes them.
- Add a regression test for every defect when a practical test layer exists.
- Update affected documentation and add an ADR for durable cross-module
  decisions.
- Generated-import conformance tests must use isolated temporary storage and
  must never point at the user's live ostudy data directory.

## Repository safety

Never commit:

- secrets, credentials, tokens, or `.env` files;
- `node_modules/`, `dist/`, or `src-tauri/target/`;
- the live `library.sqlite3`;
- personal imported documents or managed image libraries;
- generated artifacts that are not intentional lockfiles or release assets.

Do not use destructive Git commands, force-push, rewrite shared history, merge a
pull request, or push directly to `main` unless the human explicitly requests
that exact operation.

## Required validation

Use the repository's tiered validation model instead of repeatedly running the
most expensive gate while editing:

```bash
./scripts/check-changed.sh  # iteration aid based on the current diff
./scripts/check-static.sh   # repository/IPC/type/format contracts
./scripts/check-fast.sh     # static + complete frontend tests + Rust compile
./scripts/check.sh          # authoritative full merge qualification
```

Draft pull requests run the fast tier; ready pull requests run the full tier on
the self-hosted runner. A green result qualifies only the exact commit SHA it
ran against. Use `OSTUDY_FORCE_CHECK=1` when an exact-tree cache must be bypassed.
For release qualification, also run `./scripts/release-check.sh`.

The full Rust tier includes the generated `.ostudy.json` conformance corpus:
supported syntax, rejection cases, declared boundaries, duplicate/create/append
behavior, fresh schedules, whole-file atomicity, and cleanup of its disposable
test graph and temporary files.

Manually exercise every affected UI workflow. Static inspection, parsing, or a
successful frontend build must never be reported as a successful native Tauri
test. See `docs/development/testing-and-ci.md` for the authoritative tier details.

## Required handoff

Every completed change must report:

- exact base commit SHA and branch;
- acceptance criteria addressed;
- changed files and why each changed;
- tests added or updated;
- exact commands actually run and their results;
- commands not run and why;
- manual behavior actually exercised;
- known limitations, migration effects, and residual risks.

An agent's work is a proposal until a human reviews and merges it.
