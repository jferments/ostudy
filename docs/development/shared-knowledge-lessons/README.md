# Shared knowledge and image collections — PR #103 implementation record

Status: **LK-00 through LK-04 implemented on PR #103.** This document is the closed execution record for the foundation tranche only. Later stages were split into successor pull requests so implementation, review, qualification, and resumption can operate against smaller coherent scopes.

PR #103 starts from `main` commit `30ae169fbe1b4c70add0ed0f6db2b847060c70a0` and delivers:

- LK-01 — full-size Practice-faithful author preview;
- LK-02 — read-only current-format admission and complete-library preservation fixtures;
- LK-03 — reusable canonical evidence, qualified knowledge, coverage receipts, shared preparation, and LearningContext;
- LK-04 — automatic image-collection ingestion, retained originals, local extraction reuse, conservative upload-order-independent overlap, automatic card/guide generation, backup preservation, and optional original inspection.

No LK-05 or later product implementation belongs to this foundation record.

## Successor delivery split

PR #104 is merged with LK-05–LK-09 delivered. PR #105 is intentionally deferred as the future lessons/narration/PowerPoint plan. PR #106 was squash-merged on 2026-09-26 as `9ef91e79346b6d0d0ace983da3c1583303f15a43`; it is no longer active. PR #108 completes documentation alignment, while the [hardening register](../post-106-hardening.md) records the separate, unimplemented runtime follow-ups.

| Successor | Current status | Scope | Record |
| --- | --- | --- | --- |
| [PR #104](https://github.com/jferments/ostudy/pull/104) | Merged | collections/ranges, collection synthesis, coherent bundles, rich guide/PDF, Tutor organization | [Implementation record](../lk-followups/collections-guides-tutor.md) |
| [PR #105](https://github.com/jferments/ostudy/pull/105) | **Deferred planning reference** | future LK-10–LK-12: responsive lessons, retained narration, PowerPoint | [Preserved plan](../lk-followups/lessons-narration.md) |
| [PR #106](https://github.com/jferments/ostudy/pull/106) | **Merged** | production/dev isolation, contributor tooling, post-#104 release baseline/conversion, packaging safety and acceptance | [Completion record](../lk-followups/consolidation-qualification.md) |

Successor activation follows the accepted scope, not a permanent numeric dependency chain: PR #106 was activated while PR #105 remained deferred. Future work starts from the then-current merged baseline, not the historical #103 or #106 branches.

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
- PR #106 subsequently established `ostudy-release-v1` schema 1, retained public-v1 compatibility inputs, and retired automatic final-v26 runtime adoption while preserving explicit historical recovery. Deferred PR #105 did not own that completed boundary.

## Work reuse and invalidation

A hash is lookup identity, not authorization or ownership. Compatible callers may share prepared work without widening consent. Consumer-model changes do not invalidate compatible document knowledge. Image reordering/append work reuses unchanged extraction; changed or ambiguous pixels/text remain separate evidence. Canceling one subscriber does not destroy another consumer's useful completed work. A source disposition is not a certificate of exhaustive understanding.

## Validation and handoff

Stage implementation, source publication, static validation, fast qualification, full Linux, Android compile/package, emulator, live-provider, physical-device, visual/accessibility, review, and merge are separate states.

The final PR body and exact-SHA Actions evidence record qualification; this documentation does not convert an unrun platform or human gate into a pass. See #106's completion record for the distinction between disposable installed-copy acceptance and protected-library cutover.

For publication/recovery follow [GitHub-direct development](../../agents/GITHUB-DIRECT-DEVELOPMENT.md), [the interruption protocol](../../agents/RECOVERY-PROTOCOL.md), and [the command runbook](../../agents/COMMAND-RUNBOOK.md). No automatic merge is authorized.
