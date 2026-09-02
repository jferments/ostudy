# Agent workflow and validation

Agents work as reviewable contributors, not as autonomous maintainers of `main`.
The durable implementation is the published source branch and pull request—not a
plan, local patch, source applicator, transient runner tree, or promise to
publish later.

Read [`DIRECT-WORKSPACE.md`](DIRECT-WORKSPACE.md) for the normal implementation
loop, [`EXECUTOR-SELECTION.md`](EXECUTOR-SELECTION.md) before selecting Codex or
the full orchestrator, [`EXECUTION-STATE.md`](EXECUTION-STATE.md) for the
completion predicate, [`FAILURE-TRIAGE.md`](FAILURE-TRIAGE.md) for failure/retry
rules, and [`GITHUB-CONNECTOR.md`](GITHUB-CONNECTOR.md) for remote repository and
PR operations.

## 1. Restore or establish the exact task state

### Preferred direct-workspace task

Use GitHub to resolve `jferments/ostudy`, the intended base branch, current PR
state, and exact remote SHA. Then perform one `dev-workspace-v1` preflight and
open or resume the persistent task worktree.

For a new branch:

```text
workspace_open(
  project="ostudy",
  branch="agent/<short-description>",
  base_sha="<exact-current-main-sha>"
)
```

For an existing PR branch, pass its current exact remote head. Require the
seven-tool server contract to match, local and remote heads to agree, no
unexplained dirty state, and no active command.

The direct workspace is the default even when Codex credits are available.
Codex may advise read-only, or it may exclusively own a separate deliberately
assigned branch. It must not alternate as writer with the direct workspace.

### Local human checkout

A human contributor may use an ordinary local checkout:

```bash
git switch main
git status --short
git fetch origin --prune
git pull --ff-only
git rev-parse HEAD
git switch -c agent/<short-description>
```

Stop before destructive synchronization if the working tree contains changes
that are not part of the task. Record the base commit SHA.

### GitHub Connector fallback

The Connector is normally the remote repository, PR, review, and CI interface.
Its low-level source writer is an emergency fallback only when no persistent
workspace writer is available. Repository visibility, repository-level
permission, exposure of a connector action, authorization for a concrete
mutation, publication, and CI validation are separate states.

At the first intended fallback mutation, invoke the real Connector operation and
let its authorization boundary handle any required approval. Only an actual
rejected mutation establishes a mutation failure. Verify every successful
mutation by rereading the remote branch/head.

### Codex or full-orchestrator task

Select Codex only under
[`EXECUTOR-SELECTION.md`](EXECUTOR-SELECTION.md): read-only advisory work, or
exclusive implementation ownership of a separate bounded branch. Select the
full orchestrator only when autonomous multi-checkpoint state, approvals,
bounded retries/replans, or explicit executor handoff must survive chat turns.

Do not create a durable goal or Codex implementation job merely to edit source,
run formatting, execute focused checks, inspect a diff, or publish one coherent
checkpoint.

### Resumed task

Before editing:

1. read the open task PR and its `ostudy-agent-state` compatibility snapshot;
2. verify the actual remote base, branch, and head;
3. inspect exact-head CI and unresolved review threads;
4. inspect the current writer workspace, active job, and unpublished diff;
5. enumerate non-terminal acceptance criteria and known failures;
6. confirm exclusive writer ownership or complete an explicit stopped handoff;
7. reconcile stale draft metadata before another global status claim.

Do not reconstruct task state from conversational memory.

## 2. Define the change before implementation

Record:

- the observed problem or requested behavior;
- stable `AC-##` acceptance criteria;
- non-goals;
- architecture and compatibility surfaces;
- expected automated tests;
- manual, physical-device, live-provider, credential, or hardware boundaries.

Search the repository for all callers, related tests, commands, migrations,
durable formats, and documentation before selecting an implementation.

Acceptance criteria remain stable identifiers throughout the PR. Update their
status and evidence rather than rewriting completed requirements to match
whatever happened to be implemented.

## 3. Establish one writer and publish the actual source

Use one short-lived `agent/<description>` branch and one draft pull request.
Exactly one implementation executor owns the branch at a time.

In direct-workspace mode, the persistent worktree may hold coherent unpublished
source while formatting and focused checks run. Publish the first meaningful
checkpoint once it is reviewable. The first commit may be an acceptance-test
skeleton, ADR, migration shell, focused controller, or another truthful vertical
checkpoint.

The PR body must distinguish:

- target goal and acceptance criteria;
- current published state;
- implementation evidence;
- exact-head validation evidence;
- pending manual boundaries;
- unresolved failures;
- current writer and last expected remote head.

Do not:

