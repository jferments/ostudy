# Current-format transition, acceptance and validation

Part of the [development plan](README.md). This document defines required evidence and records the implemented LK-02 transition boundary. Later-stage acceptance remains unclaimed until its own evidence exists.

## 1. The pre-release data policy

The human has explicitly confirmed that oStudy has not been publicly released. The goal is one current canonical runtime representation, not an expanding matrix of legacy application modes. The repository's existing `ostudy-public-v1` name does not contradict that product status; it is a schema identifier.

**Migrate and verify first; archive second.** Do not delete useful data, clear application storage, overwrite historical checksums, relabel an old database as current, or force a library through a guessed conversion. Staged source implementation is authorized on PR #103; execution against the user's personal library is a separate LK-13 human gate and has not occurred.

Different formats still serve different purposes: SQLite library, validated rich/question payloads, authoring JSON, portable deck, full backup, sync payload, and PowerPoint. Each has one current application representation/writer after conversion; they are not collapsed into one universal format. Incoming formats such as Anki remain useful import boundaries, not obsolete internal schemas to delete. Human-authored and generated origins, Basic/Cloze and objective questions are distinct content kinds, not redundant migration versions.

### Placement of compatibility

Startup, explicit import and restore normalize known supported input into current data before feature code runs. Runtime guide/Tutor/lesson/Practice readers then consume the current contract. Eliminate scattered historical-version branches and dual writes. Do not normalize by losing old semantic distinctions or discarding retained learner records.

Keep historical migration SQL/fixtures unchanged while they are needed for conversion. Following final adoption, preserve them under an explicitly archival location with a source SHA, manifest and recovery instructions, excluded from normal compilation/startup/packaging. A small offline converter for an intentionally restored historical backup can remain available from that archive without turning normal runtime into a compatibility framework.

New feature schema transitions during this branch are narrow forward steps. They upgrade the library before normal use; they do not enable parallel runtime schemas. Consolidate once, after the final schema-bearing stage, rather than repeatedly rebuilding the baseline during development. Future schema changes still need a forward transition from the active development baseline. A first public release will establish a separately explicit support window.

### LK-02 implemented current-format checkpoint

LK-02 makes the transition boundary executable without retiring historical machinery. The contract below describes repository state on this branch; it is **not** evidence about the schema installed in any personal application data directory.

| Persisted surface | Current contract at the LK-02 boundary | Reader / transition owner |
| --- | --- | --- |
| SQLite library | Lineage `ostudy-public-v1`; exact registered migration prefix through schema/user version 17; registered data-repair ledger | `database.rs`, `database/entry.rs`, `data_repairs.rs` |
| Basic/Cloze rich notes | `content_format = tiptap-json`, note `content_schema_version = 1`; current rich-document validator admits the supported document schema through version 2 | `cards/content.rs`; entry validation in `database/payload_entry.rs` |
| Interactive questions | `content_format = question-json`, question schema 1 | `cards/questions/`; entry validation parses every persisted definition |
| Question attempts | checkpoint `attemptVersion = 1`, with stored instance/note/card identity, fingerprint, and revision required to match the decoded checkpoint | `cards/questions/attempt.rs`; entry validation cross-checks the row against the decoded attempt |
| Canonical knowledge/source records | knowledge content schema 1; source identity version 1 remains readable and document evidence identity version 2 is the current document form | `knowledge/contracts.rs`; successor knowledge contracts belong to LK-03 rather than scattered runtime adapters |
| Study scope | strict native `StudyScope`: general/library or one deck/document/guide, plus a canonical non-nested selection of at most 16 explicit members | `knowledge/scope.rs`; LK-05 replaces this through the same current-format transition boundary |
| Durable editor drafts | payload schema 1 for card/template/bulk/profile recovery; active drafts are validated during startup reconciliation and unsafe/unsupported recovery state becomes explicit conflict rather than guessed canonical content | `editor_drafts.rs` |
| Complete-library backup | `ostudy-library-backup` format 4; package readers retain explicit support for versions 1–4; the embedded SQLite snapshot must pass the same read-only database admission before activation | `library_backup.rs`, `database::validate_backup_lineage` |
| Trusted-device package | `ostudy-device-sync` format 3 with explicit legacy-v2 reader; package schema may not exceed the receiving build; v3 carries the selected document files while private/derived semantic state follows the existing sanitization policy | `device_sync/package.rs`, `knowledge::sanitize_for_device_transfer` |
| External study imports | `.studydeck`, strict authoring JSON, Anki/APKG and staged external files remain explicit import/interchange contracts | Their existing import owners; they are not historical internal runtime schemas to collapse into SQLite |

