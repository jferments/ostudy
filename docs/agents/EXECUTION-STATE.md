# Agent execution state and completion evidence

This document defines the durable state model for automated contributors working on ostudy. It prevents planning, local edits, publication, validation, manual acceptance, and completion from being collapsed into one vague claim that a task is "done."

## State is multidimensional

A change moves through independent states:

```text
specified
implemented
published
statically valid
fast-qualified
fully qualified
manually exercised
reviewed
merged
```

No state implies the next one. A generated patch is not publication. A published commit is not validation. A compile result is not a native application test. A green run for an older commit does not qualify the current pull-request head. Automated qualification does not prove a physical-device, live-provider, or visual workflow that was not exercised.

Use precise language:

- **Specified:** acceptance criteria and non-goals are recorded.
- **Implemented:** source for the criterion exists in the current branch head.
- **Published:** GitHub confirms the remote branch/head containing the source.
- **Passed:** the named command completed successfully against the named source tree.
- **Inspected:** source, diff, or output was reviewed without executing the behavior.
- **Manually verified:** the named workflow was actually exercised.
- **Not run:** the check was unavailable, outside the environment, or intentionally deferred, with the reason recorded.
- **Blocked:** a prerequisite prevents the next state; this is not completion.

## Completion predicate

For exact head commit `h`, a task may be described as complete only when:

```text
Complete(h) = Acceptance(h)
           AND Published(h)
           AND AutomatedValidation(h)
           AND ManualAcceptance(h)
           AND no unresolved known failure for h
```

Where:

- `Acceptance(h)` means every in-scope acceptance criterion is implemented in `h` or explicitly recorded as a human-only/manual boundary.
- `Published(h)` means GitHub confirms `h` is the task branch and pull-request head.
- `AutomatedValidation(h)` means every required gate passed against exactly `h`.
- `ManualAcceptance(h)` means every required manual workflow was exercised, or remains explicitly assigned to the human because the agent environment cannot perform it.

When one term is missing, report the narrower state instead:

- `Implementation is published; static validation is pending.`
- `Fast qualification passed on abc123; full Linux and Android qualification are pending.`
- `Automated qualification passed; physical camera acceptance remains.`
- `Five of seven acceptance criteria are implemented.`

## Pull-request acceptance snapshot

For compatibility with current validation and handoff tooling, substantial pull requests keep one machine-readable acceptance snapshot in the pull-request body between these markers:

````markdown
<!-- ostudy-agent-state:start -->
```json
{
  "schemaVersion": 1,
  "baseSha": "0123456789abcdef0123456789abcdef01234567",
  "headSha": "fedcba9876543210fedcba9876543210fedcba98",
  "branch": "agent/example",
  "executionMode": "githubConnector",
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
      "evidence": ["workflow-run-123"]
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

This snapshot is not a source of truth for the branch head, an Actions run, runner health, or active task ownership. It must never be used as a CI trigger or branch lock. GitHub and exact-SHA workflow evidence remain authoritative.

`scripts/verify-pr-contract.mjs` validates the JSON shape and, for a non-draft pull request, requires exact agreement with the current GitHub base, head, and branch.

Draft synchronize events can race the human-readable snapshot update. A draft may therefore temporarily record `unrecorded`, `unqualified`, or an older identity without blocking source qualification. Before review readiness, the snapshot must be reconciled to the current exact head. A passed validation entry always records the exact SHA it tested.

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

A `passed` validation entry records the exact tested SHA and command/run/artifact evidence. `failed`, `blocked`, `notRun`, `notRequired`, and `humanRequired` entries require explanatory evidence; none may be used as an unexplained escape hatch.

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

`mergeMethod` is one of:

```text
squash
mergeCommit
rebase
humanDecision
```

Drafts may leave the four narrative arrays empty while facts are still being established. Before review readiness, `nonGoals`, `compatibilityNotes`, `rollbackNotes`, and `residualRisks` each contain at least one explicit entry; `None` is acceptable only when written as a checked, explanatory statement rather than omitted state.

A pull request may be marked ready only when every acceptance criterion has a terminal state, static/fast/full passed on the current exact head, every additional platform/manual entry is terminal and explained, the four readiness narrative arrays are populated, `knownFailures` is empty, and remaining manual-only boundaries are explicit.

A known failure is recorded as an object with:

```json
{
  "fingerprint": "qf-0123456789abcdef",
  "classification": "product",
  "boundary": "React tests",
  "reproduction": "pnpm exec vitest run src/example.test.tsx",
  "attempts": 1
}
```

The classification vocabulary is defined in [Failure triage](FAILURE-TRIAGE.md).

## Evidence-derived handoff

Every `Check` run writes `ostudy-qualification-result.json`. Non-draft pull-request runs also write `ostudy-pr-contract.json`; draft handoffs remain qualification-only so compatibility snapshot errors cannot block fast source feedback. `scripts/render-agent-handoff.mjs` combines the available validated records with the exact Git diff into `ostudy-agent-handoff.md`.

The renderer includes:

- base, branch, PR state, and exact published/tested head;
- acceptance status and implementation/test evidence;
- changed files grouped by purpose;
- non-goals, compatibility effects, rollback notes, and residual risks;
- validation evidence and the commands represented by the run;
- exact workflow/run/step state;
- manual acceptance and known failures;
- requested merge method;
- evidence-integrity warnings or errors.

It never silently combines mismatched SHAs. A qualification/PR-head mismatch produces an artifact marked `invalid` and a nonzero exit. When a draft snapshot is supplied, its drift remains visible as a warning while the actual GitHub head remains the publication authority.

The generated handoff is the factual starting point for the final response and PR review record. It does not replace human review or allow an agent to claim manual behavior it did not exercise.

## Session resumption

When work resumes in another conversation or after interruption, do not reconstruct task state from memory:

1. Read the repository and intended base through the active repository-access mode.
2. Read the open task pull request and its acceptance snapshot.
3. Verify the actual remote branch and head SHA.
4. Read exact-head CI state separately from publication state.
5. Enumerate acceptance criteria that are not terminal.
6. Read unresolved review threads and known failures.
7. Continue from that verified state.

If the acceptance snapshot and repository disagree, the repository is authoritative and the snapshot must be corrected before another completion claim.

## Progress checkpoints

For long work, report resumable checkpoints rather than unstructured activity:

```text
Published head: <sha>
Acceptance: 4/9 implemented
Latest gate: static passed on <sha>
Current blocker: one named regression
Next action: focused repair and reproduction
```

These checkpoints are not a substitute for the final handoff. They prevent silence, context loss, and unsupported global claims while implementation is underway.

## Evidence rules

- Every claimed remote mutation is verified by a GitHub read.
- Every validation claim names the command or workflow and exact SHA.
- A newer push invalidates an older green result.
- Manual acceptance records the concrete path exercised, not an intention to exercise it.
- External service, credential, camera, hardware, and physical-device boundaries remain explicitly pending rather than being inferred from unit or emulator tests.
- The target scope in a pull-request description is not evidence that the scope is implemented. `currentState` and per-criterion statuses describe present reality.
