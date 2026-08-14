# Git and pull-request workflow

ostudy uses a publication-first workflow: one focused branch, one draft pull request, and a continuously recoverable remote history. Planning supports delivery, but a code task is not represented by an unpublished patch or a description of changes that exist only in a temporary environment.

This process applies to human contributors and automated maintainers. The mechanism used to access GitHub may differ, but the repository invariants do not.

## 1. Establish the exact base

Start from the current intended base, normally `origin/main`, and record its commit SHA.

### Local checkout mode

In a local checkout:

```bash
cd ~/code/ostudy
git status --short
git fetch origin --prune
git switch main
git pull --ff-only
BASE_SHA="$(git rev-parse HEAD)"
printf '%s\n' "$BASE_SHA"
```

Stop before switching or rebasing when the working tree contains unrelated changes. Protect them in a separate commit, branch, stash, or worktree rather than allowing a task branch to absorb accidental local state.

For substantial work, an isolated worktree keeps the primary checkout available:

```bash
BRANCH="agent/short-description"
WORKTREE="../ostudy-short-description"
git worktree add -b "$BRANCH" "$WORKTREE" "$BASE_SHA"
cd "$WORKTREE"
```

### Authenticated GitHub Connector mode

A connector-backed environment must use the authenticated Connector as the source of truth for remote repository state. Use Connector reads to resolve the private repository, fetch the intended base, and record its exact SHA. Do not substitute `gh` availability, anonymous scraping, temporary transfer repositories, CI runner state, or an unrelated local checkout for Connector state.

Read access and authorization for a concrete write are separate. The absence of an approval prompt is not evidence that writes are unavailable. When the task reaches its first intended mutation, invoke the actual Connector branch/file/commit operation and let the Connector authorization layer handle any required approval. Do not change the Connector to permanent or unrestricted write access merely to bypass that boundary.

Only an actual rejected Connector mutation establishes a Connector write failure. Report the exact operation and error. After a successful mutation, verify the resulting remote branch or head SHA before claiming publication.

See [`../agents/GITHUB-CONNECTOR.md`](../agents/GITHUB-CONNECTOR.md) for the complete connector-specific procedure.

## 2. Publish one focused branch

Use a short-lived branch that identifies the work, normally:

```text
agent/<description>
```

In a local checkout, publish it before substantial implementation:

```bash
git push -u origin HEAD
```

In Connector mode, create the task branch from the recorded base SHA through the Connector, then verify the remote branch state.

Do not develop directly on `main`. Do not create competing replacement branches for ordinary test failures. A new branch is appropriate only when the existing branch is genuinely unrecoverable or the scope has become a separate change.

## 3. Make the first meaningful commit

The first commit may be an acceptance-test skeleton, an ADR, a migration shell, a documentation index, or another small piece that truthfully establishes the task. It should not pretend that an unimplemented feature is complete.

In a local checkout:

```bash
git add <paths>
git diff --cached --check
git commit -m "chore: scaffold <change>"
git push
```

In Connector mode, create or update the intended files on the task branch, then verify the resulting remote head SHA.

Publishing early makes the task inspectable and recoverable without encouraging large, opaque commits. A generated local patch is not publication.

## 4. Open a draft pull request

Open one draft pull request from the task branch into `main` as soon as the branch has a meaningful commit. Record:

- exact base commit;
- user-visible goal;
- acceptance criteria;
- non-goals;
- data, migration, import/export, platform, privacy, and compatibility surfaces;
- planned automated and manual validation.

Keep the description truthful as design and risk become clearer. A draft may acknowledge incomplete behavior; it must not claim checks or manual workflows that have not happened.

## 5. Implement in coherent increments

Inspect current code, callers, tests, migrations, format fixtures, and documentation before introducing a new path. Prefer small vertical commits that are individually understandable, for example:

1. schema/domain behavior and migration coverage;
2. native service and typed IPC;
3. controller and interface;
4. regression and acceptance tests;
5. documentation and cleanup.

The exact sequence depends on the change. The rule is that each commit has a clear purpose and is pushed to the same branch. Do not leave the only copy of a large change in an agent-local scratch directory.

Avoid mixing feature behavior, dependency upgrades, broad refactors, mass formatting, and unrelated cleanup. When a refactor is necessary for the feature, explain why it is part of the same coherent boundary.

## 6. Validate during development

Use the tiered checks while editing:

```bash
./scripts/check-changed.sh
./scripts/check-static.sh
./scripts/check-fast.sh
```

Consume complete failure information from a meaningful branch head before making speculative changes. When CI fails:

1. identify the exact failing command and commit;
2. reproduce the narrowest available failure;
3. fix behavior or the legitimate test contract rather than suppressing the check;
4. add or refine regression coverage;
5. push the correction to the same pull request;
6. review the new exact-head result.

For substantial Connector-backed work, prefer the opt-in [agent diagnostic sweep](agent-diagnostics.md) when fail-fast CI is exposing independent defects one at a time. Add the appropriate PR-body marker (`agent-diagnostics:core`, `agent-diagnostics:android`, or `agent-diagnostics:all`), then let the self-hosted workflow collect independent failures, per-gate logs, a Markdown summary, and machine-readable JSON evidence.

