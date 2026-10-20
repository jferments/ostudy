# Post-PR #104 durable-data contract audit

Status: **maintained contract inventory after merged PR #106** (`9ef91e79346b6d0d0ace983da3c1583303f15a43`). This began as the 106-C inventory and now records the completed 106-D disposition. It is not authority to rewrite compatible data. See the [completion record](lk-followups/consolidation-qualification.md) for qualification and recorded manual acceptance, and the [hardening register](post-106-hardening.md) for unimplemented follow-ups.

The pre-release consolidation is complete. Future changes use reviewed forward migrations and supported readers under the rule **migrate and verify first; archive second**. Never rewrite an installed learner library, migration checksum, backup or recovery path merely to make the tree look smaller. A product's release status does not erase existing retained-data obligations.

## 106-D current release baseline

The `ostudy-release-v1` **baseline is schema 1**. Its canonical SQL is generated from the exact qualified `ostudy-public-v1` schema-30 result and protected by SHA-256. The current writer is **schema 12** after the forward release migrations `selection_deck_operations` (2), `deck_merge_operations` (3), `study_target_generation_instructions` (4), `review_event_scheduler_context` (5), `subjects` (6), `study_activity` (7), `document_annotations` (8), `lessons` (9), `note_annotation_sources` (10) `lesson_narration` (11) and `study_guide_reading` (12); fresh libraries execute the baseline and then ordinary release migrations instead of replaying thirty pre-release public-v1 migrations.

Legacy `ostudy-public-v1` schemas **10–30** remain supported complete-backup inputs. They are admitted by their immutable migration/repair ledgers, normalized through schema 30 and legacy repair versions **1–3**, then converted only after an isolated SQLite backup proves canonical-schema equality and a typed preservation receipt for every non-ledger SQLite value. The live conversion is one atomic metadata transaction: learner tables and managed files are not rewritten.

The explicit source-checkout recovery path for final pre-public v26 remains separate. External interchange readers such as .studydeck writer v3/current legacy readers, generated JSON, trusted-device packages, and UI checkpoints are compatibility contracts rather than migration cruft.

## Current durable contracts

| Surface | Current writer/current form | Supported reader/compatibility state | Completed disposition and extension rule |
| --- | --- | --- | --- |
| SQLite library | current writer lineage `ostudy-release-v1`, schema/user version **9**; immutable baseline schema 1 plus forward release migrations 2–9 | release-v1 schemas 1–9 are admitted; schemas 1–8 upgrade forward through the checkpointed path. Supported public-v1 backup schemas 10–30 retain immutable migration/repair-ledger admission and are normalized/preservation-verified before release conversion and forward migration | release-v1 is current; append reviewed forward migrations, retain public-v1 compatibility and explicit final-v26 offline recovery. Never edit historical migration bytes in place |
| Data repairs | current release-v1 baseline expects an empty release repair ledger; legacy public-v1 repairs **1–3** cover canonical semantic hashes, canonical block hashes, and document-artifact user edits | checksum-protected public-v1 repairs are verified/applied during supported legacy normalization, not replayed for fresh release-v1 libraries | retain legacy repair evidence. Introduce any future release repair through an explicit registry/admission change with preservation tests; unknown repair rows remain invalid |
| Final pre-public lineage | exact v26 migration/repair ledger, accepted SQLite marker 0 or 26 in the explicit recovery tool | normal runtime conversion retired; source-checkout recovery uses `scripts/history/pre-public-v26/ledger.json` plus `transition-pre-public-data.sh` | archived outside normal runtime after the recorded lineage gate; preserve explicit offline recovery |
| Rich note transport | `content_format=tiptap-json`, transport `content_schema_version=1` | canonical rich root schema 1–3; schema 3 owns typed math + managed audio/video and is validated through the schema-3 wrapper | retain all supported rich readers; envelope and root versions are distinct |
| Interactive questions | `question-json` schema 1 | strict typed definition/evaluator; attempt checkpoint version 1 | retain |
| Canonical documents | canonical document schema 1 | current adapters normalize PDF, Xberg non-PDF, and image-set sources; AnyDoc names survive only as historical provenance/compatibility | retain canonical evidence and supported backend identities without restoring the retired runtime adapter |
| Knowledge/source evidence | content schema 1; source identity v1 plus document v2 and image-evidence v3 identities | older compatible identities remain readable and validation fails closed on unsupported identities | retain; normalizing old identities needs coordinated writer/reader/evidence preservation |
| Complete library backup | package format **4**; package readers 1–4 | database admission recognizes release-v1 schemas **1–9** and supported legacy public-v1 schemas **10–30**; schema-10/11 Tutor regressions protect that historical floor | retain the explicit package and database support contracts; package version is not database lineage/version |
| `.studydeck` | writer v3 | readers v1/v2/v3; older versions are interchange compatibility, not internal migration residue | retain |
| Generated `.ostudy.json` | Basic/Reverse/Cloze v1; mixed typed-question v2 semantics | importer reads v1 and v2; v1 rejects interactive questions | retain both |
| Trusted-device package | writer v3 | explicit legacy v2 reader; selected data graph and embedded database admission are separately validated | retain; clarify any wire schema-metadata change before replacing the legacy numeric alias |
| UI checkpoint | writer v2 | explicit v1 compatibility reader, normalized before current use | retain until a separate compatibility decision with saved-state tests |
| Managed files | SQLite-owned card assets plus immutable document source/source-asset/page-render files | full backup owns required retained bytes; sync/studydeck have narrower explicit scopes | retain ownership; no orphan cleanup without ledger proof |
| Secrets | OS secure store keyed by application profile; session-only credentials in native memory | production/dev namespaces are separate; secrets are excluded from SQLite/backups | retain; successful live restore clears the entire native session map without deleting saved OS credentials |