For a public-lineage database, startup now performs a **read-only admission before SQLite durability/journal configuration and before any migration or repair write**. Admission requires an exact known migration-ledger prefix, matching `PRAGMA user_version`, a recognized repair ledger, supported persisted card/question payloads, and database integrity. Only then does the existing migration/repair chain normalize the input. The resulting connection is revalidated at `LATEST_SCHEMA_VERSION` before ordinary runtime state is published. Backup admission calls the same read-only validator. Unknown, future, altered-checksum, corrupt-payload, and version/ledger-mismatch fixtures are rejected before conversion writes; disk fixtures assert rejected bytes remain identical and no WAL/SHM sidecar is created.

The synthetic preservation fixture intentionally contains Basic/reverse, both Cloze modes, every interactive question kind including image occlusion, answering and checked attempts, schedules and review history, a durable draft, managed image bytes, a retained source document/citations, an edited Study Guide overlay, Tutor conversation/turn history, and deliberate deletion/suppression evidence. Its receipt hashes every SQLite value with type and length information and separately hashes managed files. Reopen, complete-backup creation, staged restore rollback, committed restore, and post-restore reopen must preserve those receipts and pass foreign-key/integrity plus current question/attempt decoders. Equal row counts alone are deliberately insufficient.

### Final consolidation checkpoint (LK-13)

LK-02 defines the retirement gate now so later schema work cannot make it up opportunistically:

1. Freeze the final post-LK-12 schema and payload contracts and construct a curated fresh baseline proven equivalent to the migrated current schema, including tables, indexes, triggers, constraints and FTS behavior.
2. Convert isolated copies from every explicitly supported retained development lineage using the known chain and narrowly scoped payload converters. Already-current input must be a verified no-op; unknown/future/corrupt input remains fail-closed.
3. Run the maintained local actual-library adoption workflow. Its privacy-safe receipt records converter/source SHA, input lineage and user/ledger version summary, coherent backup/integrity identity, preservation-receipt digest, output lineage/version, and successful reopen plus restore verification. It records no source text or private database contents.
4. Only after that evidence, move superseded migration/converter/fixture material into a versioned historical archive outside normal compilation/startup/packaging, with a manifest naming the source SHA and an explicit offline historical-backup recovery entry point. Released migration bytes/checksums are never rewritten to simulate consolidation.
5. Prove ordinary startup no longer loads archived history and repeat integrated backup/restore across the final source, question, history, collection, Tutor, lesson and retained-media graph.

Until LK-13 satisfies that gate, the current migration chain and historical recovery paths remain live and unchanged. This is intentional preservation, not unfinished LK-02 work.

### Known-source conversion procedure

