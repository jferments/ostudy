# Shared knowledge, collections, and lessons: development plan

Status: implementation underway; LK-00 through LK-02 are implemented on PR #103. Prepared 18 September 2026 and updated as stages close.

This is the execution contract for PR #103. It turns the approved Shared Knowledge and Responsive Lessons revision-2 design into ordered, reviewable implementation increments and records each completed boundary as it is published. The human has authorized staged implementation through the requested scope; that authorization does not grant access to or conversion of a personal library, authorize merge, or collapse later human/platform acceptance into source qualification.

## Read this plan

| Document | Responsibility |
| --- | --- |
| This page | Scope, baseline, dependency order, ownership, contracts, and cross-feature data flow |
| [Foundation stages](01-foundations.md) | LK-01 through LK-07: preview, data-transition preparation, reusable knowledge, images, collections, synthesis, automatic bundles |
| [Learning-experience stages](02-learning-experience.md) | LK-08 through LK-14: guides/PDF, Tutor projects, responsive lessons, retained speech, PowerPoint, consolidation, qualification |
| [Data transition and acceptance](03-data-and-validation.md) | Current-format-only policy, library preservation, acceptance matrix, test scenarios, validation and handoff |

The revision-2 design remains the product rationale. This plan is self-contained for implementation: no task requires locating a PDF in another conversation. The two newer requirements are a full-size Practice-faithful editor preview and a pre-release current-format consolidation. They supersede older instructions to retain every historical runtime representation indefinitely. They do not authorize data loss or changing existing migration bytes in place.

## 1. Verified starting point

Repository: `jferments/ostudy`. Inspected base: `main` at `30ae169fbe1b4c70add0ed0f6db2b847060c70a0`, tree `160092cb91cd2b9241838f82b178e8b96301a1b8`. PR #101 is already merged into this ancestry. Do not restart its interactive-question implementation or treat its historical delivery stages as unfinished prerequisites.

Focused source reads establish the following construction seams, not a new claim of runtime qualification:

| Existing boundary | Observation and consequence |
| --- | --- |
| `src/screens/CardEditorScreenBase.tsx`, `card-editor/QuestionDraftPreview.tsx`, `card-editor/presentation.ts` | Objective preview already uses `practice/QuestionBody`, but inside an `aside` and a split-grid preview layout. Replace the container and unify presentation; do not write another board renderer. |
| `src-tauri/src/database.rs` | Current lineage is `ostudy-public-v1`; registered migrations run through 17. That name is not evidence of a public release. The actual user's installed schema remains unobserved. |
| `src-tauri/src/knowledge/contracts.rs` and `docs/architecture/knowledge-representation.md` | Existing source identities, qualified units, native evidence references, partial receipts, and document views are the foundation. Extend them rather than add a competing knowledge database. |
| `src/ipc/knowledge.ts` | Selection supports deck/document/guide members and a 16-member frontend bound. Folder/range support needs a native expanded-scope contract, not removal of one UI limit. |
| `src/ipc/studyGuides.ts`, `src-tauri/src/document_study_guide_export.rs` | Guides, jobs, overlays, and exports are document-oriented. Introduce an explicit study target and migrate relationships; never fabricate a document for a folder. |
| `src/ipc/tutor.ts` | Conversations already have stable identity, revision, scope, and durable turn operations. Projects and organizational mutations extend that owner. |
| `.github/workflows/check.yml` | Draft PRs select fast qualification on the self-hosted runner; static runs first. Full Linux and Android are separate gates. No workflow changes are needed to publish this plan. |

Some maintained prose still reflects older component states. Resolve a disagreement through exact-head owning source and tests; do not reintroduce an old OCR/provider backend or remove a working question type to match historical wording. Each stage source-checks only its immediate owners and callers.

## 2. Scope and exclusions

Deliver reusable document knowledge; ordered image-set ingestion; overlap deduplication and reversible screenshot cleaning; folders with reusable document ranges; thin collection synthesis; scoped Tutor retrieval; automatic guides, flashcards, and eventually text-first lessons; improved guide content and local PDF; Tutor projects and chat menus; responsive lessons with pop quizzes and transcript; retained narration; and local `.pptx` export. Preserve existing source formats, Basic/Cloze, all merged interactive types, rich math/media, schedules, analytics, backup, and ordinary editing.

The narration target from the approved design is OpenRouter `hexgrad/kokoro-82m`, voice `af_river`, displayed as River. Verify the live route and supported controls at LK-11; this plan does not assert a successful live synthesis or fixed price. No silent voice/model/provider fallback is permitted.

