# Direct workspace development

Use the `dev-workspace-v1` / `dev-direct-workspace-mcp` runtime as the preferred
implementation path whenever it is connected and its seven-tool server contract
matches. This remains the default even when Codex credits are available. It
restores the ordinary development loop that earlier oStudy work used
successfully:

```text
persistent worktree
→ read source
→ edit multiple files
→ format and run focused checks locally
→ inspect the exact diff
→ commit and non-force push
→ use GitHub for the pull request and exact-SHA CI
```

The direct workspace does not invoke Codex. ChatGPT is the coding agent; the MCP
runtime supplies only persistent filesystem, Git, allowlisted command, logging,
cancellation, and publication boundaries. Codex credits, rate limits, or worker
availability therefore do not block normal development.

Read [`EXECUTOR-SELECTION.md`](EXECUTOR-SELECTION.md) for the authoritative
cross-runtime decision policy.

## Codex integration policy

Codex is optional rather than abandoned.

- **Advisory Codex** may perform read-only planning, architecture analysis, code
  review, test design, or second-opinion diagnosis. The direct workspace remains
  the sole branch writer.
- **Implementation Codex** may own a separate, deliberately assigned bounded
  branch when credits and the complete worker contract are healthy, scope and
  allowed paths are explicit, and no direct-workspace session owns that branch.
- Do not alternate Codex and direct workspace as writers on one live branch. A
  transfer requires a stopped boundary, exact remote head, no active job, a clean
  or fully accounted-for worktree, and explicit ownership release.
- The current direct-workspace contract has no arbitrary external-commit import
  or branch-sync action. Writer ping-pong is therefore prohibited rather than
  used as an iteration technique.
- A Codex quota, rate-limit, or transport failure never blocks direct-workspace
  development. Preserve any Codex-owned unpublished diff through the bounded
  handoff path before changing executors.

Use the full orchestrator only for genuinely autonomous multi-checkpoint work
that needs durable approvals, bounded repair/replan loops, or explicit executor
handoff across chat turns. Do not create a durable goal merely to edit files,
format source, run focused tests, inspect a diff, or publish one checkpoint.

## Capability preflight

At the beginning of an implementation session, perform one bounded preflight:

1. verify that the GitHub Connector can read `jferments/ostudy` and record the
   intended base SHA;
2. call `workspace_open` or `workspace_status` on `dev-workspace-v1`;
3. require the returned `workspaceMcpContract` to report exactly seven tools and
   `matchesExactly=true`;
4. require the local branch, remote branch, and expected HEAD to agree;
5. require no unexplained dirty state or active command;
6. verify that GitHub Actions/job logs remain readable.

Report the dimensions separately:

```text
Workspace read/write: available or unavailable
Local symbolic commands: available or unavailable
Git commit/non-force push: available or unavailable
GitHub repository/PR/CI reads: available or unavailable
Optional Codex executor: available or unavailable only when selected
```

Do not repeatedly rediscover integrations after this preflight. A missing tool is
a client/server tool-contract problem; a rejected workspace edit is a workspace
problem; a rejected push is a Git authentication or remote-head problem; a CI
failure is a validation problem; a Codex quota failure is a Codex-only problem.
None proves that the other capabilities failed.

## Seven-tool workflow

The direct workspace exposes exactly:

1. `workspace_open`
2. `workspace_status`
3. `workspace_read`
4. `workspace_apply`
5. `workspace_job`
6. `workspace_diff`
7. `workspace_publish`

### Start or resume

For a new task, read current `main` through GitHub, then call:

```text
workspace_open(
  project="ostudy",
  branch="agent/<short-description>",
  base_sha="<exact-current-main-sha>"
)
```

The runtime creates the exact remote task branch if necessary and allocates or
resumes one deterministic persistent worktree. For an existing branch, pass its
current exact remote HEAD. Do not create a competing branch merely because a
check failed.

When resuming, `workspace_status` is the authority for unpublished worktree
state, active commands, validation evidence, and log tail. GitHub remains the
authority for the published branch SHA and PR state. Do not reconstruct either
from conversational memory.

### Read and edit

Use `workspace_read` for relevant files. It returns bounded text, line metadata,
file mode, size, and content SHA-256.

Use `workspace_apply` for one coherent multi-file edit. Every call binds:

- the exact current Git HEAD;
- the exact complete pre-edit worktree diff hash;
- the expected SHA-256 of every existing file;
- `null` for a newly created path.

The runtime accepts full-file creation, replacement, chmod, and deletion only.
It rejects traversal, `.git`, symlinks, duplicate paths, unknown fields, stale
hashes, oversized batches, and remote-head drift. The batch is preflighted before
its first write and rolled back if a later filesystem write fails.

Prefer coherent vertical edits rather than one remote call per line. The
worktree persists across turns, so there is no need to publish an incomplete
transport scaffold merely to avoid losing source.

