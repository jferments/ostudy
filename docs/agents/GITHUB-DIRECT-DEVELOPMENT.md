# ChatGPT GitHub-direct development

This is the default implementation workflow when ChatGPT is developing **ostudy** through the authenticated GitHub Connector.

The workflow deliberately has **no custom or developer MCP dependency**. A Secure MCP Tunnel, local workspace app, observer, Codex worker, or orchestration service may be useful in a specific situation, but none is required to read source, publish commits, manage a pull request, or consume GitHub Actions evidence.

## Why this is the default

Earlier large oStudy changes were delivered through GitHub-backed source publication and repository CI. The implementation branch contained the actual reviewable source, GitHub Actions exercised the published commit, and follow-up fixes were committed to the same branch. Some early work temporarily used a bounded source-transfer helper, but that helper was removed before handoff; it was not a permanent runtime dependency.

That model is less elegant than a perfect persistent local checkout, but it depends on the two surfaces that have proved most stable in ChatGPT sessions:

1. the authenticated GitHub Connector for repository reads and writes; and
2. GitHub Actions for repeatable execution and exact-head qualification.

A developer-MCP gateway failure must not block ordinary development.

## Normal loop

```text
read exact GitHub state
→ create or resume one task branch
→ inspect relevant source and tests
→ construct one coherent source change
→ publish an expected-parent Git commit through GitHub
→ verify the remote branch head
→ inspect the exact-head CI result
→ publish a focused repair on the same branch when needed
→ complete manual acceptance
→ human review and squash merge
```

Use one `agent/<description>` branch and one draft pull request for the task. Exactly one source-writing ChatGPT session owns that branch until it explicitly releases ownership.

## Source publication protocol

For a multi-file implementation commit, prefer GitHub Git-data operations because they bind the complete commit to the exact expected parent:

1. Read the task branch's current remote head. For a new branch, read the exact `main` base SHA and create the branch from that SHA.
2. Fetch the relevant source files, tests, migrations, and documentation from that exact ref.
3. Prepare the complete intended file contents. Do not publish placeholders that are not part of the product implementation.
4. Create one blob for every changed or newly created file.
5. Create one tree based on the expected parent commit's tree.
6. Create one commit whose parent is the exact previously verified branch head.
7. Move the task branch with a **non-force** `update_ref`.
8. Re-read the remote branch and require it to equal the returned commit SHA.
9. Open or update the draft pull request.
10. Treat CI as separate evidence for that exact published SHA.

If the branch head changed between steps 1 and 7, stop and inspect the competing commit. Never force-push or silently replay an unreviewed tree.

For a genuinely small, single-file documentation change, the Connector's Contents API may be used when there is no competing writer. Re-read the branch immediately afterward. Do not use sequential per-file Contents API calls when a partially published multi-file change would be unsafe; use one Git-data commit instead.

## Reading and editing without a custom workspace

ChatGPT may read repository files directly through the GitHub Connector and reason over the surrounding implementation, callers, tests, schemas, migrations, and documentation. The resulting commit must contain ordinary source files, not merely a plan or a generated patch artifact.

In a ChatGPT-only session, local formatter or compiler execution may be unavailable. That limitation is explicit rather than hidden:

- inspect nearby formatting and type conventions before publication;
- keep commits coherent and bounded;
- use the cheapest appropriate automatic gate first;
- consume the exact failure log;
- repair all independently actionable failures from that run in one focused follow-up commit;
- do not rerun the broad matrix while a deterministic narrow failure is unresolved.

A trusted local checkout may still be used when it is genuinely available through a stable execution surface. It is an optimization, not a prerequisite, and it must not become the only location containing the implementation.

## Developer-MCP circuit breaker

Trip the conversation-local circuit breaker immediately when any of these occurs:

- `FORBIDDEN: This conversation does not support developer MCPs`;
- a developer-MCP namespace that worked earlier disappears from the callable tool registry;
- a developer-MCP write returns no usable result and a subsequent status/read call is rejected by the same gateway;
- the ChatGPT tool gateway reports that no tools are defined for an installed developer app;
- the server is independently healthy but the current conversation cannot invoke it.

