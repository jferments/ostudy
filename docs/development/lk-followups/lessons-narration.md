# PR #105 roadmap — deferred lessons, retained narration, and PowerPoint

Status: **DEFERRED planning reference** after PR #104. Owned future stages: **LK-10 through LK-12**. This branch remains open as the preserved implementation specification for responsive lessons, retained narration/TTS, and local editable PowerPoint export. It is not active implementation work and does not block post-#104 stabilization, release-readiness, installer, or developer-tooling work.

Activation rule: do not implement this PR until the human explicitly resumes it. At that time, retarget/integrate the then-current merged `main`, re-read the production schema/release contracts established after PR #104, reconcile actual owner/test paths once, and continue stage-by-stage. Do not assume the schema, provider catalog, or packaging state recorded when this plan was first written is still current.

## Scope boundary after deferral

PR #105 retains:

- **LK-10:** responsive text-first lessons, transcript, checks/Tutor context, progress, and lesson plans;
- **LK-11:** retained narration, speech settings, cost-safe voice changes, and playback lifecycle;
- **LK-12:** local editable PowerPoint export with speaker notes and optional already-retained narration.

PR #105 no longer owns the current production-baseline/migration consolidation that was previously called LK-13. That work moves to PR #106 so oStudy can establish a safe post-#104 production lineage before it is used for important real study data.

Future lesson/narration/PPTX persistence must extend the production baseline that exists when #105 is resumed using ordinary reviewed forward migrations. Each feature stage owns its own backup/restore, transfer, compatibility, and migration coverage. #105 must not perform another wholesale migration-history consolidation merely because it adds new tables.

Not included: STT/transcription, voice cloning, speech grading, video export, a general slide editor, article generation, automatic image-occlusion authoring, Tutor deck-writing tools, a new scheduler/grader, cloud presentation conversion, or hidden provider spending.

## Product result

A learner can generate a source-grounded lesson from the same shared knowledge used by cards, guides, and Tutor; read a responsive text-first transcript on desktop or mobile; answer safe embedded checks; ask contextual Tutor questions; and resume progress after restart. Optional authorized narration is synthesized once, retained as owned media, replayed offline, and changed cost-safely without rewriting the lesson. The same saved lesson exports locally to an editable `.pptx` with correctly assigned speaker notes and optional already-retained narration.

Dependency direction remains:

```text
SourceSetSnapshot + CollectionSynthesis + LearningContext
  -> LessonRevision / LessonPlan
  -> responsive semantic scene/beat rendering
  -> lesson-local checks + Tutor context + LessonProgress
  -> NarrationRendition / retained managed audio
  -> local LessonExportSnapshot -> editable PPTX

all durable feature contracts
  -> ordinary forward migrations from the production baseline
  -> complete backup/restore and transfer verification for the new data
```

Rust/SQLite owns durable identities, validation, progress, publication, lifecycle, and schema admission. React uses typed `src/ipc/` gateways and existing navigation/platform ports. Provider HTTP and credentials stay in the native AI gateway. Layout changes, playback, resume, and export do not authorize provider calls.

**LessonRevision** contains immutable source/coverage identity, objectives, ordered scenes/beats, semantic visual blocks, script segments, support links, and question references. Scene/beat IDs do not depend on screen size.

**LessonPlan** owns ordered entries and optional acyclic prerequisites. **LessonProgress** owns acknowledged lesson revision, scene/beat position, question-response state, and later narration segment/offset. Completion is not mastery.

**LessonCheck** reuses existing native question definitions/compiler/evaluator through a lesson-specific attempt context. Answering a lesson check does not silently submit an FSRS review. Explicit Practice uses existing card identities and ordinary review semantics.

**NarrationRendition** binds spoken script/configuration identity to segment manifests, verified retained hashes, producer metadata, receipts, and duration. Hash identity never replaces authorization. Expiring provider URLs are not the retained source of truth.

**LessonExportSnapshot** is a frozen, permission-filtered lesson/media graph with explicit notes/answers/audio choices and operation identity. A serializer receives no credentials, unrestricted filesystem paths, private Tutor history, or authority to call providers.

## LK-10 — text-first responsive lessons

### Persistence and coordinated authoring

Add lesson/revision/plan/progress storage with stable IDs, bounded payloads, native validators, **forward migrations from the current production baseline**, and retry-safe publication. Begin with deterministic fixtures that can publish/list/open/reopen a text-only lesson before integrating provider generation.

