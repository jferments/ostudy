# ostudy documentation

Start with the [project introduction](../README.md) for what ostudy is and why it exists. These guides are organized around what you want to do. Current behavior is described by the maintained guides and source, not by an older implementation-stage checklist.

## Use the application

| Task | Guide |
| --- | --- |
| Create a first deck and study session; find current navigation | [Getting started](user-guide/getting-started.md) |
| Browse, search, create, and edit cards in the Library Workbench, and manage selections | [Library Workbench and authoring](user-guide/flashcards-workspace.md) |
| Organize subjects, reorder decks, and understand archive/delete rules | [Subjects and deck organization](user-guide/flashcards-organization.md) |
| Enter and edit mathematical notation | [Formula entry](user-guide/formula-entry.md) |
| Choose a practice scope, rate answers, control schedules, and interpret results | [Practice, scheduling, and Progress](user-guide/practice-and-analytics.md) |
| Configure provider credentials and task-specific models/reasoning | [AI providers](user-guide/ai-providers.md) |
| Import source material and generate learning artifacts | [Documents and AI generation](user-guide/documents-and-ai-generation.md) |
| Write, highlight, draw and type notes on a PDF's pages | [Annotating PDF pages](user-guide/pdf-annotation.md) |
| Read, annotate, export, delete, and practice from generated guides | [Study guides](user-guide/study-guides.md) |
| Use optional source-grounded chat, projects and learner-controlled memory | [Tutor memory and privacy](user-guide/tutor-memory-and-privacy.md) |
| Adjust themes, fonts, motion, help, and explicitly started tutorials | [Appearance and guidance](user-guide/appearance-and-guidance.md) |
| Share a deck, import generated JSON, back up, or restore | [Backups, imports, and sharing](user-guide/backups-imports-and-sharing.md) |
| Send supported study data directly between trusted devices | [Device synchronization](user-guide/device-sync.md) |

Home opens first, with today's review count and your subjects. Library opens the Workbench, which lists subjects, decks, and tags beside the chosen cards, under tabs for its Flashcards, Documents and Study guides pages; Review starts today's review; Tutor and Progress are destinations of their own. Card templates live in Settings; transfer actions are contextual. Tutor can also open contextually from supported study workflows. A complete library backup and a device sync offer have different scopes. The guides explain these distinctions without exposing internal implementation details in ordinary instructions.

## Install, build, and test locally

[Installation overview](installation/README.md) distinguishes source-build support, regular qualification, and distribution. Platform guides cover [Linux](installation/linux.md), [Android](installation/android.md), [Windows](installation/windows.md), [macOS](installation/macos.md), and [mobile status](installation/mobile.md).

Local development uses ordinary plain-text shell output. The [command runbook](development/command-runbook.md) lists the small `scripts/ostudy-dev` convenience dispatcher as well as the underlying setup/build/run/check scripts; those maintained wrappers remain the qualification contract.

For a development PR, use [Local testing and handoff](development/local-testing-and-handoff.md): exact-source checkout, native build/run, Android installation, test-data isolation, and manual acceptance. A frontend build, native hot reload, standalone application, package, and passing full gate are different results.

## Understand the architecture

The [comprehensive architecture](architecture/overview.md) is the current-system entry point: philosophy, ownership, data flow, persistence, study behavior, document/AI processing, platform services, security, recovery, testing, and tradeoffs. Its source map identifies the implementation owners.

Focused references provide deeper contracts:

| Area | Reference |
| --- | --- |
| Frontend state, interaction, navigation, styling, and accessibility | [UI architecture](architecture/ui.md) |
| Notes/presentations, rich content, templates, scheduling, and interchange | [Card format and data model](architecture/card-format-and-data-model.md) |
| Native transport, profile aggregates, probes, and credential isolation | [AI infrastructure](architecture/ai-infrastructure.md) |
| Task model/reasoning assignments and provider profiles | [AI model profiles](architecture/ai-model-profiles.md) |
| Canonical sources, parsing/OCR, lifecycle, and learning artifacts | [AI and document processing](architecture/ai-and-document-processing.md) |
| Native evidence, fallible interpretations, partial analysis, and revision safety | [Knowledge representation](architecture/knowledge-representation.md) |
| Bounded task routing, prompts, concurrency, and checkpoint policy | [Document AI routing](architecture/document-ai-routing.md) |
| Editor adapter and validated structured content | [Rich document editor](architecture/rich-document-editor.md) |
| File, lifecycle, time, secret, and resource ports | [Platform services](architecture/platform-services.md) |
| One-way offers, merge semantics, and encrypted transport | [Device sync architecture](architecture/device-sync.md) |
| Complete-library activation, request generations and durable receipts | [Library restore ownership](architecture/library-restore.md) |
| Current writer versus supported legacy data contracts | [Durable-data inventory](development/post-104-data-contract-audit.md) |

The [Architecture Decision Records index](decisions/README.md) explains durable reasoning, explicit supersessions and historical numeric collisions. Cite full filenames rather than an ambiguous number. An older ADR or parser evaluation is historical evidence when a later accepted decision replaces its implementation; it is not a second current backend choice.

## Develop and maintain

[CONTRIBUTING.md](../CONTRIBUTING.md) explains useful reports and reviewable changes. The [development guide](development/guide.md) maps the repository and day-to-day commands. [Git and PR workflow](development/workflow.md) defines branches, pushes, evidence, review, merge, and rollback. [Testing and CI](development/testing-and-ci.md) defines focused/static/fast/full/platform gates and exact-head evidence.