Explicitly deferred: speech-to-text and lecture transcription, articles/blog/magazine output, Tutor deck-writing tools, voice cloning, speech grading, automatic image occlusion, video export, and a general slide editor. Already-written lecture notes/transcripts remain ordinary text sources. No STT is hidden inside audio validation or alignment. Deferred Tutor authoring remains one approved destination, at most one new deck, a presentation-count ceiling, no destructive model tools, and retry-safe publication; no active stage exposes those tools.

Do not introduce a graph server, universal ontology, external task orchestrator, new scheduler, cloud document converter, bundled local TTS runtime, or a second source publisher. Linux and Android remain the principal qualification targets; other platform/office compatibility claims require actual evidence.

## 3. Delivery order

Use this one branch and draft PR for the planned continuation unless the human explicitly changes the delivery arrangement. Publish small coherent increments on it, not one giant final implementation. The order below is the default for a single source writer; independent work does not imply parallel writers.

| Stage | Deliverable | Dependency | Revision-2 mapping |
| --- | --- | --- | --- |
| LK-00 | Publish this plan, acceptance contract, and draft PR | Current request | Planning only; not all of old S0 |
| LK-01 | Full-size, Practice-faithful editor preview | LK-00 | New preview requirement |
| LK-02 | Current-format conversion policy and preservation fixtures | LK-00 | S0 plus new pre-release requirement |
| LK-03 | Shared evidence, knowledge, context, and reuse contracts | LK-02 | S1 |
| LK-04 | Ordered image sets, overlap, reversible content filters | LK-03 | S2 |
| LK-05 | Folders, reading ranges, source-set snapshots, Tutor selection | LK-03 | S3, organization/scope |
| LK-06 | Thin collection synthesis and coverage-aware context | LK-05 | S3, synthesis |
| LK-07 | Automatic coherent study bundles and status UI | LK-04, LK-06 | S4 |
| LK-08 | Rich guide composition, viewer, professional PDF | LK-03; LK-07 for collection end-to-end | S5 |
| LK-09 | Tutor projects and chat organization | LK-05 | S6 |
| LK-10 | Text-first lessons, responsive player, checks, lesson plans | LK-01, LK-06, LK-07; shared blocks from LK-08 | S7 |
| LK-11 | Speech settings, retained narration, playback lifecycle | LK-10 | S8 |
| LK-12 | Local editable PowerPoint export with notes/audio | LK-10; LK-11 for audio | S9, export |
| LK-13 | Final current-schema baseline, library conversion, archive old code | All schema-bearing stages | S9, data integration |
| LK-14 | Cross-feature qualification and human handoff | LK-13 | S9, final gates |

Current implementation status: **LK-00 through LK-02 complete; LK-03 is the next unfinished stage.** Stage completion records implementation separately from later human/platform acceptance.

LK-04 and LK-05 can be built in either order after LK-03. PDF capability work can begin against a single-document fixture before collections finish. PPTX layout can begin once lesson composition is stable; its audio gate waits for retained audio. An outstanding human library-conversion check prevents retirement of its needed old converter, not unrelated implementation or publication.

The first product increment is LK-01, not another repository inventory. LK-02 identifies the preservation contract early. Perform one final baseline consolidation at LK-13 after the last schema-bearing feature, avoiding repeated baseline squashes while the data model is changing. During development, small forward transitions normalize libraries before ordinary runtime use; there is never a requirement for every feature to operate directly on multiple historical schemas.

## 4. Ownership and dependency direction

```text
Screen / feature controller
  -> typed src/ipc command
  -> Rust use-case service
  -> current-schema repositories + managed files + platform ports
  -> selected native AI gateway when authorized

Retained source
  -> canonical evidence + occurrence map
  -> document knowledge + completeness receipts
  -> source-set snapshot + optional collection synthesis
  -> LearningContext / coverage plan
  -> cards | guide | lesson scenes + script + questions
  -> Practice | reader | responsive player
  -> local PDF | local PPTX | retained TTS rendition
```

Arrows describe dependencies, not one transaction spanning the whole pipeline. Keep provider calls, pixel/audio decoding, and layout outside database locks. Publication is a short revision-checked transaction after assets have entered the existing staged-file recovery protocol.

### Existing owners to change

Paths name verified existing seams or documented active owners at the base. A directory-wide entry does not authorize rewriting every file beneath it.

