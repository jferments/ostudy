# Agent workflow and validation

Agents must work as reviewable contributors, not as autonomous maintainers of
`main`.

## 1. Establish the exact starting point

```bash
git switch main
git status --short
git fetch origin --prune
git pull --ff-only
git rev-parse HEAD
git switch -c <type>/<short-description>
```

Stop before destructive synchronization if the working tree contains changes
that are not part of the task. Record the base commit SHA in the handoff.

## 2. Define the change before editing

Write:

- the observed problem or requested behavior;
- concrete acceptance criteria;
- non-goals;
- architecture and compatibility surfaces involved;
- expected tests and manual workflows.

Search the repository for all callers, related tests, commands, migrations, and
documentation before selecting an implementation.

## 3. Implement the smallest coherent change

- Keep the branch limited to one logical behavior.
- Follow the owning layer described in the architecture guardrails.
- Reuse current domain and IPC boundaries.
- Add regression coverage with the fix.
- Avoid dependency changes and broad refactors unless they are the task.
- Update affected documentation and ADRs in the same branch.

Inspect continuously:

```bash
git status --short
git diff --check
git diff --stat
git diff
```

## 4. Validate in layers

Use the cheapest gate that answers the current question, then escalate as the
change stabilizes:

```bash
./scripts/check-changed.sh  # diff-aware iteration helper
./scripts/check-static.sh   # manifests, IPC, policy, typecheck, rustfmt
./scripts/check-fast.sh     # static + all frontend tests + locked Rust compile
./scripts/check.sh          # full merge qualification and native smoke
```

Draft pull requests are expected to pass the fast tier. Marking a pull request
ready triggers full qualification. Always inspect the result for the exact head
SHA; an older green run does not qualify a newer commit. `OSTUDY_FORCE_CHECK=1`
forces re-execution when exact-tree cache reuse should be bypassed. For a release
candidate, also run `./scripts/release-check.sh`.

Manually exercise the affected UI path with `./run-app.sh`. For UI work, include
keyboard behavior, pending/error states, and all relevant themes. For persistence
work, restart the application and verify the stored result.

Additional checks depend on the change:

- **Documentation-only rename:** run
  `git grep -n -E 'Study[[:space:]]+App|Flashcard[[:space:]]+[Aa]pp' -- '*.md' LICENSE`
  and confirm it returns no legacy project-name references.
- **IPC change:** verify frontend wrapper names, Rust command registration,
  serialization, and stable errors.
- **Database change:** test fresh and upgraded databases, transaction rollback,
  checksums, and constraints.
- **Review/scheduling change:** test rating mapping, due-state transition,
  immutable event creation, and duplicate submission.
- **Import/export change:** round-trip existing packages and verify title,
  assets, IDs, schedules, and excluded personal history.
- **UI change:** test empty/loading/error states, keyboard access, repeated
  clicks, and Light/Dark/Console/Synthwave rendering.

## 5. Review the exact proposed commit

Stage specific files whenever practical:

```bash
git add <paths>
git diff --cached --check
git diff --cached --stat
git diff --cached
```

Confirm that no secrets, personal data, generated build output, unrelated
formatting, or accidental lockfile changes are staged.

Use a behavioral commit message, for example:

```text
docs: standardize project name and add agent guidance
```

After committing:

```bash
git diff --name-only origin/main...HEAD
git diff --stat origin/main...HEAD
git log --oneline origin/main..HEAD
```

## 6. Push and propose; do not self-merge

```bash
git push -u origin HEAD
```

Open a pull request against `main`. The description must include:

- exact base commit;
- problem and acceptance criteria;
- changed files;
- tests and commands that passed;
- checks not run;
- manual workflows exercised;
- migration, compatibility, and rollback notes;
- known limitations.

A human reviews, tests, and merges. Additional fixes belong on the same branch
and update the same pull request.

## Validation language

Use precise statements:

- **Passed:** the named command completed successfully in the current
  environment.
- **Inspected:** code or output was reviewed without execution.
- **Not run:** the check was unavailable or outside the environment.
- **Manually verified:** the named behavior was actually exercised.

Never say that code is fully tested merely because it parses, compiles, or looks
correct.