## 106-C findings and final disposition

1. **Schema 30 is the terminal pre-0.1 public-v1 registry, not the current writer.** Older stage documents naming 17/18/21 describe historical checkpoints; current runtime writes release-v1 schema 12 from the immutable schema-1 baseline plus forward migrations 2–12.
2. **The public-v1 repair registry is retained compatibility evidence.** Three repairs remain required for supported legacy admission/normalization even though fresh release-v1 does not replay that ledger. Clearing a pre-public ledger did not authorize deleting the compatibility framework.
3. **The backup schema-10 floor is intentional.** Historical Tutor backup tests construct real schema-10/11 snapshots. Current release-v1 backups are a separate admitted lineage, not a reduction of that compatibility floor.
4. **Rich-content docs were stale.** Native admission supports rich-root schema 3 for math/media while preserving the transport-envelope version 1.
5. **Generated-import docs were stale.** The importer supports v1 and v2; interactive questions require v2.
6. **The v26 runtime adoption path was retired in 106-D after the recorded gate.** Historical recovery remains explicit and staged; current startup leaves unsupported pre-public data unchanged.
7. **Public migrations 1–30 are retained compatibility, not automatic cleanup targets.** The release-v1 baseline/converter provides runtime simplification while immutable public-v1 bytes remain available for supported historical inputs.

## Historical 106-D entry conditions

The completed consolidation required an exact schema-30 source, canonical schema manifest, typed preservation receipts, fresh/reopen/historical-backup/current-backup tests, a concrete simplification benefit, an explicit historical recovery route, and isolated-copy evidence before retiring runtime adoption. The [archived plan](../history/pr-106-implementation-plan.md) preserves the original checklist.

These conditions are history, not instructions to consolidate the release baseline again or to inspect a protected learner library during documentation cleanup. Disposable installed-copy acceptance and the protected-library cutover are distinct; see the completion record.

## Archive versus retain rule

**Already archived outside normal runtime:** the exact final pre-public-v26 recognition/conversion machinery, with explicit offline recovery preserved.

**Retain:** public repair definitions, current-format validators, external interchange readers (`.studydeck`, generated JSON, Anki), backup compatibility, sync v2 reader, UI checkpoint v1 reader, rich schema 1–3 readers, and any compatibility path with a demonstrated supported input.

A historical name is not evidence of dead code. For example, the legacy-named PDF semantic tables still have production synchronization consumers. Retirement requires caller/fixture/package/restore evidence plus a replacement recovery path, not a global search that misses generic table-driven readers.
