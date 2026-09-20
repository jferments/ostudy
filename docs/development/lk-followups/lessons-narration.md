# PR #105 roadmap — responsive lessons, retained narration, PowerPoint and final data consolidation

Status: planning-only successor to PR #104. Owned stages: **LK-10 through LK-13**. This roadmap supersedes the earlier planning draft on this branch where scope differs. It does not implement product behavior, authorize provider spending, inspect a personal library, retire migrations, or authorize merge.

Activation rule: implement only after PR #103 and PR #104 are reviewed and merged. Integrate the then-current merged `main`, verify the actual source-set/coverage/bundle/guide/Tutor contracts once, reconcile owner paths, and continue stage-by-stage. Do not stack implementation on an unfinished predecessor branch.

## Product result

A learner can generate a source-grounded lesson from the same shared knowledge used by cards, guides, and Tutor; read a responsive text-first transcript on desktop or mobile; answer safe embedded checks; ask contextual Tutor questions; and resume progress after restart. Optional authorized narration is synthesized once, retained as owned media, replayed offline, and changed cost-safely without rewriting the lesson. The same lesson exports locally to an editable `.pptx` with correctly assigned speaker notes and optional already-retained narration.

After the final schema-bearing feature stabilizes, this PR also owns **LK-13**: one curated current-format baseline, isolated conversion proof, privacy-safe actual-library adoption, integrated backup/transfer verification, and retirement/archive of obsolete migration/readers only after recovery is proven.

Not included: STT/transcription, voice cloning, speech grading, video export, a general slide editor, article generation, automatic image-occlusion authoring, Tutor deck-writing tools, a new scheduler/grader, cloud presentation conversion, or a second baseline consolidation in PR #106.

## Architecture and durable contracts

Dependency direction remains:

```text
SourceSetSnapshot + CollectionSynthesis + LearningContext
  -> LessonRevision / LessonPlan
  -> responsive semantic scene/beat rendering
  -> lesson-local checks + Tutor context + LessonProgress
  -> NarrationRendition / retained managed audio
  -> local LessonExportSnapshot -> editable PPTX

all durable feature contracts
  -> final current-format baseline + preservation/adoption
```

Rust/SQLite owns durable identities, validation, progress, publication, lifecycle, and current-format admission. React uses typed `src/ipc/` gateways and existing navigation/platform ports. Provider HTTP and credentials stay in the native AI gateway. Layout changes, playback, resume, and export do not authorize provider calls.

**LessonRevision** contains immutable source/coverage identity, objectives, ordered scenes/beats, semantic visual blocks, script segments, support links, and question references. Scene/beat IDs do not depend on screen size.

**LessonPlan** owns ordered entries and optional acyclic prerequisites. **LessonProgress** owns acknowledged lesson revision, scene/beat position, question-response state, and later narration segment/offset. Completion is not mastery.

**LessonCheck** reuses existing native question definitions/compiler/evaluator through a lesson-specific attempt context. Answering a lesson check does not silently submit an FSRS review. Explicit Practice uses existing card identities and ordinary review semantics.

**NarrationRendition** binds spoken script/configuration identity to segment manifests, verified retained hashes, producer metadata, receipts, and duration. Hash identity never replaces authorization. Expiring provider URLs are not the retained source of truth.

**LessonExportSnapshot** is a frozen, permission-filtered lesson/media graph with explicit notes/answers/audio choices and operation identity. A serializer receives no credentials, unrestricted filesystem paths, private Tutor history, or authority to call providers.

## LK-10 — text-first responsive lessons

### Persistence and coordinated authoring

Add current lesson/revision/plan/progress storage with stable IDs, bounded payloads, native validators, forward migrations, and retry-safe publication. Begin with deterministic fixtures that can publish/list/open/reopen a text-only lesson before integrating provider generation.

Plan objectives, prerequisites, depth, and coverage from LearningContext. Author each beat's teaching blocks, script, and check opportunity together so the channels cannot drift silently. Preserve qualifications, conditions, units, polarity, exceptions, and source support. Failed candidates may be repaired independently; valid siblings survive.

Use admitted figures or constrained typed data for diagrams. Do not accept arbitrary model-authored executable HTML/CSS/SVG. Scientific geometry and equations require semantic correctness, not decorative plausibility.

### Responsive player and transcript

Reuse semantic content blocks established by guide work. Support concept, comparison, process, worked-example, figure, and retrieval-check presentations. Layout uses container width, text scale, safe areas, and existing platform ports—not OS sniffing or a second mobile manuscript. Independent blocks reflow; spatial figures remain atomic with zoom/detail/text alternatives.

Transcript is available immediately. Resize, rotation, enlarged text, opening Tutor, or returning from another route preserves scene/beat/question state and makes zero lesson-generation calls.

### Checks, Tutor, progress, navigation

