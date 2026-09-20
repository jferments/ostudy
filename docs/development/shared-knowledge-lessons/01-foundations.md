# Foundation stages: LK-01 through LK-04

Part of the [PR #103 development record](README.md). All stages in this file are implemented on PR #103; automated qualification and human/platform acceptance remain separately recorded in the PR evidence ledger.

## LK-01 — Full-size Practice-faithful editor preview

**Implementation status: complete.** The full native/shared implementation is published on PR #103. Automated source qualification and human-only visual/device acceptance remain distinct evidence; completing this stage does not claim an unperformed physical-device review.

Outcome: Preview temporarily replaces the editor's content workspace with the complete study presentation at normal Practice proportions. A prominent, persistently available **Close preview** button appears at the top. It is not a compressed sidebar, a new browser window, or a screenshot of the card.

Owners: `CardEditorScreenBase.tsx`, `card-editor/QuestionDraftPreview.tsx`, `card-editor/presentation.ts`, the existing card-editor controller, `practice/QuestionBody.tsx`, shared recall/rich-content rendering, native question compilation and occlusion preview.

- [x] **01.1 Characterize and separate effects.** Capture Basic forward/reverse, Cloze, template-generated presentations, every objective type, and occlusion. Identify the actual content/frame used by ActivePractice. Extract a small shared presentation surface only where necessary; leave session creation, schedule reads/writes, analytics and Tutor side effects in their existing controllers.
- [x] **01.2 Project the current draft.** Preview unsaved content through the same native validation/compiler and managed-asset rules as authoring/Practice. Add a focused typed preview command only where no reusable command exists. Return a temporary preview identity and permitted asset handles, not a saved card or session. Do not substitute a second TypeScript grader. Incomplete drafts produce specific field guidance without silently dropping required content.
- [x] **01.3 Replace the sidebar.** Remove the preview split-grid/aside path from the full-preview action. Keep the editor controller and dirty draft alive while rendering the full-width preview workspace. Use the existing page/overlay/Back primitives; do not add a second focus trap or global overlay stack. Close preview, Escape and Android Back return to the prior field, scroll and selection without Save, cancellation or lost assets.
- [x] **01.4 Exercise all presentation states.** Support normal reveal, objective response/Check/feedback using pure native evaluation, and reset. Provide presentation selection for reverse/Cloze/template/occlusion children without creating schedule identities. Use actual Practice typography, media, math, masking, width policy and controls; omit scheduling ratings and session counters. Close preview stays visible even when content scrolls.
- [x] **01.5 Qualify and remove the obsolete path.** Delete exclusively owned sidebar layout/styles/tests once all callers use the new workspace. Keep list thumbnails and editor field mini-examples where they serve a different purpose. Add proposed `CardEditorPreviewWorkspace.test.tsx` and native preview tests colocated with the owning question/compiler module.

Exit: all supported kinds render/read/reveal at compact, medium and wide sizes; keyboard/touch/Back and dirty-state restoration pass. Database mutation spies and native tests show zero notes, cards, sessions, attempts, schedule or review changes caused by preview. Temporary preview media may exist under the established draft lease, but cannot finalize or delete canonical assets. No provider call occurs. Visual parity is checked against Practice using the same fixture, not by approving two unrelated snapshots.

## LK-02 — Current-format transition preparation

**Implementation status: complete.** Read-only entry admission, current-library preservation fixtures, restore coverage, and the LK-13 consolidation checkpoint are implemented/documented on PR #103. This does **not** claim that a personal library was inspected or converted; actual-library adoption remains an LK-13 human gate.

Outcome: future stages can change the current representation without carrying every historical variant into every runtime feature. The final consolidation is specified and testable before more data is introduced; old migration code is not deleted yet.

Owners: `database.rs`, `database/`, `data_repairs.rs`, `startup.rs`, current backup/restore owners, schema checks and format readers. Detailed retirement procedure: [data transition](03-data-and-validation.md).

- [x] **02.1 Record actual persisted contracts.** Identify current schema/lineage/checksums, rich/question/source/scope payload versions, backup envelopes, sync capability and active draft/attempt readers. Start from the inspected version-17 base but verify the branch at execution. Separate a historical representation from a currently useful external import format.
- [x] **02.2 Establish preservation fixtures.** Build a permitted synthetic current-library fixture containing every card kind, edited guides, active/completed attempts, schedules/reviews, source assets/citations, Tutor history and suppressed/deleted material. Record stable IDs, canonical row/content digests, foreign keys and asset hashes. Do not commit a user's live database or infer their installed schema from repository source.
- [x] **02.3 Isolate conversion at entry.** Define a current-only result for startup/import/restore. Normalize supported existing payloads once before ordinary reads; new feature code handles only the current representation. Keep structured external containers distinct. Unknown/future/corrupt data fails unchanged rather than receiving guessed defaults. Do not add a universal migration framework or placeholder version adapters.
- [x] **02.4 Define the final checkpoint.** Document one final baseline after the last schema-bearing stage, explicit known-source converters, a versioned archive outside normal build/runtime, and the human library-adoption receipt. Coordinate any necessary repository-policy clarification as a scoped pre-release decision; retain historical checksums and data-preservation assertions.

Exit: fixture preservation and conversion-boundary regressions run; current runtime contract is explicit. Real-library migration/retirement is still pending LK-13. There is no statement that the personal library was inspected or upgraded merely because a synthetic test passed.

## LK-03 — Reusable evidence, knowledge and context foundation

**Implementation status: complete.** The current native evidence, interpretation, preparation, context and preservation boundaries are implemented on PR #103. Exact-SHA static/fast evidence is recorded in the PR ledger; full Linux, Android and source-reviewed/live-provider acceptance remain separate gates. The completed scope does not include the LK-04 screenshot collection workflow, LK-05 collections or LK-10 lesson player.

Outcome: all consumers share compatible document understanding, exact support and resource accounting. This is an extension of `knowledge/` and canonical documents, not a new ingestion stack.

Owners: active `document_canonical*` and `document_store` modules; `knowledge/contracts.rs`, evidence, extraction, repository, document-view, indexing and authoring owners; native task coordination; `src/ipc/knowledge.ts`.

- [x] **03.1 Characterize current reuse.** Add provider-request counters and fixtures proving which existing paths already reuse evidence/units. Trace initial cards, more cards, guides and Tutor. Preserve successful per-candidate admission, source-revision checks, native quotations and partial receipts from merged work. Do not reimplement them merely to match new type names.
- [x] **03.2 Add loss-aware evidence structure.** Retain stable block identities, immutable payload references and source occurrences. Preserve table headers/cells/footnotes, figure/caption links, lists, code indentation and formula context where extractors actually provide them. Keep normalized search text separate from exact cited bytes. Backfill retained canonical data locally when possible; unavailable structure remains unresolved, not AI-invented.
- [x] **03.3 Extend qualified units narrowly.** Add typed comparison/process/quantity/classification payloads only for actual consumers. Keep current kinds and source-specific support. Store each semantic value in one authority, with deterministic text projections. Model output selects local evidence aliases; native code assigns IDs, offsets and hashes. Different documents retain distinct attribution even when their statements coincide.
- [x] **03.4 Make completeness and dependencies explicit.** Track interpreted, metadata, no-learning, unreadable and unresolved evidence regions; retain distinct duplicate occurrences without merging source ownership. Native screenshot-equivalence/filter producers are the explicit LK-04 dependency, not declarations the interpretation model can fabricate. Missing output is unresolved. Include empty dispositions and source manifest changes in publication fingerprints. Preserve valid siblings and repair only rejected/uncovered regions. Rechecking current compatible receipts must be read-only.
- [x] **03.5 Add one context service and shared preparation.** Introduce `LearningContextRequest` with authorized scope, native focus/candidate selection, coverage/retrieval policy and budgets; consumer entry points define purpose. Add a native compatible-job registry keyed by exact preparation contract/scope, with subscriber-aware cancellation and durable receipts. Current guide/card/Tutor consumers use projections of this service; LK-10 will use the same boundary when lessons exist. No background/startup/UI navigation causes new cloud interpretation; explicit preparation and configured upload work carry permission.
- [x] **03.6 Qualify unchanged work.** Add native tests for concurrent consumers, source mutation during generation, Unicode selectors, empty-result invalidation, model changes, zero reanalysis on reused sources, storage ownership and backup. Implemented owners: `knowledge/extraction/reuse_tests.rs`, `knowledge/shared_preparation/tests.rs`, `knowledge/coverage/tests.rs`, `knowledge/learning_context/tests.rs`, `knowledge/snapshot/evidence_tests.rs`, `database/knowledge_entry_tests.rs`, and `library_backup/current_format_tests/knowledge.rs`. The maintained native-contract wrapper rejects empty test selections.

Exit: prepared-source reuse, qualified evidence and partial-result preservation pass before collections depend on them. Shared storage never grants extra source permission. Performance reports distinguish provider time from native work and do not trade lost qualifiers for a faster average.

The evidence transition is migration 18: immutable payloads and document-owned occurrences/links are backfilled from retained canonical data without provider calls. Qualified comparison/process/quantity/classification details use native spans into the one authoritative statement; absent details remain unavailable rather than inferred. Coverage receipts bind exact region plans, retain accepted siblings and keep omitted images unresolved. Completed reuse is read-only. Startup and complete-backup admission reject unsupported typed knowledge or forged region receipts before conversion writes.

`LearningContext` rechecks permission before hydration, carries whole evidence dependencies, distinguishes bounded retrieval from coverage admission, and surfaces preparation limitations without launching interpretation. Tutor uses admitted image sources, drops dependent claims on eviction and checks its context stamp in the turn transaction. Whole-document publication snapshots include structural payload and occurrence-link changes, including changes invisible to flattened text. The complete-library fixture verifies typed rows, managed bytes and native context decoding through reopen, backup, rollback and committed restore. No personal library was used or converted.

## LK-04 — Ordered image sets and screenshot cleaning

Outcome: upload screenshots/scans as one image collection and automatically create flashcards and a study guide. The learner selects files; the app owns OCR, content relevance and overlap handling. Upload order is not an educational preparation requirement.

Owners: `external_file_images.rs`, `platform/imageInput.ts`, `ipc/imageCollectionFiles.ts`, `documents/image_sets/`, `document_ingestion/image_set.rs`, shared knowledge/authoring, `useImageCollectionJobs.ts` and the application-lifetime document controller.

- [x] **04.1 Admit image collections.** Upload images opens the collection page. Support multi-file selection, a + action for more files, a drop area below the list and clipboard image paste. Validate real bytes, signatures, decoded pixels and aggregate limits natively. Retain independent originals, stable member IDs and a manifest. Failed files remain retryable/removable without publishing a misleading complete collection. Never fetch a pasted or guessed image URL.
- [x] **04.2 Generate automatically through shared learning services.** Reuse compatible local OCR, then invoke the existing document knowledge/relevance, flashcard and study-guide pipeline without another user action. The collection stays alive across navigation; queued additions extend its existing source and generated deck. Preserve successful work and retry only the unfinished stage. No new image-only provider or authoring stack is introduced.
- [x] **04.3 Resolve overlap independently of upload order.** Check bounded screenshot pairs in both directions before projection, including nonadjacent duplicates and a connecting screenshot arriving last. Require exact text/pixel/geometry proof and recheck each representative directly. Protected digits, signs, units, operators, superscripts and negation remain literal; ambiguity retains candidates rather than choosing a majority reading. Keep every proved source occurrence.
- [x] **04.4 Own relevance in the backend.** Apply conservative source-role rules and shared knowledge/curation without requiring the learner to label OCR regions or edit crop coordinates. Remove the manual Content-filter/preparation workspace. Preserve educational qualifiers and attribution; unreadable text is not invented. Native persisted inclusion/filter data remains compatible rather than being erased when its old UI is removed.
- [x] **04.5 Preserve source and lifecycle boundaries.** Retain revision-safe append/amendment, compatible extraction reuse, original inspection, complete backup and deletion semantics. Original inspection is optional through View collection. Stop prevents the next automatic stage, and failed reconciliation is separate from a committed write. Full Linux/Android and real-source visual/quality review remain separately recorded acceptance gates.

Exit: multi-select, additional chooser, drop, paste, automatic card/guide calls, navigation survival, queued additions and retry identities pass integration tests. Native overlap tests cover all six permutations of three overlapping screenshots, changed-number/pixel refusal, nonadjacent duplicates, original ownership and cached reprocessing alongside existing source/filter/backup fixtures. Human review measures retained qualified claims and boilerplate leakage rather than merely OCR text length.

Implementation note: [Image collections](04-image-sources.md) defines the revised product contract that supersedes the original manual preparation UI. Deterministic tests do not imply live-provider, physical-device or visual acceptance. This single-source correction does not implement the later multi-document folder/bundle stages.


## Successor stages

LK-05 and later implementation has moved out of PR #103:

- [PR #104](https://github.com/jferments/ostudy/pull/104) owns LK-05 through LK-09.
- [PR #105](https://github.com/jferments/ostudy/pull/105) owns LK-10 through LK-13.
- [PR #106](https://github.com/jferments/ostudy/pull/106) owns LK-14 plus residual evidence-backed cleanup/qualification.

Those PRs carry their own roadmaps and acceptance ledgers. This file remains the implementation record for LK-01 through LK-04 only.
