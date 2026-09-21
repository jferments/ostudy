# Current-format transition, acceptance and validation

Part of the [PR #103 development record](README.md). This document records the implemented LK-02/LK-03 transition boundary and the LK-04 image-source preservation extension. Successor data consolidation belongs to PR #105/LK-13.

## 1. The pre-release data policy

The human has explicitly confirmed that oStudy has not been publicly released. The goal is one current canonical runtime representation, not an expanding matrix of legacy application modes. The repository's existing `ostudy-public-v1` name does not contradict that product status; it is a schema identifier.

**Migrate and verify first; archive second.** Do not delete useful data, clear application storage, overwrite historical checksums, relabel an old database as current, or force a library through a guessed conversion. PR #103 does not inspect or convert the user's personal library. The actual-library adoption/retirement gate is owned by PR #105/LK-13 and remains separate from this foundation merge.

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

### LK-03 successor checkpoint

The current registry is schema **18**. Migration `0018_canonical_evidence.sql` adds immutable evidence payloads, document-owned source occurrences and structure links. The migration backfills only retained canonical facts; it does not invent missing table/figure structure or call a provider. Historical migrations and checksums remain unchanged.

Qualified detail schema **1** stores native UTF-8 spans into the existing authoritative knowledge statement for comparison, process, quantity and classification projections. Existing units without details keep their serialization and identity; absence means unavailable structure. Region coverage receipt version **1** binds the planned evidence and source revisions, distinguishes unreadable/unresolved/non-learning content, and never equates protocol completion with exhaustive understanding. Older compatible receipts remain readable and are reported as older coverage, not silently promoted to region-complete evidence.

`database/knowledge_entry.rs` adds read-only typed-knowledge and receipt admission to the shared startup/restore boundary. Unsupported detail versions, malformed spans, forged coverage plans and dispositions fail before conversion writes. `database/knowledge_entry_tests.rs` verifies refusal and zero-write current/legacy admission. The current-format test filter executes these tests and the complete preservation fixture; `scripts/test-native-contracts.sh` rejects filters matching no tests.

The complete-library fixture now includes immutable structured evidence, a typed process and a partially successful semantic window. Its existing typed-row/managed-file receipts, plus native evidence/context decoding, are checked through reopen, export without mutation, staged-restore rollback, committed restore and post-restore reopen. Private rejected proposals stay in complete backups and are removed by device-transfer sanitization. No new interchange version or personal-library conversion is implied by these repository tests.


### LK-04 image-source preservation checkpoint

The current registry is schema **21**. Migrations 19–21 add ordered image-set ownership/manifests, reusable extraction receipts/cache, and revision-safe image edit/filter state. Released earlier migration bytes/checksums remain unchanged.

Each image collection retains independently owned originals, stable member identities, a content-addressed manifest, extraction/projection receipts, and revision-checked source state. Automatic generation does not make a stitched image or cleaned derivative the source authority. Reversible native inclusion/filter/amendment representations remain readable even though the ordinary learner UI no longer requires manual preparation.

Image evidence may use stable semantic segment keys independent of numeric chunk ordering; shared preparation preserves the authoritative seed-to-source relationship explicitly. Content-addressed image manifests are validated through their recorded fingerprint rather than forced into the generic `source.<ext>` convention.

Complete-library backup/restore covers originals, current manifest, processing receipts, pending source snapshots/filter state, canonical evidence, generated learner artifacts, and existing schedules/history. Startup and complete restore apply the same fail-closed current-format admission rules before ordinary runtime state is exposed.

## 2. PR #103 acceptance contract

| ID | Acceptance boundary | Stage |
| --- | --- | --- |
| AC-01 | Self-contained plan, branch, acceptance/data policy | LK-00 |
| AC-02 | Full-size Practice-faithful preview of unsaved drafts with no study writes | LK-01 |
| AC-03 | Read-only current-format admission, preservation fixtures, fail-closed transition policy | LK-02 |
| AC-04 | Reusable canonical evidence/knowledge/coverage/shared context with bounded work | LK-03 |
| AC-05 | Automatic image collections, conservative overlap, retained originals, reusable extraction, exact occurrences and backup preservation | LK-04 |

AC-06 and later have moved to PRs #104–#106 and are not part of PR #103 merge acceptance.

## 3. Focused regression program

| Family | Required assertions |
| --- | --- |
| PREVIEW | Practice presentation parity; unsaved draft; reveal/check/reset; Close/Back/focus; no persisted study write/provider call; no leaked unmasked occlusion source |
| ENTRY/PRESERVATION | fail-closed unknown/future/tampered/corrupt input; byte-preserving refusal; reopen/backup/restore rollback/commit; typed values and managed-file hashes |
| EVIDENCE/REUSE | Unicode selectors; structured evidence; qualified typed detail; complete dispositions; concurrent compatible subscribers; cancellation/interruption; no consumer-model reanalysis |
| IMAGE COLLECTION | multi-select/+ /drop/paste; byte admission; failed-file removal; automatic card/guide handoff; queued additions/navigation survival; stable operation identity |
| IMAGE OVERLAP | upload-order permutations; exact/nonadjacent duplicates; changed digit/sign/unit/pixel refusal; conservative geometry; exact occurrences; original-byte ownership; bounded comparison |
| IMAGE LIFECYCLE | stable semantic image keys; content-addressed manifest resolution; current filters/amendments; stale-publication refusal; original inspection; complete backup/restore |

Deterministic fixtures establish structural and lifecycle contracts. They do not establish live OCR accuracy, live-provider factual quality, physical-device chooser/drop/paste behavior, or human visual/accessibility acceptance.

## 4. Validation tiers and handoff

Use the maintained command runbook and exact-SHA repository policy. During implementation, fix the smallest failing boundary first. For the final candidate, reconcile source/docs before terminal qualification and then keep the candidate frozen except for actual blocking repairs.

Report separately:

- publication / remote head;
- static and fast qualification;
- full Linux;
- Android compile/package and emulator;
- live OCR/provider quality;
- desktop/physical-device visual/accessibility behavior;
- review and merge.

PR #103 remains data-preserving without performing the eventual final baseline consolidation. Historical recovery stays available until PR #105/LK-13 satisfies its actual-library adoption gate.

## 5. Successor ownership

- [PR #104](https://github.com/jferments/ostudy/pull/104) — AC-06 through AC-10 / LK-05 through LK-09.
- [PR #105](https://github.com/jferments/ostudy/pull/105) — AC-11 through AC-14 / LK-10 through LK-13.
- [PR #106](https://github.com/jferments/ostudy/pull/106) — AC-15 / LK-14 plus residual evidence-backed cleanup.

No successor product implementation is included in PR #103.
