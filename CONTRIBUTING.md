# Contributing to ostudy

Contributions should start with a clear learner problem and end with reviewable source and evidence for the exact commit tested. ostudy stores personal learning material and long-lived review history; correctness, recoverability, accessibility, and privacy are part of the feature, not later cleanup.

Start with the [documentation index](docs/README.md) and [current architecture](docs/architecture/overview.md). Automated maintainers also follow [AGENTS.md](AGENTS.md) and the [GitHub-direct procedure](docs/agents/GITHUB-DIRECT-DEVELOPMENT.md).

## Report a useful problem

Include the application version or exact commit, OS/device/architecture, initial state, action, expected and actual result, and the smallest reproducible sequence. Say whether it survives restart and name any failing command. For document/AI problems, include source format, scanned versus native text, provider family/model, visible phase, and safe error—not the original private document by default.

Use a synthetic or redacted example whenever possible. Do not attach keys, authorization headers, private course/patient/employer material, live SQLite libraries, operating-system secret data, or unreviewed diagnostic directories. Local diagnostic exports are bounded and privacy-conscious, but metadata such as filenames can still be sensitive. Review them before sharing.

For a feature, explain the workflow, current workaround, desired first version, and non-goals. Identify data, accessibility, platform, privacy, and compatibility effects. An AI feature must state which source data leaves the device, who chooses the provider, and what deterministic native behavior remains authoritative. Check the [roadmap](docs/roadmap.md) without treating planned work as already implemented.

## Set up and test-drive

For first-time Ubuntu/Debian development setup:

```bash
./scripts/setup-ubuntu.sh
./run-app.sh
```

For an already configured checkout after source changes:

```bash
./scripts/build.sh
./run-app.sh
```

The build produces a staged standalone debug app without release bundles. The launcher verifies its source fingerprint and binary checksum; it does not rebuild or start Vite. `pnpm desktop` is native hot reload, while `pnpm dev` and `pnpm build` are frontend operations.

Linux and Android are the maintained build/test paths. See [installation](docs/installation/README.md), [Android setup](docs/installation/android.md), and [local testing/handoff](docs/development/local-testing-and-handoff.md). A new source worktree does not isolate app data. Use temporary library storage for destructive, migration, and restore tests; never silently clear/uninstall a valuable Android library to make a test pass.

## Design a focused change

Inspect current implementation, callers, tests, schemas, migrations, transfer formats, and relevant ADRs. Define stable `AC-##` acceptance criteria and explicit non-goals. Identify automated evidence and the remaining human-only boundaries before implementation.

Reuse existing services, controllers, typed IPC, platform ports, and UI primitives. The UI can consume a bounded read model, but its current layout must not become the durable data model. Rust owns validation, transactions, scheduling, history, and provider boundaries. A new persistent representation needs migration, backup/restore, sync-scope, deletion/recovery, and privacy review.

Preserve released migrations/checksums and supported durable readers. Obsolete internal commands and unmounted duplicate screens do not gain permanent compatibility status merely because they once existed. Trace real consumers and move regression coverage to the active owner before removal. Add an ADR for a durable cross-module decision.

## Branch and publication

Use one short-lived branch and one PR, never direct development on `main`. Read the exact current base, publish a meaningful first source/test/document increment, and open a draft PR. Continue there for ordinary repairs.

Human contributors may use an authenticated local checkout. Normal ChatGPT tasks use the **GitHub Connector as the sole publisher**, with expected-parent Git-data commits, non-force ref updates, and remote verification. Exactly one writer owns a branch. Codex and custom developer MCPs/workspace/orchestrator/observer services require explicit authorization for the current task; an old deployment record is not a selection policy.

The PR must contain actual source, not an unpublished patch, task-specific applicator, finalizer, or transient runner-only tree. **All Actions workflows are source-read-only**, including manual maintenance. They may verify/build/test/package source and emit evidence, but must not stage, author, format-rewrite, commit, or push the reviewed source. See [Git and PR workflow](docs/development/workflow.md).

## Validate at the right layer

```bash
./scripts/check-changed.sh
./scripts/check-static.sh
./scripts/check-fast.sh
./scripts/check.sh
```

Use focused tests and the cheapest useful tier while editing, then required final-head qualification. The changed-file helper is an aid, not a certificate. Static parsing or mocked component tests do not establish native Tauri behavior. Android compile/package and emulator smoke are separate platform gates.

Documentation changes require documentation/repository checks and source review of claims. Removing source also requires its relevant compile/behavior/ownership checks. No tool proves all prose true or all code reachable.

The automatic **Check** workflow checks out the actual source head and qualifies it. **PR Contract** validates the acceptance metadata separately; a malformed PR body does not prevent product qualification. Draft automatic Check is fast; ready PRs and explicit full dispatches provide full platform qualification. Description edits do not run product tests.

Review readiness and completed qualification are different states. Never mark pending tests passed merely to trigger them. A green result belongs to its exact tested SHA, not indefinitely to a branch. Follow [Execution state](docs/agents/EXECUTION-STATE.md) and [Testing and CI](docs/development/testing-and-ci.md) for current mechanics.

## Repair failures without weakening the contract

Read the exact run's steps, bounded logs, and available machine-readable qualification/failure artifacts. Verify the tested SHA. Classify product, test-contract, environment, Connector, service-incident, and manual boundaries separately. Fix the smallest failed boundary and batch independent actionable findings coherently before rerunning broader gates.

One unchanged-head rerun is appropriate only with transient-cause evidence. The same fingerprint twice requires a strategy change. No no-op commits, permission expansion, PR-state toggles, sleeps, longer timeouts, skipped tests, or weakened assertions merely to obtain green CI. A runner queue is not evidence that source publication failed.

## Review, handoff, and merge

Inspect the final diff for scope, secrets, generated output, unrelated formatting, stale instructions, and missing regression coverage. Keep the PR's acceptance snapshot aligned with the verified remote head and actual evidence; the snapshot is not a source writer, lock, or job scheduler.

The handoff must include the PR/base/branch/head, changed areas, acceptance/non-goals, actual checks and tested SHAs, unrun checks and reasons, manual acceptance, known failures, compatibility/rollback risks, and writer release. Supply copyable exact-source checkout and short native build/run/Android install commands with the behavior to inspect. The [handoff guide](docs/development/local-testing-and-handoff.md) provides that contract.

Do not make the human rerun a full matrix merely to try the UI, and do not claim a live-provider, camera, physical-device, or manual test that never happened. Use precise terms: published, inspected, passed, failed, not run, and human-required.

The normal final action is human **Squash and merge** after required evidence and review. Agents must not merge, tag, force-push, delete shared branches, or push to main without an explicit request. A revert of source does not necessarily reverse a database migration; preserve a compatible backup and prefer reviewed forward repair where required.
