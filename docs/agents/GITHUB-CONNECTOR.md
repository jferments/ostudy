# GitHub Connector workflow

This document defines the remote repository, pull-request, review, and CI
workflow for ChatGPT or another agent using the authenticated GitHub Connector.
It does not make the Connector the normal source editor or local test runner.

For ordinary implementation, use
[`DIRECT-WORKSPACE.md`](DIRECT-WORKSPACE.md): GitHub supplies the current base and
published state, while `dev-workspace-v1` owns the persistent worktree, edits,
local commands, commit, and non-force push. Read
[`EXECUTOR-SELECTION.md`](EXECUTOR-SELECTION.md) before selecting Codex, the full
orchestrator, or a low-level Connector writer.

The repository invariants in [`../../AGENTS.md`](../../AGENTS.md),
[`EXECUTION-STATE.md`](EXECUTION-STATE.md),
[`FAILURE-TRIAGE.md`](FAILURE-TRIAGE.md),
[`WORKFLOW-AND-VALIDATION.md`](WORKFLOW-AND-VALIDATION.md), and
[`../development/workflow.md`](../development/workflow.md) still apply.

## 1. Use the Connector for remote authority

When an authenticated GitHub Connector is available for `jferments/ostudy`, use
it as the source of truth for:

- repository and current `main` state;
- task-branch and pull-request heads;
- PR descriptions, comments, reviews, and readiness;
- Actions runs, jobs, logs, and artifacts;
- final merge state.

Do not substitute:

- `gh` CLI availability;
- an unrelated local checkout;
- anonymous HTTP, scraping, or raw unauthenticated URLs;
- temporary transfer repositories;
- guessed branch state;
- CI runner availability.

Those mechanisms do not establish the authenticated Connector's view of the
private repository.

For direct-workspace work, record `executionMode:
githubConnectorWithExecutionWorkspace` in the compatibility snapshot. The
persistent workspace may contain unpublished implementation, but GitHub remains
authoritative for every published SHA. Use `githubConnector` only when the
Connector itself is the deliberately selected fallback writer.

## 2. Model GitHub state as separate dimensions

Do not collapse these states into a single phrase such as "GitHub access":

| State | Evidence |
|---|---|
| Repository visibility | An authenticated repository read succeeds |
| Repository-level permission | GitHub reports the account/app permission |
| Connector action exposure | The relevant connector operation exists in the current tool surface |
| Concrete write authorization | The actual mutation is accepted or rejected |
| Publication | GitHub confirms the resulting remote branch/head |
| Validation | CI or an execution environment tests that exact head |

None of these states proves another. In particular:

- successful reads do not prove or disprove a later write authorization;
- an action missing from a currently loaded tool list is a tool-surface
  limitation, not a repository permission result;
- the absence of an approval prompt is not evidence that writes are unavailable;
- a queued, offline, or mismatched runner says nothing about publication;
- a local checkout that lacks a remote commit says nothing about GitHub state;
- an Actions API limitation does not prove that a workflow did or did not run.

## 3. Establish repository and base state with reads

Before implementation, use Connector reads to:

1. resolve `jferments/ostudy`;
2. confirm the default branch and repository permissions exposed by the
   Connector;
3. fetch the intended base, normally current `main`;
4. record the exact base commit SHA;
5. inspect the relevant files, active pull request, compatibility snapshot, and
   task-branch state;
6. pass that exact SHA to the selected writer workspace.

When resuming work, also read the actual PR head, exact-head workflow state,
unresolved review threads, incomplete acceptance criteria, and any recorded
writer handoff before changing source.

## 4. Normal direct-workspace publication

The direct workspace is the normal writer. Its sequence is:

1. GitHub resolves the exact base or existing task-branch head.
2. `workspace_open` binds one persistent worktree to that exact head.
3. ChatGPT reads and edits source through the workspace.
4. Allowlisted formatting and focused validations run locally.
5. `workspace_diff` exposes the exact proposed diff and hash.
6. `workspace_publish` commits and non-force pushes only after required exact
   worktree validations pass.
