# Git and pull-request workflow

ostudy uses a publication-first, evidence-driven workflow: one focused branch, one draft pull request, directly reviewable source, a durable acceptance ledger, and exact-head qualification. Planning supports delivery, but a code task is not represented by an unpublished patch, task-specific applicator, transient CI tree, or description of changes that exist only in a temporary environment.

This process applies to human contributors and automated maintainers. The repository access mechanism may differ, but the invariants do not.

## 1. Establish or restore the exact task state

### New local-checkout task

```bash
cd ~/code/ostudy
git status --short
git fetch origin --prune
git switch main
git pull --ff-only
BASE_SHA="$(git rev-parse HEAD)"
printf '%s\n' "$BASE_SHA"
```

Stop before switching or rebasing when the working tree contains unrelated changes. Protect them in a separate commit, branch, stash, or worktree.

For substantial work, use an isolated worktree:

```bash
BRANCH="agent/short-description"
WORKTREE="../ostudy-short-description"
git worktree add -b "$BRANCH" "$WORKTREE" "$BASE_SHA"
cd "$WORKTREE"
```

### Authenticated GitHub Connector task

Use authenticated Connector reads to resolve the private repository, fetch the intended base, and record its exact SHA. Do not substitute `gh`, anonymous scraping, CI state, an unrelated checkout, or a missing connector action for repository evidence.

Repository visibility, repository-level permission, a concrete mutation authorization, remote publication, and CI validation are separate states. Invoke the real write when needed; only an actual rejection establishes a write failure. Verify every successful branch/file/commit mutation through GitHub.

See [`../agents/GITHUB-CONNECTOR.md`](../agents/GITHUB-CONNECTOR.md).

### Resuming an existing task

Before editing:

1. read the open PR and its machine-readable `ostudy-agent-state` ledger;
2. verify the actual remote base, branch, and head;
3. inspect exact-head workflow evidence and unresolved review threads;
4. enumerate non-terminal `AC-##` criteria and known failures;
5. reconcile any stale draft metadata.

Do not reconstruct state from memory or from an older green run.

## 2. Define a verifiable change contract

Record:

- the current observed problem;
- stable `AC-##` acceptance criteria;
- explicit non-goals;
- architecture and compatibility surfaces;
- migrations, data, import/export, platform, privacy, accessibility, and dependency effects;
- expected tests and manual workflows.

The acceptance identifiers remain stable throughout implementation. Change a requirement only when the human changes the intended behavior; do not rewrite criteria after the fact to fit an incomplete implementation.

The state ledger schema and completion predicate are documented in [`../agents/EXECUTION-STATE.md`](../agents/EXECUTION-STATE.md).

## 3. Publish one focused branch

Use a short-lived branch, normally:

```text
agent/<description>
```

In a local checkout:

```bash
git push -u origin HEAD
```

In Connector mode, create the branch from the recorded base SHA and verify the remote branch state.

Do not develop directly on `main`. Do not create competing replacement branches for ordinary failures. A new branch is appropriate only when the existing branch is genuinely unrecoverable or the scope is a separate reviewable change.

## 4. Make the first meaningful direct-source commit

The first commit may be an acceptance-test skeleton, ADR, migration shell, controller boundary, or another small piece that truthfully establishes the task. It must contain the actual reviewable source for that checkpoint.

In a local checkout:

```bash
git add -- <paths>
git diff --cached --check
git commit -m "chore: scaffold <change>"
git push
```

In Connector mode, create/update the intended source paths and verify the resulting remote head.

A generated local patch, task-specific source applicator, finalizer, CI-generated product tree, or proposed commit message is not publication. Automatic PR CI may validate source but may not author, stage, commit, push, or change PR readiness.

## 5. Open a draft pull request and initialize its ledger

Open one draft PR into `main` as soon as the branch has a meaningful commit. Record:

- exact base and current head;
- user-visible goal;
- stable acceptance criteria;
- non-goals;
- architecture/compatibility surfaces;
- planned automated and manual validation;
- current implementation state;
- known failures.

The PR template includes one JSON ledger between the `ostudy-agent-state` markers. Drafts may record incomplete criteria and pending gates. A synchronize event may briefly make the recorded draft head stale; the verifier warns rather than fails. Reconcile the ledger at the next checkpoint and require exact identity before review readiness.

Target scope is not current-state evidence. A PR that contains only an ADR or scaffold must say so explicitly.

## 6. Implement coherent vertical increments

Inspect current code, callers, tests, migrations, fixtures, external formats, and documentation before introducing a path. Prefer understandable commits, for example:

1. schema/domain behavior and migration coverage;
2. native service and typed IPC;
3. controller and interface;
4. regression and acceptance tests;
5. documentation and cleanup.

Keep each commit purposeful and on the same branch. Do not leave the only copy of substantial source in an agent-local workspace or self-hosted runner.

Avoid mixing feature behavior, dependency upgrades, broad refactors, mass formatting, and unrelated cleanup. Explain any refactor required by the feature.

A structural boundary or monolith-removal increment must publish the target ownership/dependency rule, move reviewable source directly, and update every enforcing manifest, static verifier, fixture, and regression test in the same pull request. When a required path is removed, register its explicit replacement paths rather than weakening or deleting the old check. A temporary compatibility module is acceptable only as a clearly documented, short-lived step on the published branch; it must not survive completion or become a second public API.

After each meaningful publication checkpoint:

