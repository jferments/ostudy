# Contributing to ostudy

Contributions should start with a clear learner problem and end with reviewable source and evidence for the exact commit tested. ostudy stores personal learning material and long-lived review history; correctness, recoverability, accessibility, and privacy are part of the feature, not later cleanup.

Start with the [documentation index](docs/README.md) and [current architecture](docs/architecture/overview.md). Automated contributors also follow [AGENTS.md](AGENTS.md); Claude Code reads it through [CLAUDE.md](CLAUDE.md).

## Report a useful problem

Include the application version or exact commit, OS/device/architecture, initial state, action, expected and actual result, and the smallest reproducible sequence. Say whether it survives restart and name any failing command. For document/AI problems, include source format, scanned versus native text, provider family/model, visible phase, and safe error—not the original private document by default.

Use a synthetic or redacted example whenever possible. Do not attach keys, authorization headers, private course/patient/employer material, live SQLite libraries, operating-system secret data, or unreviewed diagnostic directories. Local diagnostic exports are bounded and privacy-conscious, but metadata such as filenames can still be sensitive. Review them before sharing.

For a feature, explain the workflow, current workaround, desired first version, and non-goals. Identify data, accessibility, platform, privacy, and compatibility effects. An AI feature must state which source data leaves the device, who chooses the provider, and what deterministic native behavior remains authoritative. Check the [roadmap](docs/roadmap.md) without treating planned work as already implemented.

## Set up and test-drive

For first-time Ubuntu/Debian development setup:

```bash
./scripts/ostudy-dev setup
./scripts/ostudy-dev doctor
./scripts/ostudy-dev run
```

The repository-owned `doctor` command verifies ordinary local build requirements. Use `./scripts/ostudy-dev doctor --native-smoke` only when you also need WebDriver/native-smoke prerequisites. Neither path requires the repository owner's self-hosted runner registration, GitHub Actions environment, signing credentials, or runner VM configuration.

For an already configured checkout after source changes:

```bash
./scripts/build.sh
./run-app.sh
```

The build produces a staged standalone **oStudy Dev** app without release bundles. Maintained desktop development uses `io.github.jferments.ostudy.dev`, separate from the production `io.github.jferments.ostudy` library and credential namespace. The launcher verifies its source fingerprint, development-profile stamp, and binary checksum; it does not rebuild or start Vite. For native hot reload, run `pnpm install --frozen-lockfile` and then `pnpm desktop`; it also runs as `io.github.jferments.ostudy.dev`. Do not run bare `pnpm tauri dev`: that debug build resolves the production identifier and startup refuses it. `pnpm dev` and `pnpm build` are frontend operations.

Linux and Android are the maintained build/test paths. See [installation](docs/installation/README.md), [Android setup](docs/installation/android.md), and [local testing/handoff](docs/development/local-testing-and-handoff.md). The maintained desktop development profile is isolated from production, but a new source worktree does not create another independent development profile: multiple worktrees can still address the same `io.github.jferments.ostudy.dev` library. Use temporary XDG/application storage for destructive, migration, restore, and parallel-worktree tests; never silently clear/uninstall a valuable desktop or Android library to make a test pass.

## Design a focused change

Inspect current implementation, callers, tests, schemas, migrations, transfer formats, and relevant ADRs. Write down what should change and the explicit non-goals. Identify automated evidence and the remaining human-only boundaries before implementation.

Reuse existing services, controllers, typed IPC, platform ports, and UI primitives. The UI can consume a bounded read model, but its current layout must not become the durable data model. Rust owns validation, transactions, scheduling, history, and provider boundaries. A new persistent representation needs migration, backup/restore, sync-scope, deletion/recovery, and privacy review.

Preserve released migrations/checksums and supported durable readers. Obsolete internal commands and unmounted duplicate screens do not gain permanent compatibility status merely because they once existed. Trace real consumers and move regression coverage to the active owner before removal. Add an ADR for a durable cross-module decision.

## Branch and pull request

Use one short-lived branch and one PR, never direct development on `main`. Branch from current `main`, commit and test in a local checkout, push the branch, and open a draft PR early. Continue there for ordinary repairs. Exactly one writer owns a branch at a time.

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

The automatic **Check** workflow checks out the actual source head and qualifies it. Draft PRs run the fast tier; ready PRs, pushes to `main` and explicit full dispatches run full Linux and Android qualification on the single self-hosted runner, so mark a PR ready only when it is complete. Description edits do not run product tests.

A green result belongs to its exact tested commit, not indefinitely to a branch. See [Testing and CI](docs/development/testing-and-ci.md) for current mechanics.

## Repair failures without weakening the contract

Start from the failure summary at the end of the `check` job log, which names the failing step and a focused local reproduction. Verify the tested commit. Classify product, test-contract, environment, service-incident, and manual boundaries separately. Fix the smallest failed boundary and batch independent actionable findings coherently before rerunning broader gates.

One unchanged-head rerun is appropriate only with transient-cause evidence. The same fingerprint twice requires a strategy change. No no-op commits, permission expansion, PR-state toggles, sleeps, longer timeouts, skipped tests, or weakened assertions merely to obtain green CI.

## Review, handoff, and merge

Inspect the final diff for scope, secrets, generated output, unrelated formatting, stale instructions, and missing regression coverage. Keep the PR description, which follows the [pull request template](.github/pull_request_template.md), current with each push.

The description covers what changes for a learner, how it works, the checks that ran and on which commit, unrun checks and reasons, known failures, data and compatibility effects, and short native build/run/Android install commands with the behavior to inspect. The [handoff guide](docs/development/local-testing-and-handoff.md) has those commands.

Do not make the human rerun a full matrix merely to try the UI, and do not claim a live-provider, camera, physical-device, or manual test that never happened. Use precise terms: pushed, inspected, passed, failed, not run, and human-required.

The normal final action is human **Squash and merge** after required evidence and review. Agents must not merge, tag, force-push, delete shared branches, or push to main without an explicit request. A revert of source does not necessarily reverse a database migration; preserve a compatible backup and prefer reviewed forward repair where required.
