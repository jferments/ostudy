# Agent workflow and validation

Agents work as reviewable contributors, not as autonomous maintainers of `main`. The durable implementation is the published source branch and pull request—not a plan, local patch, source applicator, transient runner tree, or promise to publish later.

Read [`EXECUTION-STATE.md`](EXECUTION-STATE.md) for the completion predicate and compatibility-snapshot schema, [`FAILURE-TRIAGE.md`](FAILURE-TRIAGE.md) for failure/retry rules, and [`GITHUB-CONNECTOR.md`](GITHUB-CONNECTOR.md) when operating through the authenticated connector.

## 1. Restore or establish the exact task state

### New task

Choose the repository-access mode before editing.

#### Local checkout

```bash
git switch main
git status --short
git fetch origin --prune
git pull --ff-only
git rev-parse HEAD
git switch -c agent/<short-description>
```

Stop before destructive synchronization if the working tree contains changes that are not part of the task. Record the base commit SHA.

#### Authenticated GitHub Connector

Use Connector reads to resolve `jferments/ostudy`, fetch the intended base, and record the exact base SHA. Repository visibility, repository-level permission, exposure of a connector action, authorization for a concrete mutation, publication, and CI validation are separate states.

At the first intended mutation, invoke the real Connector operation and let its authorization boundary handle any required approval. Only an actual rejected mutation establishes a mutation failure. After every successful mutation, verify the resulting remote branch/head before claiming publication.

### Resumed task

Before editing:

1. read the open task PR and its `ostudy-agent-state` compatibility snapshot;
2. verify the actual remote base, branch, and head;
3. inspect exact-head CI and unresolved review threads;
4. enumerate non-terminal acceptance criteria and known failures;
5. reconcile stale draft metadata before another global status claim.

Do not reconstruct task state from conversational memory.

## 2. Define the change before implementation

Record:

- the observed problem or requested behavior;
- stable `AC-##` acceptance criteria;
- non-goals;
- architecture and compatibility surfaces;
- expected automated tests;
- manual, physical-device, live-provider, credential, or hardware boundaries.

Search the repository for all callers, related tests, commands, migrations, durable formats, and documentation before selecting an implementation.

Acceptance criteria remain stable identifiers throughout the PR. Update their status and evidence rather than rewriting completed requirements to match whatever happened to be implemented.

## 3. Publish the actual source early

Use one short-lived `agent/<description>` branch and one draft pull request.

The first meaningful commit may be an acceptance-test skeleton, ADR, migration shell, focused controller, or another truthful vertical checkpoint. Verify its remote head and open the draft PR before substantial implementation.

The PR body must distinguish:

- target goal and acceptance criteria;
- current published state;
- implementation evidence;
- exact-head validation evidence;
- pending manual boundaries;
- unresolved failures.

Do not:

- keep the only copy of product source in an agent scratch directory or CI runner;
- commit task-specific applicators/finalizers instead of the resulting source;
- use a PR-triggered Action to rewrite, commit, or push the product tree;
- represent planning/scaffolding as completed implementation;
- create replacement branches for ordinary test failures.

Automatic PR qualification is read-only. Any explicit source-writing maintenance workflow is separate, manual-only, narrowly scoped, refuses `main`, and is not ordinary product delivery.

## 4. Implement coherent vertical increments

Prefer commits such as:

1. domain/schema behavior and migration coverage;
2. native service and typed IPC;
3. controller and user interface;
4. focused regressions and acceptance coverage;
5. documentation and cleanup.

The exact sequence depends on the change. Each commit has one understandable purpose, contains real reviewable source, and is pushed to the same branch.

- Follow the owning layer described in the architecture guardrails.
- Reuse current domain, IPC, platform, controller, and UI boundaries.
- Add regression coverage with the behavior.
- Avoid dependency changes and broad refactors unless they are part of the accepted scope.
- Update affected documentation and ADRs in the same branch.
- Update the PR compatibility acceptance snapshot after each meaningful publication checkpoint.
- When changing a shared Rust struct, enum, serialized shape, migration schema, or other cross-module contract, search the entire repository for constructors, struct literals, deserializers, compatibility projections, tests, and documentation before declaring the increment complete. Do not rely on production call sites alone; stale test literals are compile failures too.
- Treat a migration file and its registered checksum as one atomic contract. If migration bytes change before release, update the checksum in the same coherent increment and immediately qualify the manifest/static boundary before moving on.
- For an architecture or monolith-removal refactor, publish the final ownership topology together with every required manifest, verifier, fixture, and regression-test update. Removing a required source path without registering its explicit replacement paths is an actionable static failure; do not weaken or delete the check merely to make the tree pass. A temporary compatibility module must be private, clearly documented, short-lived, and removed before terminal acceptance.

In a local checkout, inspect continuously:

```bash
git status --short
git diff --check
git diff --stat
git diff
```

In Connector mode, inspect the remote changed-file list/diff and current head through GitHub.

## 5. Validate in layers

Use the cheapest gate that answers the current question, then escalate only as the change stabilizes:

```bash
./scripts/check-changed.sh  # diff-aware iteration helper
./scripts/check-static.sh   # manifests, policy, docs, IPC, types, rustfmt
./scripts/check-fast.sh     # static + all frontend tests + locked Rust compile/Clippy/contracts
./scripts/check.sh          # authoritative Linux/Tauri merge qualification
```