- keep the only copy of product source in an untracked scratch directory or CI
  runner;
- commit task-specific applicators/finalizers instead of resulting source;
- use a PR-triggered Action to rewrite, commit, or push the product tree;
- represent planning/scaffolding as completed implementation;
- create replacement branches for ordinary test failures;
- let Codex, direct workspace, and Connector alternate writes on one live branch;
- force-push to reconcile writer drift.

Automatic PR qualification is read-only. Any explicit source-writing maintenance
workflow is separate, manual-only, narrowly scoped, refuses `main`, and is not
ordinary product delivery.

## 4. Implement coherent vertical increments

Prefer commits such as:

1. domain/schema behavior and migration coverage;
2. native service and typed IPC;
3. controller and user interface;
4. focused regressions and acceptance coverage;
5. documentation and cleanup.

The exact sequence depends on the change. Each commit has one understandable
purpose, contains real reviewable source, and is pushed to the same branch.

- Follow the owning layer described in the architecture guardrails.
- Reuse current domain, IPC, platform, controller, and UI boundaries.
- Add regression coverage with the behavior.
- Avoid dependency changes and broad refactors unless they are part of the
  accepted scope.
- Update affected documentation and ADRs in the same branch.
- Update the PR compatibility acceptance snapshot after meaningful publication
  checkpoints, not after every local edit or command.
- When changing a shared Rust struct, enum, serialized shape, migration schema,
  or other cross-module contract, search the entire repository for constructors,
  struct literals, deserializers, compatibility projections, tests, and
  documentation before declaring the increment complete.
- Treat a migration file and its registered checksum as one atomic contract. If
  migration bytes change before release, update the checksum in the same
  coherent increment and immediately qualify the static boundary.
- For an architecture or monolith-removal refactor, publish the final ownership
  topology together with every required manifest, verifier, fixture, and
  regression-test update. Do not weaken checks to accommodate missing ownership.

In direct-workspace mode, inspect continuously with `workspace_status`,
`workspace_read`, and `workspace_diff`. In a human local checkout, use:

```bash
git status --short
git diff --check
git diff --stat
git diff
```

In emergency Connector-writer mode, inspect the remote changed-file list/diff
and current head through GitHub.

## 5. Validate in layers

Use the cheapest gate that answers the current question, then escalate only as
the change stabilizes:

```text
workspace_job(action="start", kind="operation", name="format")
workspace_job(action="start", kind="validation", name="changed")
workspace_job(action="start", kind="validation", name="static")
workspace_job(action="start", kind="validation", name="fast")
```

The corresponding repository commands are:

```bash
./scripts/check-changed.sh  # diff-aware iteration helper
./scripts/check-static.sh   # manifests, policy, docs, IPC, types, rustfmt
./scripts/check-fast.sh     # static + all frontend tests + locked Rust compile/Clippy/contracts
./scripts/check.sh          # authoritative Linux/Tauri merge qualification
```

Any edit or mutating operation invalidates prior direct-workspace validation
evidence. Required publication validations must pass against the exact current
HEAD and diff before `workspace_publish` will commit and push.

Draft pull requests run the fast tier. Ready pull requests run the authoritative
Linux and Android matrix on the self-hosted runner. A successful result qualifies
only the exact source SHA it tested. `OSTUDY_FORCE_CHECK=1` bypasses exact-tree
cache reuse when deliberately required.

Validation evidence is invalidated by every later source-tree change. Never
describe a predecessor SHA as the current head's passing evidence, even when the
later edit appears mechanical. Re-run the narrowest affected boundary, then
escalate again.

For migration or manifest work, run static qualification immediately after
changing migration bytes, registered checksums, version metadata, or generated
manifests. For a shared Rust-domain refactor, run a test-compiling Rust gate
before reviewer handoff so stale `#[cfg(test)]` struct literals and
warning-as-error failures cannot remain hidden behind frontend tests.

Manually exercise the affected UI path with `./run-app.sh`. For persistence work,
restart and verify the stored result. For UI work, include keyboard,
touch/coarse-pointer, pending/error/cancel states, compact/medium/wide layouts,
reduced motion, and relevant themes.

Additional checks depend on the change:

- **IPC:** frontend wrapper names, Rust registration, serialization, validation,
  stable errors.
- **Database:** fresh/upgraded databases, rollback, checksums, constraints, data
  preservation.
- **Review/scheduling:** rating mapping, due transition, immutable event creation,
  idempotence.
- **Import/export:** round-trip formats, assets, IDs, schedules, excluded personal
  history.
- **UI:** empty/loading/error states, keyboard access, repeated clicks,
  responsive containment, themes.
