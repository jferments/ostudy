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
./scripts/check-static.sh   # manifests, UI tokens, IPC, policy, typecheck, rustfmt
./scripts/check-fast.sh     # static + frontend/Rust high-return gates
./scripts/check.sh          # full merge qualification and native smoke
```

Draft pull requests normally run the fast tier. Marking a pull request ready
triggers full qualification. Always inspect the result for the exact head SHA;
an older green run does not qualify a newer commit. `OSTUDY_FORCE_CHECK=1`
forces re-execution when exact-tree cache reuse should be bypassed. For a release
candidate, also run `./scripts/release-check.sh`.

### Agent-driven diagnostic loop

The authoritative gates intentionally fail fast. For substantial work, a
Connector-backed agent should use the integrated multi-failure diagnostic mode
when one-error-at-a-time discovery is slowing development. See
[`../development/agent-diagnostics.md`](../development/agent-diagnostics.md).

A draft development PR opts in through its body:

```text
<!-- agent-diagnostics:core -->
<!-- agent-diagnostics:android -->
<!-- agent-diagnostics:all -->
```

These markers modify the standard `Check` and `Android check` workflows rather
than launching a duplicate diagnostic workflow. On a draft PR, `core`
substitutes the core diagnostic sweep for the normal fast job, `android`
substitutes Android diagnostics for the normal Android build/emulator chain,
and `all` substitutes both. Ready-for-review PRs ignore diagnostic substitution
and run authoritative qualification.

The agent owns the feedback cycle when the self-hosted runner can perform the
check:

1. publish and verify the exact branch head;
2. inspect the standard workflow runs for that SHA;
3. retrieve failed job steps, decoded logs, and uploaded evidence through the
   GitHub Connector;
4. collect all actionable independent failures exposed by that run before
   editing;
5. repair them in a coherent batch on the same branch;
6. publish and verify the new head, then inspect its exact-head results;
7. continue until development diagnostics converge, then require authoritative
   qualification before handoff.

Do not ask the human to run a repository test solely to relay output when an
available self-hosted workflow can run it and the Connector can read the result.
The human still owns manual visual/interaction acceptance, physical-device or
OEM-specific checks, protected credentials, and runner-host administration that
cannot be performed or observed through GitHub.

A `BLOCKED` diagnostic gate is not a hidden failure: it means a declared
prerequisite did not pass. Fix the prerequisite and rerun; a newly exposed
failure in the dependent gate is then a genuine dependency boundary rather than
avoidable serial discovery.

For Android work, never use direct Gradle qualification as a substitute for
Tauri initialization. The Tauri x86_64 build must create the transient Gradle
settings/private-plugin graph and JNI output before Gradle dependency, Kotlin,
Lint, JVM, or instrumentation-package tasks run. When emulator evidence exists,
inspect the structured smoke JSON, per-case logs, Logcat, screenshot, and UI
hierarchy before asking the human to reproduce the failure.

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
  clicks, stable automation hooks, and Light/Dark/Console/Synthwave rendering.
- **Android change:** verify arm64 compile/package, x86_64 smoke package,
  Tauri-first Gradle/Kotlin/Lint/JVM contracts, package policy, source
  cleanliness, and official-emulator behavior; use the periodic minimum-SDK
  smoke for compatibility coverage rather than adding a second emulator to
  every PR.

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
- **Failed:** the named command executed and returned a failure for the stated
  exact head.
- **Blocked:** a diagnostic gate did not execute because a declared prerequisite
  failed.
- **Queued/Running:** an asynchronous workflow has not completed and must not be
  reported as passed.
- **Inspected:** code or output was reviewed without execution.
- **Not run:** the check was unavailable or outside the environment.
- **Manually verified:** the named behavior was actually exercised.
- **Published:** the Connector or remote Git operation confirmed the resulting
  branch/head state.

Never say that code is fully tested merely because it parses, compiles, or looks
correct. Never say that a Connector mutation exists remotely merely because a
local patch or proposed change was generated. Never convert an asynchronous
queued/running workflow into an invented result.
