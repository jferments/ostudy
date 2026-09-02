# Executor selection and branch ownership

This is the authoritative oStudy policy for choosing between the direct
workspace, Codex, the full orchestrator, the observer, exact-head validation,
and low-level GitHub writes.

## Default implementation path

Ordinary feature work, bug fixes, refactors, tests, migrations, and documentation
use ChatGPT plus `dev-workspace-v1` and the GitHub Connector:

```text
GitHub resolves the current base and PR state
  → dev-workspace-v1 owns one persistent task worktree
  → ChatGPT reads and edits source directly
  → allowlisted formatting and focused checks run locally
  → the exact diff is reviewed
  → dev-workspace-v1 commits and non-force pushes
  → GitHub Actions qualifies the published SHA
```

This remains the default when Codex credits are available. The existence of an
additional executor is not a reason to add another writer, task-state service,
branch, or handoff to a routine development loop.

## Codex is optional

Codex is retained in two explicit modes.

### Advisory mode

Codex may inspect published source and produce:

- a read-only implementation plan;
- architecture or migration analysis;
- a code-review pass;
- a test matrix;
- a second-opinion diagnosis.

The direct workspace remains the sole branch writer. Advisory output is a
proposal or review artifact, not source publication.

### Exclusive implementation mode

Codex may own a separate bounded task branch when every condition below holds:

- the human or coordinator deliberately selects Codex;
- credits, runtime health, and the complete Codex-worker contract are confirmed
  once;
- acceptance criteria, non-goals, allowed paths, and validations are explicit;
- the task is suited to self-contained delegation;
- no direct-workspace or Connector session owns the branch;
- Codex retains ownership through publication or an explicit stopped handoff.

Reasonable candidates include mechanical refactors, repetitive regression-test
generation, or an isolated well-specified subsystem. Interactive UI work,
architecture-heavy migrations, rapid human-feedback loops, and ordinary repairs
normally remain in the direct workspace.

## One writer; no executor ping-pong

Exactly one implementation executor owns a task branch at a time. Do not
alternate Codex, direct workspace, and low-level GitHub writes on one live branch.

A writer transfer requires:

1. the previous writer has stopped and explicitly released ownership;
2. no implementation, operation, validation, or publication job is active;
3. the exact remote head is recorded;
4. every worktree is clean or its unpublished diff is completely accounted for;
5. the receiving writer binds that exact head without force-pushing;
6. the PR records the handoff at a meaningful checkpoint.

The current direct-workspace contract intentionally has no arbitrary external
commit import or branch-sync operation. This makes branch bouncing exceptional
rather than an iteration technique.

A Codex quota, rate-limit, or transport failure does not block a task that is
already suitable for direct-workspace implementation. Preserve any Codex-owned
unpublished diff through the bounded orchestrator handoff before transferring
ownership.

## Full orchestrator

Use the full orchestrator only for a genuinely autonomous, durable goal that
needs one or more of the following:

- multiple authorized checkpoints that must continue across chat turns;
- exact plan and publication approvals;
- bounded automatic retries, repairs, or replans;
- an explicit repair budget;
- deliberate cross-executor handoff;
- a long-running phase whose next authorized action must proceed without another
  user message.

Do not create a durable goal merely to inspect files, edit source, run rustfmt,
execute focused tests, review a diff, or publish one coherent commit.

## Supporting systems

- **GitHub Connector:** authoritative remote repository, branch, PR, review, and
  Actions interface. Its low-level Git-data writer is an emergency fallback when
  no persistent writer exists, not the normal editor or compiler.
- **GitHub Actions:** independent exact-published-SHA qualification. CI never
  authors, formats, or repairs product source.
- **`dev-exact-head-v1`:** isolated clean-room validation of one published SHA;
  useful for reproduction, independent evidence, or release qualification.
- **`dev-tunnel-v3` observer:** read-only diagnostics for jobs, worktrees, logs,
  and runtimes owned by another component. It is never promoted into a writer.
- **`dev-worker-v1`:** bounded Codex implementation surface used only under the
  exclusive Codex branch policy.

## Capability preflight

Perform one preflight at the start of a task and report each capability
separately:

```text
Direct-workspace read/write
Local symbolic formatter/check execution
Task-branch commit and non-force push
GitHub repository/PR/CI access
Optional Codex availability, only when Codex was selected
```

Do not repeatedly ask whether everything is generically "connected." A missing
client tool, rejected edit, failed command, rejected push, CI failure, observer
failure, and Codex quota failure are different boundaries.

## Default decision table

| Work | Preferred path |
|---|---|
| Normal feature, bug fix, migration, UI change, or refactor | ChatGPT + direct workspace |
| Read-only second opinion or design review | Optional advisory Codex |
| Mechanical isolated task deliberately delegated end to end | Exclusive Codex branch |
| Multi-checkpoint autonomous campaign with retries/handoffs | Full orchestrator |
| Validate one already-published SHA | Exact-head validator |
| Diagnose another runtime's active process or worktree | Observer |
| Review, PR updates, CI inspection, merge | GitHub Connector |