1. **Inspect safely.** Stop or acquire the existing exclusive library-maintenance boundary. Resolve actual application storage through maintained paths. Record installed application identity, schema lineage/version, checksum/repair state and converter SHA. Unknown/future/corrupt input is rejected without mutation. The LK-02 branch registry is schema 17; repository state is not proof of the user's installed state.
2. **Preserve a recoverable input.** Produce a coherent full backup using the existing backup boundary, including managed assets and required journals handled correctly by that boundary. Verify its integrity and restore an isolated copy. Check free space for original, backup, staging and output. A raw copy of a live SQLite file is not the backup procedure.
3. **Capture invariants.** Record stable IDs and canonical sorted-row/value digests for learner-owned records, schedules/reviews, attempts, guide overlays, source associations and conversation turns, plus asset reference/hash manifests. Counts alone cannot detect silently changed values. Sensitive content and full databases stay local; diagnostics contain only the approved bounded summaries.
4. **Normalize a staged copy.** Apply the known migration/repair path and current-payload conversions. Reuse existing repair ownership and write a conversion receipt. Preserve original note/card IDs, schedules, due values, review outcomes, tags, templates, drafts, annotations, conversation scopes and managed bytes. Source/interpretation changes are separate from mere schema conversion; migration never silently contacts a provider.
5. **Prove target equivalence.** Compare the migrated schema with a fresh curated current baseline, including constraints, indexes, triggers and FTS behavior. Verify referential integrity, current decoders, canonical invariant digests and all managed references. Seeded/current defaults must not overwrite existing settings. Already-current input is a verified no-op.
6. **Activate recoverably.** Use the existing staged restore/swap/startup recovery discipline; test crashes before and after each durable boundary. Retain the prior coherent library until activation and reopen verification complete. Changing a metadata version is not activation of a valid conversion. Restoring an older binary alone is not data rollback.
7. **Exercise the real library.** Reopen, inspect representative cards and rich media, practice/resume, verify analytics, open edited guides, inspect sources/Tutor and new lesson/audio records, and create/restore a new current backup. Record the exact tested binary/converter identity and observed result.
8. **Archive old machinery.** Only after all known retained development libraries have a verified upgrade/recovery route, archive the old chain and obsolete readers. Update documentation, build references, source guards and current-schema fixtures together. Preserve historical recovery tooling and existing backup files; archive does not mean destroy. Test that ordinary startup no longer loads the archived chain.

There is no need for a remote service to inspect the library, and no demand to upload it to GitHub. The real-library gate will be performed locally through a maintained, testable wrapper once implemented. Until that evidence exists, LK-13 retirement stays pending; the rest of this plan can still be implemented and published.

### Data-preservation matrix

| Category | Must survive unchanged unless explicitly edited by the user | Allowed derived rebuild |
| --- | --- | --- |
| Cards/questions | Note/card IDs, source/key/variant relations, tags, rich content and assets | Search projections/current serialization |
| Learning history | Schedules, due state, retained reviews/attempts, first-check/assistance distinctions | Aggregate analytics indexes |
| Documents | Originals, source identity, exact evidence and retained provenance | Qualified extraction derivatives only under explicit policy |
| Guides | Published text, stable topic relationships, notes, overrides and conflicts | Effective render/export layout |
| Tutor | Chat/turn IDs, text, citation snapshots, sharing and source choices | Authorized retrieval index |
| Collections | Membership/ranges, synthesis references, output targets and deletion suppression | Compatible overview/index projections |
| Lessons | Scenes/beats/scripts, question links, progress and explicit retained revisions | Layout and prepared exports |
| Narration | Paid retained bytes, manifests, voice/configuration, receipts and integrity | Local export assembly from retained clips |

## 2. Stable acceptance contract

PR acceptance uses these IDs. AC-01 covers this documentation-only request. Product acceptance starts at AC-02; none is completed by writing this plan. Revision-2 requirement families are retained as traceability labels rather than silently replaced by new scope.

| ID | Acceptance boundary | First owner | Revision-2 coverage |
| --- | --- | --- | --- |
| AC-01 | Self-contained ordered plan, affected owners, tests, data policy, branch and draft PR | LK-00 | Planning publication |
| AC-02 | Full-size Practice preview, prominent Close preview, draft restoration, no study writes | LK-01 | New preview requirement |
| AC-03 | Current-only runtime/entry conversion, preservation fixtures and scoped retirement policy | LK-02 | New data policy |
| AC-04 | Compatible reusable knowledge, qualified evidence, complete dispositions, bounded shared work | LK-03 | K-01, K-02, K-06, K-07, K-08 |
| AC-05 | Ordered images, conservative overlap, reversible boilerplate filtering, exact occurrences | LK-04 | K-03, K-04, K-05 |
| AC-06 | Folder/range organization, deduplicated snapshots and permission-safe Tutor selection | LK-05 | C-02, C-03, U-01 |
| AC-07 | Thin synthesis with emphasis/support separation and incremental dependency safety | LK-06 | C-01, C-04 |
| AC-08 | One automatic bundle per target, honest progress, preserved edits/schedules/suppression | LK-07 | G-01, G-02, G-03 |
| AC-09 | Rich guide/viewer/PDF parity, navigable TOC/bookmarks, separately qualified semantics/math | LK-08 | P-01, P-02 |
| AC-10 | Project/chat CRUD, explicit inheritance, historical turn isolation | LK-09 | U-01, U-02 |
| AC-11 | Shared-knowledge lessons, responsive content, transcript, safe checks/Tutor and resume | LK-10 | L-01 through L-06 |
| AC-12 | Retained speech, zero-cost reuse, correct voice/segment invalidation, safe playback | LK-11 | A-01 through A-08 |
| AC-13 | Local editable PPTX, correct notes/audio/disclosure, no generation during export | LK-12 | X-01 through X-04 |
| AC-14 | Final baseline equivalence, actual library adoption, backup/transfer safety and old-code archive | LK-13 | D-01 plus new retirement requirement |
| AC-15 | Exact-head full/platform evidence, combined scenario and explicit manual gates | LK-14 | Whole-system qualification |

