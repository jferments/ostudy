# PR #106 completion record — production baseline and release readiness

Status: **squash-merged on 2026-09-26** as `9ef91e79346b6d0d0ace983da3c1583303f15a43`. The qualified source candidate was `d1d3b176a2542e7f1b89408c8a6bf88b7d84b36b`; both commits have tree `45f939843b55aa2b9813536b423bf0d83636d382`. PR #106 is no longer an active implementation branch or a dependency waiting for PR #105.

This record closes the post-#104 production/development isolation, contributor tooling, baseline/conversion, backup/restore, packaging and acceptance tranche. The [original implementation plan](../../history/pr-106-implementation-plan.md) is preserved as historical evidence, not instructions to repeat its completed stages. Current owners are [library restore](../../architecture/library-restore.md), [the durable-data inventory](../post-104-data-contract-audit.md), and [local testing and handoff](../local-testing-and-handoff.md).

PR #105 remains deferred: responsive lessons, narration/TTS, PowerPoint, STT, voice cloning and speech grading were not implemented here. Future durable changes extend the release baseline through reviewed forward migrations; they do not restart schema consolidation.

## Delivered boundary

Production retains `io.github.jferments.ostudy`. Maintained development and Android debug use `io.github.jferments.ostudy.dev`, with separate ordinary storage and credential namespaces. Tests use explicitly isolated storage. A worktree alone is not a new data profile; multiple development worktrees can still share the development library. Changing XDG directories alone does not create a separate production keyring namespace.

Fresh libraries use `ostudy-release-v1`, schema **1**, from `src-tauri/schema/0001_release_baseline.sql`. Immutable public-v1 migrations **1–30** and repairs **1–3** remain compatibility input. Supported public-v1 complete-backup inputs at schemas **10–30** are admitted, normalized and preservation-checked before atomic release-lineage conversion. The explicit final-v26 historical recovery route stays outside ordinary startup. Do not delete migration ledgers, alter checksums or relabel installed data by hand.

Complete-library backup writes format **4** and retains supported earlier package readers. Restore validates a staged package and activates it in the running native process, with journaled filesystem replacement, coordinated SQLite sidecars, request-generation fencing, durable completion receipts and UI refresh. Startup resolves interrupted activation. Saved OS credentials are neither included in the backup nor deleted by restore; successful live replacement clears the process's entire session-only credential map.

Process ownership uses a retained lock descriptor and permanent lock file. The final Android correction uses the native nonblocking lock adapter; desktop locking remains unchanged. Never unlink `library-owner.lock` to bypass ownership. Normal Debian removal owns program/system-integration files, not learner XDG data. Explicit destructive purge is separate from ordinary removal and was not the acceptance procedure.

## Acceptance boundaries

| Identifier | Completed implementation/acceptance boundary | Principal evidence owners |
| --- | --- | --- |
| AC-01 | Current baseline, supported conversion and fail-closed unsupported input | `database/release.rs`, `database/entry_tests.rs` |
| AC-02 | Production/development/test isolation; disposable installed-copy remove/reinstall preservation | `application_profile.rs`, package-data verifiers, human acceptance in PR #106 |
| AC-03 | Repository-owned setup/doctor/build/run/check tooling, independent of private runner registration | `scripts/ostudy-dev`, `scripts/verify-development-tooling.mjs` |
| AC-04 | Frozen-source Linux/package/Android qualification and disposable installed-copy acceptance | Check 36254323670 attempt 2; human acceptance in PR #106 |
| AC-05 | Journaled, recoverable activation and exclusive process ownership | `library_backup/activation_tests.rs`, `library_lock_tests.rs`, `platform/file_lock_tests.rs` |
| AC-06 | Same-process restore, stable Database clones, stale-request refusal, receipts and credential cleanup | `library_backup/live_tests.rs`, `library_session_tests.rs`, `ai/library_restore_secret_tests.rs`, frontend transport/settings tests |
| AC-07 | Release documentation, cleanup dispositions and bounded diagnostics | documentation/current-data/cleanup verifiers; documentation continuation in PR #108 |
| AC-08 | Mechanically derived release-v1 baseline and typed preservation converter, with immutable historical readers | `database/release_baseline_tests.rs`, `database/release.rs`, historical recovery tests |

