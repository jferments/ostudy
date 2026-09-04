# Source publication boundary

This document defines a repository invariant, not a preferred convention:

> **GitHub Actions never author product source.**

A workflow may compile, format-check, test, package, sign, upload artifacts, publish a release, or report qualification evidence for an already published commit. It may not create, rewrite, stage, commit, push, or update the maintained repository tree that is under review.

## Why this boundary exists

A previous implementation cycle placed one-shot Python applicators and task-specific workflows on a pull-request branch. The workflows checked out the branch, generated the intended application changes in a temporary runner worktree, and attempted to commit those results. Validation failures and clean checkouts then discarded the generated source while leaving the applicators and workflow wrappers in the pull request.

That failure mode creates several false states:

- a script that *could* generate a change is mistaken for the implemented change;
- runner-local files are mistaken for published source;
- a workflow failure can erase the only materialized implementation;
- review sees orchestration machinery instead of the product diff;
- repeated workflow commits move the branch without producing a qualified product tree.

The remedy is a hard separation between **authoring/publication** and **execution/qualification**.

## Permitted source writers

Product source may be authored only through one of these reviewed, persistent paths:

1. a trusted local repository workspace with an ordinary non-force Git push; or
2. the authenticated GitHub Connector using expected-parent Git-data publication.

For connector publication, read the exact remote branch head, create the changed blobs, create one tree based on that head's tree, create one commit whose parent is the verified head, and update the branch ref without force. Re-read the ref afterward. A rejected expected parent means another writer changed the branch; stop and inspect rather than replaying or forcing.

The commit must contain the actual product source, tests, migrations, and documentation. A patch generator, encoded diff, workflow wrapper, migration helper, or task script is not a substitute for the resulting files.

## GitHub Actions boundary

Every committed workflow must remain source-read-only regardless of trigger. This includes `pull_request`, `push`, `workflow_dispatch`, `workflow_run`, `schedule`, and reusable workflows.

Workflows must not:

- request `contents: write` or `write-all`;
- persist checkout credentials for source publication;
- run `git add`, `git commit`, `git commit-tree`, `git write-tree`, `git update-index`, or `git push`;
- call GitHub Contents or Git-data write endpoints;
- invoke auto-commit or create-pull-request actions;
- run formatters or generators in write mode against maintained source;
- invoke task-specific applicators, finalizers, source migrations, or exact-head patch scripts;
- change pull-request draft/readiness state from automatic qualification.

Formatter and generator checks must use **check-only** modes. A workflow may fail because generated or formatted files are stale; the source writer then fixes and republishes them through an approved publication path.

Release automation may upload artifacts and release metadata when separately authorized, but it still may not rewrite a product branch or synthesize the source tree being released.

## No applicator escape hatch

Do not respond to an unavailable local workspace, formatter, compiler, connector action, or developer MCP by creating a temporary workflow that edits the repository. There is **no applicator escape hatch**.

When an authoring capability is unavailable:

1. preserve the exact remote state;
2. state the missing capability precisely;
3. use another approved persistent source writer when one is available;
4. never materialize the only implementation inside a runner worktree;
5. never claim that a generator script or failed workflow constitutes delivery.

A developer-MCP failure and a GitHub Connector write-surface failure are separate incidents. Neither authorizes Actions to become a source writer.

## Mechanical enforcement

`scripts/verify-agent-workflow-policy.mjs` scans every committed workflow, not only pull-request workflows. It rejects source-write permissions, persisted credentials, Git publication commands, write-mode formatters, known auto-commit actions, and task-specific applicator artifacts.

`tests/scripts/test_agent_workflow_policy.py` includes regression coverage for pull-request, push, and manual triggers. The repository's generated-candidate workflow is validation-only and proves that formatter checks leave the tree unchanged.

Task-specific files whose names begin with `agent-`, `agent_`, `pr<number>-`, `one-off-`, or contain `applicator`/`finalizer` are rejected under `scripts/`. Permanent, generally applicable maintenance tooling requires its own reviewed architecture and must still respect this boundary.

## Review checklist

Before accepting a source-bearing pull request, verify all of the following:

- the branch diff contains the actual implementation rather than instructions to generate it;
- the latest commit has the exact previously verified parent;
- no workflow has source-write authority;
- no runner-local output is required to reconstruct the reviewed tree;
- qualification ran against the exact published SHA;
- any generated or formatted files are already committed and pass check-only validation.

If any item is false, the change is not ready for qualification or handoff.
