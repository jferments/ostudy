# GitHub outage and PR #54 workflow audit — August 17, 2026

## Scope

This audit reviews the work added to PR #54 after exact head:

```text
33cb7d14f3490f587c29f2435cd2a9352e8ff759
```

The review was requested after a broad GitHub service incident overlapped with connector reads, repository writes, pull-request events, and GitHub Actions qualification for the branch `agent/pre-public-platform-hardening`.

The purpose is to separate:

1. reproducible oStudy defects and legitimate engineering improvements;
2. mistakes in the submitted changes or test maintenance;
3. GitHub Connector capability boundaries;
4. GitHub service-incident effects;
5. outage-driven workarounds that should not remain in the repository.

## Official incident timeline

GitHub's official incident, [Incident with GitHub.com](https://www.githubstatus.com/incidents/zkxwbgr0cnmx), began at approximately **13:40 UTC on August 17, 2026**.

The official updates reported:

- API Requests degraded at 13:41 UTC;
- Actions degraded at 13:42 UTC;
- Webhooks degraded at 13:44 UTC;
- Issues and Pull Requests degraded shortly afterward;
- Pull Requests, Issues, Actions, and API Requests later reached major-outage status;
- Git Operations became degraded;
- approximately 20% error rates affected web and API traffic;
- archive and raw repository downloads experienced approximately 50% errors.

At the time this audit was written, the incident remained under investigation. The affected surfaces overlapped directly with the failing or inconsistent operations observed while PR #54 was being modified.

## Evidence classification

### GitHub service-incident evidence

The following observations were consistent with the official incident and were not reliable evidence about oStudy code:

- intermittent `502` and other `5xx` responses;
- timeouts and failed archive/raw-content reads;
- inconsistent pull-request event handling;
- missing or delayed Actions visibility;
- repository and pull-request API calls that failed transiently while unrelated calls succeeded;
- attempts to observe a workflow immediately after branch updates during the outage window.

These failures should have triggered a status-page check and a pause in GitHub-control-plane experimentation.

### Connector capability evidence

A repeatable `403 Resource not accessible by integration` from GitHub Actions endpoints while ordinary repository content reads and writes succeeded is best classified as a connected GitHub App permission or endpoint-coverage limitation.

That result does **not** prove that GitHub Actions did not run. It also does not justify changing the repository workflow token or publishing duplicate legacy status contexts solely so one connector can read them.

### Repository-defect evidence

Deterministic local command failures and reproducible runtime behavior remained valid regardless of the GitHub outage. Those findings were retained and tested.

## Change-by-change audit

