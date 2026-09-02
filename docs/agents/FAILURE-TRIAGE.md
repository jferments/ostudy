# Agent failure triage and retry discipline

This document defines how automated contributors classify failures, consume
evidence, choose reproductions, and decide whether another broad qualification
run is justified. Read [`EXECUTOR-SELECTION.md`](EXECUTOR-SELECTION.md) before
changing writers in response to a failure.

## Classify before changing code

Every failure belongs to the narrowest supported category. Do not use evidence
from one category to make claims about another.

### Product defect

Evidence is a deterministic compiler, formatter, policy, test, migration,
package, or runtime failure caused by the proposed source.

Normal response:

1. identify the exact head and failing boundary;
2. run the smallest available reproduction in the current writer workspace;
3. fix the owning source boundary;
4. add or strengthen a regression;
5. rerun the focused boundary before escalating.

Compatibility-snapshot classification: `product`.

### Test-contract defect

Evidence shows that a test encodes behavior that conflicts with an accepted
requirement, current invariant, or execution environment contract. A failing
test is not automatically a bad test. Explain the contract conflict before
editing the fixture or assertion, and never delete, skip, focus, or weaken
coverage merely to obtain green CI.

A guarded development host rejecting a synthetic `main` push is a test-contract
defect when the product under test does not require that fixture ref. Use a
non-protected synthetic branch and keep explicit tests that production
publication rejects protected branches. Do not disable the host guard.

Compatibility-snapshot classification: `testContract`.

### Direct-workspace boundary

Evidence identifies a failure in `workspace_open`, `workspace_read`,
`workspace_apply`, `workspace_job`, `workspace_diff`, or `workspace_publish`.
Preserve the persistent worktree and report:

- exact workspace operation and error;
- project, branch, local head, and remote head;
- worktree diff hash and dirty paths;
- active PID/job and bounded log tail;
- server contract hash and client-visible tools when tool discovery is involved.

A failed workspace operation does not establish a GitHub outage, CI failure, or
Codex failure. Do not replace the branch, reset the worktree, promote an observer
into a writer, or fall back to low-level GitHub writes before the exact workspace
boundary is understood.

Compatibility-snapshot classification: `workspace` when the schema supports it;
otherwise use `runnerEnvironment` with the precise workspace boundary in the
failure record until the compatibility vocabulary is extended.

### Codex/executor boundary

Evidence identifies Codex quota, rate-limit, sandbox, tool-contract, transport,
or executor failure. This affects only work deliberately assigned to Codex.

- Advisory Codex failure does not affect direct-workspace writer state.
- Exclusive Codex implementation failure preserves its branch and unpublished
  diff through the bounded handoff path.
- Do not transfer the branch until the Codex job is stopped, worktree state is
  accounted for, and ownership is explicitly released.
- Codex failure never blocks a task that is already suitable for the direct
  workspace.

Compatibility-snapshot classification: `executor` when supported; otherwise use
`runnerEnvironment` with an explicit Codex/executor boundary.

### Runner or environment defect

Evidence identifies a toolchain, resource, KVM/emulator, filesystem, native
dependency, service-account, host policy, or network-provisioning failure
independent of product semantics. Record the exact command and environment
evidence. Do not change application behavior, increase product timeouts, or
weaken tests to compensate.

Compatibility-snapshot classification: `runnerEnvironment`.

### Connector boundary

Only an actual Connector operation and its exact rejection establish a Connector
capability or authorization failure. A missing tool action, unavailable Actions
endpoint, absent approval prompt, direct-workspace error, Codex error, or
unrelated local checkout is not evidence that repository writes are unavailable.

Compatibility-snapshot classification: `connector`.

### GitHub incident

Broad `5xx`, webhook, Actions, pull-request, archive, or API failures across
unrelated operations require control-plane classification and an official
GitHub Status check. Preserve the exact head and avoid no-op commits, workflow
rewrites, PR-state toggles, or permission expansion merely to trigger another
run.

Compatibility-snapshot classification: `githubIncident`.

### External manual boundary

Physical cameras/devices, private credentials, live provider routing,
hardware-specific behavior, and subjective visual acceptance can remain
human-only when deterministic automation cannot reproduce them. Record the
exact workflow still required; do not infer success from unit, mocked, or
emulator coverage.

Compatibility-snapshot classification: `externalManualBoundary`.

Use `unknown` only while evidence is genuinely insufficient. The next action
must gather evidence, not guess.

## Failure record

Each unresolved failure has one durable record:

```json
{
  "fingerprint": "qf-0123456789abcdef",
  "classification": "product",
  "boundary": "React tests",
  "reproduction": "pnpm exec vitest run src/screens/DecksScreen.test.tsx --reporter=verbose",
  "attempts": 1
}
```

The machine-readable qualification artifact generated by
`scripts/write-qualification-result.mjs` supplies a stable fingerprint,
boundary, classification hint, failed-test names, log filter, and focused
reproduction. A local direct-workspace job supplies its own exact head, diff
hash, PID, command name, status, and bounded log tail. These are evidence to
review, not automatic authorities that override the actual source and logs.

A fingerprint excludes commit SHAs, durations, and incidental numbers so the
same underlying failure can be recognized across repair commits. When multiple
independent failures are present, retain separate records rather than combining
them into one generic "development failed" entry.

