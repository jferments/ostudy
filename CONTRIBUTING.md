# Contributing to ostudy

ostudy welcomes bug reports, feature proposals, documentation improvements, tests, and code changes. The project stores personal study data and contains several long-lived compatibility boundaries, so a useful contribution starts by describing the user problem clearly and ends by showing exactly how the proposed change was validated.

Read the [documentation index](docs/README.md) before changing a subsystem. Architecture and testing documents are maintained separately so that a contributor can learn the relevant boundary without reverse-engineering every historical pull request.

## Report a bug

Use the repository's bug-report form and include enough information to reproduce the behavior:

- the ostudy version or exact commit;
- operating system and architecture;
- the screen and action that produced the problem;
- what you expected and what actually happened;
- whether the problem survives an application restart;
- the smallest reproducible sequence;
- the validation command or manual test that fails;
- a privacy-conscious diagnostic archive when it is relevant and safe to share.

Do not attach API keys, Authorization headers, private PDFs, card contents, the live SQLite library, operating-system credential data, or an unreviewed full diagnostic directory. Redact paths and source excerpts that identify a patient, student, employer, or private course material.

For PDF and AI problems, also report the provider family, model identifier, whether the PDF is text-based or scanned, whether OCR ran, the processing phase shown by the interface, and the normalized error message. A document can usually be diagnosed without publishing the document itself.

## Propose a feature

A strong feature request explains the workflow rather than only naming a technology. Describe:

1. the learner or contributor problem;
2. the current workaround;
3. the desired user-visible behavior;
4. important data, privacy, accessibility, platform, and compatibility constraints;
5. what would count as a successful first version;
6. what can be deferred.

Check the [roadmap](docs/roadmap.md) first. A roadmap item still benefits from a focused issue when implementation is ready, but the issue should refine the accepted direction rather than assume every possible version of the idea is in scope.

Features involving AI must identify what deterministic code remains authoritative, which source data may leave the device, how provider selection works, and what happens when the model is unavailable or wrong. Features involving synchronization or sharing must identify ownership, conflict, deletion, encryption, and recovery semantics before implementation begins.

## Prepare a development environment

Ubuntu/Debian Linux is the primary qualified development path:

```bash
./scripts/setup-ubuntu.sh
./run-app.sh
```

Platform status and source-build instructions are documented under [`docs/installation/`](docs/installation/). The [development guide](docs/development/guide.md) explains the repository layout and the [testing guide](docs/development/testing-and-ci.md) explains the validation tiers.

## Design the change

Before editing code:

- inspect the existing implementation, callers, tests, migrations, transfer formats, and relevant documentation;
- state concrete acceptance criteria and non-goals;
- identify persistence, migration, import/export, scheduling, analytics, platform, accessibility, and security effects;
- use an existing boundary or shared primitive before adding a parallel abstraction;
- add an Architecture Decision Record when the change establishes a durable cross-module rule.

Do not edit released migrations or release-schema fixtures. Add a new migration or versioned data repair when current data must change. Do not introduce a compatibility shim solely to preserve an obsolete internal command; preserve compatibility when a released durable format or stored library actually requires it.

## Branch and pull-request workflow

Never develop directly on `main`. Start from the current intended base, create one short-lived branch, publish a meaningful first commit, and open one draft pull request. Continue on that branch rather than creating replacement branches for ordinary failures.

The authoritative workflow is [Git and pull-request workflow](docs/development/workflow.md). A completed handoff records the base and head commits, changed behavior, files, tests, commands actually run, commands not run, manual acceptance, migration effects, known limitations, and residual risks.

Use **Squash and merge** after the exact pull-request head passes automated qualification and human acceptance. Do not force-push shared history or self-merge unless the repository owner explicitly requests that action.

## Code and documentation standards

- Keep one coherent behavior per pull request.
- Do not mix feature work with unrelated dependency upgrades or broad formatting churn.
- Add a regression test for a defect whenever a practical test layer exists.
- Preserve keyboard, touch, compact-layout, reduced-motion, theme, and assistive-technology behavior.
- Keep provider networking and credential access in their native boundaries.
- Keep authoritative validation, transactions, scheduling, and analytics in Rust/SQLite rather than duplicating them in React.
- Use ordinary domain names in production code and documentation; implementation-stage labels belong in history.
- Write documentation for a reader's task. Put implementation detail in architecture or development references instead of the product README.
- Update affected links and run the documentation verifier when files move.

Automated-maintainer rules are collected in [`AGENTS.md`](AGENTS.md) and [`docs/agents/`](docs/agents/). Human contributors should understand the same architectural constraints even when they do not use an AI coding tool.

## Validate the change

Use the tier that fits the current iteration, then run the complete gate before merge:

```bash
./scripts/check-changed.sh
./scripts/check-static.sh
./scripts/check-fast.sh
./scripts/check.sh
```

A documentation-only change still runs the documentation/link and repository-organization checks. A parse check, TypeScript build, or mocked frontend test is not a substitute for native Tauri qualification when native behavior changed.

After the automated gate, launch the standalone build:

```bash
./run-app.sh
```

Manually exercise every changed workflow at relevant widths, themes, input methods, persistence boundaries, and error states. Record what was actually tested rather than copying a generic checklist into the pull request.

## Pull-request description

Keep the pull request current as implementation changes. Before review it should contain:

- a concise user-visible summary;
- exact base and head commits;
- acceptance criteria and non-goals;
- an explanation of important design choices;
- changed files grouped by purpose;
- tests added or updated;
- exact local commands and CI runs that passed;
- skipped or unavailable checks and why;
- manual workflows actually exercised;
- migration, backup, compatibility, rollback, and residual-risk notes.

The pull request remains a proposal until a human reviews and merges it.
