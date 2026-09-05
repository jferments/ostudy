# Git and pull-request workflow

ostudy uses one focused branch, one pull request, ordinary reviewable source, and evidence tied to an exact commit. The workflow exists to make development predictable, not to make the maintainer operate a chain of custom services.

## Source and execution ownership

For normal ChatGPT work, the authenticated **GitHub Connector is the sole source publisher** and GitHub Actions are the execution/qualification surface. Follow [GitHub-direct development](../agents/GITHUB-DIRECT-DEVELOPMENT.md). Codex, custom developer MCPs, workspaces, observers, exact-head services, and orchestrators require an explicit current-task request; availability or a historical deployment does not select them.

Human contributors may use their normal authenticated local Git checkout. That does not permit a ChatGPT session to alternate between competing writers. Exactly one writer owns a task branch. Local test-drive commands are for the human's machine and are not a second publishing workflow.

| Fact | Authority |
| --- | --- |
| Scope and acceptance criteria | Current human request and explicit agreements |
| Published source and branch/PR head | GitHub's exact remote commit |
| Automated qualification | Actual Actions run, gate, and tested SHA |
| Local uncommitted work | The real selected local checkout, when one exists |
| Manual acceptance | The report from the person who exercised the workflow |
| PR acceptance snapshot | A projection of the above, not an independent authority |

Do not invent workspace state for GitHub-only tasks. A PR-body ledger is not a lock, CI trigger, runner owner, or source repository. A failed optional service must not block the working GitHub path.

## Start or resume a task

For new work, read the current intended base, normally `main`, and record its full SHA. Create one short-lived `agent/<description>` branch from that commit. Never develop directly on `main`.

For resumed work, first read the actual PR, remote head, reviews, exact-head runs, and incomplete acceptance criteria. Do not restart from a remembered SHA or treat a previous conversational “done” as evidence. Preserve the same branch for ordinary repairs.

A human beginning local work should inspect `git status --short` before switching, fetch current refs, and create a branch/worktree from the named base. Preserve unrelated edits. The [local handoff guide](local-testing-and-handoff.md) gives a safe detached review-worktree procedure; no destructive reset or automatic stash is necessary.

## Specify a reviewable change

Record the observed problem, stable `AC-##` acceptance criteria, explicit non-goals, and relevant design/compatibility constraints. Identify automated tests and human-only boundaries. For a UI change include initial state, action, expected result, compact/keyboard behavior, and dirty-state or navigation consequences. For data work include transactions, retry identity, migrations, transfer formats, and recovery.

Do not rewrite acceptance criteria after implementation merely to match incomplete work. A requested feature, enum variant, or planned architecture is not evidence of current availability.

## Publish directly and coherently

Publish a meaningful first source/test/document increment and open a draft PR early. Do not create throwaway ownership files, no-op access probes, or task-specific source applicators.

In Connector mode, every commit uses complete file blobs, a tree based on the expected parent's tree, a commit naming that parent, an immediate remote-head check, and a non-force ref update. Verify the ref again afterward. Preserve unchanged files and file modes; include intended deletions explicitly. The ref API is not a compare-and-swap old-SHA API, so exclusive writer ownership is an essential part of the protocol.

If the remote head moved, stop and inspect. Do not force-update, replay an unreviewed tree, or start a competing repair session. If a response was lost, read GitHub before retrying: an unreferenced commit is not branch publication.

A coherent increment includes all affected callers, tests, schemas, checksums, manifests, and documentation. Prefer changes through current ownership boundaries. Keep unrelated dependency upgrades, formatting churn, and infrastructure redesign out of feature work.

## Integrate newer main without rewriting history

When required, read both exact heads and perform a normal reviewed merge into the task branch. Resolve conflicts without discarding either side's unrelated work. A Connector-created merge commit includes both ordered parents and the reconciled tree. Requalify the resulting SHA. Do not replace the feature tree with `main`, rebase shared history, or infer integration merely from the PR's base-branch label.

## Qualification

Use [Testing and CI](testing-and-ci.md) and the [command runbook](../agents/COMMAND-RUNBOOK.md). During iteration choose the smallest relevant test, then escalate:

