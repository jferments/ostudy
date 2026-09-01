# Agent and LLM maintenance guide

These files provide operational instructions for automated maintainers of **ostudy**. [`AGENTS.md`](../../AGENTS.md) is the repository-wide entry point and remains authoritative when this directory is read in isolation.

## Start here

For a substantial task, read these in order:

1. [`EXECUTION-STATE.md`](EXECUTION-STATE.md) — the compatibility acceptance snapshot, completion predicate, exact-head evidence vocabulary, progress checkpoints, and session-resumption protocol.
2. [`FAILURE-TRIAGE.md`](FAILURE-TRIAGE.md) — failure classification, stable fingerprints, focused reproduction, retry budget, and stopping rules.
3. [`WORKFLOW-AND-VALIDATION.md`](WORKFLOW-AND-VALIDATION.md) — how to scope, publish, implement, test, review, and hand off a change.
4. [`GITHUB-CONNECTOR.md`](GITHUB-CONNECTOR.md) — repository access, concrete write authorization, remote verification, and connector/service failure diagnosis.
5. [`COMMAND-RUNBOOK.md`](COMMAND-RUNBOOK.md) — the canonical short build/test/install commands. Read this before giving shell instructions instead of reconstructing commands from memory.
6. The architecture and code standards relevant to the requested subsystem.

When resuming existing work, do not begin from conversational memory. Read the open pull request's compatibility acceptance snapshot, then verify its actual remote head and exact-head qualification evidence as the authorities and enumerate incomplete acceptance criteria first.

## Documents

- [`EXECUTION-STATE.md`](EXECUTION-STATE.md) prevents planning, implementation, publication, validation, manual acceptance, and completion from being collapsed into one vague state.
- [`FAILURE-TRIAGE.md`](FAILURE-TRIAGE.md) defines the evidence required before another repair or broad CI run.
- [`ARCHITECTURE-GUARDRAILS.md`](ARCHITECTURE-GUARDRAILS.md) explains the boundaries and invariants that a change must preserve.
- [`CODE-STANDARDS.md`](CODE-STANDARDS.md) defines TypeScript/React, Rust, SQL, shell, documentation, and dependency expectations.
- [`WORKFLOW-AND-VALIDATION.md`](WORKFLOW-AND-VALIDATION.md) defines the reviewable contributor workflow.
- [`GITHUB-CONNECTOR.md`](GITHUB-CONNECTOR.md) defines connector-backed repository operation without conflating read access, write authorization, publication, CI state, or service incidents.
- [`COMMAND-RUNBOOK.md`](COMMAND-RUNBOOK.md) is the concise command source of truth for local iteration, desktop running/building, Android emulator qualification, and physical-device installation.

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

Files under [`../history/`](../history/) are context about completed stages and superseded audits. They are not current implementation instructions.

## Operating rules

- Begin with the current human request, stable acceptance IDs, explicit non-goals, and the current published source.
- Publish the actual implementation directly to one task branch and one draft pull request. A plan, local patch, task-specific applicator, or transient CI-generated tree is not delivery.
- Keep automatic pull-request qualification read-only. It must not push source, author commits, persist write credentials, or change review readiness.
- Consume the machine-readable exact-head qualification result before asking the human to reproduce a CI-capable failure.
- Use [`COMMAND-RUNBOOK.md`](COMMAND-RUNBOOK.md) before giving build/test/install commands; if it lacks a required path, inspect maintained scripts/docs before proposing a new command.
- Do not call a task complete until acceptance, publication, exact-head automated validation, manual boundaries, and known-failure state all agree.

These files do not grant permission to redesign the application. Make the smallest coherent change and preserve every unrelated invariant.
