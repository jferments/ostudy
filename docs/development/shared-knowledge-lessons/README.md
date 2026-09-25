# Shared knowledge and image collections — PR #103 implementation record

Status: **LK-00 through LK-04 implemented on PR #103.** This document is now the closed execution record for the foundation tranche only. Later stages were split into successor pull requests so implementation, review, qualification, and resumption can operate against smaller coherent scopes.

PR #103 starts from `main` commit `30ae169fbe1b4c70add0ed0f6db2b847060c70a0` and delivers:

- LK-01 — full-size Practice-faithful author preview;
- LK-02 — read-only current-format admission and complete-library preservation fixtures;
- LK-03 — reusable canonical evidence, qualified knowledge, coverage receipts, shared preparation, and LearningContext;
- LK-04 — automatic image-collection ingestion, retained originals, local extraction reuse, conservative upload-order-independent overlap, automatic card/guide generation, backup preservation, and optional original inspection.

No LK-05 or later product implementation belongs to this branch.

## Successor delivery split

The successor split remains the authority for scope, but its lifecycle has advanced beyond the original planning-only state. PR #104 is the active successor and has implemented LK-05–LK-09; its remaining boundary is final cross-stage/terminal qualification and handoff. PR #105 and PR #106 remain successor scopes and must not be treated as implemented merely because their planning records exist.

| Successor | Current status | Scope | Roadmap |
| --- | --- | --- | --- |
| [PR #104](https://github.com/jferments/ostudy/pull/104) | Merged | folders/ranges, collection synthesis, coherent bundles, rich guide/PDF, Tutor organization | `docs/development/lk-followups/collections-guides-tutor.md` |
| [PR #105](https://github.com/jferments/ostudy/pull/105) | **Deferred planning reference** | future LK-10–LK-12: responsive lessons, retained narration, PowerPoint | `docs/development/lk-followups/lessons-narration.md` |
| [PR #106](https://github.com/jferments/ostudy/pull/106) | **Active** | production/dev isolation, contributor tooling, post-#104 data baseline/adoption, cleanup, packaging safety and qualification | `docs/development/lk-followups/consolidation-qualification.md` |

Each successor is activated only after its predecessors are merged. Activation means retargeting that PR to the then-current `main`, integrating the merged predecessor source, reconciling actual owner/test paths once, and then resuming implementation from its first unfinished acceptance boundary.

## PR #103 architecture boundary

```text
source bytes / retained images
  -> canonical evidence + owned occurrences
  -> qualified knowledge + honest coverage receipts
  -> permission-bounded LearningContext
  -> existing card / guide / Tutor consumers

author draft
  -> native validation / projection
  -> Practice-faithful preview without scheduling writes

image collection intake
  -> native byte admission + originals
  -> reusable local extraction
  -> conservative whole-collection overlap proof
  -> existing document generation services
  -> existing deck + study guide artifacts
```

Rust and SQLite remain authoritative for durable identities, validation, current-format admission, evidence ownership, question evaluation, publication, backup, and deletion. React renders typed intent/results through `src/ipc/` and feature controllers. Provider HTTP remains in the native AI gateway. No alternate source publisher, graph service, hidden cloud converter, or second scheduler was introduced.

## PR #103 durable contracts

The bullets below deliberately describe the closed PR #103 boundary, not current `main` or an active successor branch.

- PR #103's public-lineage schema is **21**. Historical migrations/checksums in that tranche remain unchanged.
- Migration 18 adds immutable canonical evidence payloads, document-owned occurrences, and structure links.
- Migrations 19–21 add ordered image-set ownership/manifests, extraction receipts/cache, and revision-safe image edit/filter state.
- Typed knowledge detail and region coverage retain versioned validation and fail-closed startup/restore admission.
- Original image bytes remain independently owned. Cleaning/projection never replaces source authority.
- Existing authoring JSON, `.studydeck`, backup, trusted-device sync, Anki/APKG, rich content, Basic/Cloze/objective questions, schedules/reviews, Tutor history, and learner edits remain distinct contracts.
- Historical migration/recovery machinery remains live until active PR #106 performs the separately authorized actual-library adoption and retirement gate. Deferred PR #105 no longer owns that boundary.

## Work reuse and invalidation

A hash is lookup identity, not authorization or ownership. Compatible callers may share prepared work without widening consent. Consumer-model changes do not invalidate compatible document knowledge. Image reordering/append work reuses unchanged extraction; changed or ambiguous pixels/text remain separate evidence. Canceling one subscriber does not destroy another consumer's useful completed work. A source disposition is not a certificate of exhaustive understanding.

## Validation and handoff

Stage implementation, source publication, static validation, fast qualification, full Linux, Android compile/package, emulator, live-provider, physical-device, visual/accessibility, review, and merge are separate states.

The final PR body is the acceptance ledger for exact SHA/run evidence. This documentation does not convert an unrun platform or human gate into a pass.

For publication/recovery follow [GitHub-direct development](../../agents/GITHUB-DIRECT-DEVELOPMENT.md), [the interruption protocol](../../agents/RECOVERY-PROTOCOL.md), and [the command runbook](../../agents/COMMAND-RUNBOOK.md). No automatic merge is authorized.
