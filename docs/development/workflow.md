# Git and pull-request workflow

ostudy uses one focused branch, one persistent writer workspace, one draft pull
request, directly reviewable source, a compatibility acceptance snapshot, and
exact-head qualification. Planning supports delivery, but a code task is not
represented by an unpublished patch, task-specific applicator, transient CI
tree, or description of changes that exist only in a temporary environment.

This process applies to humans and automated maintainers. The repository access
mechanism may differ, but the authority and safety invariants do not.

## Current execution policy

Ordinary ChatGPT development uses GitHub plus `dev-workspace-v1`:

```text
GitHub resolves current remote state
→ one persistent task worktree owns unpublished source
→ ChatGPT edits and runs focused checks locally
→ the exact validated diff is committed and non-force pushed
→ GitHub PR and Actions qualify the published SHA
```

This remains the default when Codex credits are available. Codex is optional:

- advisory Codex may perform read-only planning, review, or diagnosis while the
  direct workspace remains the sole writer;
- implementation Codex may exclusively own a separate deliberately assigned
  bounded branch;
- direct workspace, Codex, and low-level Connector writes must not alternate on
  one live branch.

Use the full orchestrator only for genuinely autonomous multi-checkpoint goals
that need durable approvals, bounded retries/replans, repair budgets, or an
explicit executor handoff across chat turns. Use the observer for read-only
diagnostics and exact-head for isolated qualification of published source.

The authoritative agent policies are:

- [`../agents/DIRECT-WORKSPACE.md`](../agents/DIRECT-WORKSPACE.md)
- [`../agents/EXECUTOR-SELECTION.md`](../agents/EXECUTOR-SELECTION.md)
- [`../agents/WORKFLOW-AND-VALIDATION.md`](../agents/WORKFLOW-AND-VALIDATION.md)

## Authority and writer boundaries

| Concern | Authority | Allowed writer |
|---|---|---|
| Requested scope | Current human request and explicitly approved stable acceptance criteria | Human and active task owner by explicit agreement |
| Unpublished source | HEAD, bytes, status, and active job of the selected writer workspace | Exactly one selected branch writer |
| Published source and PR head | GitHub remote task branch and exact SHA | The selected writer through a non-force publication path |
| Automated qualification | GitHub Actions or exact-head evidence attached to the exact tested SHA | Read-only validators record results but never author source |
| Active task/job state | PID/job verified in the runtime that launched it | That runtime only |
| Compatibility acceptance snapshot | Projection of scope, publication, and evidence | Active task owner at meaningful checkpoints |
| Manual acceptance | Concrete report from the person who exercised the workflow | That tester |

PR descriptions, comments, summaries, observers, runner output, and generated
artifacts may project these facts but do not override their authority.

Exactly one implementation writer owns a task branch at a time. Before each
publication, the writer rereads the remote head, requires it as the expected
parent, and updates without force. A mismatch is a stop-and-reconcile boundary.
It is not permission to overwrite, create a replacement branch, or start a
competing repair.

## 1. Establish or restore the exact task state

### New direct-workspace task

Use GitHub to read the exact current `main` SHA, then call:

```text
workspace_open(
  project="ostudy",
  branch="agent/<short-description>",
  base_sha="<exact-current-main-sha>"
)
```

Require:

- the effective server contract contains exactly seven expected tools;
- local and remote heads equal the intended base;
- no unexplained dirty state exists;
- no command is active;
- direct editing, symbolic commands, and non-force publication are available.

For an existing branch, pass its current exact remote head. Do not use an old
conversation's SHA.

### New human local-checkout task

```bash
cd ~/code/ostudy
git status --short
git fetch origin --prune
git switch main
git pull --ff-only
BASE_SHA="$(git rev-parse HEAD)"
BRANCH="agent/short-description"
WORKTREE="../ostudy-short-description"
git worktree add -b "$BRANCH" "$WORKTREE" "$BASE_SHA"
cd "$WORKTREE"
```

Stop before switching, rebasing, resetting, or cleaning when unrelated changes
exist. Preserve them in a separate branch, commit, stash, or worktree.

### Connector fallback task

GitHub Connector remains the normal remote repository, PR, review, and CI
interface. Use its low-level source writer only when no persistent writer
workspace is available and the human explicitly accepts that fallback. Bind
every commit to the verified expected remote parent and use a non-force ref
update.

See [`../agents/GITHUB-CONNECTOR.md`](../agents/GITHUB-CONNECTOR.md).

### Codex or orchestrator task

Select Codex only for read-only advice or exclusive ownership of a separate
bounded branch. Select the full orchestrator only when durable autonomous state
is actually required. Confirm credits, runtime health, complete tool contract,
allowed paths, and branch ownership once; do not repeatedly rediscover them.

### Resuming an existing task

Before editing:

1. read the open PR and its compatibility acceptance snapshot;
2. verify actual remote base, branch, and head;
3. inspect exact-head CI and unresolved review threads;
4. inspect the selected writer workspace, active job, and unpublished diff;
5. enumerate incomplete criteria and known failures;
6. confirm exclusive writer ownership or complete an explicit stopped handoff;
7. reconcile stale draft metadata at the next meaningful checkpoint.

Do not reconstruct state from conversational memory.

## 2. Define a verifiable change contract

Record:

- the observed problem and requested behavior;
- stable `AC-##` acceptance criteria;
- explicit non-goals;
- architecture and compatibility surfaces;
- migration, data, import/export, platform, privacy, accessibility, and
  dependency effects;
- expected automated tests and manual workflows.

Acceptance identifiers remain stable. Change requirements only when the human
changes intended behavior; do not rewrite criteria after implementation to hide
missing work.

The acceptance snapshot and completion predicate are documented in
[`../agents/EXECUTION-STATE.md`](../agents/EXECUTION-STATE.md).

## 3. Create or bind one focused branch and workspace

Use a short-lived branch:

```text
agent/<description>
```

The direct workspace creates the remote branch from the exact current base when
it does not exist and allocates a deterministic persistent worktree. A human
local checkout may create and push the branch conventionally.

Do not develop directly on `main`. Do not create competing replacement branches
for ordinary failures. Do not let Codex, direct workspace, and Connector share
or alternate branch ownership.

A writer transfer is exceptional and requires:

1. no active implementation, operation, validation, or publication job;
2. exact current remote head;
3. clean or fully accounted-for unpublished worktree state;
4. explicit release by the previous writer;
5. exact binding by the receiving writer;
6. no force push.

## 4. Implement and publish coherent vertical increments

Inspect current code, callers, tests, migrations, fixtures, external formats,
and documentation before introducing a path. Prefer increments such as:

1. schema/domain behavior and migration coverage;
2. native service and typed IPC;
3. controller and interface;
4. regression and acceptance tests;
5. documentation and cleanup.

The persistent worktree may contain coherent unpublished source during local
iteration. Publish a meaningful checkpoint once it is reviewable; do not create
transport-only scaffolds merely to avoid losing work.

The first meaningful commit may be an acceptance-test skeleton, ADR, migration
shell, controller boundary, or another truthful vertical checkpoint. It must
contain actual reviewable source.

After each meaningful publication checkpoint:

- verify the GitHub remote head;
- update acceptance status, evidence, tests, and known failures;
- record the selected writer and expected remote SHA;
- record only validation actually performed against a named tree/SHA.

Avoid mixing feature behavior, dependency upgrades, broad refactors, mass
formatting, and unrelated cleanup unless the task explicitly requires the
combination.

A structural refactor must publish target ownership, replacement paths,
manifests, verifiers, fixtures, and regressions together. Do not weaken a guard
because the replacement topology was not completed.

## 5. Format and validate during development

In direct-workspace mode, use symbolic jobs:

```text
workspace_job(action="start", kind="operation", name="format")
workspace_job(action="start", kind="validation", name="changed")
workspace_job(action="start", kind="validation", name="static")
workspace_job(action="start", kind="validation", name="fast")
```

The corresponding maintained commands are:

```bash
./scripts/check-changed.sh
./scripts/check-static.sh
./scripts/check-fast.sh
./scripts/check.sh
```

Use the cheapest gate that answers the current question. Formatting, type
checking, focused unit tests, and test compilation should ordinarily happen
before publication. CI is the exact-published-SHA qualification environment, not
the normal formatter, editor, or first compiler.

Any edit or mutating operation invalidates prior workspace validation evidence.
`workspace_publish` requires current configured publication validations against
the exact head and diff.

A validation must not mutate the worktree. A configured operation may mutate
only its declared path roots. Long commands run asynchronously; inspect bounded
status/log output and cancel the process group when required rather than using
chat-side sleeps.

For migration or manifest work, run static immediately after changing migration
bytes, registered checksums, version metadata, or generated manifests. For a
shared Rust contract, run a test-compiling gate before publication so stale test
constructors and warnings cannot remain hidden.

## 6. Use Actions and exact-head for independent qualification

Draft PR source updates run static and fast qualification. Ready PRs run the
full Linux/Tauri and Android matrix. Explicit manual dispatch supports:

```text
preflight
static
fast
linux-full
android-full
all-full
```

Every run checks out and verifies the exact target SHA. A newer source push
invalidates an older green result. Automatic qualification is source-read-only
and may not stage, commit, push, alter PR readiness, or persist write credentials.

Use exact-head validation when one already-published SHA needs independent
clean-room reproduction or qualification without involving a mutable task
worktree.

Wall-clock UI/WebKit timing remains advisory or manual diagnostic evidence and
must not suppress correctness gates.

## 7. Consume failures before another broad run

When a local workspace job fails:

1. verify the workspace head and diff;
2. read the symbolic job status and bounded log;
3. classify the failure;
4. reproduce the narrow boundary;
5. repair one coherent batch in the same writer workspace;
6. rerun the focused job before publication.