7. GitHub is reread to confirm the resulting branch and PR head.
8. Actions qualifies that exact published SHA.

Do not duplicate those edits through low-level Connector writes. Exactly one
writer owns the branch.

## 5. Connector source writing is an emergency fallback

Use the Connector as a source writer only when no persistent writer-capable
workspace is available and the human still wants the bounded change completed
through GitHub. Do not choose it merely because its write actions are visible.

At the first intended fallback mutation, invoke the actual Connector operation
and let its authorization boundary handle any required approval. Do not simulate
the write locally and infer that GitHub is inaccessible. Do not change the
Connector to unrestricted access merely to avoid a write prompt.

Only an actual failed mutation establishes a mutation failure. Report:

- the exact Connector operation;
- target repository, branch, and path/ref;
- exact error;
- whether repository reads still succeed;
- whether failure is authorization, missing action family, invalid request, or a
  suspected GitHub incident.

### Expected-head fallback publication protocol

The selected Connector session becomes the branch's only source writer until
explicit handoff. Immediately before every commit/ref mutation:

1. fetch the remote branch head;
2. require it to equal the last verified expected head;
3. create the new commit with that head as its parent;
4. update the ref without force;
5. read the remote head again and record the confirmed SHA.

For multi-file publication, use Git-data operations that bind the expected
parent (`create_tree` → `create_commit(parent_sha=expected)` → non-force
`update_ref`). A Contents API update binds a file blob, not the branch head, and
cannot satisfy the stop-on-branch-mismatch protocol for a concurrent task.

If the expected head changed, stop. Do not force-update, replay an unreviewed
local tree, start a replacement branch, or let another chat independently repair
the same branch.

A generated patch, proposed commit message, transient execution workspace,
task-specific applicator, or CI-generated product tree is not delivery. GitHub
must contain the actual reviewable source before qualification.

## 6. Codex and Connector interaction

Codex is optional under
[`EXECUTOR-SELECTION.md`](EXECUTOR-SELECTION.md). An advisory Codex run may read
published GitHub source without taking writer ownership. An implementation Codex
run owns its own bounded branch exclusively.

Do not let direct workspace, Codex, and Connector alternate source writes on one
live branch. A writer transfer requires a stopped boundary, exact remote head,
no active job, a clean or fully accounted-for worktree, and explicit ownership
release.

## 7. Keep the PR acceptance snapshot current

After each meaningful publication checkpoint:

- verify the actual remote head;
- update criterion evidence and tests;
- update `currentState`;
- record the selected writer and last expected remote SHA;
- record known failure fingerprints and focused reproductions;
- record only validation that actually passed against the exact tested SHA;
- preserve pending human-only boundaries explicitly.

Do not update the compatibility snapshot after every local edit or command. A
draft synchronize event can race a PR-body update, so temporary draft drift is a
warning. Before readiness, the snapshot must exactly match the GitHub base,
branch, and head, and every required gate must pass on that head.

## 8. Keep CI and repository access independent

CI runner state does not determine Connector access.

A missing, offline, busy, or mismatched self-hosted runner may prevent
validation, but it does not prevent branch creation, commits, or pull-request
updates. Conversely, a healthy runner does not prove that a Connector mutation
will be authorized.

Report separately:

- **Workspace state:** unpublished diff, active command, local head, validations.
- **Repository publication state:** branch, head SHA, PR state.
- **Validation state:** workflow run, runner, gate, tested SHA, result, and
  failure fingerprint.
- **Manual acceptance state:** concrete workflow exercised or still human-owned.

## 9. Consume exact-head evidence before another repair

When the `Check` workflow fails:

1. verify which SHA ran and whether it is still the PR head;
2. fetch job steps and decoded logs;
3. inspect `ostudy-qualification-result.json` and bounded diagnostics when
   available;
4. classify the failure using [`FAILURE-TRIAGE.md`](FAILURE-TRIAGE.md);
5. use the recorded focused reproduction before another broad run;
6. update `knownFailures` with the stable fingerprint and attempt count;
7. repair one coherent batch in the current writer workspace;
8. rerun the local focused boundary before publication when possible.

