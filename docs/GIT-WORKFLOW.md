# Git workflow for ostudy

Use short-lived branches and merge through GitHub. The repository history and
release tags replace old archive revision labels.

## Start a change

```bash
cd ~/code/ostudy
git switch main
git pull --ff-only
git switch -c feat/short-description
```

Use `fix/`, `chore/`, `docs/`, or `test/` instead of `feat/` when appropriate.

If switching branches is blocked by an accidental unstaged lockfile change from
an older build, inspect it before restoring only that file:

```bash
git diff -- src-tauri/Cargo.lock
git restore --worktree src-tauri/Cargo.lock
```

Current build scripts reject and prevent this condition. Do not use a broad
`git reset --hard` or discard unrelated work.

## Validate

```bash
./scripts/check.sh
./run-app.sh
```

The first command is the full automated gate. The second starts the current
standalone build for manual testing. You do not need to run each underlying
formatter, linter, test runner, Cargo command, and WebDriver command separately.

Inspect the intended change:

```bash
git status --short
git diff --check
git diff --stat
git diff
```

## Commit and push the branch

Stage only the files that belong to the change:

```bash
git add <file-or-directory> ...
git status --short
git commit -m "chore: stabilize repository build and CI"
git push -u origin "$(git branch --show-current)"
```

Open a pull request into `main`. The short template records acceptance criteria,
the `scripts/check.sh` result, manual validation, and any skipped checks. It is a
memory aid for a solo developer, not a bureaucratic approval system.

## Merge and synchronize local main

After GitHub Actions passes and the Phase 2 local test plan is satisfactory, use
**Squash and merge** in GitHub. Then:

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

Do not force-push `main`.
