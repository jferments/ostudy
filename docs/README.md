# ostudy documentation

Start with the [project introduction](../README.md) for what ostudy is and why it exists. These guides are organized around what you want to do. Current behavior is described by the maintained guides and source, not by an older implementation-stage checklist.

## Use the application

| Task | Guide |
| --- | --- |
| Create a first deck and study session; find current navigation | [Getting started](user-guide/getting-started.md) |
| Create/edit cards, use templates, search, and manage selections | [Flashcards and authoring](user-guide/flashcards-workspace.md) |
| Organize folders, reorder decks, and understand selection/lifecycle rules | [Folders and deck organization](user-guide/flashcards-organization.md) |
| Choose a practice scope, rate answers, control schedules, and interpret results | [Practice and analytics](user-guide/practice-and-analytics.md) |
| Configure local/cloud models and per-profile reasoning/credentials | [AI providers](user-guide/ai-providers.md) |
| Import source material and generate learning artifacts | [Documents and AI generation](user-guide/documents-and-ai-generation.md) |
| Read, annotate, export, delete, and practice from generated guides | [Study guides](user-guide/study-guides.md) |
| Adjust themes, fonts, motion, help, and explicitly started tutorials | [Appearance and guidance](user-guide/appearance-and-guidance.md) |
| Share a deck, import generated JSON, back up, or restore | [Backups, imports, and sharing](user-guide/backups-imports-and-sharing.md) |
| Send supported study data directly between trusted devices | [Device synchronization](user-guide/device-sync.md) |

Flashcards has Decks and Search/Filter. Card templates live in Settings; transfer actions are contextual. Study Guides and Documents are separate destinations. A complete library backup and a device sync offer have different scopes. The guides explain these distinctions without exposing internal implementation details in ordinary instructions.

## Install, build, and test locally

[Installation overview](installation/README.md) distinguishes source-build support, regular qualification, and distribution. Platform guides cover [Linux](installation/linux.md), [Android](installation/android.md), [Windows](installation/windows.md), [macOS](installation/macos.md), and [mobile status](installation/mobile.md).

Local development uses ordinary plain-text shell output. The [command runbook](agents/COMMAND-RUNBOOK.md) lists the small `scripts/ostudy-dev` convenience dispatcher as well as the underlying setup/build/run/check scripts; those maintained wrappers remain the qualification contract.

For a development PR, use [Local testing and handoff](development/local-testing-and-handoff.md): exact-source checkout, native build/run, Android installation, test-data isolation, and manual acceptance. A frontend build, native hot reload, standalone application, package, and passing full gate are different results.

## Understand the architecture

The [comprehensive architecture](architecture/overview.md) is the current-system entry point: philosophy, ownership, data flow, persistence, study behavior, document/AI processing, platform services, security, recovery, testing, and tradeoffs. Its source map identifies the implementation owners.

Focused references provide deeper contracts:

| Area | Reference |
| --- | --- |
| Frontend state, interaction, navigation, styling, and accessibility | [UI architecture](architecture/ui.md) |
| Notes/presentations, rich content, templates, scheduling, and interchange | [Card format and data model](architecture/card-format-and-data-model.md) |
| Native transport, profile aggregates, probes, and credential isolation | [AI infrastructure](architecture/ai-infrastructure.md) |
| Model-owned reasoning and task defaults | [AI model profiles](architecture/ai-model-profiles.md) |
| Canonical sources, parsing/OCR, lifecycle, and learning artifacts | [AI and document processing](architecture/ai-and-document-processing.md) |
| Bounded task routing, prompts, concurrency, and checkpoint policy | [Document AI routing](architecture/document-ai-routing.md) |
| Editor adapter and validated structured content | [Rich document editor](architecture/rich-document-editor.md) |
| File, lifecycle, time, secret, and resource ports | [Platform services](architecture/platform-services.md) |
| One-way offers, merge semantics, and encrypted transport | [Device sync architecture](architecture/device-sync.md) |

[Architecture Decision Records](decisions/) preserve durable reasoning and explicit supersessions. An older ADR or parser evaluation is historical evidence when a later accepted decision replaces its implementation; it is not a second current backend choice.

## Develop and maintain

[CONTRIBUTING.md](../CONTRIBUTING.md) explains useful reports and reviewable changes. The [development guide](development/guide.md) maps the repository and day-to-day commands. [Git and PR workflow](development/workflow.md) defines source publication, evidence, review, merge, and rollback. [Testing and CI](development/testing-and-ci.md) defines focused/static/fast/full/platform gates and exact-head evidence.

Automated maintainers begin with [AGENTS.md](../AGENTS.md), [the agent index](agents/README.md), and the canonical [GitHub-direct procedure](agents/GITHUB-DIRECT-DEVELOPMENT.md). [Execution state](agents/EXECUTION-STATE.md), [failure triage](agents/FAILURE-TRIAGE.md), and [Connector diagnostics](agents/GITHUB-CONNECTOR.md) explain evidence and resumption without turning optional services into prerequisites.

[Documentation maintenance](agents/DOCUMENTATION-MAINTENANCE.md) defines canonical ownership and stale-code review. The [documentation/cleanup audit](development/documentation-and-cleanup-audit.md) records concrete findings, removals, intentional compatibility, and remaining engineering observations.

The planned presentation-layer migration is documented by the [Phase 0 UI overhaul affected-surface inventory](development/ui-overhaul-phase-0-inventory.md), the [shadcn/ui + Tailwind UI overhaul roadmap](development/ui-overhaul-roadmap.md), and [ADR 0051](decisions/0051-shadcn-tailwind-presentation-architecture.md). These are planning/decision records; the current UI architecture remains `architecture/ui.md` until implementation phases change the running application.

Focused development references include [AI testing](development/ai-infrastructure-testing.md), [Android architecture](development/android.md), [Android build/install](development/android-build-and-install.md), [local diagnostics](development/diagnostics.md), [dependency policy](development/dependency-audit.md), and the [portability inventory](development/platform-portability-inventory.md). Other files in `development/` are subsystem regression/acceptance procedures or clearly labeled implementation records.

## CI infrastructure and performance

The maintained runner topology is described in [Unified Linux/Android runner](development/unified-linux-android-runner.md) and [runner setup/operations](development/unified-runner-setup.md). [Legacy split-runner notes](development/android-self-hosted-runner.md) are migration context, not a request to redeploy that topology.

[UI performance evidence](development/ui-workflow-redesign-performance-baseline.md) and [visual baseline records](development/ui-workflow-redesign-visual-baseline.md) define retained test/provenance contracts. Baseline measurements are historical comparisons, not a claim that the old UI is the current specification. Wall-clock performance remains advisory/manual rather than a substitute for correctness qualification.

## External formats

The [generated JSON reference](reference/generated-json-import.md), [generator-agent instructions](reference/flashcard-import-agent.md), [version-1 JSON Schema](ostudy-flashcards-v1.schema.json), and [example](examples/cardiovascular.ostudy.json) define compatible content-only imports. They are not instructions to supply internal database IDs or learner history.

## Roadmap and historical material

The [roadmap](roadmap.md) separates implemented foundations from limitations and planned work. [History](history/README.md) preserves earlier audits/designs. Some stage-plan paths remain in `development/` because tests and provenance refer to them; their historical status and current replacement must be explicit.

A maintained document should answer its reader's question cohesively. Update the owning section rather than append a contradictory PR-specific note. Keep this index, current source links, and relative file links valid. The documentation verifier catches organization/link contracts; source review is still required to establish that prose matches the application.
