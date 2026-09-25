# Post-PR #104 durable-data contract audit

Status: PR #106 / 106-C executable inventory. Baseline source: merged PR #104 plus the earlier #106 isolation/tooling increments. This document is an **inventory and disposition register**, not authority to rewrite compatible data.

The product has not had a public release. That permits a final pre-release consolidation, but only under the established rule: **migrate and verify first; archive second**. Never rewrite an installed learner library, migration checksum, backup or recovery path merely to make the tree look smaller.

## Current durable contracts

| Surface | Current writer/current form | Supported reader/compatibility state | Disposition for 106-D |
| --- | --- | --- | --- |
| SQLite library | lineage `ostudy-public-v1`, schema/user version **30**, immutable migrations `0001`–`0030` | exact known migration-prefix admission plus current repair-ledger validation; one exact final pre-public-v26 adoption path remains live | **retain the v1 migration chain by default**: it is current compatibility, supports historical backups, and is already fail-closed. A replacement baseline needs a concrete simplification/release benefit plus synthetic + isolated real-copy preservation; never edit migration bytes in place |
| Public data repairs | repair versions **1–3**: canonical semantic hashes, canonical block hashes, document-artifact user edits | checksum-protected and applied/verified by normal startup | **retain as current compatibility**, or fold only as part of the same explicitly verified new baseline; never confuse with the old pre-public repair ledger |
| Final pre-public lineage | exact v26 migration/repair ledger, accepted SQLite marker 0 or 26 | normal runtime conversion retired; explicit source-checkout recovery uses `scripts/history/pre-public-v26/ledger.json` plus `transition-pre-public-data.sh` | archived outside runtime after the human actual-library gate; preserve explicit offline recovery |
| Rich note transport | `content_format=tiptap-json`, transport `content_schema_version=1` | canonical rich root schema 1–3; schema 3 owns typed math + managed audio/video and is validated through the schema-3 wrapper | retain all supported rich readers |
| Interactive questions | `question-json` schema 1 | strict typed definition/evaluator; attempt checkpoint version 1 | retain |
| Canonical documents | canonical document schema 1 | current adapters normalize PDF/AnyDoc/image-set sources into this representation | retain |
| Knowledge/source evidence | content schema 1; source identity v1 plus document v2 and image-evidence v3 identities | older compatible identities remain readable and validation fails closed on unsupported identities | retain |
| Complete library backup | package format **4**; package readers 1–4 | embedded public SQLite snapshot admission is intentionally schema **10–30** today; schema-10/11 Tutor backup regression proves that floor is deliberate | retain support window until a separately reviewed backup-version change |
| `.studydeck` | writer v3 | readers v1/v2/v3; older versions are explicit interchange compatibility, not internal migration residue | retain |
| Generated `.ostudy.json` | Basic/Cloze v1; mixed typed-question v2 semantics | importer reads v1 and v2; v1 rejects interactive questions | retain both |
| Trusted-device package | writer v3 | explicit legacy v2 reader | retain |
| UI checkpoint | writer v2 | explicit v1 compatibility reader, normalized before current use | retain until a separate UI-checkpoint compatibility decision |
| Managed files | SQLite-owned card assets plus immutable document source/source-asset/page-render files | full backup owns required retained bytes; sync/studydeck have narrower explicit scopes | retain ownership; no orphan cleanup without ledger proof |
| Secrets | OS secure store keyed by application profile | production/dev namespaces are separate; secrets are excluded from SQLite/backups | retain |

## 106-C findings

1. **Schema 30 is the actual post-#104 registry.** Older stage documents naming 17/18/21 describe historical checkpoints, not current runtime.
2. **The public repair registry is not empty.** Three public repairs are active compatibility contracts. The pre-public transition clearing its old repair ledger does not authorize deleting the current repair framework.
3. **The backup schema-10 floor is intentional.** Historical Tutor backup tests construct real schema-10/11 snapshots and require them to remain admissible after later migrations.
4. **Rich-content docs were stale.** Current native admission supports rich-root schema 3 for math/media while preserving the transport-envelope version 1. Canonical docs are corrected in this increment.
5. **Generated-import docs were stale.** The importer supports both v1 and v2; interactive questions require v2. Canonical reference docs are corrected in this increment.
6. **The v26 runtime adoption path was retired in 106-D after the human gate.** Historical recovery remains explicit and staged; current application startup leaves non-public data unchanged.
7. **Public migrations 1–30 are current compatibility, not automatic cleanup targets.** Keep them unless 106-D demonstrates a concrete release/runtime benefit from a replacement baseline. If a replacement is justified, use an explicit new lineage/baseline transition with preservation receipts rather than rewriting known migration bytes.

## 106-D entry conditions

Before any retirement/consolidation commit:

- freeze an exact current schema-30 source head;
- generate/verify a canonical current-schema manifest and complete preservation receipt;
- test fresh creation, current reopen, schema-10 backup restore, v26 adoption, and current backup/restore;
- first decide whether a new pre-release baseline has a concrete benefit over retaining the already-supported v1 chain; if yes, design it as an explicit lineage transition rather than rewriting known migration checksums;
- provide an offline recovery path for intentionally restored historical backups;
- test the user's retained library only as an **isolated local copy**; no personal library leaves the user's machine;
- retire the old runtime transition only after the human lineage gate; preserve a tested explicit historical recovery path outside normal runtime.

## Archive versus retain rule

**Archive candidate:** code/data used only to recognize/convert the exact final pre-public v26 development lineage once the actual-library gate is complete.

**Retain:** public repair definitions, current-format validators, external interchange readers (`.studydeck`, generated JSON, Anki), backup compatibility, sync v2 reader, UI checkpoint v1 reader, rich schema 1–3 readers, and any compatibility path with a demonstrated supported input.

A historical name is not evidence of dead code. Retirement requires caller/fixture/package/restore evidence plus a replacement recovery path.
