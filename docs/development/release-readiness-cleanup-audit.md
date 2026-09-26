# PR #106 release-readiness cleanup audit

Status: active cleanup ledger for PR #106 / 106-F. This is a deletion **decision record**, not a license to remove anything whose compatibility or recovery consumer is still live.

## Disposition ledger

| Candidate | Consumer/caller evidence | Disposition |
| --- | --- | --- |
| retired Rust final-v26 runtime transition + exact v26 ledger | human lineage gate confirmed the retained production library had already reached terminal public-v1 schema 30 before the release baseline cutover, and the development library was absent | **retire from runtime**; preserve the exact ledger and explicit staged recovery tool under historical source tooling |
| `scripts/restore_pre_public_backup.py` | explicit recovery path for users who previously ran the old reset workflow | **retain** as offline historical recovery even after normal runtime v26 adoption is retired |
| `scripts/reset-pre-public-data.sh` | explicit user-confirmed “start empty” recovery action and startup failure guidance | **retain**; it is destructive by design but not ordinary uninstall/setup |
| public-v1 migrations 1–30 | legacy `ostudy-public-v1` reader, startup normalization/conversion, backup schema-10–30 admission, migration fixtures | **retain as compatibility evidence**; fresh 0.1 libraries use the release-v1 baseline instead |
| public-v1 repairs 1–3 | legacy startup normalization and checksum validation before release-lineage conversion | **retain as compatibility evidence**; their effects are folded into the release baseline |
| complete-backup readers 1–4 / public-v1 SQLite schema 10–30 / release-v1 schema 1 | disaster-recovery compatibility and regression fixtures | **retain** |
| `.studydeck` v1/v2 readers, generated JSON v1/v2, sync v2 reader, UI checkpoint v1 reader | supported external/interchange or local compatibility inputs | **retain** |
| PR #103/#104/#105 roadmap prose describing old ownership | no runtime consumer; current docs must not describe superseded lifecycle as current | **correct current docs**, retain historical stage detail where explicitly labeled |
| old split-runner/history/performance records | provenance/history only and clearly labeled | **retain as history**, do not delete merely to shorten docs |
| unused product/runtime dependency | no evidence of an exclusive dead dependency was established by this audit | **no deletion** |

## Cleanup rules applied

- Current guides describe the actual post-#104 / active-#106 ownership state.
- Historical stage documents may preserve old schema numbers or implementation facts only when the text clearly identifies the historical checkpoint.
- Compatibility readers remain because they have named supported inputs and tests.
- A source path is not “dead” merely because ordinary new data no longer writes that format.
- No dependency is removed without proving its remaining compile/runtime/packaging/test consumers are absent.

## Completed deletion gate

The only meaningful runtime archival candidate found in this tranche was the exact final pre-public-v26 automatic adoption path.

The human gate completed before retirement:

1. `./scripts/ostudy-dev library-status --verify` classified the retained production library as the terminal supported public-v1 schema 30 with integrity and foreign keys OK before release-v1 conversion was introduced;
2. the development profile had no retained library requiring v26 adoption;
3. production/development profile isolation was already established;
4. explicit staged recovery remains available for an intentionally restored historical v26 archive.

Therefore PR #106 removes the v26 converter from normal Rust startup/package paths while retaining the immutable ledger and tested explicit recovery tooling. It also makes `ostudy-release-v1` / schema 1 the fresh 0.1 runtime baseline while preserving public-v1 migrations/repairs as immutable compatibility inputs and retaining supported interchange readers.