## Local workspace evidence first

For a failure before publication:

1. call `workspace_status` and verify branch, local/remote head, dirty paths,
   diff hash, active job, and contract status;
2. use `workspace_job(action="status")` or `action="log"` for the named symbolic
   command;
3. classify product, test-contract, workspace, executor, or environment failure;
4. run the narrowest configured reproduction;
5. repair in the same writer workspace;
6. rerun the focused local boundary before publication.

Do not publish merely to obtain formatter, type-check, or focused-test feedback
from CI. Do not invoke the observer when the direct workspace already owns and
can report its own job.

## CI log transport and Connector triage

The `Check` workflow separates complete raw command logs from bounded live
Actions output. Qualification commands run through `scripts/run-ci-captured.sh`:
stdout/stderr are written in full to the corresponding `ostudy-*-check.log`,
while the live job log receives a metadata-only liveness heartbeat and a
byte-bounded tail. On failure, `scripts/summarize-check-failure.mjs` writes
`ostudy-failure-summary.md`, which is printed and uploaded with exact-head
diagnostics.

For Connector-backed CI triage:

1. verify the exact PR head and Check run;
2. read the machine-readable qualification result and failure summary first;
3. use the focused reproduction named there;
4. open the complete raw log artifact only when bounded evidence is insufficient.

A continuing heartbeat means the wrapper and command are live; zero byte growth
is a diagnostic clue, not proof of a hang. Use the runtime that launched the job
for PID/process state. Heartbeats never constitute pass/fail evidence.

Do not restore unbounded raw log streaming to `check.yml`; it defeats the bounded
evidence design.

## Failure-response sequence

After any failure:

1. Identify which runtime or service owned the failing operation.
2. Verify exact local and/or published head and current branch ownership.
3. Read the narrowest available structured evidence and bounded log.
4. Identify every independent actionable failure.
5. Classify each failure without borrowing evidence from another boundary.
6. Execute the smallest reproduction in the current writer workspace when
   possible.
7. Repair one coherent batch on the same branch.
8. Run the focused boundary first.
9. Publish only after required local validation passes.
10. Escalate to static, fast, full, and platform qualification as appropriate.
11. Update the compatibility snapshot and known-failure attempts at the next
    meaningful checkpoint.

Do not ask the human to reproduce evidence already available through the current
workspace or CI artifacts.

## Retry budget

An unchanged head may receive one bounded rerun only when evidence supports a
transient external cause, such as a connection reset during dependency
provisioning or a documented WebDriver transport failure. Record both the
original failure and unchanged-head rerun.

The same failure fingerprint twice requires a strategy change before another
broad run:

- use the focused reproduction;
- inspect the owning source and test boundary;
- gather additional structured diagnostics;
- split independent failures;
- or reclassify the problem with evidence.

Prohibited retry behavior:

- no-op commits to retrigger CI;
- closing/reopening or toggling draft state to force a workflow;
- repeating the complete Linux/Android matrix while a focused failure remains;
- adding sleeps, retries, larger timeouts, skips, or weakened assertions without
  evidence that they model intended behavior;
- changing workflow permissions or source merely because an Actions read is
  unavailable;
- replacing a recoverable task branch for ordinary test failures;
- switching between Codex, direct workspace, and Connector merely because one
  attempt failed;
- disabling host branch guards or global safety policy to make a synthetic
  fixture pass.

## Stopping and handoff rules

Stop broad iteration and report a concrete partial handoff when:

- the same fingerprint recurs after a targeted repair;
- evidence points to a different ownership boundary than the current approach;
- branch writer ownership is ambiguous;
- a dirty worktree exists without accountable ownership state;
- the branch contains generated/applicator scaffolding instead of reviewable
  source;
- the next required step is human-only;
- GitHub is degraded and exact-head qualification cannot be established;
- or the accepted scope must be decomposed before safe review.

A writer handoff is allowed only after the current implementation job stops, the
exact remote head and unpublished diff are recorded, and ownership is explicitly
released. Stopping a bad strategy is not abandonment; preserve source and
evidence, then correct the approach transparently.

## Examples

### Correct product-failure response

```text
Direct workspace head abc123 and diff def456 failed the DecksScreen Vitest file.
The controller overwrites dirty-state protection. Repair the controller and
regression in the same worktree, rerun that test boundary, then static and fast.
```

### Correct test-contract response on a guarded host

```text
The direct-workspace installer qualification failed before service behavior ran
because a synthetic fixture pushed refs/heads/main. The host correctly rejected
that protected ref. Change the fixture default to fixture/base, retain the test
that product publication rejects main, and rerun the focused suite.
```

### Incorrect access inference

```text
CI is queued with no runner, therefore GitHub writes are unavailable.
```

Runner capacity and repository publication are independent.

### Correct Connector report

```text
Authenticated repository reads succeed. No fallback mutation has been attempted,
so concrete Connector write authorization is untested.
```

### Correct Codex report

```text
Codex reached its quota while advising on a published SHA. The direct workspace
still owns the branch and remains healthy; implementation is not blocked.
```

### Incorrect completion report

```text
The feature is done; cargo fmt and the full suite still need to be run.
```

The correct state is `implementation present; required validation pending`.