Reuse current question surfaces and native evaluation. Capture each response against immutable lesson/question revision. New incompatible lesson revisions offer a controlled resume transition rather than mapping old responses onto new keys.

Define answer privacy across every channel: visible content, transcript, captions, alt text, media manifests, Tutor context, narration preloads, and exports. Before checking, only public prompt material is admitted. After reveal, solution access is explicit.

Tutor receives current scene/beat plus the permitted source snapshot. Replies and assistance do not rewrite the saved lesson. Add Lessons library/player through existing navigation and Back restoration. Bundle integration exposes Lesson status only after text-only behavior qualifies.

Focused tests: transaction rollback, duplicate operation IDs, stale source revisions, all semantic block types, narrow/wide/enlarged layouts, no-provider resize, question parity/no-FSRS writes, answer sentinel leakage across DTO/media channels, stale-attempt resume, Tutor exclusions, restart/backup/restore, keyboard/touch/Back.

**Exit / AC-11:** a complete text-first lesson is readable, resumable, source-grounded, answer-safe, and useful without speech configuration.

## LK-11 — retained narration and cost-safe playback

### Capability and settings

At implementation time, verify the selected speech route/catalog and supported controls against current official provider documentation plus one deliberately authorized bounded live request. The requested design target is OpenRouter `hexgrad/kokoro-82m` with `af_river` (River), but route availability, encoding, controls, limits, producer revision, and pricing are evidence to establish—not assumptions.

Keep binary speech transport separate from text/structured-output decoding. Add narration role/provider/model/voice settings and cached capability metadata. Opening Settings or saving a voice never synthesizes audio. Preview synthesis is explicit and observes privacy/budget/credential boundaries. Do not assume upstream idempotency or exactly-once billing.

Store readable and spoken script forms with explicit pronunciation bindings and cue IDs. Segment coherent sentence/beat groups, preserving numbers, symbols, units, and qualifications. Do not assume undocumented SSML or introduce hidden STT alignment.

### Retention, voice changes, playback

Derive compatible audio identity from spoken text, language, model/voice, known producer version, pronunciation/options, normalizer/segmenter version, and encoding. Join matching authorized in-flight work. Stream into bounded staging, validate/decode/hash returned bytes, move them into managed storage, then attach receipt/reference transactionally.

Successful segments survive failed siblings and cancellation. Unknown remote acceptance is recorded as indeterminate rather than blindly repeated when a duplicate charge is possible.

Changing desired voice does **not** synthesize the whole library. Preserve old clips as previous renditions. The next authorized Listen or explicit bounded batch generates only missing compatible segments. Old in-flight work retains its captured profile and cannot activate as the new voice. Activate only a coherent rendition; never silently mix voices. Switching back reuses compatible retained audio.

Implement Read/Listen, play/pause, scene/beat seek, local speed/volume, transcript, and acknowledged resume offset. Prompt and solution clips have separate reveal permissions/preload manifests. Define Tutor pause/resume explicitly. Qualify Android audio focus, interruption, Bluetooth, background/process recreation, and Back behavior through platform ports.

Paid retained media is not LRU cache. Full backup owns active and explicitly retained previous narration. Storage pressure stops optional new synthesis rather than deleting owned transcript/audio. Deliberate removal warns about regeneration cost.

Focused tests: mocked capability/error boundaries, request counts, concurrent matching work, partial failure, cancellation, lost response, corrupt/truncated audio, disk full, crash boundaries, voice race, switch-back zero calls, single-segment invalidation, offline replay/seek/speed, reveal privacy, backup/restore, managed-file integrity. Live pronunciation and physical-device listening remain separate observed gates.

**Exit / AC-12:** saved narration is reusable offline, voice changes are cost-safe, and playback lifecycle does not mutate lesson meaning.

## LK-12 — local editable PowerPoint export

### Serializer and deterministic layout

Qualify a bounded local serialization adapter behind an application-owned interface. PptxGenJS is a candidate, not a predetermined dependency. Verify actual WebView/worker/binary behavior, notes, fonts, editable shapes/tables, figures, embedded audio, licensing, and exact version. If it cannot meet required capability, choose a local/native alternative behind the same contract. No cloud converter or runtime Python/Office prerequisite.

Compile deterministic 16:9 slides from the saved semantic lesson. Use editable text/shapes/tables where practical, admitted figures, and qualified math/vector fallbacks. Preserve source/accessible text. Stable overflow generates continuation slides instead of tiny text or whole-slide rasterization.

Assign each slide only the corresponding script portions in speaker notes. Include compatible current saved narration only when explicitly selected. Missing narration permits audio-free export or a separate deliberate generation action; export itself never synthesizes. Re-encoding/joining uses a qualified local codec path, not arbitrary MP3 byte concatenation.

Learner export excludes answers/solution audio by default. Any instructor/answer-section export is explicit. Private Tutor text, learner notes, and unrelated media cannot leak merely because an asset is reachable.