When CI fails:

1. verify the tested SHA and current PR head;
2. read job steps, decoded logs, `ostudy-qualification-result.json`, and bounded
   diagnostics;
3. identify all independent actionable failures;
4. classify each failure using
   [`../agents/FAILURE-TRIAGE.md`](../agents/FAILURE-TRIAGE.md);
5. reproduce the narrow boundary locally when possible;
6. repair a coherent batch on the same branch;
7. requalify locally, publish, and then rerun the appropriate remote gate.

If a formatter reports exact differences, run the formatter locally rather than
approximating the output by hand. If a guarded development host rejects a
synthetic `main` push, use a non-protected fixture ref and retain the host guard;
do not disable or bypass branch protection for tests.

An unchanged-head retry is allowed once only with evidence of a transient
external cause. The same fingerprint twice requires a strategy change. Do not
use no-op commits, PR-state toggles, permission expansion, arbitrary sleeps,
longer timeouts, skipped tests, or weakened assertions to manufacture a pass.

## 8. Review the exact proposed source

In direct-workspace mode, call `workspace_diff` and inspect:

- changed paths and modes;
- exact local and remote heads;
- complete bounded diff and truncation state;
- exact complete-worktree hash;
- validation evidence bound to that head and hash.

Publish with `workspace_publish`, then verify the resulting branch and PR head
through GitHub.

In a human local checkout:

```bash
git add -- <paths>
git diff --cached --check
git diff --cached --stat
git diff --cached
git commit -m "<subject>"
git push
```

Confirm that no secrets, personal data, build output, unrelated formatting,
accidental lockfile changes, or task-specific source applicators are included.

## 9. Maintain the PR acceptance snapshot

Open one draft PR after the first coherent published checkpoint. Record:

- exact base and current head;
- current writer/execution mode and expected remote SHA;
- goal, stable acceptance criteria, and non-goals;
- architecture/compatibility surfaces;
- implementation and test evidence;
- automated and manual validation state;
- known failures and residual risks.

The compatibility snapshot is evidence, not a branch lock, runner state store,
or CI trigger. Update it at meaningful checkpoints and before readiness—not
after every local edit or command.

Before readiness:

- `currentState` is `readyForReview`;
- snapshot base, branch, and head match GitHub exactly;
- every acceptance criterion is terminal and evidenced;
- static, fast, and full passed on the exact current head;
- Android passed or is explicitly `notRequired`;
- manual paths are passed, `notRequired`, or explicitly `humanRequired`;
- `knownFailures` is empty;
- the writer has no unpublished source or active job and releases ownership for
  review.

## 10. Human review, merge, and synchronization

An automated maintainer proposes source and does not merge its own PR unless the
repository owner explicitly requests that operation. The normal method is
**Squash and merge** after exact-head qualification and human acceptance.

After merge in a human local checkout:

```bash
git switch main
git pull --ff-only
git branch -d <merged-branch>
git push origin --delete <merged-branch>
```

Remove a managed worktree only after all unpublished source and evidence have
been reviewed. Do not use forced removal as a routine cleanup mechanism.

## Explicit maintenance workflows

Ordinary `Check` qualification is read-only. The separately named `Maintain
generated Rust candidates` workflow is manual-only, requires explicit
confirmation, refuses `main`, and restricts changes to canonical Rust
source/lockfile paths. It is an escape hatch, not the normal implementation
path.

## Releases

A normal merge does not automatically change the application version. A release
updates together:

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

Reverse a bad merged commit with a new revert rather than rewriting shared
history:

```bash
git revert <commit-sha>
git push origin main
```

A migration or external-format rollback may require a forward repair. Document
data effects and recovery in the original PR.

If a development direction is unsound, stop adding source, preserve the
published branch and persistent worktree, and correct it transparently. A clean
replacement branch is reserved for a genuinely unrecoverable branch, not an
ordinary failure.

If a direct-workspace service or Git credential fails, preserve its state and
worktree. Repair the service or authentication path without reconstructing
source. If Codex fails, preserve any owned diff through the bounded handoff and
continue direct-workspace work only after explicit ownership transfer.

## Completion record

A handoff identifies:

- base SHA, branch, head SHA, and PR number;
- selected writer/execution mode and last expected remote SHA;
- workspace path, unpublished clean/dirty state, and active PID/job;
- acceptance status and non-goals;
- files and migrations changed;
- tests and documentation added;
- local commands and exact results;
- exact-head CI state and qualification artifact;
- manual acceptance performed or still human-required;
- known failures, limitations, compatibility effects, rollback concerns, and
  merge method;
- explicit release or transfer of branch writer ownership.

The durable result is published source plus exact evidence. Completion is a
conjunction of acceptance, publication, validation, manual boundaries, and no
unresolved failure—not a narrative impression that substantial activity
occurred.
