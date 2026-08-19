# GitHub Connector workflow

This document defines the repository-access workflow for ChatGPT or another agent operating through an authenticated GitHub Connector rather than a normal local checkout.

The repository invariants in [`../../AGENTS.md`](../../AGENTS.md), [`WORKFLOW-AND-VALIDATION.md`](WORKFLOW-AND-VALIDATION.md), and [`../development/workflow.md`](../development/workflow.md) still apply. This document only defines how to establish and verify GitHub state in a connector-backed environment.

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

## 2. Establish repository and base state with reads

Before editing, use Connector reads to:

1. resolve `jferments/ostudy`;
2. confirm the default branch and repository permissions exposed by the Connector;
3. fetch the intended base, normally current `main`;
4. record the exact base commit SHA;
5. inspect the relevant files, active pull request, and branch state.

Successful reads establish that the authenticated Connector can see the repository. They do not by themselves prove or disprove authorization for a later write.

## 3. Keep read access and write authorization separate

Connector/plugin policy and authorization for a concrete mutation are separate concerns.

The normal ostudy ChatGPT policy is read access without repeated approval and explicit authorization before repository changes. Do not change the Connector to permanent or unrestricted write access merely to avoid a write prompt.

The absence of a visible approval prompt is not evidence that writes are unavailable. Likewise, a successful read followed by no attempted mutation is not evidence of read-only access.

## 4. Cross the write boundary with the real mutation

When the task reaches its first intended repository mutation, invoke the actual Connector write operation, such as creating the task branch or creating/updating a file on that branch.

Let the Connector's authorization layer handle any required user approval. Do not simulate the write locally and then infer that GitHub is inaccessible.

If the mutation fails for authorization reasons, report the exact Connector error and operation that failed. Do not generalize the failure into claims such as "GitHub writes are unavailable" unless an actual Connector write was attempted and rejected.

## 5. Publish and verify through GitHub

For a normal change:

1. create one short-lived `agent/<description>` branch from the recorded base SHA;
2. make the first meaningful commit through Connector file/commit operations;
3. verify the branch head remotely;
4. open one draft pull request against `main`;
5. continue subsequent changes on the same branch and PR;
6. verify the current remote head after each meaningful publication step.

A generated patch, local working tree, or proposed commit message is not delivery. Do not claim that a commit or file change exists remotely until the Connector confirms the resulting branch/head state.

## 6. Keep CI and repository access independent

CI runner state does not determine Connector access.

A missing, offline, busy, or mismatched self-hosted runner may prevent validation, but it does not prevent branch creation, commits, or pull-request updates. Conversely, a healthy runner does not prove that the Connector can mutate the repository.

Report these separately:

- **Repository publication state:** branch, head SHA, PR state.
- **Validation state:** workflow run, runner availability, command result.

## 7. Distinguish repository defects, connector limitations, and GitHub incidents

Before changing code, workflow permissions, triggers, or pull-request state in response to GitHub failures, classify the evidence.

### Repository or workflow defect

Evidence points to the repository when a deterministic local command or a completed workflow log reports a concrete source, test, manifest, permission, or runner error. Fix that defect and add the narrowest useful regression.

### Connector permission or capability boundary

A stable `403 Resource not accessible by integration` from one API family while ordinary repository reads and writes continue to succeed usually indicates that the connected GitHub App does not expose that endpoint or permission. It does **not** prove that GitHub Actions failed to run and must not be worked around by expanding the repository workflow token.

Record the unsupported read and use the ordinary GitHub UI or another already-authorized GitHub client when human verification is required. Keep publication state and validation state separate.

### GitHub service incident

Repeated `5xx`, `502`, timeout, archive/raw download, webhook, pull-request, Actions, or broad API failures across otherwise unrelated operations require an official GitHub Status check before repository changes are made.

During a confirmed or strongly suspected service incident:

1. record the exact branch head SHA and the first failing operation, timestamp, HTTP class, and affected GitHub surface;
2. check the official GitHub Status incident and note the affected components;
3. retry only bounded, idempotent reads needed to determine whether a write actually landed;
4. do not create no-op commits, close/reopen the PR, toggle draft state, edit unrelated files, widen workflow permissions, add status-mirroring REST calls, or change triggers merely to force GitHub to acknowledge the branch;
5. do not interpret a missing Actions run, check-run, webhook, or empty legacy commit-status response as a product-code result;
6. continue only work that can be validated independently of the degraded GitHub service;
7. keep the PR draft and report the exact head as unqualified;
8. after GitHub reports recovery, run the normal exact-head workflow without incident-specific shims.

The dated audit in [`../history/github-outage-audit-2026-08-17.md`](../history/github-outage-audit-2026-08-17.md) records why these rules were added.

## 8. Failure handling

Use the narrowest truthful diagnosis.

Examples:

- `Connector read failed`: repository visibility/authentication problem, connector limitation, or service incident requiring classification.
- `Connector write rejected`: write authorization or repository permission problem unless broader GitHub service degradation is confirmed.
- `Branch write succeeded but CI did not start`: workflow/trigger problem or GitHub Actions/webhook incident; do not assume either without evidence.
- `CI queued with no matching runner`: runner-capacity or label problem.
- `Local checkout differs from Connector branch`: local synchronization problem only.

Never use one category as evidence for another.

## 9. Required connector-mode handoff

In addition to the normal handoff requirements, report:

- the base SHA read through GitHub;
- the task branch name;
- the current remote head SHA;
- the pull-request number and draft/ready state;
- whether each claimed mutation was verified remotely;
- any Connector operation that failed and its exact error;
- whether an official GitHub incident affected the work window;
- CI state separately from publication state;
- the exact head SHA that actually passed qualification.

## Non-negotiable reasoning rules

Do not make these inferences:

- "My local checkout does not show it, therefore GitHub does not have it."
- "I have not seen an approval prompt, therefore writes are unavailable."
- "CI has no runner, therefore I cannot commit."
- "The Connector can read, so I should change its permanent permission setting before trying a write."
- "The Actions API returned 403, therefore the workflow did not run."
- "The legacy commit-status list is empty, therefore the Actions check is absent."
- "GitHub is degraded, therefore I should mutate the PR or workflow until a run appears."

The normal sequence is: **read remote state, invoke the real mutation when needed, let authorization occur at that boundary, verify the remote result, and classify GitHub service failures before changing repository behavior.**
