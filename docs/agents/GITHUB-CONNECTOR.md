# GitHub Connector operation and diagnostics

[GitHub-direct development](GITHUB-DIRECT-DEVELOPMENT.md) owns the publication protocol. This reference explains access and failure evidence. It does not authorize an alternate source writer or a manual Actions source-writing exception.

## Use the authenticated repository connection

For `jferments/ostudy`, use the GitHub Connector for repository/source reads, branch creation, Git-data publication, PR updates, and Actions inspection. Do not infer repository access from `gh`, an unrelated checkout, anonymous HTTP, scraping, a transfer repository, or runner availability. Discover the needed action once, then use it; do not repeatedly rediscover a working integration.

Record `executionMode: githubConnector` for ordinary ChatGPT tasks. Other snapshot modes describe explicitly selected environments, not permission to introduce a developer MCP. In the ordinary mode, no remote execution worktree, PID, or workspace clean state is implied.

## Keep independent states independent

| Concern | Evidence |
| --- | --- |
| Repository visibility | Authenticated repository/source read succeeds |
| Repository-level permission | GitHub's returned permission metadata |
| Action exposure | The required action exists in the callable Connector surface |
| Concrete mutation authorization | The actual requested mutation succeeds or is rejected |
| Publication | A subsequent GitHub read confirms the branch/PR head |
| Qualification | A completed run proves the named gate on that exact SHA |
| Manual acceptance | The tester actually exercised the named workflow |

No approval prompt does not mean no write access. A missing loaded action does not mean the repository is read-only. A queued runner does not prevent commits. An empty legacy status list does not mean Actions checks are absent. A 403 reading one Actions endpoint does not prove that the workflow failed or never ran.

At the first intended mutation, invoke that real operation and let the Connector handle authorization. Do not create a throwaway access-test commit or ask for broader permissions without a concrete rejection.

## Publication evidence

Read the exact current head, prepare complete source files from that ref, create blobs and a parent-based tree, create a commit naming the expected parent, re-check the remote ref, update without force, and re-read the resulting head. See the canonical protocol for details, deletions, and drift handling.

A blob, tree, unreferenced commit, local patch, or tool acknowledgement alone is not a published branch. If an acknowledgement is lost, read GitHub before retrying. Stop on branch movement and reconcile writer ownership. Do not force, silently replay an unreviewed tree, or let another chat repair the same branch concurrently.

Every source commit uses expected-parent Git-data publication, not sequential Contents API updates. All Actions remain source-read-only regardless of trigger. A named maintenance workflow is not an exception.

## Evidence after a CI failure

Resolve the actual PR head and relevant run, then inspect job steps and bounded decoded logs. Read available `ostudy-qualification-result.json`, failure summaries, and raw-log artifacts when the supported Connector surface permits it. Do not claim an artifact was read just because its filename appeared in a run.

Choose the smallest failed boundary and follow [Failure triage](FAILURE-TRIAGE.md). A workflow's dispatch ref can move before execution; verify the recorded `QUALIFIED_SHA`/checkout rather than assuming dispatch proves the intended SHA. Runs for predecessor commits remain historical.

## Classify actual access failures

**Missing action:** state the exact missing action and what remains possible. Do not call it a repository outage.

**Rejected mutation:** report the operation, repository/ref/path, exact error, and whether reads still succeed. Do not generalize one invalid request into total GitHub unavailability.

**Endpoint-specific 403:** a stable `Resource not accessible by integration` can be an endpoint/permission boundary. Use another already-authorized GitHub read surface where possible. Do not widen workflow permissions to work around Connector read authorization.

**Broad service failure:** unrelated 5xx/timeouts/webhook/Actions failures require checking official GitHub Status. Preserve the exact head and first failing operation/time. During an incident, use bounded idempotent reads to resolve ambiguous writes; do not mutate unrelated source, triggers, permissions, or PR state to force a result.

**Requested developer MCP failure:** stop that tool path and continue through GitHub after verifying actual state. Tool disappearance or conversation-gateway rejection is not evidence that the repository or CI VM is broken. Do not reconnect, redeploy, or debug plugins unless separately requested.

## Useful status language

```text
Published head: <full SHA>, verified through GitHub.
Check: <gate/run>, <pending/passed/failed>, tested <full SHA>.
Manual acceptance: <concrete path still required>.
```

For a real failure:

```text
<operation> on <repository/ref> returned <exact error>.
<Other operation> still succeeds. The unavailable boundary is <specific capability>.
```

Avoid unsupported blanket claims such as “no GitHub access,” “fully tested,” or “the workflow did not run.” The [handoff guide](../development/local-testing-and-handoff.md) defines the final evidence and local testing instructions.