After the circuit breaker trips:

1. Do not retry the same developer-MCP call repeatedly.
2. Do not ask the human to paste the same prompt into a sequence of new chats.
3. Do not redeploy, rename, reconnect, or reinstall a healthy server merely to repair a conversation-level authorization failure.
4. Do not create another tunnel, observer, coordinator, or state ledger.
5. Verify the actual GitHub branch and PR state.
6. If no GitHub commit was published, treat the remote repository as unchanged.
7. Continue through the authenticated GitHub Connector when its required write actions are present.
8. Report the exact unavailable capability only if the GitHub write operation itself is rejected.

A custom workspace may contain an indeterminate local edit after a lost response. That local state is not published source. Do not reuse stale hashes or assume the edit landed. When the developer MCP later becomes available, it may be inspected for recovery, but ordinary GitHub-direct development does not wait for it.

## Tool-selection policy

### GitHub Connector

Default ChatGPT source and PR interface. It owns:

- repository, branch, commit, and file reads;
- branch creation;
- blob, tree, commit, and non-force ref publication;
- pull-request creation and updates;
- review and CI inspection;
- remote verification after every claimed mutation.

### GitHub Actions

Default execution and qualification surface for ChatGPT-only development. It owns repeatable static, fast, full, Linux/Tauri, Android, packaging, and evidence-producing gates against a published SHA. It is read-only with respect to product source.

### Custom developer MCPs

Optional acceleration only. Use one when it is already callable and materially improves local iteration. It must never be a prerequisite for starting, publishing, repairing, or completing ordinary oStudy work. A gateway failure immediately falls back to GitHub-direct mode.

### Codex

Optional rather than abandoned. Advisory Codex may provide a read-only plan, review, or test analysis. Implementation Codex may own a separate deliberately assigned branch. It must not share or alternate writer ownership with a GitHub-direct ChatGPT session on the same live branch.

### Full orchestrator, observer, and exact-head services

Use only for their specialized roles: durable autonomous multi-checkpoint work, read-only diagnostics, or isolated validation of a published SHA. They are not ordinary coding prerequisites.

## Failure boundaries

Keep these failures separate:

| Failure | Meaning | Response |
|---|---|---|
| Developer-MCP `FORBIDDEN` or tool disappearance | ChatGPT conversation/tool-gateway failure | Trip the circuit breaker and use GitHub-direct mode |
| GitHub read succeeds but a required write action is absent | Current Connector tool-surface limitation | State the missing action precisely; do not claim a repository outage |
| GitHub write is rejected | Concrete authorization/request failure | Report the exact operation, target, and error |
| Non-force ref update rejects the expected parent | Competing branch writer or head drift | Stop, inspect, and hand off; never force |
| CI fails | Published source, test, runner, or environment failure | Read exact-head logs and publish a focused repair |
| CI runner is unavailable | Validation unavailable | Publication state remains valid and separate |
| Manual provider/UI/device test fails | Product or environment issue at a human-only boundary | Record reproduction and repair the same branch |

## Session start

At the beginning of a task, perform one bounded capability check:

```text
GitHub repository reads: available/unavailable
GitHub branch and Git-data writes: available/unavailable
GitHub PR updates: available/unavailable
GitHub Actions reads: available/unavailable
Optional execution workspace: available/unavailable, but not required
Optional Codex: available/unavailable, only when deliberately selected
```

Do not repeatedly rediscover these integrations during the task. Invoke the real mutation when the task reaches it and report only the concrete result.

## Handoff

A handoff records:

- repository, branch, base SHA, current remote head, and PR number;
- the exact expected parent used for the last publication;
- changed files grouped by purpose;
- acceptance criteria and non-goals;
- automatic gates run and their exact tested SHAs;
- manual acceptance completed or still required;
- known failure fingerprints;
- whether any optional local workspace contains unpublished state;
- explicit release of branch-writer ownership.

The remote Git branch is the authority for published source. The PR acceptance snapshot is a projection. CI is evidence about one exact SHA. A custom MCP's availability is never part of the product completion predicate.
