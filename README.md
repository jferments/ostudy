# ostudy

![ostudy](assets/images/github_banner.png)

**ostudy is a free, open-source, local-first study app.** Create flashcards, practice with spaced repetition, turn source documents into study material, and understand your progress without handing your library to a subscription platform.

Your cards, documents, schedules, and review history live on your device. Ordinary studying works offline and does not require an account or an AI provider. Optional AI authoring connects to a model server you control or a supported cloud API that you choose explicitly. There is no silent switch from local processing to a cloud model.

## Study your way

**Create and organize.** Write formatted Basic, Reverse, and Fill in the blank cards, use reusable templates, organize decks into folders, and search or filter your library. Add cards individually or in bulk, apply tags, and move or copy material between decks.

**Practice and remember.** Start a session from your decks or another learning context. Reveal an answer and rate it Wrong, Hard, Medium, or Easy. Spaced repetition, interval previews, daily limits, and scheduling controls help you decide what to study next. Analytics show your activity, review outcomes, workload, and areas that need attention.

**Learn from documents.** Keep source documents in a managed local library, generate flashcards and source-linked study guides, and return to retained material for further learning. Generated content remains editable study material, not an unquestionable answer: check it against the source, especially for consequential subjects.

**Keep control of your data.** Share individual decks without sharing personal review history, import compatible **Anki `.apkg` decks** and structured generated-card files, make whole-library backups, and explicitly synchronize study data between trusted devices. Sharing a deck, converting an Anki deck, restoring a backup, and synchronizing devices are different operations with different privacy and recovery implications.

**Choose a comfortable interface.** Flashcards brings together Decks and Search/Filter. Study Guides, Documents, and Analytics have their own destinations; card templates live in Settings. Themes, text sizing, optional guidance, and optional visual feedback let you adapt the interface without changing how your study data works.

## Optional AI, without a study-app subscription

Current provider adapters support **llama.cpp / Unsloth GGUF servers**, **DeepSeek**, and **OpenRouter**. A local server may run on this computer or another trusted computer on your network. ostudy connects to an existing server; it does not install or host the model itself.

When you use a paid API, billing is between you and that provider. Cost and capability depend on the chosen model and workload. Provider credentials are handled separately from ordinary library content and are excluded from shared decks and library backups. Credential transfer during trusted-device synchronization requires explicit sender consent.

Dedicated Ollama, OpenAI, Gemini, and Claude adapters remain planned. A model available through OpenRouter is not the same as a dedicated integration with its original provider. See [AI providers](docs/user-guide/ai-providers.md) for configuration and current limitations.

## Availability

ostudy is under active development. **Linux and Android are the maintained build-and-test paths.** Linux has desktop build/release tooling; Android has source-build, debug-install, and emulator-test tooling. These statements do not promise a public signed installer or app-store release for every platform.

Windows and macOS need regular platform qualification and release packaging. iOS remains planned. See the [installation overview](docs/installation/README.md), [Linux guide](docs/installation/linux.md), and [Android guide](docs/installation/android.md) before choosing a build path.

## Build and run on Ubuntu or Debian

From an authenticated checkout of this repository, run the first-time setup from the repository root:

```bash
./scripts/setup-ubuntu.sh
./run-app.sh
```

For an already configured checkout after source changes:

```bash
./scripts/build.sh
./run-app.sh
```

`run-app.sh` launches a current standalone build; it does not rebuild the app. Setup, optional document-processing dependencies, troubleshooting, and platform-specific installation details belong in the [installation guides](docs/installation/README.md). Contributor validation commands belong in [Testing and CI](docs/development/testing-and-ci.md).

## Learn more

Start with [Getting started](docs/user-guide/getting-started.md) or the [documentation index](docs/README.md). The documentation includes [flashcard organization](docs/user-guide/flashcards-organization.md), [practice and scheduling](docs/user-guide/practice-and-analytics.md), [documents and AI generation](docs/user-guide/documents-and-ai-generation.md), [Anki import](docs/reference/anki-import.md), [backups and sharing](docs/user-guide/backups-imports-and-sharing.md), and [trusted-device synchronization](docs/user-guide/device-sync.md).

For the project's design, read the [architecture overview](docs/architecture/overview.md). The [roadmap](docs/roadmap.md) separates implemented foundations from planned work, including additional study tools and platform support.

## Contribute

Bug reports, documentation improvements, tests, and code contributions are welcome. [CONTRIBUTING.md](CONTRIBUTING.md) explains how to report a useful issue and prepare a change. Automated maintainers start at [AGENTS.md](AGENTS.md); operational instructions are deliberately kept out of this introduction.

ostudy is licensed under the [MIT License](LICENSE).
