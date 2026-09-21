# PR #104 roadmap — collections, coherent study materials, PDF and Tutor organization

Status: active implementation successor to merged PR #103. Owned stages: **LK-05 through LK-09**. This roadmap does not authorize merge, inspect a personal library, or re-open LK-01 through LK-04.

Current boundary: **104-A** through **104-F** are implemented on the PR branch. The **104-G pre-terminal cross-stage audit is complete**: maintained user/development/architecture prose is reconciled to the implemented collection/guide/Tutor behavior, command coverage remains owned by the generated/static IPC contract rather than a hand-maintained duplicate list, backup/sync claims match their focused preservation/privacy tests, and no runtime control/helper was removed without proof of exclusive ownership. The branch is now at the candidate-freeze boundary for exact-head terminal Linux/Android qualification and human handoff.

Activation is complete against the actual merged `main`. Continue stage-by-stage on the existing PR #104 branch; do not rebase this work onto the historical PR #103 branch.

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

Implemented ownership is `study_materials/` over schema 24. The target layer records exact source snapshots, one ordinary deck identity, collection-guide revisions, retry-safe batch/artifact receipts, explicit deletion suppression, and restart interruption. It never replaces the mature individual-document authoring pipeline: document targets adopt its existing artifacts, while collection flashcards copy only range-authorized, unedited notes that already passed document generation/admission. `CollectionSynthesis` may organize the collection guide but is never a lower-quality flashcard authoring shortcut.

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

Implemented in 104-E: document and collection guides use the same semantic section renderer and source-bound learner overlays; section-specific actions are collapsed into an accessible overflow menu; generated topic inventories are compacted into human-readable display titles without changing durable topic identity. The local PDF renderer now provides a designed cover and hierarchy, linked contents, bookmarks/named destinations, running headers/footers, repeated table headers, keep-with-next heading pagination, verified canonical source figures with captions, selectable Unicode text, and bounded native math for common fractions/roots/scripts/operators/matrices. Unsupported or overly wide notation remains readable LaTeX source with explicit disclosure rather than executing TeX or silently corrupting layout. Export/re-export stays behind the prepared-export boundary and makes no provider request.

Tests cover long headings, nested structure, multi-page tables, repeated headers, wide formulas, non-Latin/Unicode text, image captions, orphan prevention, TOC/bookmark targets, selectable text, malicious rich-content rejection, learner edits, exact overlay base hashes, and zero provider calls during export/re-export. Structural PDF checks and actual rendered-page/read-order review remain separate evidence.

**Exit / AC-09:** implemented. Document and collection guides share semantics on screen/PDF and export locally with qualified typography/navigation/math behavior; final visual/accessibility inspection remains human acceptance rather than an automated claim.

## LK-09 — Tutor projects and conversation management

Implemented in 104-F on schema 25. Tutor projects persist title, description/instructions, archive state, default study scope and revision. Conversations retain a separate organization revision/archive state and optional project membership; retry-safe Rename, Move, Archive/Restore, Delete, project create/edit/archive/delete, and project-chat creation live behind typed native commands.

New project chats resolve the project's current defaults at creation time unless the learner explicitly selected replacement sources. Moving an existing conversation preserves its current sources unless **Use destination project's current default sources** is explicitly selected. Editing project defaults affects future project chats only. Changing Sources from a project conversation creates a new chat in that same project with the explicit override.

Each admitted Tutor turn freezes its source scope in `captured_scope_json` and captures current project instructions in the immutable request snapshot before provider work. The schema-25 migration backfills pre-project turns from their then-current conversation scope. Rename/move/project changes use organization revisions independent of message revisioning, so organization changes cannot invalidate an already admitted response or rewrite historical authority.

Project membership is not retrieval consent. Conversation retrieval continues to require the existing per-conversation permission and explicit current-interaction opt-in, compares against the historical turn's frozen exact scope, and never admits another chat merely because both belong to one project. Collection scopes remain source-only for conversation retrieval.

The UI uses the shared overflow menu, Dialog/Sheet, confirmation and responsive overlay infrastructure. Project deletion moves its conversations to Ungrouped without changing their sources; conversation deletion remains chat-only. Whole-library backup preserves project/default/organization state, while trusted-device sync strips projects, organization receipts, chats and semantic chat copies.

Focused regressions cover project/default persistence and retry identity, project deletion/ungrouping, moving with and without explicit default application, project instructions, pre-project migration freezing, organization during an admitted response, stale-read invalidation, same-project retrieval isolation, full backup preservation, and sync exclusion. The shared overflow primitives retain their existing compact/coarse-pointer/keyboard/focus behavior.

**Exit / AC-10:** implemented. Tutor organization works without changing historical source scopes, implicitly enabling cross-chat retrieval, or enabling deferred model-writing tools.

## Delivery sequence and qualification

Implement coherent vertical increments:

1. **104-A:** folder/range persistence + resolver + preservation — implemented.
2. **104-B:** folder UI + Tutor source selection — implemented.
3. **104-C:** synthesis + coverage-aware LearningContext — implemented on the active branch; exact-head qualification is recorded in PR #104.
4. **104-D:** study target/bundle persistence + safe publication/status UI — implemented on the active branch; exact-head qualification is recorded in PR #104.
5. **104-E:** semantic guide viewer + PDF capability/renderer work — implemented on the active branch; exact-head Static/Fast qualification is recorded in PR #104.
6. **104-F:** Tutor project/menu integration — implemented on the active branch; exact-head qualification is recorded in PR #104.
7. **104-G:** pre-terminal cross-stage audit complete; exact-head terminal Linux/Android qualification and human handoff are the remaining boundary.

Each increment: inspect the owning boundary -> implement -> add focused regression coverage -> publish through expected-parent Git data -> verify remote head -> qualify the exact SHA. Add backup/privacy/schema support in the increment that introduces durable data, not at final cleanup.

The pre-terminal audit reconciles user/development/architecture/reference documentation, relies on the maintained static IPC verifier for native/frontend command inventory, checks backup/sync claims against their owning tests, and removes only code proven dead by callers/tests. That audit is complete for this branch; no speculative runtime cleanup was introduced. Freeze this candidate before terminal qualification. Run maintained static/fast, authoritative Linux, Android compile/package, and emulator gates according to current repository policy; report live-provider, visual/accessibility, PDF rendering, and physical-device acceptance separately.

The end-to-end acceptance scenario uses overlapping screenshots, readings, slides, notes, and ranges in one Week 1 collection; creates one guide/deck; preserves edits while a source is added; proves excluded material cannot enter Tutor; exports PDF offline; organizes Tutor conversations; completes backup/restore; and adds the same prepared reading to Week 2 without re-extraction.

## Handoff to PR #105

Pass only the current merged contracts and evidence needed by lessons: SourceSelection/SourceSetSnapshot, CollectionSynthesis, LearningContext/coverage, StudyTarget/bundle lifecycle, EffectiveGuideSnapshot/semantic block interfaces, Tutor project/current-scene context boundaries, preservation fixtures, actual owner paths, and tested merge SHA.

PR #105 must consume those interfaces rather than replaying PR #104 implementation history.

Proposed PR title: **feat(study): document collections, coherent bundles, rich guides and Tutor projects**.
