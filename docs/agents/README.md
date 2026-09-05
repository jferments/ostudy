# Agent and LLM maintenance guide

[AGENTS.md](../../AGENTS.md) is the repository-wide entry point. This directory is an operational reference, not permission to choose a different development system or redesign unrelated features.

## The operating contract

For normal ChatGPT tasks, use **GitHub-direct only**: authenticated Connector reads, ordinary source in expected-parent Git-data commits, non-force publication, remote verification, and exact-SHA GitHub Actions qualification. Exactly one writer owns the branch. Do not introduce Codex, a workspace MCP, observer, orchestrator, exact-head service, or tunnel unless the human explicitly requests it for the current task.

Read [GitHub-direct development](GITHUB-DIRECT-DEVELOPMENT.md) first. It is the canonical workflow, including the prohibition on source-writing Actions and the immediate GitHub fallback after a requested MCP fails. Other guides explain individual concerns; they do not create exceptions.

## Reading map

| Task | Maintained reference |
| --- | --- |
| Start, resume, publish, or integrate main | [GitHub-direct development](GITHUB-DIRECT-DEVELOPMENT.md) |
| Understand publication versus execution | [Source-publication boundary](SOURCE-PUBLICATION-BOUNDARY.md) |
| Record acceptance, evidence, readiness, and completion | [Execution state](EXECUTION-STATE.md) |
| Diagnose an actual failure | [Failure triage](FAILURE-TRIAGE.md) and [Connector diagnostics](GITHUB-CONNECTOR.md) |
| Prepare a coherent implementation | [Workflow and validation](WORKFLOW-AND-VALIDATION.md) |
| Give local build/test/install instructions | [Command runbook](COMMAND-RUNBOOK.md) and [local testing handoff](../development/local-testing-and-handoff.md) |
| Preserve domain and dependency boundaries | [Architecture guardrails](ARCHITECTURE-GUARDRAILS.md), [code standards](CODE-STANDARDS.md), [AI provider guardrails](AI-PROVIDER-GUARDRAILS.md) |
| Update guides, audit stale code, or reconcile plans | [Documentation maintenance](DOCUMENTATION-MAINTENANCE.md) |

## Source of truth for the product

The [architecture overview](../architecture/overview.md) is the comprehensive current-system document. Use its source map and the focused [UI](../architecture/ui.md), [card/data model](../architecture/card-format-and-data-model.md), [AI infrastructure](../architecture/ai-infrastructure.md), [document processing](../architecture/ai-and-document-processing.md), [platform](../architecture/platform-services.md), and [sync](../architecture/device-sync.md) references. Accepted [ADRs](../decisions/) preserve rationale; the [roadmap](../roadmap.md) distinguishes implemented features from planned work.

The [documentation index](../README.md) routes human readers to task-oriented guides. The [cleanup audit](../development/documentation-and-cleanup-audit.md) explains retained compatibility and the evidence required before further removal. Historical stage plans and dated snapshots must not override current source or the latest explicit user request.

## Rules that prevent drift

Do not repeat integration discovery after tools are established. Invoke the real GitHub operation; absent approval prompts and queued CI do not establish write failure. Do not create ownership-marker commits, temporary source-transfer repositories, source-generating workflows, or opaque payloads.

After each publication, verify the remote head. After each test, record the exact tested SHA and gate. A PR-body snapshot is not a source writer, branch lock, CI trigger, or runner controller. Record only actual workspace/process state; in GitHub-only mode there is no agent execution worktree to certify clean.

Before a handoff, read the command runbook and inspect changed wrappers. Provide a safe checkout and exact-SHA check, a short native build/run or Android install path, and the specific acceptance observations. Never confuse `pnpm build`, a standalone Tauri build, a release package, and a passing full gate.

Do not claim that code is unused from a filename alone, that a feature is shipped from its enum variant alone, or that a planned stage is complete because a PR mentions it. Trace runtime reachability, persistence and compatibility readers, tests, and platform fallbacks. Keep publication, validation, manual acceptance, and human merge as separate facts.
