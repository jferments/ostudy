## Current state

State what exists in the current published head. Do not describe the target scope as though it is already implemented.

- Current phase: planning / scaffolding / implementation / validation / manual acceptance
- Implemented acceptance criteria: 0/N
- Latest exact-head qualification: pending
- Current blocker: none / describe one concrete blocker

## Goal

Describe the user-visible or developer-visible result.

## Acceptance criteria

Use stable identifiers and keep these synchronized with the compatibility acceptance snapshot.

- **AC-01:** Describe one independently verifiable requirement.
- **AC-02:** Describe another independently verifiable requirement.

## Non-goals

- State behavior deliberately outside this pull request.

## Architecture and compatibility surfaces

Describe affected ownership boundaries, migrations, stored data, imports/exports, platforms, privacy, accessibility, dependencies, and rollback concerns. Write `None` only after checking each category.

## Changed files

Group the current changed files by purpose. This section describes the published branch, not planned files.

## Validation

Record exact commands and workflow runs that actually completed. A passed entry must identify the exact commit SHA. Record skipped or unavailable checks explicitly.

Required merge qualification:

- `./scripts/check-static.sh`
- `./scripts/check-fast.sh`
- `./scripts/check.sh`

## Manual acceptance

List concrete workflows actually exercised, plus physical-device, live-provider, credential, visual, or other human-only boundaries that remain.

## Known limitations and residual risks

List unresolved defects, compatibility effects, rollback concerns, or `None known` when supported by evidence.

The JSON below is a compatibility evidence snapshot. GitHub's remote head and exact-SHA Actions results remain authoritative; editing this snapshot does not trigger or select product qualification.

<!-- ostudy-agent-state:start -->
```json
{
  "schemaVersion": 1,
  "baseSha": "unrecorded",
  "headSha": "unqualified",
  "branch": "agent/replace-with-task-branch",
  "executionMode": "localCheckout",
  "currentState": "planning",
  "nonGoals": [],
  "compatibilityNotes": [],
  "rollbackNotes": [],
  "residualRisks": [],
  "mergeMethod": "squash",
  "acceptance": {
    "AC-01": {
      "status": "notStarted",
      "evidence": [],
      "tests": []
    },
    "AC-02": {
      "status": "notStarted",
      "evidence": [],
      "tests": []
    }
  },
  "validation": {
    "static": {
      "status": "pending"
    },
    "fast": {
      "status": "pending"
    },
    "full": {
      "status": "pending"
    },
    "android": {
      "status": "pending"
    }
  },
  "manualAcceptance": {
    "affectedWorkflow": {
      "status": "pending"
    }
  },
  "knownFailures": []
}
```
<!-- ostudy-agent-state:end -->

## Merge

Requested method: **Squash and merge** after exact-head qualification and human review.