| Area | Existing owner | Change |
| --- | --- | --- |
| Author preview | `src/screens/CardEditorScreenBase.tsx`, `src/screens/card-editor/`, `src/controllers/cardEditorController.ts` | Full-workspace preview state, draft projection, shared rendering, focus/Back restoration |
| Practice content | `src/screens/practice/QuestionBody.tsx`, `src/screens/ActivePractice.tsx`, `src/editor/` | Extract only shared content/frame seams; keep scheduling/session effects outside preview and lesson checks |
| Native questions | `src-tauri/src/cards/questions/`, `src-tauri/src/practice/question_attempts/` | Reuse validation/compiler/evaluators for temporary previews and lesson checks without review writes |
| Evidence ingestion | `src-tauri/src/document_ingestion/`, `pdf_document_ingestion/`, `xberg_document_adapter/`, active `document_canonical*`/`document_store` owners | Stable blocks, table/figure context, image-set adapter, occurrence/filter metadata |
| Shared knowledge | `src-tauri/src/knowledge/` | Typed extensions, coverage receipts, dependency-indexed invalidation, scoped shared context |
| Generation | `src-tauri/src/ai/document_intelligence/`, active `document_generation`, `document_initial_generation`, `document_more`, `document_checkpoints` owners | Shared preparation subscription, coverage policy, target-owned artifact jobs |
| Guides | `src/ipc/studyGuides.ts`, `src/screens/StudyGuideDialog.tsx`, native `document_study_guide*` owners | Explicit target identity, semantic blocks, effective overlays, PDF projection |
| Tutor | `src/ipc/tutor.ts`, `src/ipc/knowledge.ts`, existing Tutor native/UI owners | Folder/range sources, exact turn snapshots, projects and organizational commands |
| AI profiles and transport | Existing `src-tauri/src/ai/` gateway/profile owners and AI/LLM settings | Separate speech capability, binary transport, model/voice settings and receipts |
| Media and transfer | Existing managed assets, `src-tauri/src/external_files/`, `library_backup/`, `device_sync/`, `src/platform/` | Retained audio ownership, bounded export staging, explicit transfer capabilities |
| Schema/startup | `src-tauri/src/database.rs`, `database/`, `data_repairs.rs`, `startup.rs`, `src-tauri/migrations/` | Normalize at one entry boundary, current-only runtime, final consolidated baseline |
| Navigation and task lifetime | `src/controllers/useAppController.ts`, existing navigation and task controllers | Lesson library/player route and progress without route-local job ownership |

### Proposed focused owners

Introduce these only as their working slice is implemented: native `collections/`, `knowledge/learning_context.rs`, `knowledge/collection_view.rs`, `document_ingestion/image_set.rs`, `study_materials/`, `lessons/`, and `ai/speech/`; typed `src/ipc/collections.ts`, `lessons.ts`, `speech.ts`; feature-local collection/lesson controllers; and `src/platform/presentationExport/` for the qualified local PPTX adapter. Names may be reconciled once with an existing equivalent owner. Record that mapping; do not build parallel facades because an earlier proposed filename differs.

Shared `src/ui/` remains free of IPC/provider/domain mutations. Keep shared semantic content rendering separate from the pure UI primitive layer. A third-party renderer/exporter is replaceable because its adapter consumes oStudy-owned data, not because every feature uses a generic plugin framework.

## 5. Contracts to stabilize before consumers

| Contract | Contains | Must not contain |
| --- | --- | --- |
| Evidence block/occurrence | Stable source/revision, typed payload reference, exact selectors, region transform, extraction provenance | Model-selected authoritative offsets or invented source text |
| Document knowledge | Qualified interpretation, native support links, payload/interpretation version, issues | Folder-specific importance written back into document facts |
| SourceSelection | User-selected folders/materials/ranges/exclusions | Implicit permission to everything reachable from a link |
| SourceSetSnapshot | Canonically ordered resolved membership, revisions, range and authorization identity | Display names as evidence invalidators |
| CollectionSynthesis | Compact topics, unit references, cross-source relations, emphasis/support distinction, gaps | Copies of every member's text/units/vectors |
| LearningContext | Permitted units, exact evidence, visual references, scope/depth, coverage and omissions | Unfiltered whole-folder summary or hidden quiz answers |
| StudyTarget/Bundle | Explicit document or collection target, output policy, ordinary deck ID, revision/suppression state | Fake document IDs; a new deck for each retry |
| LessonRevision | Scene/beat IDs, coordinated visuals and scripts, evidence and question links | Screen dimensions or arbitrary executable layout code |
| NarrationRendition | Script/configuration identity, segment manifest, verified asset hashes, receipts and duration | Expiring provider URLs as the only saved audio |
| Effective export snapshot | Frozen content, permitted notes/answers/media, layout policy and operation identity | Provider calls or disclosure choices inferred by a serializer |

