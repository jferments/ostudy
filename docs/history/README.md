# Historical project records

This directory preserves documents that were useful during an earlier implementation stage but are no longer current instructions. They remain available for project archaeology, design context, and understanding why a later refactor exists.

Historical records are not maintained as descriptions of the active product. They may contain completed milestones, superseded filenames, old command surfaces, obsolete platform assumptions, or validation expectations for a specific pull request.

## Contents

- `early-architecture-and-plan.md` records an early project baseline and near-term sequence that has since been completed or replaced.
- `phase-1-validation-report.md` records the expected validation for the initial repository-stabilization work.
- `phase-2-roadmap.md` preserves the completed standalone-flashcard phase and its deferred items.
- `stage-3-card-types-and-templates.md` preserves the user/design guide written when Reverse, Fill in the blank, and reusable templates were introduced.
- `code-review-2026-08-03.md` preserves the dated repository review that informed the subsequent refactoring series.

## How to use these files

Use a historical document to answer questions such as:

- What problem was a completed stage trying to solve?
- Which constraints were accepted at that time?
- Why does a migration, compatibility adapter, or regression test exist?
- Which work was explicitly deferred from an earlier phase?

Do not use it as the installation guide, current roadmap, current architecture, or required validation checklist. Start from [`../README.md`](../README.md), [`../roadmap.md`](../roadmap.md), [`../architecture/`](../architecture/), and [`../development/`](../development/) instead.

When a maintained document becomes historical, move it here with a descriptive filename and add a short note explaining its scope. Do not leave a stale document at the repository root where a new reader may reasonably mistake it for current guidance.

- [Pre-public schema evolution](pre-public-schema-evolution.md) — architectural history retained after establishing the public schema baseline.