A Connector-backed agent is responsible for consuming that CI evidence itself when the self-hosted runner can execute the relevant test. It should inspect workflow runs for the exact head, retrieve failed jobs/steps/logs/artifacts, fix the complete actionable failure set in a coherent batch, publish the same branch, and repeat against the new exact head. Do not make the human run `pnpm check:static`, `pnpm check`, `pnpm android:smoke`, or an equivalent repository command merely to paste output back when GitHub CI can provide the same evidence directly.

The diagnostic sweep does not eliminate genuine dependency boundaries. If a valid Android APK cannot be built, emulator execution is `BLOCKED`; after the build is repaired, a later run may expose a runtime failure that could not have existed as an executable test earlier.

A static parse, mocked frontend test, or successful build step must not be reported as a successful native application test. A queued/running workflow must not be reported as passed.

Repository publication and CI availability are separate. A missing, busy, offline, or mismatched self-hosted runner may block validation, but it does not establish that GitHub branch, commit, or pull-request writes are unavailable.

## 7. Qualify the completed head

When implementation is complete, run:

```bash
./scripts/check.sh
```

Then launch the standalone artifact produced by the gate:

```bash
./run-app.sh
```

Manually exercise the changed workflow, including relevant persistence after restart, keyboard and touch behavior, compact and wide layouts, themes, pending/cancel/error states, migration of existing data, and import/export or backup compatibility.

In a local checkout, inspect exactly what the pull request proposes:

```bash
git diff --name-status origin/main...HEAD
git diff --stat origin/main...HEAD
git log --oneline origin/main..HEAD
```

In Connector mode, inspect the remote PR changed-file list/diff and verify its current head SHA through GitHub.

A green diagnostic sweep is not merge qualification. The exact head must still pass the repository's authoritative shared and platform-specific workflows. A green workflow run qualifies only the commit it tested; a later push invalidates an older successful result.

## 8. Prepare for review

Update the pull-request description with:

- exact base and current head commits;
- acceptance criteria addressed;
- design decisions and important tradeoffs;
- changed files grouped by purpose;
- tests added or updated;
- exact commands and CI runs that passed;
- commands not run and why;
- manual workflows actually exercised;
- migration, compatibility, backup, rollback, known limitation, and residual-risk notes.

For Connector-backed work, also distinguish publication state from validation state: report the remote branch/head and PR state separately from CI runner/workflow results, and state which CI logs/artifacts the agent consumed directly versus which manual checks still require human execution.

Mark the pull request ready only when its published head is complete and the required automated gate has passed. Human review and acceptance remain necessary even when an automated maintainer wrote the code.

## 9. Merge and synchronize

The normal merge method is **Squash and merge** after exact-head CI and human acceptance both pass. An automated maintainer does not merge its own pull request unless the repository owner explicitly requests that operation.

After merge in a local checkout:

```bash
git switch main
git pull --ff-only
git branch -d <merged-branch>
```

Delete the remote branch after confirming the merge and updated local main:

```bash
git push origin --delete <merged-branch>
```

When using a worktree, remove it only after any local artifacts or test evidence have been reviewed:

```bash
git worktree remove ../ostudy-short-description
```

## Releases

A normal merge does not automatically change the application version. A release change updates these together:

- `package.json`;
- `src-tauri/Cargo.toml`;
- `src-tauri/tauri.conf.json`.

Run the release qualification from a clean source copy:

```bash
./scripts/release-check.sh
```

After the release commit is merged, tag the exact `main` commit:

```bash
git switch main
git pull --ff-only
git tag -a v0.1.0 -m "ostudy 0.1.0"
git push origin v0.1.0
```

The current tag workflow builds Linux artifacts on the self-hosted Linux runner. Windows, macOS, Android, and iOS release pipelines are not yet present.

## Rollback and recovery

Inspect the history before reversing a change:

```bash
git log --oneline --decorate --graph --all
```

Reverse a bad merged commit with a new commit rather than rewriting shared history:

```bash
git revert <commit-sha>
git push origin main
```

A migration or external-format rollback may require a forward repair rather than a simple source revert. Document data effects and recovery steps in the original pull request so an emergency change is not forced to rediscover them.

Do not force-push `main` or a shared review branch. If an implementation direction is unsound, stop adding code, preserve the published branch for diagnosis, and correct it transparently or start a clean replacement only when the original branch cannot be recovered safely.

## Completion record

A task handoff is complete when it identifies:

- base SHA, branch, head SHA, and pull-request number;
- implemented behavior and non-goals;
- files and migrations changed;
- tests and documentation added;
- local commands and exact results;
- CI state for the current head;
- manual acceptance performed or still required;
- limitations, compatibility effects, rollback concerns, and merge method.

For Connector-backed work, every claimed remote mutation must be verified through GitHub, and any failed Connector operation must be reported with its exact error rather than inferred from local or CI state.

The durable result is the published branch and pull request, not a promise to provide code later.
