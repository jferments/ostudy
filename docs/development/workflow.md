# Git and pull-request workflow

ostudy uses one focused branch, one pull request, ordinary reviewable source, and CI evidence tied to an exact commit. The same workflow applies to people and to coding agents: work in a real checkout, test locally, push a branch, and let the `Check` workflow confirm the result.

## Who owns what

| Fact | Authority |
| --- | --- |
| Scope and acceptance | The maintainer's current request and explicit agreements |
| Published source and branch/PR head | GitHub's remote commit |
| Automated qualification | The actual `Check` run and the commit it tested |
| Local uncommitted work | The contributor's own checkout |
| Manual acceptance | The report from the person who exercised the workflow |

Exactly one writer owns a task branch at a time. GitHub Actions never write source, and a PR description is a summary for reviewers, not a lock, CI trigger, or source of truth.

## Start or resume a task

Fetch current refs, branch from up-to-date `main`, and never develop directly on `main`. Agents use a short-lived branch such as `claude/<topic>` (or the branch their session assigns). Inspect `git status --short` before switching and preserve unrelated edits; the [local handoff guide](local-testing-and-handoff.md) gives a safe detached review-worktree procedure.

When resuming, read the branch head, the PR, open review comments, and the latest `Check` result before editing. Do not restart from a remembered commit or treat an earlier conversational "done" as evidence. Keep the same branch for ordinary repairs.

## Specify a reviewable change

Write down the observed problem, what should change, what is out of scope, and the design or compatibility constraints that apply. For a UI change include the starting state, the action, the expected result, compact/keyboard behavior, and dirty-state or navigation consequences. For data work include transactions, retry identity, migrations, transfer formats, and recovery.

Do not rewrite the goal after implementation merely to match incomplete work. A requested feature, enum variant, or planned architecture is not evidence of current availability.

## Commit and push

Open a draft PR early with the first meaningful increment. A coherent commit includes all affected callers, tests, schemas, migration checksums, verifier inventories, and documentation. Prefer changes through current ownership boundaries, and keep unrelated dependency upgrades, formatting churn, and infrastructure redesign out of feature work.

Commit messages use an imperative subject under 72 characters and a body that explains why. Run `./scripts/check-static.sh` and the focused tests for what changed before every push.

## Integrate newer main without rewriting history

When a branch needs newer `main`, merge `main` into it and resolve conflicts without discarding either side's unrelated work. Regenerate lockfiles and generated files with the repository tooling rather than by hand. Do not rebase or force-push a branch someone else is using. The merged commit needs its own `Check` result.

## Qualification

Use [Testing and CI](testing-and-ci.md) and the [command runbook](command-runbook.md). During iteration choose the smallest relevant test, then escalate:

```bash
./scripts/check-changed.sh
./scripts/check-static.sh
./scripts/check-fast.sh
./scripts/check.sh
```

`check-changed` is an aid, not a final certificate. Static is not native runtime testing. Full Linux/Tauri qualification and Android compile/package/emulator qualification establish different boundaries. Native UI appearance, physical devices, real credentials, and live provider behavior remain explicit manual boundaries when not exercised.

`Check` tests the exact PR head. Draft PRs run the fast tier; ready PRs, pushes to `main`, drafts labelled `qualification:full` and explicit full dispatches run the full Linux and Android gates on the single self-hosted runner `ostudy-unified`. Every push to a ready PR runs that full gate again, so mark a PR ready only when it is complete and move it back to draft for substantial rework. PR description edits do not trigger `Check`.

All Actions workflows are source-read-only regardless of trigger, including manual maintenance. They verify, build, test and package pushed source; they never author, format-rewrite, commit, or push it. `scripts/verify-repository-policy.mjs` enforces this.

## Respond to failures at the correct boundary

A failed `check` job ends with a failure summary: the failing tier and step, the matched error lines, and a focused command to reproduce it locally. The same summary is in the job summary, and the headline appears as an annotation on the run. [Testing and CI](testing-and-ci.md#failure-evidence-and-triage) describes the evidence in detail.

Reproduce the failure locally, fix the owning boundary, add a regression where practical, and push once the focused check passes. Separate product, test, runner/environment, and GitHub-incident failures. One unchanged-head re-run is allowed only for evidence of a transient cause, and a repeated fingerprint requires a different approach. Do not widen permissions, add sleeps or longer timeouts, weaken or skip tests, push empty commits, or toggle PR state to manufacture green CI.

## Handoff and review

The PR description follows the [pull request template](../../.github/pull_request_template.md): what changes for a learner, how it works, what was tested and on which commit, what was not tested, data and compatibility effects, and short commands to try it locally. Keep it current with each push. [Local testing and handoff](local-testing-and-handoff.md) has copyable checkout, build, run and install commands.

Do not require a full CI run merely to let the maintainer try the application. Do not claim manual acceptance just because the automated suite passed. A pull request remains a proposal until it is reviewed.

The maintainer merges with **Squash and merge** after review. Agents do not merge, delete branches they did not create, or tag a release without explicit authorization. After a merge, update a clean local `main` with `git pull --ff-only` and inspect local work before deleting a worktree or branch. Squash ancestry can make `git branch -d` refuse deletion, and that refusal must not be bypassed automatically.

## Release and rollback boundaries

A normal merge does not change the application version. Release preparation coordinates `package.json`, `src-tauri/Cargo.toml`, and `src-tauri/tauri.conf.json`, required platform packaging, and `scripts/release-check.sh`. Tag the intended reviewed `main` commit only for an explicitly requested release.

A rollback normally uses a new reviewed revert or fix branch and PR, not a direct push to `main` or a history rewrite. Reverting source cannot necessarily reverse a migrated data format; forward repair or restoration may be required. Document the compatibility and backup implications before telling anyone to install an older build.