Do not ask the human to copy logs the Connector can already read. Do not use CI
as the formatter or first compiler. Do not rerun the full matrix while a narrow
deterministic failure remains.

## 10. Distinguish repository, workspace, Connector, and GitHub failures

### Repository or workflow defect

A deterministic local command or completed workflow log reports a concrete
source, test, manifest, permission, or runner-contract error. Fix the defect and
add the narrowest useful regression.

### Workspace defect

A workspace read, apply, symbolic command, diff, or publication action fails
while GitHub reads remain healthy. Preserve the persistent worktree and report
the exact workspace operation and error. Do not replace the branch or promote a
read-only observer into a writer.

### Connector permission or capability boundary

A stable `403 Resource not accessible by integration` from one API family while
ordinary repository operations continue usually indicates that the connected
GitHub App does not expose that endpoint or permission. It does not prove GitHub
Actions failed to run and must not be worked around by widening workflow tokens.

### GitHub service incident

Repeated `5xx`, timeout, archive/raw download, webhook, PR, Actions, or broad API
failures across unrelated operations require an official GitHub Status check
before repository changes are made.

During a confirmed or strongly suspected incident:

1. record exact branch head, first failing operation, timestamp, HTTP class, and
   affected surface;
2. check the official incident and note affected components;
3. retry only bounded idempotent reads needed to determine whether a write
   landed;
4. do not create no-op commits, toggle PR state, widen permissions, change
   triggers, or add status-mirroring code merely to force acknowledgement;
5. do not interpret a missing workflow/check/status response as a product result;
6. continue only work that can be validated independently of the degraded
   service;
7. keep the PR draft and report the exact head as unqualified;
8. after recovery, use the normal exact-head workflow without incident shims.

## 11. Truthful status examples

Before a Connector fallback mutation:

```text
Authenticated repository reads succeed. Direct-workspace publication is
unavailable, and no Connector mutation has yet been attempted.
```

After direct-workspace publication:

```text
workspace_publish advanced agent/example to abc123. GitHub confirms abc123 as
the PR head. CI is separate and currently queued.
```

After a rejected Connector fallback write:

```text
create_commit/update_ref on agent/example was rejected with <exact error>.
Repository reads still succeed. This establishes failure of that concrete
operation, not a general GitHub outage.
```

Unsupported statement:

```text
I do not have GitHub access.
```

unless the exact relevant operation was attempted and the evidence supports that
narrow claim.

## 12. Required Connector-backed handoff

Report:

- base SHA read through GitHub;
- task branch and current remote head;
- selected writer/execution mode and last expected remote SHA;
- workspace path, clean/dirty state, and active PID/job when applicable;
- PR number and draft/ready state;
- whether each claimed mutation was verified remotely;
- any Connector operation that failed and its exact error;
- whether an official GitHub incident affected the work window;
- CI state separately from publication state;
- exact head that passed each required gate;
- manual acceptance performed or still human-owned;
- explicit release or transfer of task-branch writer ownership.

## Non-negotiable reasoning rules

Do not infer:

- "My local checkout does not show it, therefore GitHub does not have it."
- "I have not seen an approval prompt, therefore writes are unavailable."
- "A tool is absent, therefore repository permission is read-only."
- "CI has no runner, therefore I cannot commit."
- "The Connector can read, so I should widen its permanent permissions before
  trying a write."
- "The Actions API returned 403, therefore the workflow did not run."
- "A legacy status list is empty, therefore the Actions check is absent."
- "GitHub is degraded, therefore I should mutate the PR until a run appears."
- "A patch exists in a runner, therefore it has been published."
- "Codex credits are available, therefore Codex should share this branch."

The normal sequence is: **read GitHub state, bind one writer, implement and test
in its persistent workspace, publish non-force, verify the remote result,
consume exact-head evidence, and keep workspace, publication, validation, and
manual acceptance separate.**
