# Agent and LLM maintenance guide

These files provide operational instructions for automated maintainers of **ostudy**. [`AGENTS.md`](../../AGENTS.md) is the repository-wide entry point and remains authoritative when this directory is read in isolation.

## Documents

- [`ARCHITECTURE-GUARDRAILS.md`](ARCHITECTURE-GUARDRAILS.md) explains the boundaries and invariants that a change must preserve.
- [`CODE-STANDARDS.md`](CODE-STANDARDS.md) defines TypeScript/React, Rust, SQL, shell, documentation, and dependency expectations.
- [`WORKFLOW-AND-VALIDATION.md`](WORKFLOW-AND-VALIDATION.md) defines how to scope, publish, implement, test, review, and hand off a change.
- [`PROJECT-CONTEXT.md`](PROJECT-CONTEXT.md) records stable product and repository context that helps an automated maintainer orient itself before reading task-specific code.

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

## Operating rule

Do not treat these files as permission to redesign the application. Begin with the current behavior and the human's acceptance criteria, make the smallest coherent change, and preserve every unrelated invariant. A plan or local patch is not delivery: publish work to the task branch and draft pull request, keep the PR truthful, validate the exact head, and report what was actually tested.