- verify the remote head;
- update acceptance status/evidence/tests;
- update `currentState`;
- record known failure fingerprints;
- record only validation actually performed against the named SHA.

## 7. Validate during development

Use increasingly expensive checks:

```bash
./scripts/check-changed.sh
./scripts/check-static.sh
./scripts/check-fast.sh
```

The `Check` workflow runs static first, then the selected gate. Draft PRs use fast qualification. Every run checks out and verifies the exact PR head.

When CI fails:

1. verify the tested SHA and current head;
2. read job steps, decoded logs, `ostudy-qualification-result.json`, and bounded diagnostics;
3. identify every independent actionable failure;
4. classify failures using [`../agents/FAILURE-TRIAGE.md`](../agents/FAILURE-TRIAGE.md);
5. use the focused reproduction;
6. repair a coherent batch on the same branch;
7. rerun the narrow boundary before escalating.

An unchanged-head retry is allowed once only with evidence of a transient external failure. The same fingerprint twice requires a strategy change before another broad run. Do not use no-op commits, PR-state toggles, expanded permissions, arbitrary retries/sleeps/timeouts, skipped tests, or weakened assertions to manufacture a green result.

A static parse, mocked frontend test, or successful build step must not be reported as a successful native application test.

## 8. Qualify the completed head

When every in-scope acceptance criterion is implemented, run:

```bash
./scripts/check.sh
```

Then launch the exact standalone artifact produced by the gate:

```bash
./run-app.sh
```

Manually exercise the changed workflow, including relevant persistence after restart, keyboard/touch behavior, compact/medium/wide layouts, themes, pending/cancel/error states, migration of existing data, and import/export or backup compatibility.

Android-affecting work also runs:

```bash
./scripts/android-test.sh
./scripts/android-emulator-smoke.sh
```

Physical camera/device paths, live providers, private credentials, or hardware-specific behavior remain explicitly human-required where deterministic CI cannot perform them.

Inspect exactly what the PR proposes:

```bash
git diff --name-status origin/main...HEAD
git diff --stat origin/main...HEAD
git log --oneline origin/main..HEAD
```

In Connector mode, inspect the remote PR changed-file list/diff and verify the current head through GitHub.

A newer push invalidates an older green result.

## 9. Satisfy the readiness contract

A PR may be marked ready only when:

- `currentState` is `readyForReview`;
- the ledger base, branch, and head exactly match GitHub;
- every acceptance criterion is terminal and has evidence;
- static, fast, and full passed on the current head;
- Android passed or is explicitly `notRequired` for a non-Android change;
- manual workflows are passed, `notRequired`, or explicitly `humanRequired`;
- `knownFailures` is empty.

`notRun`, inspection alone, an older green result, or a target-scope paragraph does not qualify readiness.

Update the PR with:

- implemented behavior and non-goals;
- design decisions and tradeoffs;
- changed files grouped by purpose;
- tests/documentation added;
- exact commands and workflow run IDs that passed;
- checks not run and why;
- manual workflows actually exercised;
- migration, compatibility, backup, rollback, limitations, and residual risks.

For Connector-backed work, distinguish publication state from validation and manual acceptance state.

## 10. Merge and synchronize

The normal merge method is **Squash and merge** after exact-head CI and human acceptance. An automated maintainer does not merge its own PR unless the repository owner explicitly requests that operation.

After merge locally:

```bash
git switch main
git pull --ff-only
git branch -d <merged-branch>
git push origin --delete <merged-branch>
```

When using a worktree, remove it only after local artifacts/evidence are reviewed:

```bash
git worktree remove ../ostudy-short-description
```

## Explicit maintenance workflows

Ordinary `Check` qualification is read-only. The separately named `Maintain generated Rust candidates` workflow is manual-only, requires explicit confirmation, refuses `main`, and restricts changes to canonical Rust source/lockfile paths. It is a maintenance escape hatch—not the normal implementation path and never a mechanism for CI-authored product features.

## Releases

A normal merge does not automatically change the application version. A release updates together:

- `package.json`;
- `src-tauri/Cargo.toml`;
- `src-tauri/tauri.conf.json`.

Run:

```bash
./scripts/release-check.sh
```

After merge, tag the exact `main` commit:

```bash
git switch main
git pull --ff-only
git tag -a v0.1.0 -m "ostudy 0.1.0"
git push origin v0.1.0
```

## Rollback and recovery

Inspect history before reversing a change:

```bash
git log --oneline --decorate --graph --all
```

Reverse a bad merged commit with a new commit rather than rewriting shared history:

```bash
git revert <commit-sha>
git push origin main
```

A migration or external-format rollback may require a forward repair. Document data effects and recovery steps in the original PR. Do not force-push `main` or a shared review branch.

If an implementation direction is unsound, stop adding code, preserve the published branch/evidence, and correct it transparently. Starting a clean replacement is reserved for genuinely unrecoverable branches, not ordinary failures.

## Completion record

A handoff identifies:

- base SHA, branch, head SHA, and PR number;
- acceptance status and non-goals;
- files/migrations changed;
- tests/documentation added;
- local commands and exact results;
- exact-head CI state and qualification artifact;
- manual acceptance performed or still human-required;
- known failures, limitations, compatibility effects, rollback concerns, and merge method.

The durable result is the published branch and PR. Completion is a conjunction of acceptance, publication, exact-head validation, manual boundaries, and no unresolved failure—not a narrative impression that substantial work occurred.