Rust authorizes a frozen bounded snapshot. Serialize locally, validate OOXML container/relationships/size, and use existing prepared external-file staging/save/share lifecycle with operation identity and crash cleanup. Reject macros, executable relationships, and arbitrary remote resources.

Tests: OOXML structure, notes mapping, continuation layout, media relationships, editability expectations, clipping, source text, answer disclosure, no-provider counters, crash cleanup, and actual Microsoft PowerPoint/LibreOffice interoperability where environments are available.

**Exit / AC-13:** unchanged lesson re-export works offline with zero provider calls and correctly mapped editable content/notes/audio.

## LK-13 — final current-format baseline, actual-library adoption, and migration retirement

LK-13 occurs **after LK-12 freezes the last schema-bearing contracts**. It is part of PR #105, not deferred to PR #106.

### Freeze contracts and build one curated baseline

Inventory the exact current schema, indexes, triggers, constraints, FTS configuration, payload versions, managed-file kinds, repair ledger, backup/sync formats, and current readers. Construct a curated fresh baseline with explicit lineage/baseline identity where needed. Prove structural and behavioral equivalence to a database migrated through every supported retained development transition.

Do not concatenate old SQL, rewrite released migration bytes/checksums, or relabel an incompatible database. Current-to-current admission must be a verified no-op; unknown/future/tampered/corrupt input remains fail-closed.

### Convert isolated copies first

Use known forward migrations plus narrowly scoped payload converters on isolated synthetic and restored-copy fixtures. Preserve primary identities, card/question content, schedules/due state, reviews/attempts, drafts, annotations, source/evidence identity, image/filter state, collection/bundle state, Tutor scopes, lessons/progress, and retained narration bytes/receipts.

Preservation receipts compare typed values and managed-file hashes, not row counts alone. Fault-inject interruption, disk full, corrupt asset, duplicate invocation, activation boundaries, and successful mutation followed by refresh failure. Migration performs no extraction, LLM, or speech calls.

### Actual-library adoption

Provide the maintained local backup/inspection/conversion/verification workflow only after the converter is qualified. Personal data stays on the user's machine. Record privacy-safe evidence: exact converter/build SHA, input lineage/version/ledger summary, verified complete-backup and asset-manifest identity, preservation digest, allowed transformations, output identity, successful reopen, representative functional checks, and restore of a new current backup.

Repository/CI tests cannot claim this human gate. Until it is performed, do not retire the recovery path required by the user's retained library.

### Archive obsolete runtime machinery after adoption

Only after adoption evidence, move superseded migration/converter/fixture material into a versioned historical archive outside normal startup/compile/package paths, with source SHA, immutable hashes, and an explicit offline historical-backup recovery entry point. Archive is not destruction; backup files remain valid recovery inputs.

Remove obsolete runtime version branches/readers in one coordinated change and prove normal startup no longer loads the archived chain. Future schema changes resume as small forward migrations from the new baseline.

### Integrated backup/transfer

Repeat complete-library backup/restore across documents/evidence, cards/questions/history, collections/bundles, guides, Tutor, lessons/progress, and retained audio. Content-only transfer keeps fresh schedules and excludes donor private history. Trusted-device sync explicitly negotiates supported lesson/media graph versions or rejects before partial application.

**Exit / AC-14:** one qualified current runtime/baseline exists, actual retained-library adoption is recorded separately from synthetic evidence, and historical backups still have a tested explicit recovery route.

## Delivery increments and PR #106 handoff

Implement coherent increments:

1. **105-A:** lesson persistence + deterministic text fixture.
2. **105-B:** coordinated authoring + responsive player/transcript.
3. **105-C:** checks/Tutor/progress/navigation + answer privacy.
4. **105-D:** speech capability/settings/segmentation.
5. **105-E:** retained audio + voice switching + playback/lifecycle.
6. **105-F:** local PPTX serializer/layout/export.
7. **105-G:** final contract freeze + curated baseline/equivalence.
8. **105-H:** staged converter/preservation + authorized actual-library adoption.
9. **105-I:** archive obsolete runtime machinery + integrated restore/transfer.
10. **105-J:** broad source/docs audit, exact-head terminal qualification, and handoff.

Every increment gets focused native/UI/preservation tests before publication. Finish broad source/docs/compatibility review before terminal qualification. Run exact-head static/fast, full Linux, Android compile/package, and emulator gates under current repository policy. Record live speech, physical Android, visual/accessibility, office interoperability, and actual-library acceptance separately.

Handoff to PR #106 only the **merged contracts and evidence**: final baseline identity, current readers/writers, archive/recovery entry point, adoption receipt, active schema/payload versions, platform/manual limitations, test entry points, exact merge SHA, and a bounded issue register. PR #106 audits and repairs residual defects; it does not perform a second baseline consolidation.

Proposed PR title: **feat(lessons): responsive lessons, retained narration, local PowerPoint and final data baseline**.