Each criterion inherits privacy, existing question behavior, local-first access, managed-asset and transaction tests from the stage that introduces it. Delaying a final cross-feature test does not postpone that stage's basic preservation/backup support.

## 3. Concrete regression program

Names in this table are test families to add/extend, not invented existing executable commands. Co-locate tests with the current owner identified in the plan. Proposed filenames in stage documents remain proposed until committed.

| Family | Required assertions |
| --- | --- |
| PREVIEW | Same fixture/rendering as Practice for all kinds; current unsaved draft; reveal/check/reset; Close/Back/focus; zero persisted study writes and zero provider calls; no leaked unmasked occlusion asset |
| EVIDENCE | Unicode exact selectors; table headers/units/footnotes; code/math fidelity; same payload/different occurrences; source-change publication refusal; complete manifests including no-learning/unresolved records |
| REUSE | Concurrent callers join compatible jobs; narrow scope cannot broaden disclosure; consumer-model changes keep units; embedding changes rebuild vectors only; completed receipts are read-only |
| SCREENSHOT | Exact/overlap duplicates; sticky headers; repeated educational UI; changed digit/sign/inequality; hidden text; table continuation; reorder/crop/append; reversible filtering; original-location inspection |
| SCOPE | Folder cycles and depth/expanded-budget bounds; ancestors/descendants/range union; exclusion sentinel in source, unit, summary, guide, transcript and asset; member change during provider work |
| SYNTHESIS | Sparse slide selects objective, reading supplies explanation; contradiction/conditions retained; no all-pairs explosion; changed/minority/empty members covered; document rows unchanged |
| BUNDLE | Duplicate upload/retry; one deck per target; failed sibling survival; late membership change; edited-card protection; guide overlay conflict; deletion suppression; restart without automatic cloud replay |
| GUIDE/PDF | Meaningful term marks; true bold; shared effective content; notes setting; table/page boundaries; TOC links/bookmarks; reading order; glyph/math/figure fidelity; offline/provider-free export |
| PROJECT | Rename/move/archive/delete; stale writes; preserve sources on move; explicit adopt-defaults; old-turn snapshots; no automatic cross-chat permission; delete-project-preserves-chats |
| LESSON | Scene/beat identity; coordinated visuals/script; qualified support; narrow/wide/text-scale parity; atomic scientific figures; progress through resize; text-only offline use; no FSRS writes from pop quizzes |
| SPEECH | Captured model/voice; same configuration no-op; return-to-voice reuse; one-segment edits; unsupported controls; ambiguous billing; truncated/corrupt audio; cancel/resume/full-disk; retain paid bytes |
| PLAYBACK | Zero TTS replay/seek/speed/volume; transcript correspondence; correct cue phases; no solution preload; pause/resume with Tutor; Android audio focus/background/process recreation |
| PPTX | Local serialization; editable objects; per-beat notes; correct continuation mapping; learner/instructor disclosure; embedded audio integrity; no remote resources/macros; real office-app inspection |
| DATA | Fresh vs migrated schema equivalence; current-to-current no-op; value/ID/asset preservation; fail-closed unknown input; crash-safe activation; legacy backup via explicit converter; archive not in runtime |

