# PR #104 roadmap — collections, coherent study materials, PDF and Tutor organization

Status: planning-only successor to PR #103. Owned stages: **LK-05 through LK-09**. This roadmap does not implement product behavior, authorize merge, inspect a personal library, or re-open LK-01 through LK-04.

Activation rule: implementation begins only after PR #103 is reviewed and merged. At activation, integrate the actual merged `main`, re-read the owning boundaries once, reconcile any renamed paths, and then proceed stage-by-stage. Do not stack product implementation on the unmerged PR #103 branch.

## Product result

A learner can organize retained documents into folders, reuse bounded reading ranges across different weeks, and select those folders or individual materials in Tutor. oStudy resolves one permission-safe source snapshot, reuses already prepared document knowledge, builds a compact cross-document understanding, and creates one coherent Study Guide plus one ordinary flashcard deck for the selected study target. Later source additions update that target without multiplying decks, overwriting learner edits, or resetting schedules.

Guides use richer semantic teaching content with the same effective meaning in the viewer and a professional local PDF. Tutor conversations gain project organization and Rename, Move, Archive, and Delete operations without rewriting historical source scopes.

Not included: responsive lessons, narration, PowerPoint, final schema consolidation, STT, article generation, automatic image-occlusion generation, Tutor deck-writing tools, a new scheduler, a graph service, cloud PDF conversion, or unrelated navigation redesign.

## Architecture and ownership

Dependency direction remains:

```text
retained documents + canonical evidence + prepared knowledge
  -> folder membership + bounded ranges
  -> authorized SourceSetSnapshot
  -> thin CollectionSynthesis + explicit coverage
  -> LearningContext
  -> StudyTarget / bundle orchestration
  -> ordinary deck + effective guide
  -> Practice / guide viewer / local PDF

Tutor project defaults
  -> same SourceSelection resolver
  -> frozen per-turn source snapshot
```

Native Rust/SQLite owns durable rules, identities, permission checks, revisions, publication, backup, and deletion semantics. React renders typed intent and results through `src/ipc/`. Provider calls, decoding, synthesis, and PDF layout stay outside database locks. Publication remains a short revision-checked transaction after any managed files are staged.

Expected owners to extend include current `knowledge/`, document repositories/controllers, `src/ipc/knowledge.ts`, `src/ipc/documents.ts`, guide generation/export owners, Tutor repository/services, backup/sync, and navigation/task controllers. Proposed focused owners such as `collections/`, `knowledge/collection_view.rs`, `study_materials/`, and `src/ipc/collections.ts` are names to reconcile with the merged codebase, not mandates to create duplicate facades.

## Contracts that must remain stable

**Folder membership** is organization, not source ownership. One document can belong to multiple folders. Moving or deleting a membership does not silently move/delete the underlying document elsewhere.

**SourceSelection** records folders/materials/ranges/exclusions chosen by the learner. **SourceSetSnapshot** is the canonical, immutable resolution used for a specific operation. It binds resolved members, ranges, source/interpretation/filter revisions, exclusions, and authorization identity. Display names are never evidence identities.

**CollectionSynthesis** stores compact topics, relationships, instructional emphasis, unresolved objectives, and dependency references. It does not copy every member fact, canonical block, or embedding into folder-owned storage.

**StudyTarget** is explicitly a document or collection and owns output policy, exact source snapshot, artifact references, revision/suppression state, and retry-safe publication receipts. No synthetic source document is invented for a collection.

**EffectiveGuideSnapshot** combines the immutable generated guide revision with compatible learner overlays/conflicts, semantic blocks, source links, and disclosure settings once. Screen and PDF renderers consume that same meaning.

**TutorProject** is distinct from a document folder. Project defaults can reference source selections, but project membership does not grant cross-conversation retrieval or rewrite historical turns.

## LK-05 — folders, reading ranges, exact source snapshots, Tutor selection

### Persistence and native resolver

Add a bounded folder hierarchy, organization revision, document membership, and range records through a forward migration. Support create/rename/archive/delete-folder and add/move/remove-membership with native validation, expected revisions, retry identities, cycle prevention, and bounded traversal.