Use indexed relational rows for ownership, dependencies, ranges, operations and lifecycle; bounded versioned payloads for heterogeneous content. Reuse current rich-content and question definitions. Never add a second authoritative string for a typed number, equation, or answer relationship. Old payloads are converted at the explicit transition boundary; normal consumers receive the current contract.

## 6. Work reuse and invalidation rules

| Event | Required new work | Work expressly forbidden as an automatic side effect |
| --- | --- | --- |
| Add prepared document to another folder | Membership, affected synthesis, requested new composition | Re-extracting or rewriting document knowledge |
| Change consumer model | New requested composition, if any | Invalidating compatible knowledge |
| Change embedding model | Selected derived index | Reinterpreting sources |
| Add/reorder screenshots | Novel extraction or changed adjacency dependencies | Rerunning every unchanged image |
| Restore a filtered region | That region and affected dependents | Full-library analysis |
| Update a study target | Successor guide/lesson and novel cards in the existing deck | Overwriting edited cards or resetting schedules |
| Resize/rotate/open Tutor pane | Local layout | LLM/TTS requests |
| Change voice | Desired rendition; missing speech on authorized Listen or explicit batch | Rewriting the lesson or immediately billing for the whole library |
| Replay/export saved output | Local reads/serialization | Repeat TTS or extraction |

A hash enables lookup, not authorization. Shared bytes keep distinct ownership and occurrence links. Similarity identifies candidates, not equivalent meaning. Preserve differences in numbers, units, polarity, conditions and operators. A complete source disposition is not a certificate of exhaustive understanding.

Compatible callers join existing preparation; a narrow caller cannot widen consent by joining broader work. Canceling one subscriber does not destroy work another subscriber needs. Keep successful siblings on partial failure. Snapshots include added/deleted sources and empty/unresolved dispositions, not only units returned by the model.

## 7. Publication and stage completion

Follow [GitHub-direct development](../../agents/GITHUB-DIRECT-DEVELOPMENT.md) and [recovery](../../agents/RECOVERY-PROTOCOL.md). One writer, expected-parent Git-data commits, independent blob acknowledgement checks, off-ref diff inspection, non-force publication, verified remote head, and exact-SHA qualification remain the normal loop. No merge without explicit instruction.

For each stage, implement its smallest end-to-end behavior, focused regressions, native persistence/privacy/backup support, and relevant documentation before enabling it. Do not postpone data integrity until LK-13. Record implemented, published, static, fast, full Linux, Android package, emulator, live provider, physical device, visual/manual, review, and merge separately.

The stage lists are a backlog, not execution evidence. Update checkboxes only against published implementation and identify the tested SHA in the PR acceptance snapshot. A stage may be implemented while its human-only gate remains pending; that does not erase the gate. Final expensive qualification begins after the source/documentation audit and candidate freeze.

## 8. Baseline evidence and design provenance

Pinned source references: [editor](https://github.com/jferments/ostudy/blob/30ae169fbe1b4c70add0ed0f6db2b847060c70a0/src/screens/CardEditorScreenBase.tsx), [question preview](https://github.com/jferments/ostudy/blob/30ae169fbe1b4c70add0ed0f6db2b847060c70a0/src/screens/card-editor/QuestionDraftPreview.tsx), [database](https://github.com/jferments/ostudy/blob/30ae169fbe1b4c70add0ed0f6db2b847060c70a0/src-tauri/src/database.rs), [knowledge](https://github.com/jferments/ostudy/blob/30ae169fbe1b4c70add0ed0f6db2b847060c70a0/docs/architecture/knowledge-representation.md), [ingestion owners](https://github.com/jferments/ostudy/blob/30ae169fbe1b4c70add0ed0f6db2b847060c70a0/docs/architecture/ai-and-document-processing.md), [guide export](https://github.com/jferments/ostudy/blob/30ae169fbe1b4c70add0ed0f6db2b847060c70a0/src-tauri/src/document_study_guide_export.rs), and [qualification workflow](https://github.com/jferments/ostudy/blob/30ae169fbe1b4c70add0ed0f6db2b847060c70a0/.github/workflows/check.yml).

Design input: conversation attachment `oStudy_Shared_Knowledge_and_Responsive_Lessons_v2.md`, revision 2, dated 17 September 2026; its earlier collections proposal is superseded where different. This implementation plan adds the human's 18 September preview and pre-release consolidation instructions. OCrawl contributes no product architecture or dependencies to this plan.

Input Markdown SHA-256: `a77684c915ea5e0e33ca64a9cdfd108e9b26905eac22910b6a8e032f2537494d`. This fingerprint identifies the supplied design, not an implemented repository tree.
