# Git workflow for ostudy

The revision 5.4 full-source archive is the final archive-based baseline. Future
updates should be normal Git commits or pull requests made against a named base
commit.

## 1. Generate and qualify the lockfiles

The source archive intentionally does not fabricate lockfiles. On the first run,
`run-app.sh` creates them from the exact direct dependency pins if they are not
already present:

```text
pnpm-lock.yaml
src-tauri/Cargo.lock
```

Generate the lockfiles and launch once, then run the strict gate:

```bash
./run-app.sh
./scripts/check.sh
```

Commit both lockfiles. They are part of the reproducible source of truth.

## 2. Create the local baseline repository

From the project root:

```bash
git init -b main
git add .
git status --short
git commit -m "baseline: functional ostudy v0.3.0"
git tag -a v0.3.0 -m "Known-good ostudy v0.3.0 baseline"
```

If Git needs an identity:

```bash
git config --global user.name "Your Name"
git config --global user.email "you@example.com"
```

Tag only after `scripts/check.sh`, `scripts/release-check.sh`, and the manual UI
smoke test all pass.

## 3. Add a remote repository

Create an empty remote repository without an auto-generated README, license, or
`.gitignore`. Then:

```bash
git remote add origin https://github.com/OWNER/REPOSITORY.git
git remote -v
git push -u origin main
git push origin v0.3.0
```

## 4. Make every future change on a branch

```bash
git switch main
git pull --ff-only
git switch -c feat/short-description
```

Before committing:

```bash
./scripts/check.sh
git diff --check
git status --short
git diff --stat
git diff
```

Commit only the cohesive files for that behavior:

```bash
git add <specific-files>
git commit -m "feat(cards): add reverse card generation"
git push -u origin feat/short-description
```

Open a pull request and merge only after the branch passes the automated gate
and the changed UI path has been manually exercised.

## 5. How AI-generated updates should be delivered

Preferred order:

1. pull request against the repository;
2. a commit that can be cherry-picked;
3. a small unified diff generated from a named commit;
4. a full source archive only for disaster recovery or a new baseline.

Every update must state:

- exact base commit SHA;
- changed files;
- acceptance criteria;
- tests added or changed;
- exact commands that passed;
- checks that could not be executed;
- known limitations.

## 6. Rollback

Inspect history:

```bash
git log --oneline --decorate --graph --all
```

Revert a bad merged commit without rewriting history:

```bash
git revert <commit-sha>
```

Inspect the baseline tag:

```bash
git switch --detach v0.3.0
```

Return to development:

```bash
git switch main
```

## 7. What belongs in Git

Commit source, migrations, documentation, tests, `pnpm-lock.yaml`, and
`src-tauri/Cargo.lock`.

Do not commit `node_modules/`, `dist/`, `src-tauri/target/`, logs, secrets,
`library.sqlite3`, or imported personal images/documents. The live database and
managed assets reside in the operating system's application-data directory,
outside the source repository.
