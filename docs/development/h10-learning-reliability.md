# H-10 and document-learning reliability

Status: **source implementation complete in draft PR #113; exact-head terminal qualification and human live-provider/device acceptance remain.**

Base: `3d94096df1209a644fe8cd0da5b506c62a18a392`. The [hardening register](post-106-hardening.md) owns the H-series scope; [document AI routing](../architecture/document-ai-routing.md) and [UI architecture](../architecture/ui.md) own maintained behavior.

## Report and implemented diagnosis

A 138-page cardiovascular-drugs chapter exposed several independent failures: Study Guide authoring exhausted the provider completion limit twice; whole-document metadata collapsed to one hyperlipidemia subsection; generated cards over-selected isolated factoids; some failed/partial streams lost token-accounting evidence; and matching drag/drop was unreliable under pointer capture.

PR #113 repairs those owners rather than adding an unbounded retry. Guide authoring now preflights the recovery request as well as the first attempt, reserves 6,144 visible tokens initially and 12,288 for a truncation recovery, and uses a compact recovery schema/instruction that preserves the narrative overview, high-yield key points, necessary qualifications and supported terms while dropping optional expansion fields. Multi-unit work that cannot reserve that recovery budget partitions before provider dispatch instead of silently shrinking away supported knowledge.

OpenRouter reasoning requests obey the provider's unified-object constraint: when an explicit reasoning-token cap is used, the request sends that cap instead of simultaneously sending an effort value. The requested/effective reasoning choice remains provenance. This keeps hidden reasoning and visible structured output from competing through an invalid/ambiguous wire combination.

Streaming accounting is owned by the physical provider attempt. Cumulative usage snapshots are merged rather than summed, later partial/cost-only reports enrich rather than erase known counts, and usage observed before a truncation/protocol failure is persisted with the failed attempt. Missing fields remain unknown; ostudy never invents zero or retries successful work merely to obtain usage.

Whole-document display metadata now receives valid bounded JSON representing an evenly distributed sample of the whole outline, including the final section, instead of byte-truncating serialized JSON. Provider/source-title candidates that are clearly leaf labels are rejected for multi-section documents. Deterministic fallback prefers a broad repeated parent or descriptive source-level subject rather than promoting the first narrow section. Derived metadata generator/prompt versions change; retained source knowledge and learner edits do not.

Card generation now states a concept-first teaching policy before choosing a question family. Distinct supported concepts, mechanisms, relationships, comparisons and decision-relevant exceptions outrank repeated low-value details and soft format weights. Evidence/importance labels remain advisory and native admission/evidence binding remains authoritative. The policy fingerprint changes derived candidate reuse without rewriting retained knowledge or schedules.

Matching drag/drop now resolves the actual scoped drop target beneath a pointer-captured moving tile, treats the row gutter as the same semantic target, keeps unrelated row order stable, cancels safely on lost capture/blur/Escape/question reset/disabled state, suppresses delayed touch compatibility clicks, and retains keyboard/click alternatives.

H-10 cleanup removes dead deck-library props, the fixture-only global reasoning setting, the compatibility Modal facade, dead live Cards/Templates route aliases and the redundant template dirty owner. Historical checkpoint/workspace values are still normalized at their boundary; Templates remains a Settings tab and shares Settings dirty-state protection.

## Acceptance status

| ID | Boundary | Status and evidence owner |
| --- | --- | --- |
| AC-01 | H-10 bundled props | **Implemented.** Current deck-library props and frontend settings types no longer carry `bindPreview`, `onManageTemplates` or global `aiReasoningEffort`; native historical readers remain. |
| AC-02 | H-10 overlays | **Implemented.** Card workflows import shared Dialog directly; mutation dismissal/focus/native-Back behavior has focused React coverage. |
| AC-03 | H-10 routes/settings | **Implemented.** Live application sections/workspaces are current-only; old checkpoint/workspace inputs normalize before live state. Settings owns template dirty state and practice-return/checkpoint contracts remain covered. |
| AC-04 | Guide feasibility/recovery | **Implemented.** First/recovery output and reasoning/context reserves are preflighted; truncation gets a bounded larger/leaner recovery rather than identical replay. |
| AC-05 | Whole-document metadata | **Implemented.** Whole-outline context is valid/bounded and sampled across the represented source; narrow leaf titles are rejected with deterministic broad fallback. |
| AC-06 | Concept-first cards | **Implemented.** Authoring/selection prioritizes distinct high-value supported concepts before soft type mix or incidental details; native evidence admission is unchanged. |
| AC-07 | Usage observation | **Implemented.** Physical attempts retain known streaming usage on success/failure, merge cumulative snapshots once, and preserve unknown fields as unknown. |
| AC-08 | Matching interaction | **Implemented.** Captured-pointer targeting, row stability, reassignment/cancellation and keyboard/click/touch alternatives have deterministic regressions. |
| AC-09 | Documentation and qualification | **In progress.** Maintained owners are reconciled before candidate freeze; Static/Fast and terminal Linux/package/Android/emulator evidence must be attached to the frozen exact head before handoff. |

## Non-goals and manual boundaries

No new provider, silent model fallback, paid service, database pool, scheduler redesign, compatibility-reader purge, released migration rewrite, blanket stored-title rewrite, arbitrary retry/timeouts, weakened validators, new drag/drop framework, runner change or source-writing Action is introduced.

The synthetic regressions deliberately do not hardcode the reported textbook or a preferred medical answer. The actual pharmacology PDF, credentials and learner library are never committed. Reprocessing that source with the user's selected OpenRouter model is a **human live-provider quality boundary**: inspect whether the title reflects the whole cardiovascular-drugs chapter, the Study Guide completes, cards emphasize transferable concepts, and displayed token/cache/retry observations match what the provider actually reports. Physical touch behavior remains a separate device acceptance boundary after emulator qualification.
