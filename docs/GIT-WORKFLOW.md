# Git workflow for ostudy

Use the publication-first process defined in
[`OSTUDY-GIT-AND-CODING-WORKFLOW.md`](OSTUDY-GIT-AND-CODING-WORKFLOW.md). The
branch and draft pull request are created before feature implementation so work
is continuously inspectable, recoverable, and validated by GitHub Actions.

## Establish the exact base

From the repository root:

```bash
cd ~/code/ostudy
git switch main
git status --short
git fetch origin --prune
git pull --ff-only
git rev-parse HEAD
```

Stop if the working tree contains unrelated local changes. Record the exact base
commit in the pull request.

## Publish the branch before implementation

```bash
git switch -c feat/short-description
git push -u origin HEAD
```

Immediately open a **draft pull request** against `main`. Its initial description
should state:

- the exact base commit;
- concrete acceptance criteria;
- non-goals and compatibility surfaces;
- the expected automated and manual validation.

Use `fix/`, `chore/`, `docs/`, or `test/` instead of `feat/` when appropriate.

## Implement in reviewable vertical commits

Write and push one coherent behavior at a time. Do not leave the only copy of a
large implementation in an unpublished scratch directory. Keep unrelated
refactors, dependency upgrades, generated output, and formatting churn out of
the branch.

Inspect scope continuously:

```bash
git status --short
git diff --check
git diff --stat
git diff
git push
```

GitHub Actions should validate each published head. Correct failures on the same
branch and pull request rather than starting an opaque replacement branch.

## Validate the completed head

Run the full local gate:

```bash
./scripts/check.sh
```

Then launch the exact standalone build produced by that gate:

```bash
./run-app.sh
```

Manually exercise every changed workflow, including persistence after restart,
keyboard and pending/error behavior, responsive layouts, and all relevant
themes.

Inspect the exact proposed change:

```bash
git diff --name-only origin/main...HEAD
git diff --stat origin/main...HEAD
git log --oneline origin/main..HEAD
```

The successful workflow run must belong to the **current branch head**. An older
successful run does not validate later commits.

## Prepare the pull request for review

Update the pull-request description with:

- exact base and head commits;
- acceptance criteria addressed;
- changed files and why;
- tests added or updated;
- exact commands and CI runs that passed;
- checks not run and why;
- manual workflows actually exercised;
- migration, compatibility, rollback, and residual-risk notes.

Mark the pull request ready for review only after the automated gate passes. A
human still performs the local application acceptance test before merging.

## Merge and synchronize local main

After GitHub Actions passes and the manual test is satisfactory, use **Squash and
merge** in GitHub. Then:

```bash
git switch main
git pull --ff-only
git branch -d <merged-branch-name>
```

Delete the remote branch in GitHub or with:

```bash
git push origin --delete <merged-branch-name>
```

## Releases

A normal merge does not change the release version. When a release is ready,
update the version in `package.json`, `src-tauri/Cargo.toml`, and
`src-tauri/tauri.conf.json` together, pass the release gate, merge, and tag the
exact main commit:

```bash
git switch main
git pull --ff-only
./scripts/release-check.sh
git tag -a v0.1.0 -m "ostudy 0.1.0"
git push origin v0.1.0
```

The tag triggers consistent Linux artifact generation.

## Rollback

Inspect history:

```bash
git log --oneline --decorate --graph --all
```

Reverse a bad merged commit without rewriting shared history:

```bash
git revert <commit-sha>
git push origin main
```

Do not force-push `main` or a shared review branch.
