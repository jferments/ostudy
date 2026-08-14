# Agent workflow and validation

Agents must work as reviewable contributors, not as autonomous maintainers of
`main`.

## 1. Establish the exact starting point

Choose the repository-access mode before editing.

### Local checkout

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

### Authenticated GitHub Connector

When operating through an authenticated GitHub Connector, do not use local
shell state as a proxy for remote repository state. Use Connector reads to
resolve `jferments/ostudy`, fetch the intended base, and record the exact base
SHA. Then use the actual Connector mutation when the task reaches its first
write and let the Connector's authorization boundary handle any required user
approval.

Read access, concrete write authorization, CI runner availability, and local
checkout state are separate concerns. In particular:

- a successful read does not prove or disprove authorization for a later write;
- not seeing an approval prompt does not mean writes are unavailable;
- an unavailable CI runner does not prevent publishing a branch or commit;
- do not change the Connector to permanent/unrestricted write access simply to
  avoid a write prompt;
- only an actual failed Connector mutation establishes a Connector write
  failure, and the exact error must be reported.

After each meaningful Connector mutation, verify the resulting remote branch or
head SHA before claiming that the change was published. See
[`GITHUB-CONNECTOR.md`](GITHUB-CONNECTOR.md) for the complete connector-specific
workflow.

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

In a local checkout, inspect continuously:

```bash
git status --short
git diff --check
git diff --stat
git diff
```

In Connector mode, inspect the remote branch and proposed diff through GitHub
instead of inferring state from an unrelated local checkout.

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

In a local checkout, stage specific files whenever practical:

```bash
git add <paths>
git diff --cached --check
git diff --cached --stat
git diff --cached
```

Confirm that no secrets, personal data, generated build output, unrelated
formatting, or accidental lockfile changes are staged. In Connector mode,
inspect the exact changed-file list and remote diff for the same hazards.

Use a behavioral commit message, for example:

```text
docs: standardize project name and add agent guidance
```

After committing locally:

```bash
git diff --name-only origin/main...HEAD
git diff --stat origin/main...HEAD
git log --oneline origin/main..HEAD
```

In Connector mode, verify the same information from the remote branch/PR and
record the current remote head SHA.

## 6. Push and propose; do not self-merge

In a local checkout:

```bash
git push -u origin HEAD
```

In Connector mode, create or update the remote task branch through the
Connector and verify its resulting head SHA.

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
- **Published:** the Connector or remote Git operation confirmed the resulting
  branch/head state.

Never say that code is fully tested merely because it parses, compiles, or looks
correct. Never say that a Connector mutation exists remotely merely because a
local patch or proposed change was generated.