### Format and test locally

Run only configured symbolic commands:

```text
workspace_job(action="start", kind="operation", name="format")
workspace_job(action="start", kind="validation", name="changed")
workspace_job(action="start", kind="validation", name="static")
workspace_job(action="start", kind="validation", name="fast")
```

The start call returns immediately. Use `status` or `log` for bounded progress
and `cancel` for immediate process-group cancellation. Complete logs remain on
disk. Do not use chat-side sleep loops.

Any edit or mutating operation invalidates prior validation evidence. A
validation must leave the Git worktree unchanged. A mutating operation may
change only its configured path roots.

CI is the exact-published-SHA qualification environment. It is not the normal
editor, formatter, or first compiler. A rustfmt, type-check, or focused unit-test
failure should ordinarily be found and repaired in the direct workspace before
publication.

### Review and publish

Call `workspace_diff` before publication. Inspect:

- changed paths;
- complete bounded diff;
- executable modes;
- exact worktree hash;
- local and remote branch heads.

`workspace_publish` requires current `static` and `fast` evidence against that
exact HEAD and diff. It creates an ordinary Git commit and performs a non-force
push. Any local, remote, or diff drift stops publication.

After the first coherent source checkpoint is published, use the GitHub
Connector to open one draft pull request. Update the PR acceptance snapshot at
meaningful subsystem checkpoints and before readiness—not after every local edit
or command. The branch, worktree, CI, and human-test evidence remain the
authorities; the PR ledger is only a projection.

## Division of responsibility

### Direct workspace owns

- persistent task worktree;
- direct source reads and edits;
- local formatter and focused validation execution;
- command PID tree, cancellation, and full logs;
- exact diff review data;
- ordinary commit and non-force task-branch push.

### GitHub Connector owns

- private-repository and current-base reads;
- draft PR creation and updates;
- review comments and unresolved-thread inspection;
- Actions run/job/log/artifact inspection;
- ready-state transition when the repository contract is satisfied.

### GitHub Actions / exact-head validation owns

- authoritative qualification of the exact published SHA;
- Linux/Tauri, Android, packaging, and other expensive gates;
- read-only evidence production.

### Observer owns

- read-only diagnostics when another runtime owns an active job.

### Full orchestrator and Codex worker own

Only work that genuinely meets the thresholds in
[`EXECUTOR-SELECTION.md`](EXECUTOR-SELECTION.md). They are not prerequisites for
normal interactive coding.

## Failure and fallback rules

When the direct workspace is unavailable, identify the exact missing capability
once. Do not spend the session repeatedly checking whether GitHub or MCP is
"connected."

- If the seven-tool server is healthy but the ChatGPT client exposes fewer
  tools, record the server contract hash and the client-visible names for OpenAI
  support. Do not redeploy the server into another architecture merely to force a
  refresh.
- If workspace reads work but edits fail, report the concrete `workspace_apply`
  error and preserve the worktree.
- If local commands fail, use the symbolic command's log and focused
  reproduction. Do not immediately rerun the complete CI matrix.
- If publication fails because remote HEAD moved, stop and inspect the competing
  commit. Never force-push.
- If Git authentication fails, repository files and local test results remain in
  the persistent worktree; repair credentials without reconstructing source.
- The GitHub Connector's low-level Git-data writer is an emergency publication
  fallback only when no persistent workspace writer is available. It is not the
  normal editor or test loop.
- A read-only observer is never promoted into a writer.
- A Codex quota failure is not a reason to stop direct workspace development.

When no writer-capable workspace or GitHub operation exists, state that exact
capability boundary immediately. Do not claim implementation is progressing and
do not create another control plane.

## Existing branch handoff

Exactly one writer owns a task branch. Before moving an existing PR such as
`agent/ui-workflow-redesign-stages-6-13` into the direct workspace:

1. verify that no Codex worker, orchestrator goal, Connector writer, or other
   direct-workspace session is still writing the branch;
2. record the current exact remote HEAD;
3. inspect every existing worktree for unpublished changes;
4. explicitly transfer writer ownership;
5. call `workspace_open` with the current branch HEAD;
6. continue on the same branch and PR.

Do not discard a dirty worktree, replace the branch, replay an unreviewed tree,
or use force push.

## Non-negotiable retained protections

The simpler workflow retains:

- one branch and one PR per coherent task;
- one source writer;
- no direct development on `main`;
- append-only released migrations;
- exact-head qualification;
- static/fast checks before publication;
- no force pushes;
- read-only automatic CI;
- bounded logs and hard timeouts;
- human review and squash merge;
- explicit manual acceptance for visual, hardware, live-provider, and physical
  device boundaries.

The simplification removes coordination machinery from the hot path; it does not
weaken source, data, test, migration, publication, or review safety.
