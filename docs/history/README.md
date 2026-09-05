# Historical project records

This directory preserves documents that were useful during earlier implementation stages but are no longer current product or workflow instructions. They remain available for project archaeology, design context, regression provenance, and understanding why compatibility code or later refactors exist.

Historical records can contain completed milestones, superseded filenames, old command surfaces, obsolete platform assumptions, or qualification expectations for a particular pull request. When they conflict with active source, accepted ADRs, or maintained current documentation, they are historical evidence—not authority.

## Early project records

- [Early architecture and plan](early-architecture-and-plan.md) — initial project baseline and near-term sequence that has since been completed/replaced.
- [Phase 1 validation report](phase-1-validation-report.md) — expected validation for initial repository stabilization.
- [Phase 2 roadmap](phase-2-roadmap.md) — completed standalone-flashcard phase and its deferred items.
- [Stage 3 card types and templates](stage-3-card-types-and-templates.md) — design record from the introduction of Reverse, Fill in the blank, and reusable templates.
- [Code review 2026-08-03](code-review-2026-08-03.md) — dated repository review that informed subsequent refactoring.
- [Pre-public schema evolution](pre-public-schema-evolution.md) — schema history retained after establishing the public lineage.

## Completed frontend/UI work

- [Frontend/CSS ownership refactor checkpoint](frontend-css-refactor-checkpoint.md) — PR #75/#76 completion record for explicit CSS/UI ownership and the standalone artifact boundary. Its old branch-specific source-export workflow has been removed.
- [UI workflow redesign roadmap](ui-workflow-redesign-roadmap.md) — staged plan that produced the current simplified navigation/workflow structure.
- [UI redesign current-behavior characterization](ui-workflow-redesign-current-behavior-characterization.md) — pre-redesign behavior snapshot.
- [UI redesign affected-surface inventory](ui-workflow-redesign-affected-surface-inventory.md) — implementation inventory used during the staged redesign.
- [UI redesign performance baseline](ui-workflow-redesign-performance-baseline.md) — historical comparable measurement methodology/evidence.
- [UI redesign visual baseline](ui-workflow-redesign-visual-baseline.md) — historical visual evidence from the earlier interface.

Stable pointer files remain at the old `docs/development/ui-workflow-redesign-*.md` paths where repository references/test provenance benefit from a known location, but those pointers direct readers here instead of presenting the stage plans as current architecture.

## Workflow/tooling history

The repository previously experimented with source-checkpoint workflows, orchestrator checklist projection, custom developer MCPs, and multiple publication/execution services. The current workflow deliberately removed task-specific source writers and automatic orchestrator PR-ledger mutation. Current automated-maintainer instructions are in [`../agents/`](../agents/) and the normal source publisher is the explicitly selected branch writer—GitHub-direct for ordinary ChatGPT tasks.

Dated outage/development-workflow records may remain in this directory when they explain why a current safeguard exists. They do not authorize restoring the old machinery.

## How to use historical files

Historical records can answer:

- What problem was a completed stage trying to solve?
- Which constraints were accepted at that time?
- Why does a migration, compatibility adapter, regression fixture, or benchmark exist?
- Which work was explicitly deferred then?
- What evidence established a boundary that current code still protects?

Do **not** use them as the installation guide, current roadmap, current architecture, source-publication procedure, or required validation checklist. Start from:

- [documentation index](../README.md);
- [current roadmap](../roadmap.md);
- [comprehensive architecture](../architecture/overview.md);
- [development guide](../development/guide.md);
- [agent workflow](../agents/GITHUB-DIRECT-DEVELOPMENT.md).

When maintained material becomes historical, move/preserve it here or replace its old path with an explicit historical pointer. Do not leave a completed stage plan in a current-doc directory in a form a new reader could reasonably mistake for today's product or workflow.