Create small authored fixtures plus permitted representative sources: prose, sparse slides/readings, tables, diagrams, equations/code, scans and overlapping screenshots. Keep provider fixtures deterministic for mandatory CI. Separate development examples from held-out quality sources. Never commit private course documents, patient information, raw credentials, personal audio or live library files.

Quality sampling measures qualified-claim recall, unsupported statements, duplicate rate, contradiction retention, localization and table context. Human annotations establish the reference; the same model's outputs are not their own ground truth. Report strata, missing/unreadable material and sample size. Schema validity and a valid citation ID do not prove entailment.

## 4. Performance and work-accounting gates

Measure baseline and candidate on identical manifests/configuration, with a constrained Android profile as well as desktop. Track time to first usable result, total latency, provider calls/bytes/usage, queue delay, memory, and storage broken down into originals, canonical data, knowledge, indexes, compositions, retained media and temporary files. Do not claim a speedup from a smoother progress indicator.

The following are exact unnecessary-work assertions, not statistical performance targets:

| Operation on compatible prepared material | Required avoided work |
| --- | --- |
| Add source to another folder | Zero document interpretation calls; unchanged document knowledge IDs/revisions |
| Change guide/Tutor/lesson writing model | Zero invalidation of compatible saved knowledge |
| Reopen transcript, resize/rotate or open Tutor pane | Zero lesson-writing or TTS calls |
| Replay saved audio, seek, change local speed or export ready audio | Zero TTS calls |
| Choose the same voice or a previously retained compatible voice | Zero synthesis for existing matching segments |
| Modify one segment/pronunciation rule | No synthesis for unrelated segments |
| Re-export unchanged lesson | Zero source analysis or lesson-writing calls |

An ordinary Tutor question may use its selected answer/embedding service; the zero-call claim applies to unnecessary reanalysis, not to answering the question. A new collection overview can require synthesis without violating document reuse. Numerical latency/storage targets must be measured and agreed before tuning; do not invent percentages or reduce coverage/resolution/assertions to pass them.

## 5. Validation tiers and evidence

Use the maintained [command runbook](../../agents/COMMAND-RUNBOOK.md), [testing guide](../testing-and-ci.md), [execution state](../../agents/EXECUTION-STATE.md), and [local handoff](../local-testing-and-handoff.md). Existing focused entry points include `pnpm verify:docs`, `./scripts/check-changed.sh`, `./scripts/check-static.sh`, `./scripts/check-fast.sh`, `./scripts/check.sh`, `pnpm android:test`, and `pnpm android:smoke`. Select exact test filenames only after they exist; do not present proposed files as commands already executable.

For this initial plan: inspect links, stage/acceptance traceability and scope; publish documentation; inspect the automatic exact-head Check and PR Contract runs. Do not manually dispatch full Linux/Android merely for Markdown. For implementation: run the smallest affected regression first, then the required broader gates. All source is published through the GitHub connector; self-hosted Actions qualify it read-only.

Each stage handoff records branch, expected parent/tree, acknowledged changed blobs, candidate/published SHA, acceptance boundary, exact run and tested SHA, actual commands, failures/unrun gates, manual boundaries and next concrete action. Update the PR body for progress; do not keep a second contradictory source-of-truth ledger in timeless documentation.

One unchanged-head rerun requires transient evidence. The same failure fingerprint twice requires a changed hypothesis/strategy. Stop optional investigations once the boundary is understood. Finish the full source/documentation audit before terminal qualification; after candidate freeze, fix actual blocking failures rather than opportunistically revisiting scope.

## 6. Human gates and continuation

Actual-library migration, live model quality, speech pronunciation, visual/accessibility behavior, physical Android lifecycle and PowerPoint/Impress compatibility are distinct from unit/static/emulator checks. Record which were actually performed, on which exact source/platform, and what remains. Never mark all gates passed because one workflow is green.

The next product action is **LK-03 — reusable evidence, knowledge and context foundation**. Continue on the same branch/PR, preserve the LK-01/LK-02 boundaries, and implement the next coherent acceptance increment before starting screenshot or collection work.
