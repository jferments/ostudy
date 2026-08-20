# GitHub Connector workflow

This document defines the repository-access workflow for ChatGPT or another agent operating through an authenticated GitHub Connector rather than a normal local checkout.

The repository invariants in [`../../AGENTS.md`](../../AGENTS.md), [`EXECUTION-STATE.md`](EXECUTION-STATE.md), [`FAILURE-TRIAGE.md`](FAILURE-TRIAGE.md), [`WORKFLOW-AND-VALIDATION.md`](WORKFLOW-AND-VALIDATION.md), and [`../development/workflow.md`](../development/workflow.md) still apply. This document only defines how to establish and verify GitHub state in a connector-backed environment.

## 1. Choose connector mode explicitly

When an authenticated GitHub Connector is available for `jferments/ostudy`, use it as the source of truth for remote repository state.

Do not substitute:

- `gh` CLI availability;
- an unrelated local checkout;
- anonymous HTTP, scraping, or raw unauthenticated URLs;
- temporary transfer repositories;
- guessed branch state;
- CI runner availability.

Those mechanisms do not establish whether the Connector can read or write the private repository.

Record `executionMode: githubConnector` in the pull-request state ledger. Use `githubConnectorWithExecutionWorkspace` only when a separate workspace can execute the exact published source; that workspace is never the sole location of the implementation.

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
- an action missing from a currently loaded tool list is a tool-surface limitation, not a repository permission result;
- the absence of an approval prompt is not evidence that writes are unavailable;
- a queued, offline, or mismatched runner says nothing about branch/commit/PR publication;
- a local checkout that lacks a remote commit says nothing about the connector-backed branch;
- an Actions API limitation does not prove that a workflow did or did not run.

## 3. Establish repository and base state with reads

Before editing, use Connector reads to:

1. resolve `jferments/ostudy`;
2. confirm the default branch and repository permissions exposed by the Connector;
3. fetch the intended base, normally current `main`;
4. record the exact base commit SHA;
5. inspect the relevant files, active pull request, state ledger, and branch state.

When resuming work, also read the actual PR head, exact-head workflow state, unresolved review threads, and incomplete acceptance criteria before changing source.

## 4. Cross the write boundary with the real mutation

When the task reaches its first intended repository mutation, invoke the actual Connector write operation, such as creating the task branch or creating/updating a file on that branch.

Let the Connector authorization layer handle any required user approval. Do not simulate the write locally and then infer that GitHub is inaccessible. Do not change the Connector to permanent or unrestricted write access merely to avoid a write prompt.

Only an actual failed mutation establishes a mutation failure. Report:

- the exact connector operation;
- the target repository/branch/path;
- the exact error;
- whether repository reads still succeed;
- and whether the failure is authorization, unsupported action family, invalid request, or suspected GitHub incident.

### Truthful status examples

Before a mutation:

```text
Authenticated repository reads succeed and GitHub reports push permission.
No mutation has been attempted in this session, so concrete write authorization is untested.
```

After successful publication:

```text
create_file succeeded on agent/example. GitHub now reports remote head abc123.
CI state is separate and currently queued.
```

After a rejected write:

```text
create_file on agent/example/path was rejected with <exact connector error>.
Repository reads still succeed. This establishes failure of that concrete operation, not a general GitHub outage.
```

Unsupported statement:

```text
I do not have GitHub access.
```

unless the relevant operation was actually attempted and the exact evidence supports that narrow statement.

## 5. Publish the actual source through GitHub

For a normal change:

1. create one short-lived `agent/<description>` branch from the recorded base SHA;
2. make the first meaningful source/test/ADR commit through Connector file or Git-data operations;
3. verify the remote branch head;
4. open one draft pull request against `main`;
5. initialize the state ledger with stable `AC-##` criteria and truthful current status;
6. continue direct source changes on the same branch and PR;
7. verify the new remote head after every meaningful publication step.

A generated patch, proposed commit message, transient execution workspace, task-specific applicator, or CI-generated product tree is not delivery. The branch must contain the actual reviewable source before qualification.

Automatic pull-request CI is read-only and may not author commits, push source, persist write credentials, or change PR readiness. Explicit manual maintenance workflows are separate from ordinary qualification and may never push directly to `main`.

## 6. Keep the PR state ledger current

After each coherent publication checkpoint:

- verify the actual remote head;
- update criterion evidence and tests;
- update `currentState`;
- record known failure fingerprints and focused reproductions;
- record only validation that actually passed, with the exact tested SHA;
- preserve pending human-only boundaries explicitly.

