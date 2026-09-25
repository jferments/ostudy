# PR #106 release-readiness cleanup audit

Status: active cleanup ledger for PR #106 / 106-F. This is a deletion **decision record**, not a license to remove anything whose compatibility or recovery consumer is still live.

## Disposition ledger

| Candidate | Consumer/caller evidence | Disposition |
| --- | --- | --- |
| `src-tauri/src/startup/pre_public_transition.rs` + exact v26 ledger | normal startup still calls it when a retained database lacks the public lineage; startup smoke and restore helpers exercise it | **retain pending human library-status/adoption gate**; archive only after the local retained library is proven current/adopted |
| `scripts/restore_pre_public_backup.py` | explicit recovery path for users who previously ran the old reset workflow | **retain** as offline historical recovery even after normal runtime v26 adoption is retired |
| `scripts/reset-pre-public-data.sh` | explicit user-confirmed “start empty” recovery action and startup failure guidance | **retain**; it is destructive by design but not ordinary uninstall/setup |
| public migrations 1–30 | current `ostudy-public-v1` reader, startup upgrade, backup schema-10–30 admission, migration fixtures | **retain**; not dead pre-release clutter |
| public repairs 1–3 | current startup repair ledger and checksum validation | **retain** |
| complete-backup readers 1–4 / SQLite schema 10–30 | disaster-recovery compatibility and regression fixtures | **retain** |
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

## Remaining deletion gate

The only meaningful runtime archival candidate found in this tranche is the exact final pre-public-v26 automatic adoption path. Its removal is intentionally blocked until:

1. `./scripts/ostudy-dev library-status --verify` classifies the retained local library safely;
2. any valuable pre-public library is adopted on an isolated local copy and backed up;
3. the current production/dev profile split is established;
4. offline recovery remains available for an intentionally restored historical backup.

Until that human gate exists, keeping the transition is safer and less complex than pretending it is dead.