Ranges bind to actual source capabilities: page, slide, image occurrence, or stable section/block selectors where supported. Human-readable labels such as “Chapter 3” are display metadata unless backed by a precise selector. Overlapping intervals normalize deterministically; invalidated ranges become explicit after source revisions rather than being guessed.

Resolve folder/material unions and exclusions in one native service. Deduplicate parent/child selection, repeated document references, and overlapping ranges before work/byte limits are enforced. Admit complete evidence dependencies together. If a requested claim depends on excluded evidence, omit/flag it or require explicit scope expansion; never silently widen permission.

### UI and Tutor

Add folder Overview/Sources views, breadcrumbs, thumbnails, contextual operations, empty states, keyboard/touch move alternatives, and a range selector that exposes the precision the source actually supports. Browsing organization makes zero provider calls.

Tutor uses the exact same resolver before retrieval and after hydration. Each turn stores its captured snapshot. Updated folder membership may affect later turns only under current authorization; in-flight and historical turns remain frozen.

### Focused tests

Cover hierarchy cycles/depth, duplicate membership, overlapping/disjoint ranges, unsupported locator precision, stale revisions, exclusions at text/unit/image/summary/artifact layers, member changes during provider work, backup/restore, duplicate retries, and refresh failure after committed mutations. Adding an already prepared source must not rewrite its document knowledge or trigger interpretation.

**Exit / AC-06:** collection organization and permission-safe Tutor selection work independently of automatic generation.

## LK-06 — thin cross-document synthesis and coverage-aware context

Build a complete permitted catalog of member knowledge/coverage references without copying source knowledge. Use bounded semantic candidate retrieval together with lexical identifiers and structural context to produce topic summaries, complementary/contrast/prerequisite relations, unresolved objectives, and dependency links.

Separate **instructional emphasis** from **factual support**. Slides or assignment prompts can influence what deserves attention without becoming evidence for facts they do not state. Repetition is not independent corroboration. Preserve conditions, units, polarity, exceptions, and contradictions.

Incremental updates inspect every changed member manifest, including minority topics and empty/unresolved dispositions. Removal invalidates every dependent synthesis item, even if the removed document was only transitively referenced. Member knowledge rows remain untouched.

Tutor retrieval and complete-material coverage are separate policies: Tutor receives a bounded relevant context; guide/card generation receives an explicit selected-scope/depth coverage plan and records omissions/gaps rather than pretending top-k retrieval is complete coverage.

Tests cover sparse slides plus dense readings, contradictions, qualified values, minority topics, new sources resolving gaps, deletion invalidation, exclusions, request counts, and zero document reanalysis when a prepared member is added.

**Exit / AC-07:** collection synthesis improves combined context while preserving source ownership and honest coverage.

## LK-07 — automatic coherent study bundles

Persist real study targets with source snapshots, output policy, ordinary deck/guide identities, work/retry receipts, and explicit deletion suppression. One durable upload/collection batch coordinates shared preparation, optional synthesis, and parallel guide/card composition.

The first successful publication creates the target's ordinary deck exactly once. Ambiguous acknowledgements/retries recover the same publication. Later source additions append only novel, validated cards while preserving learner edits, stable card identities, schedules, reviews, and surviving cards whose sources later leave the collection.

Guide updates create successor revisions with overlay reconciliation. A failed/cancelled sibling never deletes a successful artifact. Deliberate user deletion records suppression so retries do not resurrect the artifact; explicit recreation is a new authorized action.

Replace generation-button clutter with compact Study Guide and Flashcards status links. Show measured progress when real, indeterminate phases when not, and completion only after durable commit. Restart reconciles known jobs but never silently reauthorizes cloud requests or generates across the historical library.

Tests cover duplicate upload events, simultaneous consumers, partial provider failure, cancellation at publication boundaries, late source edits, lost responses, restart, deletion suppression, edited-card protection, guide conflicts, embedding unavailability, and import-success/generation-failure separation.

**Exit / AC-08:** one target produces one coherent deck/guide bundle and updates are nondestructive.

## LK-08 — rich guide semantics and professional local PDF

Normalize effective guide content into supported semantic headings, paragraphs, key terms, callouts, comparisons, tables, processes, figures, math, worked examples, misconceptions, and retrieval checks. Reuse the existing validated rich-content language; do not create a second general editor format.

