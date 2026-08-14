# Agent-driven diagnostics

ostudy provides an opt-in diagnostic workflow for development branches where a Connector-backed coding agent needs broad failure information from the self-hosted runners without asking a human to copy command output between GitHub and the agent.

The diagnostic sweep is a **development aid**, not a merge gate. The existing `check-static.sh`, `check-fast.sh`, `check.sh`, `android-check.yml`, and official Android Emulator smoke remain authoritative because they preserve the repository's normal fail-fast qualification semantics.

## Why this exists

The authoritative gates intentionally stop when a prerequisite fails. That is correct for merge qualification, but it can create a slow repair loop during development: fix the first TypeScript error, rerun, discover a Vitest error, rerun, discover Clippy errors, and so on.

`./scripts/check-diagnostic.sh` instead treats independent checks as diagnostic gates. It continues after failures, records each gate as `PASS`, `FAIL`, or `BLOCKED`, and only blocks work that genuinely depends on a failed prerequisite. A single runner pass can therefore expose several unrelated defects at once while still avoiding misleading cascades from prerequisites that never succeeded.

## Local diagnostic command

```bash
./scripts/check-diagnostic.sh --profile core --output-dir .diagnostics/core
./scripts/check-diagnostic.sh --profile android --output-dir .diagnostics/android
```

`--profile all` is available when one machine satisfies both the shared and Android/KVM prerequisites.

The command writes:

- `results.tsv`: append-friendly gate status records used by the summarizer;
- `diagnostics.json`: schema-versioned machine-readable results, dependencies, exit codes, and bounded highlights;
- `summary.md`: a concise human/agent-oriented status table and failure excerpts;
- `logs/*.log`: complete output for each diagnostic gate;
- Android emulator artifacts under the selected output directory when the Android smoke gate runs.

The process exits non-zero when any diagnostic gate fails. `BLOCKED` gates do not independently make the process fail because their prerequisite failure is already represented.

## Core profile

The core sweep independently checks repository and documentation contracts, test policy, shell/script tests, frozen JavaScript installation, dependency versions, TypeScript, Rust formatting, PDFium provisioning, Cargo metadata, Vitest, Cargo compile, Clippy, Rust tests, and the production frontend build.

JavaScript checks that require installed dependencies are blocked only when `pnpm install --frozen-lockfile` fails. Rust compile/test gates are blocked when the locked Cargo graph or audited PDFium runtime cannot be prepared. Other repository-policy checks continue so one infrastructure failure does not hide unrelated structural errors.

## Android profile

The Android sweep uses the Android/KVM self-hosted runner and separates environment/project checks from Kotlin compilation, Gradle dependency resolution, the x86_64 debug build, instrumentation-package generation, and official-emulator smoke.

The emulator gate is intentionally blocked when a debuggable APK cannot be built or the runner doctor fails. That dependency is real: an emulator cannot reveal application-runtime defects until a valid test artifact exists. The diagnostic workflow removes avoidable serial discovery, but it does not pretend genuinely dependent failures can be known in advance.

## Opting a pull request into diagnostics

`Agent diagnostics` is intentionally inactive for ordinary pull requests. This prevents a development helper from consuming runner capacity or changing the behavior of unrelated work.

A pull-request body may contain one of these markers:

```text
<!-- agent-diagnostics:core -->
<!-- agent-diagnostics:android -->
<!-- agent-diagnostics:all -->
```

- `core` runs only the shared diagnostic sweep on the ordinary ostudy self-hosted runner.
- `android` runs only the Android diagnostic sweep on the Android/KVM runner.
- `all` enables both independent jobs.

Editing the PR body to add a marker triggers the workflow. Subsequent pushes to that PR automatically rerun the selected diagnostic jobs. Concurrency cancellation stops obsolete diagnostic runs for the same PR; it does not cancel the repository's authoritative workflows.

Removing the marker disables future diagnostic jobs. The workflow has read-only repository permission and never commits, pushes, edits source, or writes to `main`.

## Connector-backed agent repair loop

For substantial Connector-backed work, the agent should own the CI feedback loop when the self-hosted runner can execute the relevant test:

1. publish a coherent branch head and verify the remote SHA;
2. opt the PR into the smallest useful diagnostic profile;
3. inspect workflow state for that exact head through the GitHub Connector;
4. retrieve failed job steps, decoded logs, and uploaded diagnostic artifacts directly;
5. identify all actionable independent failures from the same sweep before editing;
6. repair the failures in a coherent batch on the same PR branch;
7. publish and verify the new remote head;
8. inspect the new exact-head diagnostic and authoritative workflow results;
9. repeat until the relevant diagnostic sweep and authoritative qualification are green, or report a concrete blocker that cannot be resolved from repository/CI access.

Do not ask the human to run `pnpm check:static`, `pnpm check`, `pnpm android:smoke`, or another repository test merely to relay output when an available self-hosted workflow can execute it and the Connector can read the resulting logs. Human intervention remains appropriate for manual UI judgment, physical-device behavior, credentials that must not enter CI, host administration, or a runner failure that requires access outside GitHub.

GitHub Actions is asynchronous. A queued or running diagnostic is not a passing result. If the runner is busy, report the exact queued/running state separately from publication rather than polling aggressively or asking the human to duplicate the job locally.

## Interpreting results

Use the statuses literally:

- **PASS:** the gate completed successfully for the recorded head.
- **FAIL:** the gate executed and returned a non-zero exit code or timed out.
- **BLOCKED:** the gate was not executed because a declared prerequisite did not pass.

Fix `FAIL` results before reasoning about their `BLOCKED` dependents. After a prerequisite is repaired, the next sweep may legitimately expose a previously blocked defect. That is a real dependency boundary, not the avoidable one-error-at-a-time loop this workflow is designed to remove.

## Relationship to merge qualification

A green diagnostic sweep does not qualify a pull request for merge. Before handoff, the exact published head must still satisfy the normal repository requirements in `testing-and-ci.md`, including the full shared gate and Android qualification when applicable.

The diagnostic workflow never modifies the application, never suppresses failures, and never grants an agent permission to waive an authoritative gate. Its purpose is to make the development feedback cycle broader and to move routine log collection from the human to the agent.
