# Getting started

ostudy is organized around a local study library. A **deck** groups related material, a **note** contains the editable source, and one or more **cards** present that note for independent review. A normal Basic note produces one forward card; enabling Reverse adds a second schedule; a Fill in the blank note keeps one stable schedule while changing which marked phrase is hidden.

You do not need an account or an AI provider to create cards, practice them, search your library, or view analytics.

## 1. Create a deck

Open **Decks** and create a deck for one subject or course. A useful deck has a clear scope, such as “Cardiovascular pharmacology” or “Linear algebra: eigenvalues,” rather than mixing unrelated material solely because it was studied in the same week.

Decks may later be renamed, archived, exported, or deleted. Archiving removes a deck from normal study while preserving its material and historical analytics. Permanent deletion removes the deck and the historical data owned by material that is actually deleted.

## 2. Add a card

Open the deck and choose the card template that matches the material:

- **Basic** for a question and answer;
- **Basic with Reverse** when both directions are independently useful;
- **Fill in the blank** when the surrounding sentence provides meaningful context;
- a **reusable template** when the same set of fields should generate several cards.

The editor stores validated structured content rather than arbitrary HTML. You can use formatting, lists, links, and managed local images without allowing an imported deck or model response to execute code.

Write one main retrieval task per card. A precise question with a focused answer is usually easier to schedule and interpret than a card that asks for an entire chapter. Reverse cards work best for symmetric relationships such as terminology, abbreviations, labels, and simple associations; they are often unsuitable when the answer is a long explanation.

## 3. Organize and find material

Attach human-readable tags for concepts that cross deck boundaries. The **Search and filter cards** workspace at the bottom of **Decks** can query the whole library and narrow results by deck, tags, due state, or study history. Each scheduled presentation appears as its own result, so a forward and reverse card can be selected independently even though note-level actions such as tagging or moving apply to their shared source.

Bulk entry is useful for a set of simple question-and-answer pairs. Exact duplicate detection prevents accidental repetition while allowing an explicit duplicate when the same wording is genuinely needed.

## 4. Run a practice session

Open **Practice**, select the decks and session mode, and begin. After revealing an answer, record your confidence:

| Choice | Meaning |
| --- | --- |
| **Wrong** | The answer was not recalled correctly. |
| **Hard** | The answer was recalled with substantial difficulty or uncertainty. |
| **Medium** | The answer was recalled correctly with normal effort. |
| **Easy** | The answer was immediate and clearly known. |

ostudy maps these choices to the FSRS scheduling engine and records an immutable review event together with the new current schedule. FSRS computes the next interval after each result; the rating buttons stay intentionally compact rather than repeating interval estimates. Press **Space** or **Enter** to reveal an answer, then **0**, **1**, **2**, or **3** for Wrong, Hard, Medium, or Easy.

You can suspend cards that should leave normal circulation, bury them temporarily, reset a schedule, reschedule the next due time, and set daily limits. A **Needs more practice** classification identifies repeatedly missed material without changing the underlying review history.

## 5. Review your progress

Open **Analytics** to examine practice volume, remembered-answer rate, confidence, time spent, due workload, repeated misses, deck and tag performance, recent sessions, and individual reviews. Current inventory and historical evidence are treated separately so that renaming, moving, archiving, and deleting material have predictable effects.

Use analytics to choose a useful next action, not as a score to maximize. A high review count can mean productive practice or an overloaded schedule; retention, response time, and topic-level patterns provide the surrounding context.

## 6. Configure AI only when needed

Open **Settings → AI/LLM** to configure a provider profile. Current builds implement:

- **llama.cpp / Unsloth GGUF**, using an existing server that you run or manage;
- **DeepSeek**, using the official cloud API and your own credential.

Ollama, OpenAI, Gemini, and Claude appear as planned provider families but are not active adapters in the current build. ostudy does not install or run a local model server. It connects to the endpoint you provide, tests the profile's capabilities, and uses only the provider selected for the task.

See [AI providers](ai-providers.md) before entering a credential or exposing a LAN endpoint.

## 7. Generate flashcards from a PDF

Open **Documents**, import a PDF, choose a destination and provider, and start generation. The source is copied into the managed local library. ostudy extracts and organizes the document locally, renders pages when needed, uses OCR for selected pages whose text layer is unusable, sends bounded source context to the configured model, validates the returned cards, and writes the accepted deck transactionally.

This workflow is newer than the core flashcard system and still has known reliability and compatibility work. Read [Documents and AI-generated flashcards](documents-and-ai-generation.md) before relying on it for a large or unusual document.

## 8. Make a backup

A `.studydeck` file is for sharing one deck without personal review history. An `.ostudybackup` file is for restoring your complete local library and managed files. AI credentials are excluded from both.

Create a complete backup after meaningful authoring work and before testing a migration-heavy development branch. See [Backups, imports, and sharing](backups-imports-and-sharing.md) for the difference between each format and the restore rules.

## A practical first session

A useful first test of the application is:

1. Create a small deck with five Basic cards, one Reverse note, and one Fill in the blank note.
2. Add two tags and find the cards through the Search and filter cards workspace in Decks.
3. Practice the deck and use more than one confidence response.
4. Open Analytics and inspect the resulting session and review details.
5. Export the deck, create a full backup, restart ostudy, and confirm the material remains available.
6. Configure an AI provider and import a short, non-sensitive PDF only after the non-AI workflow is understood.

This sequence tests the durable study core before adding the extra variables of provider configuration and document processing.
