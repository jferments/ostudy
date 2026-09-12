# ChatGPT GitHub-direct development

This is the canonical operating procedure for ChatGPT maintenance of `jferments/ostudy`. GitHub is the authority for published source, branch and PR identity, review state, and Actions evidence. The authenticated GitHub Connector is the normal repository reader and **sole normal source publisher**. GitHub Actions execute the published source; they do not write it.

## Do not change execution modes without a request

Do not use Codex, `dev-workspace-v1`, the orchestrator, observer, exact-head MCP, Secure MCP Tunnel, or another custom developer MCP unless the human explicitly requests that tool for the current task. Tool availability, an old deployment record, an old conversation, or an agent's preference is not authorization. Do not discover or diagnose optional developer services during ordinary product work.

A human's local checkout and the commands supplied for local testing are not an alternate agent publishing service. A sandbox may help inspect already retrieved text, prepare ordinary file contents, or perform explicitly identified auxiliary checks; it must not be represented as the user's workstation, the CI VM, or an unpublished authoritative worktree. Publish source only through the selected GitHub writer.

## The complete normal loop

1. Read current GitHub state. For a new task, resolve current `main` to a full commit SHA. For resumption, read the actual PR, head ref, reviews, and exact-head Actions state.
2. Record stable acceptance criteria and non-goals. Recent conversation explains intent; current source establishes implementation. Do not treat a requested feature or PR title as proof of completion.
3. Create one `agent/<description>` branch from the exact base. Publish a meaningful first document, test, or source increment and open one draft PR. Do not create temporary ownership markers or no-op commits merely to exercise access.
4. Inspect the owning source, callers, tests, contracts, and relevant guides. Prepare a coherent multi-file change.
5. Publish with the expected-parent protocol below and verify the remote head.
6. Use focused checks, then static, fast, and required full/platform qualification on that exact SHA. Read failure evidence before changing source again.
7. Update the PR acceptance snapshot and give a handoff that separates publication, automated qualification, and manual acceptance. Supply local testing instructions using [the command runbook](COMMAND-RUNBOOK.md).

Stay on the same branch/PR for ordinary repairs. Never develop directly on `main`, force-push, rewrite shared history, or merge without the human's explicit instruction.

## Keep feature continuations productive

Finish one end-to-end acceptance increment at a time: inspect the verified head,
implement its native and UI boundaries, add focused regression coverage, publish,
and verify the new remote head. Keep the next increment explicit. Do not accumulate
a second unpublished implementation while reporting the branch as complete.

Preserve a discovered local candidate before editing it, but do not make recovery
of a merely claimed historical candidate a permanent prerequisite. After one
bounded recovery pass, use the actual remote tree and reimplement any still-missing
behavior. Record what was recovered rather than inventing missing workspace state.

Discover each needed GitHub action once, then call it. Distinguish written,
published, compiled, tested, and manually accepted work in every status report.
A failing unrelated gate does not prohibit coherent feature publication, but must
be repaired before qualification is claimed. Repair ordinary code/test failures
at their owning boundary rather than expanding infrastructure or weakening tests.

CI failure archives may carry an immutable `git archive` plus commit/tree hashes.
Those are read-only copies of published source, useful for auxiliary inspection;
they are not source writers, edited candidates, or proof of test success.

## Expected-parent Git-data publication

Use this sequence for every task-branch commit, including documentation commits:

```text
read remote head H and its tree T
→ read changed files at H
→ create_blob for each complete changed/new file
→ create_tree(base_tree_sha=T, reviewed entries)
→ create_commit(parent_sha=H, tree_sha=new tree)
→ re-read remote ref; require H
→ update_ref(new commit, force=false)
→ re-read remote ref; require new commit
→ reconcile the PR head and acceptance snapshot
```

Use ordinary UTF-8 source, not an opaque patch payload, task-specific applicator, runner-only tree, or source-generating workflow. Include deletions explicitly as tree entries with a null SHA and preserve unaffected entries through the parent tree. Preserve file modes. Inspect the final changed-file list for accidental deletions, credentials, generated artifacts, and unrelated edits.

