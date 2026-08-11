# Getting started

ostudy is a local-first study application for creating, organizing, practicing, importing, exporting, and analyzing flashcards. AI features are optional. You can use the ordinary study system without configuring a model provider.

This guide follows a normal first-use path rather than the internal implementation order.

## 1. Create your first deck

Open **Decks** and create a deck for one subject or study goal. A deck can contain Basic cards, reversible Basic cards, Fill in the blank cards, or cards produced from a custom template.

A deck remains the main unit for organization, practice scope, import/export, archiving, and analytics. Renaming a deck changes its current display name without discarding its retained review history.

## 2. Add cards

Open a deck and choose **Add card**. Select the card type that matches the material:

- **Basic** for a question and answer;
- **Reversible** when both directions are independently useful;
- **Fill in the blank** when a sentence or statement contains one or more useful recall targets;
- a **custom template** when you repeatedly study the same structured fields.

Write one main retrieval task per card. A precise question with a focused answer is usually easier to study than a card containing an entire paragraph of unrelated facts.

## 3. Organize and find material

Attach human-readable tags for concepts that cross deck boundaries. The **Search and filter cards** workspace at the bottom of **Decks** can query the whole library and narrow results by deck, tags, due state, or study history. Each scheduled presentation appears as its own result, so a forward and reverse card can be selected independently even though note-level actions such as tagging or moving apply to their shared source.

Bulk entry is useful for a set of simple question-and-answer pairs. Exact duplicate detection prevents accidental repetition while allowing an explicit duplicate when the same wording is genuinely needed.

## 4. Run a practice session

Open **Practice** when you want the normal scheduled workload. Scheduled practice applies your daily new-card and review limits and prioritizes cards according to the current schedule.

You can also press **Practice** directly on a deck. A deck-launched session is an explicit request to practice that deck, so it includes all currently studyable cards in the deck rather than silently truncating the session to the remaining daily new-card allowance. Archived, suspended, or currently buried cards remain excluded. If you end a session early, ostudy returns to the screen that launched it; a session started from a deck detail page returns to that same deck.

After revealing an answer, record your confidence:

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

Open **Settings → AI/LLM** to configure an AI provider profile. The settings page only exposes providers implemented by the current build; future provider work is tracked in the project roadmap rather than shown as inactive settings controls.

The current provider profile supplies the endpoint, model, and credential boundary used by AI tasks. **Default AI model** selects the default profile for flashcard generation. There is no separate second “default task profile” setting.

ostudy does not install or run a local model server. It connects to the endpoint you provide, tests the profile's capabilities, and uses only the provider selected for the task. There is no silent cloud fallback from a failed local provider.

See [AI providers](ai-providers.md) before entering a credential or exposing a LAN endpoint.

## 7. Generate flashcards from a PDF

Open **Documents** and import a PDF. ostudy retains the source in its local document library, extracts usable native text, renders page images for visual evidence, and uses OCR only as a last-resort recovery path when native text extraction cannot provide usable text. It then sends bounded document context to the AI provider you configured and creates an ordinary deck from the generated cards.

The generated deck uses the same editing, scheduling, practice, archive, search, and analytics tools as a manually authored deck. If the AI produces a card you do not want, edit or delete it through the normal deck/card tools.

## 8. Back up your library

A **complete backup** preserves your library database plus managed assets and document files. Provider credentials are excluded. Use a complete backup before risky migrations, large imports, or destructive cleanup.

A `.studydeck` export is different: it is for moving or sharing one deck's study content and managed card images. It does not carry the original learner's schedule or review history.

See [Backups, imports, and sharing](backups-imports-and-sharing.md) for the exact distinction.

## A useful first test

A useful first test of the application is:

1. Create a small deck with five Basic cards, one Reverse note, and one Fill in the blank note.
2. Add two tags and find the cards through the Search and filter cards workspace in Decks.
3. Practice the deck and use more than one confidence response.
4. Open Analytics and inspect the resulting session and review details.
5. Export the deck, create a full backup, restart ostudy, and confirm the material remains available.
