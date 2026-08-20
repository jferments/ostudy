# Agent execution state and completion evidence

This document defines the durable state model for automated contributors working on ostudy. It prevents planning, local edits, publication, validation, and manual acceptance from being collapsed into one vague claim that a task is "done."

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

## Pull-request state ledger

Substantial pull requests keep one machine-readable ledger in the pull-request body between these markers:

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
    "android": {"status": "notRequired"}
  },
  "manualAcceptance": {
    "affectedWorkflow": {"status": "pending"}
  },
  "knownFailures": []
}
```
<!-- ostudy-agent-state:end -->
````

`scripts/verify-pr-contract.mjs` validates the JSON shape and, for a non-draft pull request, requires exact agreement with the current GitHub base, head, and branch.

Draft synchronize events can race the human-readable ledger update. A draft may therefore temporarily record `unrecorded`, `unqualified`, or an older identity; the verifier reports that drift as a warning. Before review readiness, the ledger must be reconciled to the current exact head. A passed validation entry always records the exact SHA it tested.

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

A draft pull request may contain incomplete statuses. A pull request may be marked ready only when every acceptance criterion has a terminal state, every required automated gate is terminal and any passed gate refers to the current exact head, `knownFailures` is empty, and remaining manual-only boundaries are explicit.

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

## Session resumption

When work resumes in another conversation or after interruption, do not reconstruct task state from memory:

1. Read the repository and intended base through the active repository-access mode.
2. Read the open task pull request and its state ledger.
3. Verify the actual remote branch and head SHA.
4. Read exact-head CI state separately from publication state.
5. Enumerate acceptance criteria that are not terminal.
6. Read unresolved review threads and known failures.
7. Continue from that verified state.

If the ledger and repository disagree, the repository is authoritative and the ledger must be corrected before another completion claim.

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
