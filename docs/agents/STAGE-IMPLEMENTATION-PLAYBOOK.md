# Stage implementation playbook

Use this checklist at the beginning of every ostudy stage. Its purpose is to
prevent repeated repository-discovery work and to make a real code diff the
first deliverable rather than a narrated plan.

## Fixed repository context

- Repository: `jferments/ostudy`
- Default branch: `main`
- Development branch convention: `agent/<phase-stage-description>`
- Repository instructions: `AGENTS.md` and `docs/agents/`
- Complete local/CI gate: `./scripts/check.sh`

When the thought **“How do I access the repo?”** occurs, stop exploring the
filesystem and use this context directly. Open the installed GitHub connector,
request or refresh repository write access when necessary, resolve the exact
`main` SHA, inspect the named branch, and compare it with `main`. Use a local
checkout only when one is actually mounted and verified. A stage is not
complete while its code exists only in a scratch directory: when GitHub write
access is available, commit the readable source to the development branch and
open the PR in the same work session.

## Mandatory publish contract

For every stage request that asks for implementation, the default outcome is a
GitHub branch and pull request—not a local patch bundle and not a design-only
response. At the beginning of the stage:

1. use the installed GitHub connection for `jferments/ostudy`;
2. request or refresh the required repository write access immediately when a
   write is not authorized;
3. create or reuse the single stage development branch from the verified
   `main` SHA;
4. commit readable source files to that branch as the implementation proceeds;
5. push the completed source and open one draft PR before reporting the stage as
   delivered.

Do not tell the user to apply a scratch-directory patch when the GitHub
connection can publish it. Do not leave encoded payloads, temporary mutation
workflows, delivery fragments, or generated transport files in the final branch.

## Required sequence

1. **Resolve one exact base.** Fetch `main`, record its SHA, and verify the prior
   stage PR is merged. Never build from a guessed or stale branch.
2. **Establish GitHub write access and use one branch.** Use the installed
   GitHub connector first. If a write action is not yet authorized, request or
   refresh GitHub access immediately instead of probing the filesystem or
   declaring the work unpublishable. Create or reuse exactly one `agent/...`
   branch from the recorded SHA and compare it with `main`. Do not create
   payload branches, repair branches, or duplicate PRs.
3. **Read the guardrails once.** Inspect `AGENTS.md`, architecture, data model,
   workflow, and the previous stage's changed files. Do not repeatedly rediscover
   the project structure after this step.
4. **Write the first vertical slice immediately.** Add the migration/data types,
   backend command, IPC wrapper, and smallest usable UI path. The branch must
   contain a readable source diff before producing a status response.
5. **Finish the entire requested surface.** Trace every new durable behavior
   through create, edit, list/search, practice, analytics, move/copy,
   import/export, deletion, documentation, and tests. Update callers instead of
   preserving obsolete signatures.
6. **Keep compatibility narrowly defined.** Preserve released user data through
   append-only migrations and supported package readers. Do not retain dead
   functions, ignored request fields, fallback branches, lint suppressions, or
   shims for removed features.
7. **Validate in increasing scope.** Run the stage verifier and focused tests,
   then `./scripts/check.sh`. Fix concrete compiler, lint, migration, test, and
   smoke failures before opening the PR. Never describe unrun checks as passed.
8. **Publish before reporting completion.** Use normal GitHub blob/tree/commit
   operations or an authenticated local `git` checkout to push ordinary source
   files and documentation to the established branch. Do not stop after writing
   files into `/mnt/data` or another scratch directory. Temporary transport
   scaffolding must never remain in the branch or PR.
9. **Open one draft PR after code exists.** Include the base SHA, scope, data
   compatibility, exact checks, and manual acceptance list. Mark ready only when
   CI is green and temporary scaffolding is removed.
10. **Return only actionable completion information.** Report the branch, commit,
    PR, checks, manual test steps, and merge method. If a tool genuinely blocks a
    write, say exactly which write was blocked and still provide the readable
    code artifact; do not substitute a design essay for implementation.

## Success and failure signals

Successful iterations have started from an exact branch, produced source commits
early, used the repository gate as the feedback loop, and fixed each reported
failure. Failed iterations spent time probing access, narrated plans without a
diff, used opaque transfer machinery, or preserved stale interfaces to avoid
updating callers. Prefer the successful pattern every time.