Guide authoring uses shared LearningContext/coverage to produce coherent explanations, definitions, prerequisites, assumptions, exceptions, and examples with component-level support. Do not require empty template sections. Semantic emphasis is represented structurally, not by a post-hoc search-and-bold pass.

Viewer and PDF consume the same effective snapshot, source links, learner overlays/conflicts, and notes/disclosure setting. PDF styling is print-specific and independent of app dark theme. Preserve selectable text where possible.

Before changing renderers, qualify the existing native PDF boundary with fixtures for linked contents, bookmarks, heading/read-order semantics, fonts/glyphs, tables across pages, figures, and typeset math. Extend/replace only a demonstrated missing capability behind the existing prepared-export boundary. No cloud/Python/Office runtime dependency.

Tests cover long headings, nested structure, multi-page tables, repeated headers, wide formulas, non-Latin fallback, image captions, orphan prevention, TOC/bookmark targets, selectable text, malicious rich-content rejection, learner edits, and zero provider calls during export/re-export. Structural PDF checks and actual rendered-page/read-order review are separate evidence.

**Exit / AC-09:** document and collection guides share semantics on screen/PDF and export locally with qualified typography/navigation/math behavior.

## LK-09 — Tutor projects and conversation management

Persist Tutor project title, description/instructions, archive state, default SourceSelection, and organization revision. Conversations gain optional project membership and their own title/archive revision. Add focused Rename, Move, Archive, Delete commands with expected revisions and retry identities.

New conversations inherit current project defaults. Moving an existing conversation preserves its current sources unless the learner explicitly applies destination defaults. Project default changes affect future choices, never historical turn snapshots. Common project membership does not opt conversations into cross-chat retrieval.

Use the existing overflow menu, confirmation, focus, touch, and Back infrastructure. Project deletion defaults to moving conversations to ungrouped; deleting a conversation does not delete independently created study materials.

Tests cover rename/move while a response is active, archive/delete during stale refresh, duplicate retry, source-default changes, exclusion preservation, committed write followed by refresh failure, compact/wide/touch/keyboard behavior, backup/deletion semantics, and explicit sync privacy boundaries.

**Exit / AC-10:** Tutor organization works without changing historical source scopes or enabling deferred model-writing tools.

## Delivery sequence and qualification

Implement coherent vertical increments:

1. **104-A:** folder/range persistence + resolver + preservation.
2. **104-B:** folder UI + Tutor source selection.
3. **104-C:** synthesis + coverage-aware LearningContext.
4. **104-D:** study target/bundle persistence + safe publication/status UI.
5. **104-E:** semantic guide viewer + PDF capability/renderer work.
6. **104-F:** Tutor project/menu integration.
7. **104-G:** cross-stage audit, exact-head terminal qualification, and human handoff.

Each increment: inspect the owning boundary -> implement -> add focused regression coverage -> publish through expected-parent Git data -> verify remote head -> qualify the exact SHA. Add backup/privacy/schema support in the increment that introduces durable data, not at final cleanup.

Before terminal qualification, reconcile user/development/architecture/reference documentation, native command inventories, backup/sync claims, and proven dead controls/helpers. Then freeze the candidate. Run maintained static/fast, authoritative Linux, Android compile/package, and emulator gates according to current repository policy; report live-provider, visual/accessibility, PDF rendering, and physical-device acceptance separately.

The end-to-end acceptance scenario uses overlapping screenshots, readings, slides, notes, and ranges in one Week 1 collection; creates one guide/deck; preserves edits while a source is added; proves excluded material cannot enter Tutor; exports PDF offline; organizes Tutor conversations; completes backup/restore; and adds the same prepared reading to Week 2 without re-extraction.

## Handoff to PR #105

Pass only the current merged contracts and evidence needed by lessons: SourceSelection/SourceSetSnapshot, CollectionSynthesis, LearningContext/coverage, StudyTarget/bundle lifecycle, EffectiveGuideSnapshot/semantic block interfaces, Tutor project/current-scene context boundaries, preservation fixtures, actual owner paths, and tested merge SHA.

PR #105 must consume those interfaces rather than replaying PR #104 implementation history.

Proposed PR title: **feat(study): document collections, coherent bundles, rich guides and Tutor projects**.
