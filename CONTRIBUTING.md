# Contributing to ostudy

ostudy welcomes bug reports, feature proposals, documentation improvements, tests, and code changes. The project stores personal study data and contains long-lived compatibility boundaries, so a useful contribution starts by describing the user problem clearly and ends by showing exactly which published commit was validated.

Read the [documentation index](docs/README.md) before changing a subsystem. Architecture and testing documents are maintained separately so contributors do not have to reverse-engineer historical pull requests.

## Report a bug

Use the bug-report form and include enough information to reproduce the behavior:

- ostudy version or exact commit;
- operating system and architecture;
- screen/action that produced the problem;
- expected and actual behavior;
- whether it survives restart;
- smallest reproducible sequence;
- validation command or manual test that fails;
- a privacy-conscious diagnostic archive when relevant and safe.

Do not attach API keys, Authorization headers, private PDFs, card contents, the live SQLite library, operating-system credential data, or an unreviewed full diagnostic directory. Redact paths and excerpts that identify a patient, student, employer, or private course material.

For PDF/AI problems, also report provider family, model identifier, text/scanned source, OCR state, visible processing phase, and normalized error. A document can usually be diagnosed without publishing it.

## Propose a feature

A strong request explains the workflow rather than naming a technology:

1. learner/contributor problem;
2. current workaround;
3. desired user-visible behavior;
4. data, privacy, accessibility, platform, and compatibility constraints;
5. successful first version;
6. deferred behavior.

Check the [roadmap](docs/roadmap.md). A roadmap item still benefits from a focused issue when implementation is ready.

AI features identify deterministic authority, source data that may leave the device, provider selection, and failure behavior. Synchronization/sharing features identify ownership, conflict, deletion, encryption, and recovery semantics before implementation.

## Prepare a development environment

Ubuntu/Debian Linux is the primary qualified path:

```bash
./scripts/setup-ubuntu.sh
./run-app.sh
```

Platform instructions live under [`docs/installation/`](docs/installation/). The [development guide](docs/development/guide.md) explains repository layout and [Testing and CI](docs/development/testing-and-ci.md) explains qualification.

## Design the change

Before editing:

- inspect implementation, callers, tests, migrations, transfer formats, and relevant docs;
- assign stable `AC-##` acceptance criteria and explicit non-goals;
- identify persistence, migration, import/export, scheduling, analytics, platform, accessibility, security, dependency, and rollback effects;
- identify automated tests and human-only boundaries;
- use an existing boundary/shared primitive before adding a parallel abstraction;
- add an ADR for a durable cross-module rule.

Do not edit released migrations or release-schema fixtures. Add a new migration/versioned repair when current data must change. Preserve compatibility for released durable formats and stored libraries, not obsolete internal commands.

## Branch and pull-request workflow

Never develop directly on `main`. Start from the exact current base, create one short-lived `agent/<description>` branch, publish a meaningful direct-source commit, and open one draft PR. Continue on that branch rather than creating replacements for ordinary failures.

The authoritative process is [Git and pull-request workflow](docs/development/workflow.md).

### Direct-source invariant

The PR branch contains the actual source under review. The following are not delivery:

- a local/generated patch that was not published;
- a task-specific applicator/finalizer;
- a transient source tree produced only inside CI;
- a PR description claiming target behavior that the branch does not contain.

Automatic PR qualification is read-only. It may not persist write credentials, stage/commit/push source, or change PR readiness. Explicit manual maintenance workflows are separate and narrowly scoped.

### Pull-request state ledger

The PR template includes one machine-readable JSON ledger. Keep it synchronized with:

- exact base, branch, and current head;
- `currentState`;
- per-criterion status, implementation evidence, and tests;
- exact-head validation evidence;
- manual/human-only boundaries;
- known failure fingerprints and focused reproductions.

Draft metadata can briefly lag a synchronize event and will warn. Before readiness, the ledger must exactly match GitHub; all acceptance criteria must be terminal; static, fast, and full must pass on the current head; platform/manual requirements must be terminal; and known failures must be empty.

See [Agent execution state](docs/agents/EXECUTION-STATE.md).

Use **Squash and merge** after exact-head automated qualification and human acceptance. Do not force-push shared history or self-merge unless explicitly requested by the owner.

## Code and documentation standards

- Keep one coherent behavior per pull request.
- Do not mix feature work with unrelated dependency upgrades or formatting churn.
- Add a regression for a defect whenever a practical layer exists.
- Preserve keyboard, touch, compact layout, reduced motion, themes, and assistive behavior.
- Keep provider networking/credentials in native boundaries.
- Keep authoritative validation, transactions, scheduling, and analytics in Rust/SQLite rather than React.
- Use durable domain names; stage labels belong in history.
- Write docs for a reader's task; put implementation detail in architecture/development references.
- Update links and run the documentation verifier when files move.

Automated-maintainer rules are in [`AGENTS.md`](AGENTS.md) and [`docs/agents/`](docs/agents/). Human contributors follow the same architectural/evidence constraints.

## Validate the change

Use the cheapest useful tier, then complete merge qualification:

```bash
./scripts/check-changed.sh
./scripts/check-static.sh
./scripts/check-fast.sh
./scripts/check.sh
```

A documentation-only change still runs documentation/repository checks. Parse, TypeScript build, or mocked frontend tests do not substitute for native Tauri qualification when native behavior changed.

The automatic `Check` workflow validates the PR ledger, checks out the exact head, runs static first, and uploads `ostudy-qualification-result.json` on success or failure. The artifact records step outcomes and, on failure, a stable fingerprint and focused reproduction.

When a gate fails:

1. verify tested SHA/current head;
2. inspect steps, logs, machine-readable result, and bounded diagnostics;
3. identify/classify every independent failure;
4. run the focused reproduction;
5. repair one coherent batch;
6. rerun the narrow boundary before broad qualification.

One unchanged-head rerun is appropriate only with evidence of a transient external failure. Repeated fingerprints require a strategy change—not no-op commits, PR-state toggles, sleeps, blanket retries, larger timeouts, skipped tests, or weakened assertions. See [Agent failure triage](docs/agents/FAILURE-TRIAGE.md).

After automated qualification, launch:

```bash
./run-app.sh
```

Manually exercise every changed workflow at relevant widths, themes, input methods, persistence boundaries, and error states. Record what actually ran. Mark physical-device, camera, live-provider, private-credential, or hardware paths explicitly human-required when automation cannot perform them.

## Pull-request description and handoff

Keep the PR current. Before review it contains:

- truthful current state and target goal;
- exact base/head and branch;
- stable acceptance criteria/non-goals;
- important design choices;
- changed files grouped by purpose;
- tests/docs added;
- exact commands and CI runs that passed on the current head;
- unavailable checks and why;
- manual workflows actually exercised or human-required;
- migration, backup, compatibility, rollback, limitations, residual risks;
- empty known-failure list.

Use precise terms: **published**, **passed**, **inspected**, **not run**, **manually verified**, and **blocked**. Do not call a task complete while required acceptance, exact-head validation, manual boundaries, or known failures remain.

The pull request remains a proposal until a human reviews and merges it.
