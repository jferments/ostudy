# Agent execution state and completion evidence

This document defines the durable state model for automated contributors working
on ostudy. It prevents planning, local edits, publication, validation, manual
acceptance, review, and completion from being collapsed into one vague claim
that a task is "done."

Read [`EXECUTOR-SELECTION.md`](EXECUTOR-SELECTION.md) for writer selection and
handoff policy.

## State is multidimensional

A change moves through independent states:

```text
specified
writer bound
implemented in workspace
published
statically valid
fast-qualified
fully qualified
manually exercised
reviewed
merged
```

No state implies the next one. A generated patch is not publication. A dirty
persistent worktree is not a published commit. A published commit is not
validation. A compile result is not a native application test. A green run for
an older commit does not qualify the current PR head. Automated qualification
does not prove a physical-device, live-provider, or visual workflow that was not
exercised.

Use precise language:

- **Specified:** acceptance criteria and non-goals are recorded.
- **Writer bound:** one named workspace/executor exclusively owns the task
  branch at an exact head.
- **Implemented in workspace:** source exists in the selected writer worktree but
  may not yet be published.
- **Published:** GitHub confirms the remote branch/head containing the source.
- **Passed:** the named command completed successfully against the named source
  tree or exact SHA.
- **Inspected:** source, diff, or output was reviewed without executing behavior.
- **Manually verified:** the named workflow was actually exercised.
- **Not run:** the check was unavailable, outside the environment, or deferred,
  with the reason recorded.
- **Blocked:** a prerequisite prevents the next state; this is not completion.

## Completion predicate

For exact head commit `h`, a task may be described as complete only when:

```text
Complete(h) = Acceptance(h)
           AND Published(h)
           AND AutomatedValidation(h)
           AND ManualAcceptance(h)
           AND WriterReleased(h)
           AND no unresolved known failure for h
```

Where:

- `Acceptance(h)` means every in-scope criterion is implemented in `h` or
  explicitly recorded as a human-only/manual boundary.
- `Published(h)` means GitHub confirms `h` is the task branch and PR head.
- `AutomatedValidation(h)` means every required gate passed against exactly `h`.
- `ManualAcceptance(h)` means every required manual workflow was exercised, or
  remains explicitly assigned to the human because automation cannot perform it.
- `WriterReleased(h)` means no implementation/validation/publication job is
  active, no unpublished source remains, and the branch writer has released
  ownership for review.

When one term is missing, report the narrower state instead:

- `Implementation exists in the direct workspace; publication is pending.`
- `Implementation is published; static validation is pending.`
- `Fast qualification passed on abc123; full Linux and Android qualification are pending.`
- `Automated qualification passed; physical camera acceptance remains.`
- `All gates passed, but the writer still has unpublished changes.`
- `Five of seven acceptance criteria are implemented.`

## Pull-request acceptance snapshot

For compatibility with current validation and handoff tooling, substantial pull
requests keep one machine-readable acceptance snapshot between these markers:

````markdown
<!-- ostudy-agent-state:start -->
```json
{
  "schemaVersion": 1,
  "baseSha": "0123456789abcdef0123456789abcdef01234567",
  "headSha": "fedcba9876543210fedcba9876543210fedcba98",
  "branch": "agent/example",
  "executionMode": "githubConnectorWithExecutionWorkspace",
  "currentState": "implementation",
  "nonGoals": ["No scheduling behavior changes."],
  "compatibilityNotes": ["No durable format changes."],
  "rollbackNotes": ["Revert the source-only commit."],
  "residualRisks": ["Human review remains required."],
  "mergeMethod": "squash",
  "acceptance": {
    "AC-01": {
      "status": "implemented",
      "evidence": ["src/example.ts"],
      "tests": ["src/example.test.ts"]
    },
    "AC-02": {
      "status": "notStarted",
      "evidence": [],
      "tests": []
    }
  },
  "validation": {
    "static": {
      "status": "passed",
      "sha": "fedcba9876543210fedcba9876543210fedcba98",
      "evidence": ["workspace-static-job and workflow-run-123"]
    },
    "fast": {"status": "pending"},
    "full": {"status": "pending"},
    "android": {
      "status": "notRequired",
      "evidence": ["No Android qualification surface is changed by this documentation-only example."]
    }
  },
  "manualAcceptance": {
    "affectedWorkflow": {"status": "pending"}
  },
  "knownFailures": []
}
```
<!-- ostudy-agent-state:end -->
````

The current schema does not have dedicated writer fields. Until it does, record
the selected writer, workspace/task identifier, last expected remote SHA, active
job state, and ownership release in the human-readable PR state and handoff. Do
not overload `executionMode` to imply that Codex or direct workspace owns the
branch when the actual writer differs.

This snapshot is not a source of truth for unpublished workspace bytes, branch
head, Actions run, runner health, or active task ownership. It must never be
used as a CI trigger or branch lock. The selected workspace owns unpublished
source, GitHub owns published source, and exact-SHA workflow evidence owns remote
qualification.

`scripts/verify-pr-contract.mjs` validates the JSON shape and, for a non-draft
PR, requires exact agreement with the current GitHub base, head, and branch.

Draft synchronize events can race a human-readable snapshot update. A draft may
temporarily record an older identity without blocking source qualification.
Before review readiness, reconcile it to the exact current head. A passed
validation entry always records the exact tree/SHA it tested.