Native paths in this table are relative to `src-tauri/src/`. A passed gate does not establish that every later audit observation is fixed. The [post-release-baseline hardening register](../post-106-hardening.md) explicitly separates confirmed observations, qualified findings and future implementation.

## Qualification and human evidence

[Check 36254323670, attempt 2](https://github.com/jferments/ostudy/actions/runs/36254323670/attempts/2) tested exactly `d1d3b176a2542e7f1b89408c8a6bf88b7d84b36b`. Its named Static, Linux full, production Debian package, Android compile/package and Android emulator smoke steps succeeded. The standalone Fast step was **skipped** in that all-full run; do not describe it as a separately executed passing step. The PR's evidence records the Linux/nested/reuse coverage separately. The post-squash `main` Check is [36257586166](https://github.com/jferments/ostudy/actions/runs/36257586166); its coverage belongs to its own step evidence, not an inference from the word success.

[PR #106](https://github.com/jferments/ostudy/pull/106) records completed disposable installed-copy acceptance: historical-backup restoration without native PID replacement, a post-restore write surviving reopen, independent production/development libraries, and actual ordinary package removal/reinstallation preserving learner data. This is recorded human evidence, not a new execution by the documentation author.

The protected school-library release-v1 cutover was **not** part of that completed disposable acceptance. An earlier read-only public-v1/schema-30 audit is not evidence of its conversion. Physical-device, live-provider, visual/accessibility and public signing/distribution acceptance likewise are not implied by the automated run or by merge.

## Completed delivery stages

### 106-A — development / production / test profile isolation

Implemented explicit profile/storage/credential boundaries and development guards. Current commands and cautions belong in [local testing](../local-testing-and-handoff.md).

### 106-B — contributor setup, build, doctor, and local test tooling

Implemented repository-owned contributor wrappers. See [the command runbook](../../agents/COMMAND-RUNBOOK.md); do not reproduce private CI registration or caches for ordinary development.

### 106-C — post-#104 schema and durable-data audit

Completed the [inventory](../post-104-data-contract-audit.md). The terminal public-v1 schema is compatibility evidence, not the current writer.

### 106-D — production baseline and gated archival of obsolete development migration machinery

Implemented release-v1 and staged preservation conversion after the recorded lineage gate. Explicit historical recovery remains; normal startup does not adopt arbitrary pre-public databases.

### 106-E — backup, upgrade, uninstall, and learner-data safety

Implemented and exercised the backup/restore/package boundaries. Retain original backups, prior coherent libraries and restore journals; cleanup is not permission to erase recovery evidence.

### 106-F — proven-dead code, dependencies, documentation, and bounded workflow cleanup

Closed the release cleanup scope. PR #108 completes documentation alignment; runtime audit findings remain separately scoped in the hardening register.

### 106-G — production package and release boundary required for local use

Qualified the production Debian/Ubuntu package boundary. A signed multi-platform public distribution, Flathub/F-Droid publication and the website remain separate projects.

### 106-H — freeze and exact-head terminal qualification

Completed on the source SHA and run recorded above. Preserve the freeze boundary; do not reopen the merged release tranche for incidental cleanup.

### 106-I — human production/dev isolation and release-readiness acceptance

Disposable installed-copy acceptance is recorded complete. Valuable-library cutover is a separate deliberate operation, not a prerequisite for documentation work and not an automated cleanup action.

## Post-merge handoff

PR #108 must integrate the actual squash commit with a reviewed non-force merge, target `main`, and retain only its intended documentation diff. Its earlier runtime terminology changes are deferred, not silently merged into #106. Keep both histories; do not force-push, replay the old #106 branch over main, or infer acceptance for a new SHA from its ancestry alone.

Use the [hardening register](../post-106-hardening.md) for the next bounded work after documentation. No branch cleanup, package cleanup or documentation action authorizes deleting learner libraries, backups, restore transactions, lock files, runner caches, or unrelated branches. Human review still controls merge.