| Change after `33cb7d1` | Decision | Reason |
| --- | --- | --- |
| Diagnostic failure-summary narrowing to actual failed Vitest files | Keep | A dedicated regression failed locally because earlier passing filenames were included in the replay command. |
| Selection of the newest application session before severity filtering | Keep | A dedicated regression failed locally because a stale process error displaced the current process timeline. |
| Structured diagnostic artifacts, focused `reproduce.sh`, bounded retention, and nested-capture deduplication | Keep | These improve failure attribution without depending on GitHub availability. |
| Startup compatibility boundary in `startup.rs` | Keep | The real built application panicked when opening retained pre-public data. This was a product/runtime defect, not an API symptom. |
| Lineage inspection before SQLite WAL/durability configuration | Keep | The old order could write SQLite state before rejecting an unsupported database, contradicting the non-modification promise. |
| On-disk byte-for-byte legacy-database rejection test | Keep | It verifies the durable safety claim against a real SQLite file, including absence of WAL/SHM sidecars. |
| Built-binary `smoke-startup-compatibility.sh` in the full Linux gate | Keep | Earlier unit tests covered legacy DB logic and WebDriver covered fresh startup, but not the full binary against legacy data. |
| `100755` executable mode for the startup smoke | Keep | The missing mode was a submission error caught by static validation. The correction is required. |
| Worktree mutation guard and regression that creates `?? Native` | Keep | Qualification should identify the exact boundary that dirties the repository. This is independent of GitHub connectivity. |
| Android restoration of Tauri-rewritten `BuildTask.kt` and `AndroidManifest.xml` | Keep | The tracked source rewrite was observed locally and is a concrete source-tree leak. |
| Relocating startup assertions in device-sync and external-file verifiers from `lib.rs` to `startup.rs` | Keep | The startup refactor moved the implementation; the static verifiers had stale source-location assumptions. |
| Static qualification as the first substantive Actions boundary | Keep | It stops expensive KVM, Rust, Linux, and Android work when repository contracts already fail. |
| Extra legacy `ostudy/static` and `ostudy/qualification` commit-status publication | Remove | This was added to compensate for missing Actions visibility during the incident, expanded token permissions, duplicated GitHub state, and did not improve product qualification. |
| `statuses: write` permission in the normal qualification workflow | Remove | Normal qualification needs only read-only repository contents. The expanded permission existed only for the removed status mirror. |
| No-op commits, PR close/reopen, draft/ready toggles, and unrelated file edits intended to retrigger GitHub | Do not repeat | These are control-plane experiments, not oStudy changes. They add history noise and can obscure whether a real write landed. |
| Temporary README edit used while testing event propagation | No net content change | The README was restored to its exact previous blob. The history noise will disappear through squash merge. |
| Keeping PR #54 in draft while exact-head qualification is unavailable | Keep as process state | Draft state accurately communicates that the final head is not yet qualified. It must not be toggled repeatedly to force events. |

## Mistakes that were not caused by the outage

The outage made remote diagnosis harder, but it did not cause the following mistakes:

1. The new startup smoke was initially committed as `100644` instead of `100755`.
2. Static verifiers still searched `lib.rs` after startup work moved into `startup.rs`.
3. The earlier native test matrix did not launch the built application against a real legacy database.
4. The first diagnostic analyzer implementation selected severity before application-session identity.
5. The first Vitest replay implementation treated every mentioned test file as failed.
6. The handoff occurred before exact-head static qualification had been observed.

These were engineering and workflow errors. They remain addressed through direct regression coverage rather than being attributed to GitHub.

## Reasoning errors caused or amplified by the outage

The main reasoning failure was treating lack of remote observability as if it were a repository design problem.

Specifically:

- Missing Actions data was conflated with a missing workflow run.
- An empty legacy commit-status response was treated as a useful proxy for Actions state even though Actions uses check-runs.
- A connector-specific Actions `403` was mixed together with broader incident failures.
- Workflow permissions were expanded so the workflow could mirror its result into an API surface visible to the connector.
- PR state and unrelated repository content were changed to provoke new events while GitHub Webhooks, Pull Requests, API Requests, and Actions were degraded.

The correct response should have been:

1. preserve the exact branch head;
2. check GitHub Status;
3. classify the failing surface;
4. avoid nonessential GitHub mutations;
5. continue only independently verifiable work;
6. wait for ordinary exact-head qualification after service recovery.

## Permanent workflow rules

The maintained Connector and CI documents now require:

- an official GitHub Status check when broad API/Actions/webhook failures occur;
- separate reporting of publication state and validation state;
- no inference that an Actions run is absent merely because a connector cannot read check-runs;
- no status-mirroring API calls or expanded workflow permissions for connector convenience;
- no no-op commits, unrelated edits, PR close/reopen cycles, or draft toggling to force events during an incident;
- one recorded exact head SHA throughout the incident;
- PR draft state until normal post-incident qualification completes;
- normal static-first, full Linux, Android package, and emulator gates after recovery.

## Final audit conclusion

The substantive product and test changes made after `33cb7d1` remain justified. They address locally reproduced defects in diagnostics, startup compatibility, database safety, generated Android source restoration, and static-test architecture.

The outage-specific legacy commit-status mirror and its `statuses: write` permission have been removed. No other retained behavior depends on GitHub being healthy or on one connector's API coverage.

PR #54 should be squash-merged only after GitHub service recovery and successful exact-head qualification through the ordinary read-only Actions workflow.