Plan objectives, prerequisites, depth, and coverage from LearningContext. Author each beat's teaching blocks, script, and check opportunity together so the channels cannot drift silently. Preserve qualifications, conditions, units, polarity, exceptions, and source support. Failed candidates may be repaired independently; valid siblings survive.

Use admitted figures or constrained typed data for diagrams. Do not accept arbitrary model-authored executable HTML/CSS/SVG. Scientific geometry and equations require semantic correctness, not decorative plausibility.

### Responsive player and transcript

Reuse semantic content blocks established by guide work. Support concept, comparison, process, worked-example, figure, and retrieval-check presentations. Layout uses container width, text scale, safe areas, and existing platform ports—not OS sniffing or a second mobile manuscript. Independent blocks reflow; spatial figures remain atomic with zoom/detail/text alternatives.

Transcript is available immediately. Resize, rotation, enlarged text, opening Tutor, or returning from another route preserves scene/beat/question state and makes zero lesson-generation calls.

### Checks, Tutor, progress, navigation

Reuse current question surfaces and native evaluation. Capture each response against immutable lesson/question revision. New incompatible lesson revisions offer a controlled resume transition rather than mapping old responses onto new keys.

Define answer privacy across every channel: visible content, transcript, captions, alt text, media manifests, Tutor context, narration preloads, and exports. Before checking, only public prompt material is admitted. After reveal, solution access is explicit.

Tutor receives current scene/beat plus the permitted source snapshot. Replies and assistance do not rewrite the saved lesson. Add Lessons library/player through existing navigation and Back restoration. Bundle integration exposes Lesson status only after text-only behavior qualifies.

Focused tests include transaction rollback, duplicate operation IDs, stale source revisions, semantic block types, narrow/wide/enlarged layouts, no-provider resize, question parity/no-FSRS writes, answer-sentinel leakage across DTO/media channels, stale-attempt resume, Tutor exclusions, restart, backup/restore, keyboard/touch/Back, and migration from the then-supported production lineage.

**Exit / AC-11:** a complete text-first lesson is readable, resumable, source-grounded, answer-safe, recoverable, and useful without speech configuration.

## LK-11 — retained narration and cost-safe playback

### Capability and settings

At implementation time, verify the selected speech route/catalog and supported controls against current official provider documentation plus one deliberately authorized bounded live request. The previously requested design target was OpenRouter `hexgrad/kokoro-82m` with `af_river` (River), but route availability, encoding, controls, limits, producer revision, and pricing are evidence to establish at implementation time—not durable assumptions.

Keep binary speech transport separate from text/structured-output decoding. Add narration role/provider/model/voice settings and cached capability metadata. Opening Settings or saving a voice never synthesizes audio. Preview synthesis is explicit and observes privacy/budget/credential boundaries. Do not assume upstream idempotency or exactly-once billing.

Store readable and spoken script forms with explicit pronunciation bindings and cue IDs. Segment coherent sentence/beat groups, preserving numbers, symbols, units, and qualifications. Do not assume undocumented SSML or introduce hidden STT alignment.

### Retention, voice changes, playback

Derive compatible audio identity from spoken text, language, model/voice, known producer version, pronunciation/options, normalizer/segmenter version, and encoding. Join matching authorized in-flight work. Stream into bounded staging, validate/decode/hash returned bytes, move them into managed storage, then attach receipt/reference transactionally.

Successful segments survive failed siblings and cancellation. Unknown remote acceptance is recorded as indeterminate rather than blindly repeated when a duplicate charge is possible.

Changing desired voice does **not** synthesize the whole library. Preserve old clips as previous renditions. The next authorized Listen or explicit bounded batch generates only missing compatible segments. Old in-flight work retains its captured profile and cannot activate as the new voice. Activate only a coherent rendition; never silently mix voices. Switching back reuses compatible retained audio.

Implement Read/Listen, play/pause, scene/beat seek, local speed/volume, transcript, and acknowledged resume offset. Prompt and solution clips have separate reveal permissions/preload manifests. Define Tutor pause/resume explicitly. Qualify Android audio focus, interruption, Bluetooth, background/process recreation, and Back behavior through platform ports.

Paid retained media is not LRU cache. Full backup owns active and explicitly retained previous narration. Storage pressure stops optional new synthesis rather than deleting owned transcript/audio. Deliberate removal warns about regeneration cost.

