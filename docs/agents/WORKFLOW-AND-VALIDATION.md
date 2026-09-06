# Agent workflow and validation

The canonical publishing procedure is [GitHub-direct development](GITHUB-DIRECT-DEVELOPMENT.md). Use it for normal ChatGPT work; this document describes change design and validation, not an alternative executor selection policy. Custom developer MCPs, Codex, observers, and orchestrators require an explicit current-task request. GitHub Actions never author product source, including manual maintenance workflows.

## Establish scope and evidence

Start from the exact current GitHub base or resumed PR head. Read the implementation, callers, tests, schemas, and relevant ADRs. Recent conversations describe what the user asked for, but do not prove what merged or passed.

Record stable `AC-##` acceptance criteria, explicit non-goals, and the affected ownership boundaries. Distinguish current behavior, the requested change, and the test that proves it. Do not silently weaken a criterion to fit the implementation. Do not make new product behavior part of a documentation refresh unless it is explicitly in scope.

For a resumed task, inspect the actual PR, remote ref, reviews, and exact-head runs. Enumerate unfinished criteria and unresolved failures. The [acceptance snapshot](EXECUTION-STATE.md) is useful progress metadata, not a source authority or branch lock.

## Publish meaningful increments

Use one task branch and one draft PR. Publish a useful first source/test/document increment, then coherent vertical changes through expected-parent Git-data commits and verified non-force updates. Do not create no-op commits or temporary ownership files as access probes.

A vertical increment includes its necessary callers, contracts, tests, and documentation. A shared Rust DTO change includes test constructors and deserializers. A migration includes its checksum registration. A removed module includes explicit replacements in verifiers and manifests. Do not weaken an architectural check because a rename exposed an incomplete refactor.

Keep dependency upgrades, unrelated formatting, infrastructure repair, and speculative refactors out of the task. A local scratch file, patch proposal, generated applicator, or transient CI tree is not published implementation.

## Freeze the candidate before terminal qualification

Treat terminal Linux/Android qualification as validation of a finished candidate, not as another discovery phase. Before requesting `all-full` or moving a PR into the state that triggers it, complete the implementation audit: compare the full remote diff with the original acceptance criteria, review maintained documentation for stale claims, reconcile architecture and user guidance, remove temporary cleanup, and finish the PR handoff/ledger facts that can be established without the terminal run.

Once terminal qualification starts, the candidate is frozen. While it is running, observe results and prepare failure diagnosis or handoff evidence; do not continue broad documentation searches, opportunistic cleanup, refactors, formatting, or "one more thing" edits. A failed gate can reopen implementation for the owning fix. A newly noticed non-blocking cleanup belongs in a follow-up change.

If a genuinely blocking prose defect is found after a green terminal run, classify it before editing. A root-level or `docs/` Markdown-only correction may be made as one coherent documentation commit; the new exact head must still pass static qualification, while previously passed expensive product gates may be reused only when the maintained product/toolchain fingerprint proves that no qualification-bearing input changed. Any source, test, workflow, configuration, asset, lockfile, or other non-neutral change invalidates that reuse and requires the applicable expensive qualification again.

## Validate the smallest useful boundary

Use [the command runbook](COMMAND-RUNBOOK.md) rather than inventing commands:

```bash
./scripts/check-changed.sh
./scripts/check-static.sh
./scripts/check-fast.sh
./scripts/check.sh
```

The changed-file helper is an iteration aid. Static verifies repository, documentation, policy, type, and format contracts. Fast adds frontend behavior tests and native compile/check coverage. Full is authoritative Linux/Tauri qualification; Android compile/package and emulator gates are additional platform evidence. The exact contents of each tier are the maintained scripts, not a remembered command summary.

A pure documentation change still needs link/policy validation and a truthful record of broader qualification. Static and fast use the exact source-tree fingerprint. Full Linux and Android use a conservative product fingerprint that excludes only root-level Markdown and Markdown under `docs/`; every other repository path is qualification-bearing by default. Android fingerprints also include the relevant Java/Android toolchain. Reuse is permitted only after the current exact source has passed static qualification, only when the applicable expensive-tier fingerprint is unchanged, and only when the workflow has explicitly enabled qualification reuse. `OSTUDY_FORCE_CHECK=1` disables reuse and forces fresh execution.

A source deletion requires checking production and test consumers, dynamic imports, platform compilation, and enforcing inventories. Do not claim a complete native application test from parsing, a frontend build, or mocked component tests.

All evidence belongs to the exact current workflow run and the source it checked. After another publication, an older green run remains historical evidence only; if the current run safely reuses an expensive result, report that reuse explicitly together with the original qualified source recorded in the cache. Never relabel an older run as though it executed the new commit.

## Consume failure evidence

Read the exact run's steps, bounded logs, qualification result, and available diagnostic artifacts. Match the tested SHA to the current head. Classify each independent actionable failure using [Failure triage](FAILURE-TRIAGE.md), fix the owning boundary, add a regression where practical, and run the focused reproduction before another broad gate.

One unchanged-head rerun is allowed only for evidence of a transient external cause. The same fingerprint twice requires a strategy change. Do not use sleeps, blanket retries, longer timeouts, no-op commits, PR-state toggles, permission expansion, skipped tests, or weakened assertions to manufacture a green run.

Do not ask the human to copy logs that the Connector can already read. An offline runner is a validation limitation, not a source-write failure. A failed optional MCP is not a reason to abandon the GitHub path.

## Readiness is not completion

`PR Contract` validates the acceptance snapshot independently of `Check`. Product qualification must not depend on a valid PR-body ledger. Review readiness may trigger required full qualification; it is therefore legitimate to record explained pending/not-run qualification at that transition. It is never legitimate to mark an unrun gate passed merely to trigger it.

Follow [Execution state](EXECUTION-STATE.md) for the exact schema and readiness rules. Before calling work fully qualified or complete, the required current-head gates must actually pass, criteria must be implemented or explicitly human-required, manual boundaries must be identified, and no known automated failure may be hidden. Keep a PR draft while implementation is incomplete or qualification is unresolved unless the human explicitly directs otherwise.

## Review and hand off

The broad implementation/documentation audit belongs before candidate freeze. After qualification, verify rather than reopen the scope: inspect the final remote changed-file list and diff for source identity and unintended files, reconcile the acceptance snapshot to the verified head, and confirm that qualification evidence matches the current run. Do not turn this final verification into another cleanup pass; defer non-blocking improvements instead of invalidating a qualified candidate.

Verify that the PR contains ordinary reviewable source, no secrets/personal data/build output, no unintended lockfile or migration changes, and no leftover applicators. Report changes grouped by purpose, exact base/branch/head/PR, actual check commands and run IDs, checks not run, manual acceptance, compatibility/rollback risks, and writer release. Provide the copyable local checkout/build/run/install path and task-specific observations from [Local testing and handoff](../development/local-testing-and-handoff.md).

The human reviews and normally squash-merges. Do not merge, tag, delete shared branches, or push to `main` without explicit authorization. Post-merge cleanup must preserve unmerged local work; a squash merge does not make `git branch -d` universally succeed and does not justify automatically forcing deletion.
