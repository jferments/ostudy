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

## 7. Failure handling

Use the narrowest truthful diagnosis.

Examples:

- `Connector read failed`: repository visibility/authentication problem.
- `Connector write rejected`: write authorization or repository permission problem.
- `Branch write succeeded but CI did not start`: workflow/trigger problem.
- `CI queued with no matching runner`: runner-capacity or label problem.
- `Local checkout differs from Connector branch`: local synchronization problem only.

Never use one category as evidence for another.

## 8. Own the CI feedback loop

When the self-hosted GitHub runner can execute a repository test and the Connector can read the resulting workflow data, a Connector-backed agent should retrieve that evidence itself rather than asking the human to copy command output into the conversation.

For substantial implementation work:

1. publish and verify a coherent branch head;
2. inspect workflow runs for that exact SHA;
3. retrieve failed jobs, step status, decoded job logs, and diagnostic artifacts through the Connector;
4. gather the complete actionable failure set available from that run before editing;
5. repair those failures in a coherent batch on the same PR branch;
6. publish and verify the new head;
7. inspect the new exact-head results and repeat until the required gates are green or a concrete external blocker remains.

Use the opt-in [`Agent diagnostics`](../development/agent-diagnostics.md) workflow when broader multi-failure information would reduce serial repair cycles. The agent may add the appropriate PR-body marker to its own development PR and consume the resulting logs/artifacts. The workflow is diagnostic only and does not replace authoritative merge qualification.

Do not ask the human to run `pnpm check:static`, `pnpm check`, `pnpm android:smoke`, or an equivalent repository test solely to relay output when a suitable self-hosted workflow is available. Human intervention is still appropriate for manual visual/interaction judgment, physical-device behavior, protected credentials, runner-host administration, or other work that cannot be observed or performed through GitHub.

GitHub Actions is asynchronous. A queued/running workflow is not a pass, and an agent must not fabricate completion. Report its exact state without aggressive polling or duplicating the same expensive test on the human's machine.

## 9. Required connector-mode handoff

In addition to the normal handoff requirements, report:

- the base SHA read through GitHub;
- the task branch name;
- the current remote head SHA;
- the pull-request number;
- whether each claimed mutation was verified remotely;
- any Connector operation that failed and its exact error;
- CI state separately from publication state;
- which CI logs/artifacts the agent inspected itself and which manual checks still require the human.

## Non-negotiable reasoning rules

Do not make these inferences:

- "My local checkout does not show it, therefore GitHub does not have it."
- "I have not seen an approval prompt, therefore writes are unavailable."
- "CI has no runner, therefore I cannot commit."
- "The Connector can read, so I should change its permanent permission setting before trying a write."
- "A CI failure means the human must run the command and paste the output."

The correct sequence is: **read remote state, invoke the real mutation when needed, let authorization occur at that boundary, verify the remote result, and consume available CI evidence directly.**
