# ostudy

![ostudy v0.10](assets/images/github_banner.png)

**ostudy** is a free, open-source study application that combines flexible flashcards, spaced repetition, learning analytics, and optional AI-assisted authoring. Your library lives on your own computer, core study features do not require an account, and AI is used only through a provider that you explicitly configure.

The project is being built for learners who want the convenience of modern AI study tools without giving a single subscription service permanent control over their notes, review history, or choice of model. You can connect ostudy to an existing local model server or to a supported cloud API. When a paid API is used, you pay that provider directly rather than buying a separate study-app subscription; the actual cost depends on the model, document size, and usage.

ostudy is under active development. Linux is the currently qualified desktop path. Windows and macOS source-build support is being prepared for regular qualification, and Android and iOS are planned. See [Platform availability](#platform-availability) and the [roadmap](docs/roadmap.md) for the current status rather than assuming that every planned platform or AI feature has already shipped.

## Why ostudy

- **Free and open source.** The application and its direct dependencies can be inspected, modified, and built without a proprietary study-platform subscription.
- **Local first.** Decks, cards, documents, schedules, review history, settings, and analytics remain in your local library.
- **Choice of AI.** Current builds support existing llama.cpp/Unsloth GGUF servers and DeepSeek. Ollama, OpenAI, Gemini, and Claude integrations are represented in the provider catalog but remain planned until their native adapters are implemented.
- **No forced cloud fallback.** A local provider is never silently replaced with a cloud provider. ostudy sends requests only to the profile selected for the task.
- **Portable data.** Share individual decks, import generated cards, and make complete local backups without depending on an online account.
- **Built for serious review.** Spaced repetition, scheduling controls, search, tags, and detailed analytics are part of the study system rather than optional add-ons.
- **A cross-platform direction.** The architecture is designed for Linux, Windows, macOS, Android, and iOS while keeping platform-specific capabilities behind explicit adapters.

## Features

### Create and organize study material

- Create rich-text **Basic**, **Reverse**, and **Fill in the blank** flashcards.
- Build reusable card templates for subjects that follow a consistent structure.
- Organize notes with decks and tags, search the full library, and filter by schedule or study history.
- Add cards individually or through bulk entry, detect exact duplicates, and move or copy cards between decks.
- Import portable `.studydeck` files or strict `.ostudy.json` files produced by another authoring tool.

### Learn with spaced repetition

- Practice one deck or several decks through an FSRS-based spaced-repetition schedule.
- Record each answer as **Wrong**, **Hard**, **Medium**, or **Easy** and preview the resulting intervals.
- Suspend, bury, reset, or manually reschedule cards; set daily new-card and review limits; and identify cards that need more practice.
- Review progress through library, deck, tag, session, confidence, response-time, workload, and retention analytics.

### Generate flashcards with AI

- Keep source PDFs in a managed document library and generate a new deck from a selected document.
- Use an explicitly configured local, LAN, or supported cloud provider profile.
- Perform PDF extraction, page rendering, OCR recovery, document organization, validation, and final card persistence in the native application.
- Retry or stop long-running document jobs without giving model output direct authority over the database.

PDF processing is one of the newest parts of ostudy and is still being hardened. The [document-generation guide](docs/user-guide/documents-and-ai-generation.md) explains the current workflow and limitations; the detailed extraction, rendering, OCR, and model architecture belongs in the [AI and document-processing design](docs/architecture/ai-and-document-processing.md), not in this overview.

### Own, back up, and share your library

- Export a deck as a versioned `.studydeck` package for another ostudy user.
- Create a complete `.ostudybackup` of the local library and managed files.
- Keep AI credentials outside the SQLite library and outside shared deck or backup files.
- Use validated structured card content rather than executable HTML supplied by an import or model.

## Planned study tools

The document platform is intended to support more than flashcard generation. Planned work includes source-grounded study guides, practice tests and quizzes, interactive AI tutoring, user-controlled synchronization between devices, regularly qualified Windows and macOS releases, and native Android and iOS applications. These are roadmap items, not current feature claims. See the [project roadmap](docs/roadmap.md) for priorities and status.

## Platform availability

| Platform | Current status |
| --- | --- |
| Ubuntu/Debian Linux | Primary qualified development path; Linux release automation is present. |
| Other Linux distributions | May build from source, but package and native-dependency setup varies by distribution. |
| Windows | Source-build support is in progress and is not yet continuously qualified. |
| macOS | Source-build support is in progress and is not yet continuously qualified. |
| Android | Planned; no installable mobile application is currently provided. |
| iOS | Planned; no installable mobile application is currently provided. |

Detailed and platform-specific instructions live in the [installation guide](docs/installation/README.md).

## Build and run on Ubuntu or Debian

The repository currently provides a complete setup path for Ubuntu and Debian development systems. From the repository root:

```bash
./scripts/setup-ubuntu.sh
./run-app.sh
```

The setup script installs the required system packages, pinned development tools, JavaScript and Rust dependencies, and native PDF/OCR requirements, then creates an initial standalone debug build. `run-app.sh` starts that existing build without rebuilding it.

After changing source code:

```bash
./scripts/build.sh
./run-app.sh
```

Before proposing a change:

```bash
./scripts/check-changed.sh  # quick iteration aid
./scripts/check-static.sh   # repository, type, link, and format contracts
./scripts/check-fast.sh     # frontend tests and Rust compile checking
./scripts/check.sh          # complete merge qualification
```

See [Linux installation](docs/installation/linux.md) for prerequisites and troubleshooting, and [Testing and CI](docs/development/testing-and-ci.md) for the purpose of each validation tier.

## Documentation

The [documentation index](docs/README.md) is the starting point for all detailed material. Major sections include:

- [Getting started](docs/user-guide/getting-started.md)
- [Backups, imports, and sharing](docs/user-guide/backups-imports-and-sharing.md)
- [AI providers](docs/user-guide/ai-providers.md)
- [Roadmap](docs/roadmap.md)
- [Installation](docs/installation/README.md)
- [Architecture](docs/architecture/overview.md)
- [Development guide](docs/development/guide.md)
- [Contributing and issue reports](CONTRIBUTING.md)

## Contributing

Bug reports, feature proposals, documentation improvements, tests, and code contributions are welcome. Read [CONTRIBUTING.md](CONTRIBUTING.md) before opening a change. It explains how to submit a useful issue, how the repository is organized, and which checks protect the current data and architecture contracts.

ostudy is licensed under the [MIT License](LICENSE).