Draft pull requests run the fast tier. Ready pull requests run the authoritative Linux and Android matrix on the self-hosted runner. A successful result qualifies only the exact source SHA it tested. `OSTUDY_FORCE_CHECK=1` bypasses exact-tree cache reuse when deliberately required.

Validation evidence is invalidated by every later source-tree change. Never describe a predecessor SHA as the current head's passing evidence, even when the later edit appears mechanical. Re-run the narrowest affected boundary, then escalate again.

For migration or manifest work, run static qualification immediately after changing migration bytes, registered checksums, version metadata, or generated manifests. Do not wait for a broad test run to discover a checksum mismatch. For a shared Rust-domain refactor, run a test-compiling Rust gate (`check-fast.sh` or the repository's focused equivalent) before reviewer handoff so stale `#[cfg(test)]` struct literals and warning-as-error failures cannot remain hidden behind successful frontend tests.

Manually exercise the affected UI path with `./run-app.sh`. For persistence work, restart and verify the stored result. For UI work, include keyboard, touch/coarse-pointer, pending/error/cancel states, compact/medium/wide layouts, reduced motion, and relevant themes.

Additional checks depend on the change:

- **IPC:** frontend wrapper names, Rust registration, serialization, validation, stable errors.
- **Database:** fresh/upgraded databases, rollback, checksums, constraints, data preservation.
- **Review/scheduling:** rating mapping, due transition, immutable event creation, idempotence.
- **Import/export:** round-trip formats, assets, IDs, schedules, excluded personal history.
- **UI:** empty/loading/error states, keyboard access, repeated clicks, responsive containment, themes.
- **Android:** compile/package and emulator smoke; physical camera or device transfer remains manual where required.

## 6. Consume failures before another broad run

A failed workflow must produce and upload `ostudy-qualification-result.json`. It records the exact head, gate, step outcomes, stable failure fingerprint, boundary, failed tests, classification hint, diagnostic filter, and focused reproduction.

After failure:

1. verify the tested SHA and current PR head;
2. read job steps, logs, machine-readable evidence, and bounded diagnostics;
3. identify all independent actionable failures;
4. classify each failure;
5. execute the narrowest reproduction;
6. repair one coherent batch;
7. publish and verify the new head;
8. rerun the focused boundary before escalating.

If static fails, consume the static failure before spending a fast/full run. If a formatter reports exact diffs, apply or reproduce the formatter output rather than repeatedly approximating formatting by hand.

An unchanged-head rerun is allowed once only when evidence supports a transient external failure. The same fingerprint twice requires a strategy change before another broad run. Never use no-op commits, PR-state toggles, permission expansion, blanket retries, longer timeouts, sleeps, test skips, or weakened assertions merely to obtain green CI.

## 7. Review the exact proposed commit

In a local checkout, stage specific paths whenever practical:

```bash
git add -- <paths>
git diff --cached --check
git diff --cached --stat
git diff --cached
```

Confirm that no secrets, personal data, build output, unrelated formatting, accidental lockfile changes, or task-specific source applicators are staged.

After committing:

```bash
git diff --name-only origin/main...HEAD
git diff --stat origin/main...HEAD
git log --oneline origin/main..HEAD
```

In Connector mode, verify the corresponding remote branch/PR diff and record the current remote head.

## 8. Maintain the pull-request acceptance snapshot

The compatibility snapshot is strictly machine-validated for non-draft PR events; draft source checks do not depend on it.

For draft work it may contain incomplete criteria and pending gates. A synchronize event may briefly make its recorded head stale. Reconcile it at the next checkpoint. Before requesting or interpreting a final qualification run, re-fetch the actual PR head and synchronize the snapshot to that SHA.

Before review readiness:

- `currentState` is `readyForReview`;
- actual base, branch, and head exactly match the snapshot;
- every acceptance criterion is terminal and has evidence;
- static, fast, and full passed on the current head;
- Android passed or is explicitly `notRequired` for a non-Android change;
- manual paths are passed, `notRequired`, or explicitly `humanRequired`;
- `knownFailures` is empty.

`notRun` is not completion evidence and cannot qualify readiness. Do not hand a branch to a reviewer as complete while any known automated failure remains, even when the failure appears mechanical or a predecessor SHA was green.

## 9. Propose; do not self-merge

Open or update one pull request against `main`. The description records:

- exact base/current head;
- goal, acceptance criteria, and non-goals;
- design decisions and tradeoffs;
- changed files grouped by purpose;
- tests and documentation;
- exact commands and CI runs that passed;
- commands not run and why;
- manual workflows actually exercised or still human-required;
- migration, compatibility, backup, rollback, known limitations, and residual risks.

Mark ready only after the compatibility snapshot and exact-head evidence satisfy the readiness contract. A human reviews, accepts, and squash-merges. Additional fixes stay on the same branch and PR.

## Validation language

Use exact statements:

- **Passed:** the named command completed successfully against the named tree/SHA.
- **Inspected:** source or output was reviewed without executing behavior.
- **Not run:** the check did not execute; reason recorded.
- **Manually verified:** the named behavior was actually exercised.
- **Published:** GitHub confirmed the remote branch/head.
- **Blocked:** a prerequisite prevents the next state; work is not complete.

Never say code is fully tested merely because it parses, compiles, or looks correct. Never say a Connector mutation exists remotely because a local patch was generated. Never say a task is complete while required acceptance, exact-head validation, manual boundaries, or known failures remain unresolved.