Automated contributors begin with [AGENTS.md](../AGENTS.md) (Claude Code reads it through [CLAUDE.md](../CLAUDE.md)), then the [architecture guardrails](architecture/guardrails.md), [AI provider guardrails](architecture/ai-provider-guardrails.md), [code standards](development/code-standards.md), and the [command runbook](development/command-runbook.md). They follow the same workflow as human contributors.

[Documentation maintenance](development/documentation-maintenance.md) defines canonical ownership and stale-code review. The [documentation/cleanup audit](development/documentation-and-cleanup-audit.md) preserves earlier findings and removals. The [post-#106 hardening register](development/post-106-hardening.md) records the latest accepted, qualified, rejected and deferred review findings, their owning boundaries and proposed PR sequence. Planned remedies are not implementation claims.

PR #96 completed the shadcn/Base UI/Tailwind presentation migration described by the [Phase 0 affected-surface inventory](development/ui-overhaul-phase-0-inventory.md), [completed migration roadmap](development/ui-overhaul-roadmap.md), [ADR 0051](decisions/0051-shadcn-tailwind-presentation-architecture.md), and [PR #96 implementation record](development/ui-overhaul-current-implementation.md). The post-migration aesthetic direction is the [visual refresh roadmap](development/ui-visual-refresh-roadmap.md). [UI architecture](architecture/ui.md) owns the implemented dependency and styling rules rather than either roadmap.

Focused development references include the [generation contract refactor](development/generation-contract-refactor.md), [AI testing](development/ai-infrastructure-testing.md), [Android architecture](development/android.md), [Android build/install](development/android-build-and-install.md), [local diagnostics](development/diagnostics.md), [dependency policy](development/dependency-audit.md), the [portability inventory](development/platform-portability-inventory.md), and the [editor preview boundary](development/editor-preview.md). Other files in `development/` are subsystem regression/acceptance procedures or clearly labeled implementation records.

## CI infrastructure and performance

The maintained runner topology is described in [Unified Linux/Android runner](development/unified-linux-android-runner.md) and [runner setup/operations](development/unified-runner-setup.md). [Legacy split-runner notes](development/android-self-hosted-runner.md) are migration context, not a request to redeploy that topology.

[UI performance evidence](development/ui-workflow-redesign-performance-baseline.md) and the historical [Stage-0 visual baseline record](history/ui-workflow-redesign-visual-baseline.md) are retained provenance, not the current appearance specification. For an intentional redesign, use `./scripts/ui-visual-baselines.sh candidate` to capture exact-head review evidence into ignored `.artifacts/` output. [UI screenshots](development/ui-screenshots.md) captures before/after screens of a UI branch in every theme and width with a comparison page. The committed historical baseline remains read-only until a separately reviewed replacement baseline is deliberately accepted. Wall-clock performance remains advisory/manual rather than a substitute for correctness qualification.

## External formats

The [generated JSON reference](reference/generated-json-import.md) and [generator-agent instructions](reference/flashcard-import-agent.md) distinguish the legacy v1 Basic/Cloze reader from the current v2 mixed-card contract. The [version-1 JSON Schema](ostudy-flashcards-v1.schema.json) remains the compatibility reference; the [version-2 JSON Schema](reference/ostudy-flashcards-v2.schema.json) and [mixed-family example](examples/mixed-card-types.ostudy.json) cover Basic, Cloze, single-choice, select-all, true/false, matching, grouping, and ordering. Content imports do not supply internal database IDs or learner history.

## Roadmap and historical material

The [shared knowledge, collections, and lessons development plan](development/shared-knowledge-lessons/README.md) records the staged delivery program. PR #103 delivered the shared-knowledge/image foundation (LK-00–LK-04), and merged PR #104 delivered collections/ranges, collection synthesis, coherent study bundles, rich guide/PDF behavior, and Tutor organization (LK-05–LK-09). PR #105 is intentionally deferred as the preserved plan for future responsive lessons, retained narration, and PowerPoint work (LK-10–LK-12). Main now stores lessons and can write a subject's course with AI (schema 9), but no screen shows lessons yet; see the [roadmap](roadmap.md).

PR #106 is merged. Its [completion record](development/lk-followups/consolidation-qualification.md) identifies the release-v1 baseline, conversion and same-process restore owners, final source/merge identities, Linux/Android evidence and recorded disposable installed-copy acceptance. Its [original plan](history/pr-106-implementation-plan.md) is historical. Documentation cleanup does not authorize a protected-library cutover or make the proposed hardening changes current behavior.

[Interactive question types](development/interactive-question-types.md) is the implementation record for the seven question types delivered by merged PR #101, including shared authoring, deterministic evaluation, evidence-based generation, single-best-answer multiple choice, and the free/open-source non-AI offline requirement. Its stage history remains useful provenance; current source and the maintained user/architecture guides describe the available product.

The [roadmap](roadmap.md) separates implemented foundations from limitations and planned work. [History](history/README.md) preserves earlier audits/designs. Some stage-plan paths remain in `development/` because tests and provenance refer to them; their historical status and current replacement must be explicit.

A maintained document should answer its reader's question cohesively. Update the owning section rather than append a contradictory PR-specific note. Keep this index, current source links, and relative file links valid. The documentation verifier catches organization/link contracts; source review is still required to establish that prose matches the application.