Focused tests include mocked capability/error boundaries, request counts, concurrent matching work, partial failure, cancellation, lost response, corrupt/truncated audio, disk full, crash boundaries, voice race, switch-back zero calls, single-segment invalidation, offline replay/seek/speed, reveal privacy, backup/restore, managed-file integrity, and forward-migration compatibility. Live pronunciation and physical-device listening remain separate observed gates.

**Exit / AC-12:** saved narration is reusable offline, voice changes are cost-safe, and playback lifecycle does not mutate lesson meaning.

## LK-12 — local editable PowerPoint export

### Serializer and deterministic layout

Qualify a bounded local serialization adapter behind an application-owned interface. PptxGenJS is a candidate, not a predetermined dependency. Verify actual WebView/worker/binary behavior, notes, fonts, editable shapes/tables, figures, embedded audio, licensing, and exact version. If it cannot meet required capability, choose a local/native alternative behind the same contract. No cloud converter or runtime Python/Office prerequisite.

Compile deterministic 16:9 slides from the saved semantic lesson. Use editable text/shapes/tables where practical, admitted figures, and qualified math/vector fallbacks. Preserve source/accessible text. Stable overflow generates continuation slides instead of tiny text or whole-slide rasterization.

Assign each slide only the corresponding script portions in speaker notes. Include compatible current saved narration only when explicitly selected. Missing narration permits audio-free export or a separate deliberate generation action; export itself never synthesizes. Re-encoding/joining uses a qualified local codec path, not arbitrary MP3 byte concatenation.

Learner export excludes answers/solution audio by default. Any instructor/answer-section export is explicit. Private Tutor text, learner notes, and unrelated media cannot leak merely because an asset is reachable.

Rust authorizes a frozen bounded snapshot. Serialize locally, validate OOXML container/relationships/size, and use existing prepared external-file staging/save/share lifecycle with operation identity and crash cleanup. Reject macros, executable relationships, and arbitrary remote resources.

Tests include OOXML structure, notes mapping, continuation layout, media relationships, editability expectations, clipping, source text, answer disclosure, no-provider counters, crash cleanup, and actual Microsoft PowerPoint/LibreOffice interoperability where environments are available.

**Exit / AC-13:** unchanged lesson re-export works offline with zero provider calls and correctly mapped editable content/notes/audio.

## Production-baseline dependency inherited from PR #106

When #105 is eventually resumed, first read the actual merged production baseline, supported forward-migration lineage, backup/restore formats, application identities, and release/install contracts established after PR #104. New lesson/narration schema changes extend that lineage; they do not replace it.

Every new durable kind introduced here must be covered by:

- fresh-install schema creation;
- supported upgrade from the then-current released lineage;
- complete-library backup/restore;
- content-only transfer and trusted-device admission policy where applicable;
- unknown/future/tampered input refusal;
- managed-file preservation and deletion semantics;
- exact-source qualification.

If future feature work reveals a real defect in the production baseline, add a small reviewed forward repair. Do not rewrite released migration bytes/checksums or reopen historical-development cleanup merely to make the migration directory look shorter.

## Delivery increments when resumed

1. **105-A:** lesson persistence + deterministic text fixture.
2. **105-B:** coordinated authoring + responsive player/transcript.
3. **105-C:** checks/Tutor/progress/navigation + answer privacy.
4. **105-D:** speech capability/settings/segmentation.
5. **105-E:** retained audio + voice switching + playback/lifecycle.
6. **105-F:** local PPTX serializer/layout/export.
7. **105-G:** broad source/data/docs/compatibility audit, exact-head terminal qualification, and handoff.

Every increment gets focused native/UI/preservation tests before publication. Finish broad source/docs/compatibility review before terminal qualification. Run exact-head static/fast, full Linux, Android compile/package, and emulator gates under the repository policy current at that time. Record live speech, physical Android, visual/accessibility, and office interoperability separately.

## Resume handoff

Until explicitly resumed, this PR is a design reference only. No product acceptance is claimed.

On resume, the first bounded reconstruction must record:

- current `main` and the production release/baseline contract;
- active schema/payload versions and supported upgrade lineage;
- current lesson-adjacent owner paths after intervening work;
- provider/model availability verified at that time;
- current platform/manual limitations;
- focused test entry points and required terminal gates.

Proposed implementation title when resumed: **feat(lessons): responsive lessons, retained narration and local PowerPoint**.
