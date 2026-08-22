# ostudy documentation

This directory contains the maintained documentation for ostudy. It is organized by what a reader is trying to do rather than by the order in which features were implemented.

Start with the project [README](../README.md) for a concise introduction. The guides below provide the detail needed to install, use, understand, or contribute to the application.

## Use ostudy

- [Getting started](user-guide/getting-started.md) introduces the application, its main workspaces, and a practical first study session.
- [AI providers](user-guide/ai-providers.md) explains local, LAN, and cloud model connections without exposing provider-specific implementation details.
- [Documents and AI-generated flashcards](user-guide/documents-and-ai-generation.md) describes the cross-platform PDF workflow, local extraction/OCR boundary, provider privacy, retries, and current limitations.
- [Synchronize trusted devices](user-guide/device-sync.md) explains QR/LAN synchronization, conflict resolution, file fallback, settings, and optional credential transfer.
- [Backups, imports, and sharing](user-guide/backups-imports-and-sharing.md) explains personal backups, portable deck files, generated JSON imports, and system sharing.

## Install or build ostudy

- [Installation overview](installation/README.md) summarizes current platform availability and prerequisites.
- [Linux](installation/linux.md) is the currently qualified desktop development and release path.
- [Android](installation/android.md) explains first-time SDK/NDK setup, emulator and physical-device development, APK/AAB builds, smoke testing, and installation.
- [Windows](installation/windows.md) describes the source-build path and its current qualification status.
- [macOS](installation/macos.md) describes the source-build path and its current qualification status.
- [Mobile platform status](installation/mobile.md) summarizes implemented Android support and planned iOS work.

## Follow the project

- [Roadmap](roadmap.md) separates completed foundations, current priorities, and later product goals.
- [Contributing](../CONTRIBUTING.md) explains how to report a problem, propose a feature, prepare a change, and validate it.

## Understand the design

- [Architecture overview](architecture/overview.md) explains the frontend, native domain, persistence, and platform boundaries.
- [Platform services](architecture/platform-services.md) defines the typed operating-system ports, opaque file flow, secure-store boundary, lifecycle/time/resource observations, and enforcement rules.
- [AI and document processing](architecture/ai-and-document-processing.md) documents provider isolation and the PDF extraction, rendering, OCR, and generation pipeline.
- [Document AI architecture, routing, and concurrency](architecture/document-ai-routing.md) defines the format-neutral reasoning modules, provider-transport boundary, multimodal evidence, deterministic large-job orchestration, checkpoints, and routing policy.
- [Device synchronization](architecture/device-sync.md) documents the trusted-device identity, encrypted LAN transport, common-base merge, conflicts, and asset/credential boundaries.
- [Card format and data model](architecture/card-format-and-data-model.md) describes notes, scheduled presentations, rich content, templates, search, and interchange formats.
- [UI architecture](architecture/ui.md) describes shared interaction primitives, responsive layouts, accessibility, and platform adapters.
- [Architecture Decision Records](decisions/) preserve the reasoning behind durable cross-module decisions.

## Develop and maintain ostudy

- [Development guide](development/guide.md) covers setup, repository layout, change design, and day-to-day commands.
- [Git and pull-request workflow](development/workflow.md) defines direct-source publication, the PR acceptance ledger, exact-head review readiness, merge, release, and rollback.
- [Testing and CI](development/testing-and-ci.md) defines validation tiers, read-only automatic qualification, machine-readable exact-head evidence, failure fingerprints, Android gates, and GitHub incident handling.
- [Automated-maintainer index](agents/README.md) is the operational entry point for agents and LLM maintenance tools.
- [Agent execution state](agents/EXECUTION-STATE.md) defines the multidimensional task state, completion predicate, PR ledger, and resumption protocol.
- [Agent failure triage](agents/FAILURE-TRIAGE.md) defines classification, focused reproduction, retry budgets, and stopping rules.
- [GitHub Connector workflow](agents/GITHUB-CONNECTOR.md) separates repository visibility, concrete write authorization, publication, CI, and service incidents.
- [Android development architecture](development/android.md) documents the Kotlin/Rust boundary, generated project, emulator tests, package checks, and Android workflows.
- [Android build and install](development/android-build-and-install.md) gives the exact debug/release build, APK install, artifact, and qualification commands.
- [Unified Linux/Android runner](development/unified-linux-android-runner.md) documents the isolated single-VM CI architecture, security boundary, nested-KVM qualification, workflow model, recovery, and rollback.
- [Unified runner setup and operations](development/unified-runner-setup.md) gives the maintained ML-tower/libvirt provisioning, registration, service-management, snapshot, and retirement procedure.
- [Legacy split Android runner notes](development/android-self-hosted-runner.md) records the retired split-runner topology and migration context.
- [Platform portability inventory](development/platform-portability-inventory.md) records the current OS assumptions and the phase responsible for removing each one.
- [Dependency audit](development/dependency-audit.md) records the pinned dependency graph and native runtime trust boundaries.
- The remaining documents in [`development/`](development/) are focused acceptance and regression plans for high-risk subsystems.
- [`AGENTS.md`](../AGENTS.md) remains the repository-wide instruction entry point for automated maintainers.

## Reference material

- [Generated JSON import](reference/generated-json-import.md) defines the strict `.ostudy.json` authoring format.
- [Flashcard import agent guide](reference/flashcard-import-agent.md) provides instructions for tools that generate compatible import files.
- [Version 1 JSON Schema](ostudy-flashcards-v1.schema.json) and the [example import](examples/cardiovascular.ostudy.json) are machine-readable companions to the format guide.

## Historical records

Documents that explain an earlier implementation stage, completed roadmap, or dated audit live under [`history/`](history/). They are retained for project archaeology, not as current product instructions. When historical material conflicts with maintained documentation or source, the maintained documentation and current implementation take precedence.

- [GitHub outage and PR #54 workflow audit — August 17, 2026](history/github-outage-audit-2026-08-17.md) records the service incident, retained engineering fixes, removed outage-specific workaround, and permanent incident-response rules.

## Documentation maintenance

A maintained guide should answer a reader's question in cohesive prose, link to deeper material instead of repeating it, and clearly distinguish current behavior from planned work. Implementation-stage notes and validation snapshots should move to `history/` when they stop describing the active project. Relative links and the conventional root/docs organization are checked by the static gate.
