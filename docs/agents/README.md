# Agent and LLM maintenance guide

These files provide operational instructions for automated maintainers of
**ostudy**. `AGENTS.md` is the repository-wide entry point and remains
authoritative when this directory is read in isolation.

## Documents

- [`ARCHITECTURE-GUARDRAILS.md`](ARCHITECTURE-GUARDRAILS.md) explains the
  boundaries and invariants that a change must preserve.
- [`CODE-STANDARDS.md`](CODE-STANDARDS.md) defines TypeScript/React, Rust, SQL,
  shell, documentation, and dependency expectations.
- [`WORKFLOW-AND-VALIDATION.md`](WORKFLOW-AND-VALIDATION.md) defines how to scope,
  implement, test, review, and hand off a change.

The canonical product and data-model descriptions remain:

- [`../ARCHITECTURE.md`](../ARCHITECTURE.md)
- [`../CARD-FORMAT-AND-DATA-MODEL.md`](../CARD-FORMAT-AND-DATA-MODEL.md)
- [`../DEVELOPMENT-AND-QUALITY.md`](../DEVELOPMENT-AND-QUALITY.md)
- [`../GIT-WORKFLOW.md`](../GIT-WORKFLOW.md)
- [`../decisions/`](../decisions/)

## Operating rule

Do not treat these files as permission to redesign the application. Begin with
the current behavior and the human's acceptance criteria, make the smallest
coherent change, and preserve every unrelated invariant.