**One writer per branch is mandatory.** The ref-update API does not expose a compare-and-swap expected-old-SHA parameter. Expected-parent commits, an immediate pre-update head check, non-force updates, and exclusive writer ownership together implement the project's stop-on-drift protocol; do not falsely describe a plain ref update as atomic compare-and-swap. A moved head requires inspection and an explicit ownership/resumption decision, never force.

A Contents API update protects one blob, not the complete branch parent, and is not the normal publication path for this workflow. Do not fall back to sequential per-file writes because the Git-data sequence appears verbose.

If a write returns no usable acknowledgement, read GitHub before retrying. A created blob/tree/commit is not branch publication until the ref points to it. A verified commit is not a passing test. Do not reissue an ambiguous mutation blindly or create a competing branch to work around uncertainty.

## Integrating current main

Read both exact heads. When the task requires integrating newer `main`, preserve the task branch and use a normal non-force merge with reviewed conflict resolution; a Git-data merge commit names both ordered parents and the reconciled tree. Do not replace the task tree with `main`, rebase shared history, or claim integration because the PR base label still says `main`. Requalify the resulting exact SHA. A mere documentation task does not authorize rewriting unrelated application history.

## Qualification and failure handling

[Testing and CI](../development/testing-and-ci.md) defines the current workflows. Automatic draft `Check` is fast; explicit manual gates allow full qualification while keeping a PR draft. Read the actual workflow input schema instead of reconstructing a command or gate name from memory.

Always match the run's tested SHA to the current branch head. A success for an ancestor, merge ref, other branch, or cached different tree is not current-head evidence. A pending or unavailable runner does not prevent source publication. An empty legacy status list does not prove that Actions checks are absent.

After failure, read job steps, bounded decoded logs, and available qualification artifacts. Fix the smallest failed boundary first and batch independent actionable findings coherently. Follow [Failure triage](FAILURE-TRIAGE.md): one unchanged-head rerun only for a supported transient cause; the same fingerprint twice requires a changed strategy, not another broad matrix run. Do not weaken tests, add sleeps, widen permissions, or toggle PR state to manufacture green CI.

All workflows are source-read-only, including manual, maintenance, dispatch, and push-triggered workflows. No exception permits Actions to stage source, run maintained-source formatters in write mode, create blobs/trees/commits, push, or finalize an agent's implementation. Existing policy enforcement is [verify-agent-workflow-policy.mjs](../../scripts/verify-agent-workflow-policy.mjs).

## Tool failures must not hijack the task

An explicitly requested developer MCP that fails or disappears trips a conversation-local circuit breaker: stop using that path, verify the actual GitHub head, and continue through GitHub. Do not debug plugins, reconnect services, redeploy healthy servers, or ask for repeated replacement chats unless that diagnosis itself is requested. Record an indeterminate unpublished workspace only when one actually existed; never invent a workspace path, clean state, PID, or job.

Discover the needed GitHub action once and invoke the real operation. Do not repeatedly announce that GitHub access is being checked. Distinguish missing action exposure, a rejected mutation, an Actions read limitation, a busy runner, and a GitHub incident. [Connector diagnostics](GITHUB-CONNECTOR.md) covers those distinctions.

## Handoff and resumption

Follow [Execution state](EXECUTION-STATE.md) and [Local testing and handoff](../development/local-testing-and-handoff.md). Report the exact base, branch, verified head, PR, last expected parent, changed areas, actual checks and results, outstanding manual boundaries, known failures, and writer release. In GitHub-only mode say that no execution workspace was used; do not fabricate a clean-worktree claim.

Give concrete fetch/checkout/SHA-verification instructions, the shortest maintained build/run/install wrappers, and a small task-specific acceptance checklist. Never make the human reverse-engineer a branch name or run an entire CI matrix merely to try the app. Conversely, a local test-drive command is not evidence that merge qualification passed.

On resumption, re-read GitHub. Do not resume an old MCP workflow, stale SHA, or conversationally claimed completion. The PR snapshot records progress and evidence; it is neither a lock, job scheduler, CI trigger, nor source authority.
