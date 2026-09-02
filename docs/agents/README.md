# Agent and LLM maintenance guide

These files provide operational instructions for automated maintainers of
**ostudy**. [`AGENTS.md`](../../AGENTS.md) is the repository-wide entry point and
remains authoritative when this directory is read in isolation.

## Start here

For an implementation task, read these in order:

1. [`DIRECT-WORKSPACE.md`](DIRECT-WORKSPACE.md) — the preferred persistent
   read/edit/format/test/commit workflow, capability preflight, tool contract,
   writer handoff, and precise fallback rules.
2. [`EXECUTOR-SELECTION.md`](EXECUTOR-SELECTION.md) — when direct workspace,
   advisory or exclusive Codex, the full orchestrator, observer, exact-head
   validation, and GitHub should be used.
3. [`COMMAND-RUNBOOK.md`](COMMAND-RUNBOOK.md) — canonical short build, test,
   run, Android, and installation commands. Read this before reconstructing
   commands from memory.
4. [`EXECUTION-STATE.md`](EXECUTION-STATE.md) — compatibility acceptance
   snapshot, completion predicate, exact-head evidence vocabulary, and session
   resumption protocol.
5. [`FAILURE-TRIAGE.md`](FAILURE-TRIAGE.md) — failure classification, stable
   fingerprints, focused reproduction, retry budget, and stopping rules.
6. [`WORKFLOW-AND-VALIDATION.md`](WORKFLOW-AND-VALIDATION.md) — how to scope,
   publish, implement, test, review, and hand off a change.
7. [`GITHUB-CONNECTOR.md`](GITHUB-CONNECTOR.md) — repository access, concrete
   write authorization, remote verification, and connector/service diagnosis.
8. The architecture and code standards relevant to the requested subsystem.

The direct workspace is the normal implementation path when its exact seven-tool
contract is healthy, including when Codex credits are available. Codex is
optional: use it either as a read-only adviser while direct workspace retains
writer ownership, or as the exclusive writer of a separate deliberately assigned
bounded branch. Never alternate Codex and direct workspace as writers on one live
branch.

The observer, full orchestrator, exact-head validator, Codex worker, and GitHub
low-level Git-data writer remain useful specialized or fallback mechanisms; they
are not prerequisites for ordinary coding.

When resuming existing work, do not begin from conversational memory. Read the
actual PR and remote head, then call the selected writer workspace's status. The
workspace owns unpublished source, GitHub owns published source, Actions owns
exact-SHA automated evidence, and the PR acceptance snapshot projects those
facts without replacing them.

## Documents

- [`DIRECT-WORKSPACE.md`](DIRECT-WORKSPACE.md) restores the persistent
  development loop and prevents repeated integration discovery, CI-as-editor,
  and read-only observer dead ends.
- [`EXECUTOR-SELECTION.md`](EXECUTOR-SELECTION.md) defines the default writer,
  optional Codex roles, full-orchestrator threshold, one-writer invariant, and
  component decision table.
- [`EXECUTION-STATE.md`](EXECUTION-STATE.md) prevents planning, implementation,
  publication, validation, manual acceptance, and completion from being
  collapsed into one vague state.
- [`FAILURE-TRIAGE.md`](FAILURE-TRIAGE.md) defines the evidence required before
  another repair or broad CI run.
- [`ARCHITECTURE-GUARDRAILS.md`](ARCHITECTURE-GUARDRAILS.md) explains the
  boundaries and invariants that a change must preserve.
- [`CODE-STANDARDS.md`](CODE-STANDARDS.md) defines TypeScript/React, Rust, SQL,
  shell, documentation, and dependency expectations.
- [`WORKFLOW-AND-VALIDATION.md`](WORKFLOW-AND-VALIDATION.md) defines the
  reviewable contributor workflow.
- [`GITHUB-CONNECTOR.md`](GITHUB-CONNECTOR.md) defines connector-backed
  repository operation without conflating read access, write authorization,
  publication, CI state, or service incidents.
- [`COMMAND-RUNBOOK.md`](COMMAND-RUNBOOK.md) is the concise command source of
  truth for local iteration, desktop running/building, Android emulator
  qualification, and physical-device installation.

The maintained product and engineering references are:

- [`../README.md`](../README.md)
- [`../roadmap.md`](../roadmap.md)
- [`../architecture/overview.md`](../architecture/overview.md)
- [`../architecture/card-format-and-data-model.md`](../architecture/card-format-and-data-model.md)
- [`../architecture/ui.md`](../architecture/ui.md)
- [`../architecture/ai-and-document-processing.md`](../architecture/ai-and-document-processing.md)
- [`../development/guide.md`](../development/guide.md)
- [`../development/workflow.md`](../development/workflow.md)
- [`../development/testing-and-ci.md`](../development/testing-and-ci.md)
- [`../decisions/`](../decisions/)

Files under [`../history/`](../history/) are context about completed stages and
superseded audits. They are not current implementation instructions.

## Operating rules

- Begin with the current human request, stable acceptance IDs, explicit
  non-goals, and current published source.
- For normal coding, perform one direct-workspace capability preflight, then use
  the persistent worktree for source edits and local checks. Do not repeatedly
  rediscover integrations during the task.
- Codex is never selected merely because credits exist. Advisory Codex does not
  own source; implementation Codex owns a separate branch exclusively until an
  explicit stopped handoff.
- Publish the first coherent checkpoint to one task branch and open one draft
  pull request. A plan, local patch, task-specific applicator, or transient
  CI-generated tree is not delivery.
- Keep automatic pull-request qualification read-only. It must not push source,
  author commits, persist write credentials, or change review readiness.
- Use CI to qualify a published SHA, not as the normal formatter, editor, or first
  compiler.
- Update the PR compatibility snapshot at meaningful checkpoints and before
  readiness rather than after every local edit.
- Consume machine-readable exact-head qualification before asking the human to
  reproduce a CI-capable failure.
- Use [`COMMAND-RUNBOOK.md`](COMMAND-RUNBOOK.md) before giving build, test, run,
  or install commands.
- Do not call a task complete until acceptance, publication, exact-head automated
  validation, manual boundaries, and known-failure state all agree.

These files do not grant permission to redesign the application. Make the
smallest coherent change and preserve every unrelated invariant.