Acceptance statuses are:

```text
notStarted
inProgress
implemented
humanRequired
notApplicable
```

Validation and manual statuses are:

```text
pending
passed
failed
blocked
notRun
notRequired
humanRequired
```

A `passed` validation entry records exact tested SHA and command/run/artifact
evidence. `failed`, `blocked`, `notRun`, `notRequired`, and `humanRequired`
entries require explanatory evidence; none may be used as an unexplained escape
hatch.

`currentState` is one of:

```text
planning
scaffolding
implementation
validation
manualAcceptance
readyForReview
```

`executionMode` is one of:

```text
localCheckout
githubConnector
githubConnectorWithExecutionWorkspace
```

Use the values as follows:

- `localCheckout`: a human or agent operates a normal authenticated local
  checkout without the Connector being the remote coordination interface.
- `githubConnector`: the Connector itself is the selected source writer. This is
  an emergency fallback, not the normal ChatGPT path.
- `githubConnectorWithExecutionWorkspace`: GitHub supplies remote/PR/CI
  authority and a separate workspace—normally `dev-workspace-v1`—owns local
  source editing, commands, commit, and push. This is the default ChatGPT mode.

A Codex-owned branch currently also uses
`githubConnectorWithExecutionWorkspace`; record `Codex exclusive writer` in the
human-readable state. Advisory Codex does not change execution mode or writer
identity.

`mergeMethod` is one of:

```text
squash
mergeCommit
rebase
humanDecision
```

Drafts may leave narrative arrays incomplete while facts are being established.
Before readiness, `nonGoals`, `compatibilityNotes`, `rollbackNotes`, and
`residualRisks` each contain at least one explicit entry; `None` is acceptable
only as an explanatory checked statement.

A PR may be marked ready only when every criterion is terminal, static/fast/full
passed on the exact current head, every additional platform/manual entry is
terminal and explained, narrative arrays are populated, `knownFailures` is
empty, no job is active, no unpublished source remains, and branch ownership is
released for review.

A known failure is recorded as:

```json
{
  "fingerprint": "qf-0123456789abcdef",
  "classification": "product",
  "boundary": "React tests",
  "reproduction": "pnpm exec vitest run src/example.test.tsx",
  "attempts": 1
}
```

The classification vocabulary is defined in
[Failure triage](FAILURE-TRIAGE.md). Workspace and Codex/executor failures must
name their precise boundary even when the compatibility enum temporarily uses
`runnerEnvironment`.

## Evidence-derived handoff

Every `Check` run writes `ostudy-qualification-result.json`. Non-draft PR runs
also write `ostudy-pr-contract.json`; draft handoffs remain qualification-only
so snapshot errors cannot block fast source feedback.
`scripts/render-agent-handoff.mjs` combines validated records with the exact Git
diff into `ostudy-agent-handoff.md`.

The renderer includes:

- base, branch, PR state, and exact published/tested head;
- acceptance status and implementation/test evidence;
- changed files grouped by purpose;
- non-goals, compatibility effects, rollback notes, and residual risks;
- validation evidence and represented commands;
- exact workflow/run/step state;
- manual acceptance and known failures;
- requested merge method;
- evidence-integrity warnings or errors.

The final human/agent handoff additionally records selected writer, workspace
path or task ID, unpublished diff state, active PID/job, last expected remote
head, and ownership release. Actions cannot infer unpublished workspace state.

The renderer never silently combines mismatched SHAs. A qualification/PR-head
mismatch produces invalid evidence. Draft snapshot drift remains visible while
GitHub's actual head remains authoritative.

## Session resumption

When work resumes after interruption:

1. Read the repository and intended base through GitHub.
2. Read the open PR and compatibility snapshot.
3. Verify actual remote branch and head.
4. Read the selected writer workspace's durable status.
5. Verify active job, unpublished diff, local head, and branch ownership.
6. Read exact-head CI separately from publication and workspace state.
7. Enumerate non-terminal criteria, unresolved reviews, and known failures.
8. Continue from verified state or complete an explicit stopped writer handoff.

Do not reconstruct task state from memory. If snapshot, repository, and workspace
disagree, each remains authoritative only for its own dimension; reconcile the
disagreement before another global claim.

## Progress checkpoints

For long work, report resumable checkpoints:

```text
Writer: dev-workspace-v1
Workspace head/diff: <sha> / <hash>
Published head: <sha>
Acceptance: 4/9 implemented
Latest local gate: static passed on <workspace hash>
Latest remote gate: fast passed on <published sha>
Current blocker: one named regression
Next action: focused repair and reproduction
```

These checkpoints do not replace the final handoff. They prevent silence,
context loss, and unsupported global claims.

## Evidence rules

- Every claimed remote mutation is verified by a GitHub read.
- Every local validation claim names command, workspace head, and diff hash.
- Every remote validation claim names workflow and exact SHA.
- A source edit invalidates prior local validation evidence.
- A newer push invalidates older remote green evidence.
- Manual acceptance records the concrete path exercised, not an intention.
- External service, credential, camera, hardware, and physical-device boundaries
  remain explicitly pending rather than inferred.
- The target scope in a PR description is not evidence that the scope is
  implemented.
- Codex availability does not imply Codex ownership. Observer visibility does not
  imply writer authority. A Connector tool list does not imply permission for a
  concrete write.
