# ostudy documentation

This directory contains the maintained documentation for ostudy. It is organized by what a reader is trying to do rather than by the order in which features were implemented.

Start with the project [README](../README.md) for a concise introduction. The guides below provide the detail needed to install, use, understand, or contribute to the application.

## Use ostudy

- [Getting started](user-guide/getting-started.md) introduces the application, its main workspaces, and a practical first study session.
- [AI providers](user-guide/ai-providers.md) explains local, LAN, and cloud model connections without exposing provider-specific implementation details.
- [Documents and AI-generated flashcards](user-guide/documents-and-ai-generation.md) describes the desktop PDF workflow, privacy boundary, retries, and current limitations.
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
- [Card format and data model](architecture/card-format-and-data-model.md) describes notes, scheduled presentations, rich content, templates, search, and interchange formats.
- [UI architecture](architecture/ui.md) describes shared interaction primitives, responsive layouts, accessibility, and platform adapters.
- [Architecture Decision Records](decisions/) preserve the reasoning behind durable cross-module decisions.

## Develop and maintain ostudy

- [Development guide](development/guide.md) covers setup, repository layout, change design, and day-to-day commands.
- [Android development architecture](development/android.md) documents the Kotlin/Rust boundary, generated project, emulator tests, package checks, and Android workflows.
- [Platform portability inventory](development/platform-portability-inventory.md) records the current OS assumptions and the phase responsible for removing each one.
- [Git and pull-request workflow](development/workflow.md) defines the publication, review, merge, release, and rollback process.
- [Testing and CI](development/testing-and-ci.md) is the authoritative description of the changed, static, fast, full, and Android validation tiers.
- [Dependency audit](development/dependency-audit.md) records the pinned dependency graph and native runtime trust boundaries.
- The remaining documents in [`development/`](development/) are focused acceptance and regression plans for high-risk subsystems.
- [`agents/`](agents/) contains additional guardrails for automated maintainers; [`AGENTS.md`](../AGENTS.md) remains their repository-wide entry point.

## Reference material

- [Generated JSON import](reference/generated-json-import.md) defines the strict `.ostudy.json` authoring format.
- [Flashcard import agent guide](reference/flashcard-import-agent.md) provides instructions for tools that generate compatible import files.
- [Version 1 JSON Schema](ostudy-flashcards-v1.schema.json) and the [example import](examples/cardiovascular.ostudy.json) are machine-readable companions to the format guide.

## Historical records

Documents that explain an earlier implementation stage, completed roadmap, or dated audit live under [`history/`](history/). They are retained for project archaeology, not as current product instructions. When historical material conflicts with maintained documentation or the source code, the maintained documentation and current implementation take precedence.

## Documentation maintenance

A maintained guide should answer a reader's question in cohesive prose, link to deeper material instead of repeating it, and clearly distinguish current behavior from planned work. Implementation-stage notes and validation snapshots should be moved to `history/` when they stop describing the active project. Relative links and the small set of conventional root files are checked by the repository's static validation gate.