```bash
./scripts/check-changed.sh
./scripts/check-static.sh
./scripts/check-fast.sh
./scripts/check.sh
```

`check-changed` is an aid, not a final certificate. Static is not native runtime testing. Full Linux/Tauri qualification and Android compile/package/emulator qualification establish different boundaries. Native UI appearance, physical devices, real credentials, and live provider behavior remain explicit manual boundaries when not exercised.

`Check` checks out the exact PR head or dispatched ref's resolved commit. Draft automatic checks use the fast tier; ready PRs and explicit full dispatches provide terminal platform qualification. Verify the actual workflow's input choices and tested SHA. Do not add a source-writing workflow just to run a missing command.

`PR Contract` validates the compatibility acceptance snapshot independently. A malformed PR description must not prevent product qualification. PR-body edits do not trigger Check. Automatic and manual runs have separate concurrency/check contexts; neither an unrelated run nor an older green SHA proves the current head passed.

All Actions workflows are source-read-only regardless of trigger, including manual maintenance. They may verify/build/test/package published source and emit evidence, but must not author, stage, format-rewrite, commit, or push the reviewed source. The generated-candidate workflow is check-only, not an escape hatch. See [Source-publication boundary](../agents/SOURCE-PUBLICATION-BOUNDARY.md).

## Respond to failures at the correct boundary

Read exact-head steps, bounded logs, qualification artifacts when accessible, and failure fingerprints. Separate product, test-contract, environment, Connector, service-incident, and human-only failures. Repair all independently actionable findings in one coherent batch and rerun the focused boundary before another broad matrix.

Follow [Failure triage](../agents/FAILURE-TRIAGE.md). One unchanged-head rerun is allowed only for evidence of a transient cause. A repeated fingerprint requires a strategy change. Do not widen permissions, add sleeps/timeouts, weaken tests, create no-op commits, or toggle PR state to manufacture green CI. A busy runner does not establish loss of source-write access.

## Readiness, completion, and the PR snapshot

Keep the snapshot's exact base/head/branch, criterion status, tests, known failures, and evidence current at meaningful checkpoints. Draft metadata can briefly lag a synchronize event; reconcile it rather than using it as source authority.

[Execution state](../agents/EXECUTION-STATE.md) is the canonical schema and readiness contract. Review readiness may trigger full qualification, so pending or explained not-run gates must never be falsely recorded as passed to reach that transition. Readiness is not a completion certificate. Before describing a change as fully qualified or complete, all required final-head gates must actually pass and manual boundaries must be explicitly accounted for.

Keep incomplete work or unresolved qualification clearly labeled. A newer source publication invalidates old exact-head qualification as evidence for the new head.

## Handoff and human review

Provide the PR, exact branch/base/head, changed areas, acceptance/non-goals, actual check commands/run IDs/results, unrun checks, known limitations, data/compatibility effects, rollback guidance, and writer release. Give concise copyable local checkout/build/run/install instructions plus task-specific observations using [Local testing and handoff](local-testing-and-handoff.md).

Do not require a full CI matrix merely to let the human test-drive the application. Do not claim manual acceptance just because the automated suite passed. A proposal remains a proposal until reviewed.

The normal merge method is **Squash and merge** by the human after required qualification and acceptance. The agent does not merge, delete branches, or tag a release without explicit authorization. After merge, update a clean local main using fetch and `git pull --ff-only`; inspect local work before deleting a worktree or branch. Squash ancestry can make `git branch -d` refuse deletion, and that refusal must not be bypassed automatically.

## Release and rollback boundaries

A normal merge does not automatically change the application version. Release preparation coordinates `package.json`, `src-tauri/Cargo.toml`, and `src-tauri/tauri.conf.json`, required platform packaging, and `scripts/release-check.sh`. Tag the intended reviewed main commit only for an explicitly requested release; do not paste a historical tag example as a new release instruction.

A rollback normally uses a new reviewed revert/fix branch and PR, not a direct push to main or shared-history rewrite. Reverting source cannot necessarily reverse a migrated data format; forward repair or restoration may be required. Document the compatibility and backup implications before instructing the user to install an older build.

The durable outcome is the verified branch/PR and exact-head evidence, not the number of commits, amount of tool activity, or existence of a generated patch.
