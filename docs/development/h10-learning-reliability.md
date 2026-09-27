# H-10 and document-learning reliability

Implementation scope following merged PR #112, base `3d94096df1209a644fe8cd0da5b506c62a18a392`. This is an acceptance plan, not a completion or live-provider claim. The [hardening register](post-106-hardening.md) owns the H-series scope; [document AI routing](../architecture/document-ai-routing.md) and [UI architecture](../architecture/ui.md) own the maintained contracts.

## Report and diagnosis boundaries

The learner reported that a 138-page cardiovascular-drugs chapter generated cards but failed its first Study Guide topic after two completion-limited responses. Its document title described only hyperlipidemia patient education, and many cards tested isolated details rather than transferable concepts. Matching-question drag/drop also needs behavioral diagnosis and regression coverage.

The existing guide completion plan reserves 3,200 visible tokens for both the first attempt and recovery. PR #112 changed truncation recovery in knowledge extraction; that does not establish that guide authoring has adequate output feasibility. Inspect the actual request/schema, reasoning policy, retained-knowledge partitioning and retry ownership together rather than only increasing a constant.

Missing usage is a separate observation, not a generation failure. Preserve partial reported usage, consume a final usage-only streaming event, never double count snapshots, and never invent missing token counts or retry successful generation to obtain accounting.

Document-wide metadata must not silently choose the first or most frequent narrow topic as the title of an entire chapter. A document title, section heading and retrieval question have different scopes. Retained knowledge must remain lossless while teaching artifacts prioritize core concepts, relationships, mechanisms, meaningful contrasts and decision-relevant exceptions.

## Acceptance criteria

| ID | Boundary | Required result |
| --- | --- | --- |
| AC-01 | H-10 bundled props | Remove unused `bindPreview`, `onManageTemplates` and fixture-only global `AppSettings.aiReasoningEffort` with their current callers/fixtures. Preserve native historical migrations/readers. |
| AC-02 | H-10 overlays | Migrate remaining compatibility Modal callers to the shared Dialog facade and retire the unused wrapper. Preserve busy/non-dismissible, nested-overlay, focus, Escape and native Back behavior. |
| AC-03 | H-10 routes/settings | Normalize supported old checkpoints at the reader boundary, then simplify live route aliases and obsolete appearance interfaces. Preserve Settings/Templates intent, dirty-state guards, practice return targets, real profile/task-assignment fields and restored preferences. |
| AC-04 | Guide feasibility/recovery | First-pass and recovery requests have coherent schema/instructions, visible-output/reasoning reserves and context preflight. Completion-limited work changes size or allowance within a bounded policy; it does not blindly replay the same request or discard successful sibling work. |
| AC-05 | Whole-document metadata | Title and introductory scope represent the full source, not a single chapter subsection. Preserve learner edits and compatible retained knowledge; version only affected derived artifact contracts. |
| AC-06 | Concept-first cards | Generation prioritizes major concepts, mechanisms, comparisons and source-supported applications; isolated details are subordinate and only retained when materially useful. All evidence, qualifications and native validation remain required. |
| AC-07 | Usage observation | Streaming usage after finish markers is retained once; missing or interrupted reporting is distinct from task failure, with truthful contextual presentation and no fabricated totals. |
| AC-08 | Matching interaction | Mouse/touch/pointer matching correctly uses target identity despite capture, reassigns without duplicates, cancels safely and retains keyboard/click alternatives. Reset/reveal/disabled states must not apply stale drag actions. |
| AC-09 | Documentation and qualification | Focused regressions, exact-head Static/Fast and required Linux/package/Android/emulator qualification; reconcile owning docs and PR evidence before handoff. Live-model educational quality and physical-device interactions remain explicit manual boundaries. |

## Implementation order

Complete reviewable increments on `agent/h10-learning-quality`: bundled interfaces and matching interaction; generation budgets/protocol and usage; global metadata and concept-prioritized authoring; remaining route/appearance cleanup; documentation and frozen-candidate qualification. Every increment carries focused behavioral coverage. Actual discoveries may alter ordering but not silently remove an acceptance criterion.

## Non-goals and preservation

No new provider, silent model fallback, paid service, database pool, scheduler redesign, compatibility-reader purge, released migration rewrite, blanket stored-title rewrite, arbitrary retry/timeouts, weakened validators or source-writing Actions. No host/runner infrastructure changes. Do not add a second overlay stack or drag/drop framework without a demonstrated need. Do not merge this PR without a new explicit merge request.

A chapter-specific regression may use synthetic cardiovascular topic labels, but production code must not hardcode this textbook, its name, page count or a preferred medical answer. The actual pharmacology PDF, credentials and learner library are never committed.
