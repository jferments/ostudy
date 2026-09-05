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

## Validate the smallest useful boundary

Use [the command runbook](COMMAND-RUNBOOK.md) rather than inventing commands:

```bash
./scripts/check-changed.sh
./scripts/check-static.sh
./scripts/check-fast.sh
./scripts/check.sh
```

The changed-file helper is an iteration aid. Static verifies repository, documentation, policy, type, and format contracts. Fast adds frontend behavior tests and native compile/check coverage. Full is authoritative Linux/Tauri qualification; Android compile/package and emulator gates are additional platform evidence. The exact contents of each tier are the maintained scripts, not a remembered command summary.

A pure documentation change still needs link/policy validation and a truthful record of broader qualification. Any source deletion requires checking production and test consumers, dynamic imports, platform compilation, and enforcing inventories. Do not claim a complete native application test from parsing, a frontend build, or mocked component tests.

All evidence belongs to the exact tested commit. After another publication, old green runs remain historical evidence only. Report an actual cache reuse as such, and deliberately use `OSTUDY_FORCE_CHECK=1` only when a fresh execution is needed.

## Consume failure evidence

Read the exact run's steps, bounded logs, qualification result, and available diagnostic artifacts. Match the tested SHA to the current head. Classify each independent actionable failure using [Failure triage](FAILURE-TRIAGE.md), fix the owning boundary, add a regression where practical, and run the focused reproduction before another broad gate.

One unchanged-head rerun is allowed only for evidence of a transient external cause. The same fingerprint twice requires a strategy change. Do not use sleeps, blanket retries, longer timeouts, no-op commits, PR-state toggles, permission expansion, skipped tests, or weakened assertions to manufacture a green run.

Do not ask the human to copy logs that the Connector can already read. An offline runner is a validation limitation, not a source-write failure. A failed optional MCP is not a reason to abandon the GitHub path.

## Readiness is not completion

`PR Contract` validates the acceptance snapshot independently of `Check`. Product qualification must not depend on a valid PR-body ledger. Review readiness may trigger required full qualification; it is therefore legitimate to record explained pending/not-run qualification at that transition. It is never legitimate to mark an unrun gate passed merely to trigger it.

Follow [Execution state](EXECUTION-STATE.md) for the exact schema and readiness rules. Before calling work fully qualified or complete, the required current-head gates must actually pass, criteria must be implemented or explicitly human-required, manual boundaries must be identified, and no known automated failure may be hidden. Keep a PR draft while implementation is incomplete or qualification is unresolved unless the human explicitly directs otherwise.

## Review and hand off

Inspect the final remote changed-file list and diff. Verify that the PR contains ordinary reviewable source, no secrets/personal data/build output, no unintended lockfile or migration changes, and no leftover applicators. Reconcile the acceptance snapshot to the verified head.

Report changes grouped by purpose, exact base/branch/head/PR, actual check commands and run IDs, checks not run, manual acceptance, compatibility/rollback risks, and writer release. Provide the copyable local checkout/build/run/install path and task-specific observations from [Local testing and handoff](../development/local-testing-and-handoff.md).

The human reviews and normally squash-merges. Do not merge, tag, delete shared branches, or push to `main` without explicit authorization. Post-merge cleanup must preserve unmerged local work; a squash merge does not make `git branch -d` universally succeed and does not justify automatically forcing deletion.