A draft synchronize event can race a PR-body edit, so the verifier warns rather than fails when a draft ledger temporarily lags the actual head. Before readiness, the ledger must exactly match the GitHub base, branch, and head, and static, fast, and full must pass on that head.

## 7. Keep CI and repository access independent

CI runner state does not determine Connector access.

A missing, offline, busy, or mismatched self-hosted runner may prevent validation, but it does not prevent branch creation, commits, or pull-request updates. Conversely, a healthy runner does not prove that the Connector can mutate the repository.

Report these separately:

- **Repository publication state:** branch, head SHA, PR state.
- **Validation state:** workflow run, runner availability, gate, tested SHA, result, and failure fingerprint.
- **Manual acceptance state:** concrete workflow exercised or still assigned to the human.

## 8. Consume exact-head evidence before another repair

When the `Check` workflow fails:

1. verify which SHA ran and whether it is still the PR head;
2. fetch the job steps and decoded logs;
3. inspect the uploaded `ostudy-qualification-result.json` and bounded diagnostic evidence when connector access permits;
4. classify the failure using [`FAILURE-TRIAGE.md`](FAILURE-TRIAGE.md);
5. use the recorded focused reproduction before another broad run;
6. update `knownFailures` with the stable fingerprint and attempt count;
7. publish a coherent repair batch on the same branch.

Do not ask the human to copy logs that the connector can already read. Do not rerun the complete matrix while a narrower deterministic failure remains.

## 9. Distinguish repository defects, connector limitations, and GitHub incidents

### Repository or workflow defect

Evidence points to the repository when a deterministic local command or completed workflow log reports a concrete source, test, manifest, permission, or runner-contract error. Fix that defect and add the narrowest useful regression.

### Connector permission or capability boundary

A stable `403 Resource not accessible by integration` from one API family while ordinary repository reads and writes continue to succeed usually indicates that the connected GitHub App does not expose that endpoint or permission. It does **not** prove that GitHub Actions failed to run and must not be worked around by expanding the repository workflow token.

Record the unsupported read and use another already-authorized GitHub surface when human verification is required. Keep publication state and validation state separate.

### GitHub service incident

Repeated `5xx`, `502`, timeout, archive/raw download, webhook, pull-request, Actions, or broad API failures across otherwise unrelated operations require an official GitHub Status check before repository changes are made.

During a confirmed or strongly suspected service incident:

1. record the exact branch head SHA and the first failing operation, timestamp, HTTP class, and affected GitHub surface;
2. check the official GitHub Status incident and note the affected components;
3. retry only bounded, idempotent reads needed to determine whether a write landed;
4. do not create no-op commits, close/reopen the PR, toggle draft state, edit unrelated files, widen workflow permissions, add status-mirroring calls, or change triggers merely to force acknowledgement;
5. do not interpret a missing Actions run, check-run, webhook, or empty legacy commit-status response as a product-code result;
6. continue only work that can be validated independently of the degraded service;
7. keep the PR draft and report the exact head as unqualified;
8. after recovery, run the normal exact-head workflow without incident-specific shims.

The dated audit in [`../history/github-outage-audit-2026-08-17.md`](../history/github-outage-audit-2026-08-17.md) records why these rules were added.

## 10. Required connector-mode handoff

In addition to the normal handoff requirements, report:

- the base SHA read through GitHub;
- the task branch name;
- the current remote head SHA;
- the pull-request number and draft/ready state;
- whether each claimed mutation was verified remotely;
- any Connector operation that failed and its exact error;
- whether an official GitHub incident affected the work window;
- CI state separately from publication state;
- the exact head SHA that passed each required gate;
- manual acceptance performed or still assigned to the human.

## Non-negotiable reasoning rules

Do not make these inferences:

- "My local checkout does not show it, therefore GitHub does not have it."
- "I have not seen an approval prompt, therefore writes are unavailable."
- "The tool list does not currently show a write action, therefore the repository is read-only."
- "CI has no runner, therefore I cannot commit."
- "The Connector can read, so I should change its permanent permission setting before trying a write."
- "The Actions API returned 403, therefore the workflow did not run."
- "The legacy commit-status list is empty, therefore the Actions check is absent."
- "GitHub is degraded, therefore I should mutate the PR or workflow until a run appears."
- "A source patch exists in a runner, therefore it has been published."

The normal sequence is: **read and restore durable task state, invoke the real mutation when needed, verify the remote result, consume exact-head evidence, classify failures, and keep publication, validation, and manual acceptance separate.**
