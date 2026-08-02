# Ostudy Git and Coding Workflow — Corrected Publication-First Version

This document defines the required delivery workflow for changes to **ostudy**.
It applies to human contributors, Codex CLI, ChatGPT-assisted implementation,
and other coding agents. The goal is to keep every task visible, recoverable,
reviewable, and continuously published rather than allowing important work to
exist only in an agent's temporary filesystem.

## OSTUDY CODE DELIVERY MODE

A request to implement code, create a development branch, or prepare a pull
request places the agent in **OSTUDY CODE DELIVERY MODE**. In this mode, planning
and explanation support delivery but never replace it. The required product is
published source code on a task branch and an open pull request.

The agent must not stop after writing a plan, producing an uncommitted patch,
creating files in temporary storage, or describing code that has not been
published.

## 1. Establish the exact base without editing the primary checkout

The primary checkout at `~/code/ostudy` is for the human's normal use and must
not be used as the agent's feature workspace. Before making any change:

1. inspect the primary checkout without modifying it;
2. require a clean understanding of any existing human changes;
3. fetch and identify the exact current `origin/main` commit;
4. create an isolated Git worktree dedicated to the task;
5. create a short-lived `agent/<description>` branch from that exact commit.

Example:

```bash
cd ~/code/ostudy
git status --short
git fetch origin --prune
BASE_SHA="$(git rev-parse origin/main)"
BRANCH="agent/phase-2-stage-3-card-types-safe-templates"
WORKTREE="../ostudy-stage-3"
git worktree add -b "$BRANCH" "$WORKTREE" "$BASE_SHA"
cd "$WORKTREE"
```

Never edit the primary checkout directly. Never use destructive synchronization
to hide unrelated local work.

## 2. Publication-first rule

Immediately after creating the branch, push it to GitHub **before editing files
or running tests**:

```bash
git push -u origin HEAD
```

This establishes a remote recovery point and makes the task branch visible from
the beginning. Local-only branches are not acceptable delivery state.

## 3. Publish the first meaningful commit immediately

When practical, the first commit should contain only minimal scaffolding,
acceptance criteria, an ADR, a migration shell, a test skeleton, or another
small bootstrap that does not pretend the feature is complete. Push that commit
immediately.

```bash
git add <bootstrap-paths>
git diff --cached --check
git commit -m "chore: scaffold <feature>"
git push
```

Do not wait until the entire feature is implemented before creating the first
commit.

## 4. Open a draft pull request early

Open a draft pull request as soon as the branch contains its first meaningful
commit. The pull request must target `main` and record:

- the exact base commit;
- the requested behavior and acceptance criteria;
- compatibility and migration surfaces;
- the planned validation;
- the fact that implementation is still in progress.

The pull request remains the durable task record. Additional implementation,
tests, documentation, and fixes are pushed to the same branch and update the
same pull request.

## 5. Implement in reviewable vertical increments

Search existing callers, tests, commands, migrations, transfer formats, and
documentation before choosing an implementation. Then deliver the smallest
coherent vertical increments that preserve the ownership boundaries in
`AGENTS.md`:

- React owns presentation and transient interaction state.
- `src/ipc.ts` is the typed frontend gateway.
- Rust validates untrusted input and owns durable behavior and transactions.
- SQLite owns persistent structured invariants.
- Existing review history, schedules, imports, themes, and accessibility
  behavior must remain intact unless the task explicitly changes them.

Commit and push logical increments frequently. A reasonable sequence is schema
and domain behavior, IPC, UI, regression tests, then documentation and cleanup.
Do not accumulate a large unpublished working tree.

## 6. Keep the branch and pull request truthful

A draft branch may temporarily fail while a coherent increment is being built,
but it must never falsely claim completion. Update the pull-request description
as scope, migration effects, tests, or known risks become clearer.

Do not use ignored request fields, stale compatibility shims, dead-code
allowances, one-off mutation workflows, or hidden fallback behavior to make a
partial implementation appear successful. Removed internal behavior must be
removed end to end unless a documented durable-data or external-format
compatibility requirement justifies a shim.

## 7. Validate continuously and use CI as a second environment

Run narrow checks while developing and push corrections to the same branch.
Before requesting merge, run the complete repository gate:

```bash
./scripts/check.sh
```

GitHub Actions must run against the published commits. When CI fails:

1. inspect the failing job and complete logs;
2. reproduce the smallest available failing command;
3. fix the implementation or the test contract, rather than suppressing the
   failure;
4. add or refine regression coverage;
5. push the correction to the existing branch;
6. rerun the complete gate.

A parse check, static inspection, or frontend-only build must not be described
as a successful native Tauri test.

## 8. Manual acceptance remains required

For user-visible or persistence changes, the human tests the published branch on
the target Ubuntu system after `./scripts/check.sh` succeeds. The handoff must
provide concrete steps that exercise new behavior, restart persistence,
keyboard behavior, pending and error states, all relevant themes, migration of
existing data, and import/export compatibility when affected.

## 9. Completion criteria

A coding task is not complete until all of the following are true:

- the task branch exists on GitHub;
- meaningful implementation commits are pushed;
- a pull request into `main` is open;
- affected tests and documentation are committed;
- the complete available automated gate has passed, or every unavailable or
  failing check is reported precisely;
- required manual test instructions are provided;
- the final handoff records the base SHA, branch, head SHA, PR number, changed
  behavior, migration effects, validation, limitations, and merge method.

The agent never self-merges unless the human explicitly requests that exact
action. The normal merge method is **Squash and merge** after CI and the human's
manual acceptance both pass.

## 10. Failure and recovery rules

If an implementation direction becomes unsound, stop adding code, preserve the
published branch for diagnosis, and either correct it transparently or replace
it with a clean branch from the stable base. Never conceal a failed approach by
rewriting shared history or force-pushing. Never leave the user with only a
local patch, temporary archive, opaque payload, or promise of later delivery.