- **Android:** compile/package and emulator smoke; physical camera or device
  transfer remains manual where required.

## 6. Consume failures before another broad run

A failed workflow must produce and upload `ostudy-qualification-result.json`.
It records the exact head, gate, step outcomes, stable failure fingerprint,
boundary, failed tests, classification hint, diagnostic filter, and focused
reproduction.

After failure:

1. verify the tested SHA and current PR head;
2. read local job state/logs when the failure occurred before publication;
3. read GitHub steps, logs, machine-readable evidence, and bounded diagnostics
   when the failure occurred in CI;
4. identify all independent actionable failures;
5. classify each failure;
6. execute the narrowest reproduction;
7. repair one coherent batch in the same writer workspace;
8. rerun the focused boundary before escalating;
9. publish and verify the new exact head.

If static fails, consume the static failure before spending a fast/full run. If a
formatter reports exact diffs, run the configured formatter locally rather than
repeatedly approximating formatting by hand or using CI as the editor.

An unchanged-head rerun is allowed once only when evidence supports a transient
external failure. The same fingerprint twice requires a strategy change before
another broad run. Never use no-op commits, PR-state toggles, permission
expansion, blanket retries, longer timeouts, sleeps, test skips, or weakened
assertions merely to obtain green CI.

A host-enforced protected-branch failure in a synthetic test fixture is a
runner/test-contract defect, not permission to disable the host guard. Use a
non-protected fixture ref and keep explicit product tests for protected-branch
rejection.

## 7. Review and publish the exact proposed commit

In direct-workspace mode:

1. call `workspace_diff`;
2. review changed paths, modes, exact diff, local HEAD, remote HEAD, and complete
   worktree hash;
3. require exact current `static` and `fast` evidence;
4. call `workspace_publish` with the expected head, exact diff hash, and one-line
   commit subject;
5. verify the published branch and PR head through GitHub.

In a human local checkout, stage specific paths whenever practical:

```bash
git add -- <paths>
git diff --cached --check
git diff --cached --stat
git diff --cached
```

Confirm that no secrets, personal data, build output, unrelated formatting,
accidental lockfile changes, or task-specific source applicators are staged.

After committing:

```bash
git diff --name-only origin/main...HEAD
git diff --stat origin/main...HEAD
git log --oneline origin/main..HEAD
```

In Connector fallback mode, verify the corresponding remote branch/PR diff and
record the current remote head.

## 8. Maintain the pull-request acceptance snapshot

The compatibility snapshot is strictly machine-validated for non-draft PR
events; draft source checks do not depend on it.

For draft work it may contain incomplete criteria and pending gates. A
synchronize event may briefly make its recorded head stale. Reconcile it at the
next meaningful checkpoint. Before requesting or interpreting a final
qualification run, re-fetch the actual PR head and synchronize the snapshot to
that SHA.

Before review readiness:

- `currentState` is `readyForReview`;
- actual base, branch, and head exactly match the snapshot;
- every acceptance criterion is terminal and has evidence;
- static, fast, and full passed on the current head;
- Android passed or is explicitly `notRequired` for a non-Android change;
- manual paths are passed, `notRequired`, or explicitly `humanRequired`;
- `knownFailures` is empty;
- the selected writer has no unpublished source or active job and releases branch
  ownership for review.

`notRun` is not completion evidence and cannot qualify readiness. Do not hand a
branch to a reviewer as complete while any known automated failure remains.

## 9. Propose; do not self-merge

Open or update one pull request against `main`. The description records:

- exact base/current head;
- selected writer/executor and ownership state;
- goal, acceptance criteria, and non-goals;
- design decisions and tradeoffs;
- changed files grouped by purpose;
- tests and documentation;
- exact commands and CI runs that passed;
- commands not run and why;
- manual workflows actually exercised or still human-required;
- migration, compatibility, backup, rollback, known limitations, and residual
  risks.

Mark ready only after the compatibility snapshot and exact-head evidence satisfy
the readiness contract. A human reviews, accepts, and squash-merges. Additional
fixes stay on the same branch and PR.

## Validation language

Use exact statements:

- **Passed:** the named command completed successfully against the named tree/SHA.
- **Inspected:** source or output was reviewed without executing behavior.
- **Not run:** the check did not execute; reason recorded.
- **Manually verified:** the named behavior was actually exercised.
- **Published:** GitHub confirmed the remote branch/head.
- **Blocked:** a prerequisite prevents the next state; work is not complete.

Never say code is fully tested merely because it parses, compiles, or looks
correct. Never say a Connector mutation exists remotely because a local patch
was generated. Never say a task is complete while required acceptance,
exact-head validation, manual boundaries, known failures, or branch ownership
remain unresolved.
