# Responsive lessons, retained narration and local presentation export

## Status and authority

Planning-only successor to oStudy PR #103. Intended delivery slot: **PR #105**. GitHub has not allocated that number to this plan. Proposed implementation branch: `agent/lessons-narration`. Prepared 19 September 2026.

This document authorizes no product implementation, provider spending, personal-library access, migration retirement, merge, or change to PR #103. Its publication consists only of this new Markdown file. The planning branch starts from verified `main` commit `30ae169fbe1b4c70add0ed0f6db2b847060c70a0`, not the actively changing LK-04 branch. It is not a build containing LK-04.

Product requirements are carried forward from the [PR #103 master plan](https://github.com/jferments/ostudy/blob/a29070fa3bf031a4dc172d945d72bbccf22e3171/docs/development/shared-knowledge-lessons/README.md), [learning-experience stages](https://github.com/jferments/ostudy/blob/a29070fa3bf031a4dc172d945d72bbccf22e3171/docs/development/shared-knowledge-lessons/02-learning-experience.md), and [preservation/acceptance contract](https://github.com/jferments/ostudy/blob/a29070fa3bf031a4dc172d945d72bbccf22e3171/docs/development/shared-knowledge-lessons/03-data-and-validation.md). These immutable references are design provenance, not a claim that LK-04 was complete at that SHA. Existing owner paths below come from those inspected repository contracts; proposed files and tests are explicitly proposals and must be reconciled against the eventual merged baseline.

## 1. Delivery split and prerequisites

The intended sequence is #103 (LK-01–04), #104 (LK-05–09), #105 (lessons/narration/export), then #106 (final consolidation and qualification). Implementation is sequential: finish, test, review and merge the predecessor before activating the next product PR. Planning may proceed independently.

There is a numbering/scope correction to preserve: the original **LK-13 is schema consolidation and actual-library adoption**, not a lesson feature. Split its responsibilities explicitly rather than dropping or duplicating them:

| Original item | Owner after the split |
| --- | --- |
| LK-10 / original AC-11 | This plan: semantic lessons, responsive player, transcript, safe checks/Tutor, progress and lesson plans |
| LK-11 / original AC-12 | This plan: speech settings, retained narration and playback |
| LK-12 / original AC-13 | This plan: local editable PowerPoint export |
| LK-13-P / part of original AC-14 | This plan: feature-owned forward migrations, current readers, backup/transfer integration, preservation fixtures and final-contract inventory |
| LK-13-C / remainder of original AC-14 | Final cleanup plan: curated baseline, verified conversion/adoption and historical-reader retirement |
| LK-14 / original AC-15 | Final cleanup plan: cross-feature audit, whole-product qualification and handoff |

LK-13-P is a traceability subdivision, not a claim that original LK-13 is complete. Final cleanup owns closure of original AC-14. Every product PR must already preserve data; none may defer basic integrity to cleanup.

### Required predecessor handoff

Before coding, record the merged #103/#104 commits and qualify these contracts with bounded reads of their owners: canonical evidence and reversible image filtering; permission-bound `SourceSetSnapshot`; coverage-aware `LearningContext`; collection synthesis; retry-safe `StudyTarget`/bundle jobs; semantic guide blocks/effective edits; reusable native question evaluation; Tutor projects and exact historical turn scopes. Consume those contracts rather than copy their implementations.

Integrate current `main` into this planning branch by a normal reviewed non-force merge, preserving its plan commit. Only then retarget a dormant planning PR to `main`, reconcile paths and tests, update the documentation index and explicitly activate implementation. Re-read workflow triggers before activation. Do not stack implementation on an unfinished #103 or treat a squash-merged predecessor's old branch as the current baseline.

## 2. Product result and exclusions

A learner can create a source-grounded lesson, immediately read it offline, navigate readable scenes on desktop or phone, inspect the transcript, answer safe checks, and ask scoped Tutor questions. Optional authorized speech produces retained audio once. The same lesson exports locally to editable `.pptx` with correctly partitioned notes and optional existing narration.

Text-first means text access never requires speech credentials or a speech request. It does not claim that creating new AI material works without a configured local/remote AI provider. Local-first means already-saved lessons, transcript, progress, playback and export remain useful offline.

Excluded: STT, transcription, voice cloning, speech grading, video export, article generation, a general slide editor, automatic image-occlusion authoring, Tutor deck-writing tools, a new scheduler/grader, hosted conversion, bundled TTS runtime, and unrelated navigation or infrastructure redesign. No automatic backfill of every historical document. No provider/model/voice substitution without explicit user choice.

## 3. Architecture and ownership

Dependency direction stays `screen/controller -> typed IPC -> Rust service -> repositories/managed files/platform ports`. The native AI gateway alone contacts the selected provider. React does not own SQL, credentials, grading, or durable publication. Shared `src/ui/` remains free of IPC.

| Boundary | Existing owners to extend | Proposed focused additions |
| --- | --- | --- |
| Lesson contracts/persistence | `src-tauri/src/knowledge/`, `database.rs`, `database/`, `src-tauri/migrations/` | `src-tauri/src/lessons/{contracts,repository,publication,progress}.rs` |
| Authoring and preparation | Native AI gateway, document-generation/task and future study-material owners | `lessons/generation/`, coverage-plan and candidate-admission tests |
| UI and navigation | `src/controllers/useAppController.ts`, feature controllers, semantic content/editor owners | `src/ipc/lessons.ts`, `src/screens/lessons/`, a feature-local library/player controller |
| Checks and Tutor | `cards/questions/`, `practice/question_attempts/`, `src/screens/practice/QuestionBody.tsx`, Tutor context owners | Lesson attempt adapter and answer-safe public projection, not another evaluator |
| Speech | Existing AI profile/settings/credential and task owners, managed assets | `src-tauri/src/ai/speech/`, `src/ipc/speech.ts`, rendition/segment repositories |
| Playback/export | Native platform ports, `src/platform/`, `external_files/` | `src/platform/presentationExport/`, lesson prepared-export snapshot/compiler |
| Preservation | `library_backup/`, `device_sync/`, current-format entry tests | Lesson/audio preservation and explicit transfer capability fixtures |

Names identify ownership seams, not permission to rewrite whole directories. Reuse an equivalent existing owner discovered at activation and record the mapping once. Allocate actual migration numbers only after reading the merged registry.

### Application-owned contracts

`LessonRevision` contains immutable source/coverage identity, objectives, scenes, beats, script segments, semantic visual blocks, support bindings and question references. IDs are independent of screen dimensions. A publication has bounded payloads, an operation identity and expected source/target revision; late output cannot overwrite newer material.

`LessonPlan` owns ordered entries and optional acyclic prerequisites. `LessonProgress` owns the acknowledged revision, scene/beat, responses and later audio segment/offset. Completion is not mastery. Existing question IDs and keys are not silently reinterpreted after a revision change.

`LessonCheck` reuses native definitions, compilers and evaluation but has its own attempt context. It never writes an FSRS review merely because a check was answered. An explicit Practice launch resolves ordinary card identities and follows existing review semantics.

`NarrationRendition` binds script/configuration identity to a segment manifest and verified retained asset hashes. Separate producer receipts from local operation IDs and returned-byte hashes. Unknown provider revision stays explicitly unknown.

`LessonExportSnapshot` is a frozen, permission-filtered content/media graph with notes/answer/audio choices and an operation identity. A serializer receives no credentials, unrestricted filesystem paths, private Tutor history, or permission to call providers.

## 4. Implementation roadmap

### 105-A — Contracts and a minimal persisted text lesson (LK-10.1)

Implement the native lesson/revision repository, stable scene/beat IDs, support links, plan ordering/prerequisite validation and bounded typed IPC. Use additive forward transitions and the existing entry-validation discipline. Start with a deterministic fixture that can be published, listed, opened and reopened before introducing provider generation.

Publication is a short transaction after validation and managed-asset staging. Provider requests and media decoding stay outside database locks. Add backup ownership immediately. A failed post-commit refresh is reconciliation failure, not permission to repeat the write.

Exit: text-only lesson and plan survive restart/backup; duplicate operation IDs do not create duplicate lessons; stale revisions and invalid graphs fail without partial publication.

### 105-B — Coordinated generation and the responsive reader (LK-10.2–10.3)

Plan objectives, prerequisites, depth and coverage from `LearningContext`. Author a beat's visuals, script and check opportunity together, retain factual support and qualifications, and preserve valid siblings when another candidate fails. Roughly 10–30 teaching scenes is a design guide, not a quota: large material becomes a series and small material is not padded.

Reuse semantic blocks from guides. Deliver concept, comparison, process, worked-example, figure and retrieval-check presentations. Layout uses container width, text scale and safe areas, not OS sniffing. Reflow independent blocks or step through existing beats; keep spatial figures atomic with zoom/detail and text alternatives. Do not shrink a fixed slide or generate a second mobile manuscript.

Diagrams use admitted figures or constrained typed data; model-authored executable HTML/CSS/SVG is not accepted. Mathematics, quantities, polarity, exceptions and scientific geometry must retain source meaning. Source-ID validity does not establish semantic truth: structural validation and human content review remain separate gates.

Exit: compact/medium/wide layouts and enlarged text retain all content; transcript is immediately available; resize/rotation/Tutor-pane changes make zero provider calls and preserve position.

### 105-C — Checks, Tutor, progress and bundle integration (LK-10.4–10.6)

Reuse shared question surfaces and native grading through a lesson-specific adapter. Capture attempts against immutable lesson/question revisions. Persist response state and controlled resume; incompatible updates offer an explicit transition instead of silently assigning an old answer to a new key.

Create a reveal policy covering every channel: visible blocks, transcript, captions, alternative text, media URLs, Tutor context, audio preloads and export notes. Before checking, admit only public prompt material. After checking, authorize the solution explicitly. Tests must search excluded answers in serialized DTOs and asset manifests, not only the visible DOM.

Tutor receives current scene/beat and the permitted source snapshot. Assistance and replies do not rewrite the lesson manuscript or historical context. Add the Lessons library/player through existing navigation and Back restoration. Enable the bundle's Lesson link only once text-only functionality passes. Updates preserve learner edits, existing deck schedules and deletion suppression; no surprise historical batch generation.

Exit: all merged interactive kinds remain compatible; answer privacy, retries, in-flight changes, restart and explicit Practice handoff pass focused regressions.

### 105-D — Speech capability, settings and normalization (LK-11.1–11.3)

At implementation time, verify official provider documentation/catalog and perform one deliberately authorized bounded live capability test. The requested target is OpenRouter `hexgrad/kokoro-82m` / `af_river` (River); it is a requested configuration, not a verified live route or fixed price. Record supported encoding, controls, limits, pricing units and actual producer metadata. Unsupported capability is explicit; do not silently replace the model or voice.

Separate binary speech transport from structured/text decoding. Add a narration role, model/voice capability display and cached catalog metadata. Opening settings or saving a voice never synthesizes a sample. An explicit Preview action discloses the paid/provider boundary. No upstream idempotency or exactly-once billing is assumed.

Keep readable and spoken script forms with explicit pronunciation bindings. Segment coherent sentence/beat groups and preserve numbers, units, symbols and qualifications. Lower only documented controls; do not assume SSML. Segment-duration cues suffice; no hidden STT alignment service.

Exit: mocked protocol/capability/error paths pass; live-route and pronunciation evidence remains separately recorded. Missing speech cannot break reading.

### 105-E — Retained audio, cost-safe voice changes and playback (LK-11.4–11.7)

Derive compatible audio identity from spoken text, language, model/voice, known producer version, applied pronunciation/options, normalizer/segmenter version and encoding. Authorization is checked separately from hashing. Join matching authorized jobs and preserve completed siblings on failure/cancellation.

Stream to bounded staging, validate/decode/hash bytes, move into managed storage, then transactionally attach receipt and reference. Test recovery on both sides of every durable boundary, including orphan files and missing references. Unknown remote acceptance becomes an explicit indeterminate operation; do not blindly retry a potentially paid request.

Changing desired voice preserves old audio and does no immediate synthesis. The next authorized Listen or explicit bounded batch requests only missing replacements. Old in-flight work retains its captured profile and cannot activate as the new voice. Activate only a coherent complete rendition; never silently mix voices. Switching back reuses compatible saved clips. Local playback speed changes do not alter synthesis identity.

Implement Read/Listen, play/pause, seek, speed/volume, transcript and acknowledged resume position. Prompt/solution clips have distinct reveal permissions and preload manifests. Define Tutor pause/resume. Qualify Android audio focus, interruption, background policy, Bluetooth and Back through platform ports; do not promise unsupported background behavior.

Paid retained media is not disposable LRU cache. Full backup includes bytes/manifests/receipts. Storage pressure stops optional new speech; deliberate deletion warns about regeneration cost. Transfer offers an explicit media choice without leaking credentials or private histories.

Exit: repeat/reboot/offline playback, voice switch-back and local speed changes make zero TTS calls; targeted edits invalidate only dependent segments; full disk, corruption, cancellation and profile races preserve existing work.

### 105-F — Local editable PowerPoint (LK-12)

Qualify a bounded local serializer behind an application-owned adapter. PptxGenJS is a candidate, not an assumed dependency or guaranteed WebView capability. Verify actual WebViews, memory/worker/binary handling, notes, fonts, shapes/tables, media, licensing and exact version before selection. A demonstrated gap may justify a native/local alternative. No runtime Node, Python, Office installation or cloud converter becomes an end-user prerequisite.

Compile deterministic 16:9 layouts from the same semantic lesson. Use editable text/shapes/tables where practical, admitted figures and qualified math fallbacks. Preserve source/accessible text; do not rasterize whole slides. Stable overflow creates continuation slides, not tiny fonts. Map each slide only to its actual script portions and cues.

Offer current saved narration with an explicit omit choice. Missing audio permits audio-free export or a separate authorized generation action, never hidden TTS. Join/re-encode locally through a qualified codec where needed; arbitrary MP3 concatenation is not a codec. Learner exports omit answer notes/clips; instructor/answer-section export is deliberate. Private Tutor text and learner notes are excluded unless separately selected by policy.

Rust authorizes the bounded snapshot and registers prepared output after ZIP/XML/relationships/size validation. Use native external-file save/share lifecycle, retry identities and crash cleanup. Reject macros, external executable relationships and arbitrary remote resources.

Exit: zero provider calls on offline export/re-export; deterministic continuation/notes/media mapping; structural tests plus separate actual Microsoft PowerPoint/LibreOffice interoperability evidence. Valid XML alone is not visual/audio acceptance.

### 105-G — Feature data integration and handoff (LK-13-P)

Extend current-format admission, complete-backup integrity, staged restore/rollback and managed-file lifecycle across lesson revisions, plans, responses, progress, audio and receipts. Specify each export/sync format separately: content-only transfer excludes donor schedules/private history; sync either negotiates this graph or rejects unsupported versions before partial application.

Deliver a compact final-contract inventory to cleanup: tables/indexes/triggers, payload versions, entry readers, feature migrations, managed-file kinds, transfer capabilities, operation receipts and preservation fixtures. Retain historical migration bytes/checksums and the currently necessary recovery path. Do not perform final baseline consolidation or personal-library adoption here.

Finish source/docs audit and user guides before freezing this PR. Run its own terminal gates; #106 is not permission to merge an unqualified lesson feature. Original AC-14 remains open until cleanup records its final adoption/retirement evidence.

## 5. Acceptance and regression matrix

All product criteria begin **not started**. Publishing this plan satisfies planning only.

| Local criterion / original mapping | Required tests and evidence |
| --- | --- |
| AC-01 / original AC-11 | Lesson contracts: malformed content, source revision drift, duplicate operations, stable IDs, DAG/cycle validation, transaction rollback and restart |
| AC-02 / original AC-11 | Responsive reader: all block kinds, long text/math/tables, enlarged text, narrow/wide/Tutor-open, keyboard/touch/Back and zero network counters |
| AC-03 / original AC-11 | Checks/Tutor: native evaluation parity, no FSRS writes, prompt/solution sentinel leakage across every DTO/media channel, stale-attempt resume and historical-scope isolation |
| AC-04 / original AC-11 | Generation/bundles: bounded candidates, surviving siblings, cancellation, source change at publication, no duplicate target, preservation/suppression and no historical auto-batch |
| AC-05 / original AC-12 | Speech transport/settings: supported/unsupported controls, explicit preview only, bounded errors/binary limits, no credential leakage or silent fallback |
| AC-06 / original AC-12 | Audio retention: request-counted reuse, text/pronunciation invalidation, profile races, ambiguous remote acceptance, storage failure and every crash boundary |
| AC-07 / original AC-12 | Playback: permission-safe preload/seek, reboot/offline, interruption/focus, no regeneration on layout/speed; physical Android gate separate |
| AC-08 / original AC-13 | PPTX: OOXML relationships, notes/continuations, editability, media and answer disclosure, bounded output, zero provider calls; office-app inspection separate |
| AC-09 / LK-13-P | Whole-library typed-value and asset-hash preservation, restore rollback/commit/reopen, incompatible transfer fail-closed and final-contract inventory |
| AC-10 / handoff | Exact-head static/fast/full Linux/Android evidence, maintained docs, explicit unrun human gates and no automatic merge |

Proposed colocated suites: `lessons/*_tests.rs`, `ai/speech/*_tests.rs`, `library_backup/lesson_audio_tests.rs`, `LessonPlayer.test.tsx`, `LessonCheckPrivacy.test.tsx`, `NarrationSettings.test.tsx`, and presentation-export fixtures. Reconcile names with actual owners when implemented. They are not existing executable test claims.

Use a fixed synthetic corpus spanning comparison/process/math/figures, sparse slides plus detailed readings, overlapping screenshots, incomplete evidence and an excluded-answer/source sentinel. Count calls at fake extraction/LLM/TTS gateways. Preserve exact expected values, source relationships and file hashes; equal counts alone do not prove preservation. Live educational faithfulness and pronunciation require source-reviewed samples.

## 6. Execution, qualification and bounded context

Follow current root instructions and the GitHub-direct/recovery runbooks. Exactly one writer on this branch; never write PR #103 from this task. Publish ordinary source via verified blobs, parent-based trees/commits, off-ref diff inspection and non-force ref updates. Preserve useful work on interruption. No custom developer MCP or runner-side source authoring.

Maintain one compact checkpoint: PR/branch; actual head and expected parent/tree; active increment; changed owners; completed acceptance IDs; exact check SHAs/run IDs; first failing boundary; next action; manual blockers. On resumption read that plus the relevant stage, contracts, owning code/tests and exact-head evidence once. Do not reload the entire master plan, all historical PR diffs or every transcript.

Iterate with the smallest relevant tests and maintained changed/static/fast wrappers. Before terminal qualification, reconcile code, docs, compatibility and handoff, then freeze the candidate. Run maintained full Linux, Android compile/package and emulator gates as required by current policy. Report fresh versus explicitly allowed fingerprint-based reuse. Inspect named steps and distinguish compilation from executed tests. Do not modify a green candidate for optional cleanup while expensive gates run.

Planning publication requires no build, provider call or personal data. Product handoff must provide the then-current exact checkout/build/run/diagnostics commands from the maintained runbook, concrete lesson scenarios, compatibility/rollback warnings and honest human-only gates. Do not tell anyone to test this planning branch as an LK-04 application.
