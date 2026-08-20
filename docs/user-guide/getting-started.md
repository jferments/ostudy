# Getting started

ostudy is a local-first study application for creating, organizing, practicing, importing, exporting, and analyzing flashcards. AI features are optional. You can use the ordinary study system without configuring a model provider.

This guide follows a normal first-use path rather than the internal implementation order.

## 1. Learn the main navigation

The main navigation contains five areas:

- **Flashcards** for decks, templates, Search, imports and exports, and deck-scoped card creation;
- **Documents** for retaining local source documents and generating flashcards from them;
- **Practice** for scheduled and custom study sessions;
- **Analytics** for current workload, practice history, and topic-level patterns;
- **Settings** for storage, appearance, AI providers, and review scheduling.

Each workspace begins with a compact icon and heading so the functional controls remain near the top of the screen. Use contextual help and this guide for explanations that do not need to remain permanently visible in the workspace.

On a narrow mobile window, Flashcards, Practice, and Analytics remain directly available while Documents and Settings appear under **More**. Intermediate tablet widths use a labeled medium navigation rail rather than an icon-only desktop/mobile hybrid. Android Back closes the top overlay or returns nested workspace state toward its root before leaving the current area.

Short contextual tutorials can introduce an unfamiliar workspace the first time its relevant controls are available. They are dismissible, can be restarted from related help, and can be disabled under **Settings → Appearance**. Tutorial progress is device-local presentation state; it is not learner history and is not included in library backups.

## 2. Create and organize decks

Open **Flashcards → Decks** and create a deck for one subject or study goal. A deck can contain Basic cards, reversible Basic cards, Fill in the blank cards, or cards produced from a custom template.

Folders provide one optional level of organization above decks. Folders appear first, followed by **Unsorted decks**. Create a folder from the **Folders** heading. Drag a folder handle to reorder folders. Drag a deck handle to reorder the deck or place it in a different folder; dropping a deck in **Unsorted decks** removes its folder assignment.

Dragging is not required. Focus a folder handle and use **Up**, **Down**, **Home**, or **End** to change its position. Focus a deck handle and use **Up**, **Down**, **Home**, or **End** within its current group, or **Left** and **Right** to move it between folder groups and Unsorted decks. These keyboard operations use the same revision-checked organization commands as pointer and touch dragging.

Use a folder's collapse control to hide its deck rows without changing organization. **Collapse all** closes every folder at once. The control tooltip identifies whether the next action will collapse or expand the folder.

Deleting a folder does not delete its decks. The decks return to Unsorted decks with their cards, schedules, and review history unchanged. Deleting a deck remains a separate, explicit action.

The deck library always uses the compact row presentation. It keeps deck name, card count, ready count, movement controls, and study actions visible while preserving full keyboard and touch targets; there is no separate density preference to manage.

A deck remains the main unit for practice scope, import/export, archiving, and analytics. Renaming a deck changes its current display name without discarding retained review history.

## 3. Add cards

Open a deck. At the top-right of its card table, choose **Add flashcard** for one card or **Bulk add** for several question-and-answer pairs. Card creation is always attached to its destination deck; there is no separate Create Cards workspace.

Select the card type that matches the material:

- **Basic** for a question and answer;
- **Reversible** when both directions are independently useful;
- **Fill in the blank** when a sentence or statement contains one or more useful recall targets;
- a **custom template** when you repeatedly study the same structured fields.

Use the single-card editor for focused authoring and bulk entry for a prepared set of question-and-answer pairs. Bulk entry opened from a deck keeps that deck fixed as the destination. Both routes use the same validation, duplicate analysis, managed-image boundary, and durable draft behavior.

Card previews inside a deck are compact on narrow screens: one **Q:** line and one **A:** line use the available horizontal width and then ellipsize. The preview does not reserve a separate arrow column between question and answer.

Write one main retrieval task per card. A precise question with a focused answer is usually easier to study than a card containing an entire paragraph of unrelated facts.

## 4. Use templates

Open **Flashcards → Templates** to browse reusable layouts or edit one. A template defines named fields and safe placement rules for the question and answer sides. Template layouts are data-only: they do not execute scripts or arbitrary HTML.

Use a template when many cards share the same structure, such as medication name, mechanism, adverse effects, and nursing considerations. Fields may still contain rich text, images, lists, and tables.

## 5. Search, filter, and manage cards

Open **Flashcards → Search**. The ordinary view keeps four frequent controls visible:

1. the search query;
2. deck or folder scope;
3. result sort order;
4. **All filters**.

Open **All filters** for due availability, scheduling status, last-studied date, individual decks, folders, and tags. Changes apply immediately. Active filters return above the results as removable chips so the current scope remains visible.

Each scheduled presentation appears as its own result, so a forward and reverse card can be selected independently even though note-level actions such as tagging or moving apply to their shared source.

**Select visible** affects only loaded rows. **Select all matching** resolves the complete filtered result set before a bulk action. Exact duplicate detection prevents accidental repetition while allowing an explicit duplicate when the same wording is genuinely needed.

## 6. Import, export, back up, and share

Open **Flashcards → Import/Export** for study-content transfers and complete-library protection.

A `.studydeck` file moves or shares one deck's study content and managed card images. It does not carry the original learner's schedule or review history.

A complete `.ostudybackup` preserves the entire SQLite library snapshot plus managed assets and document files. This includes deck folders and ordering. Provider credentials are deliberately excluded.

Platforms that provide a native share sheet can share a prepared deck export directly. Every file operation crosses the platform file boundary; feature screens do not exchange unrestricted filesystem paths.

See [Backups, imports, and sharing](backups-imports-and-sharing.md) for the exact distinctions and restore procedure.

## 7. Run a practice session

Open **Practice** when you want the normal scheduled workload. Choose the decks under **Select decks to study** first. The workload statistics below the deck selection then show the scheduled reviews, new cards, currently ready cards, and priority material for that scope. Scheduled practice applies your daily new-card and review limits and prioritizes cards according to the current schedule.

You can also press **Practice** directly on a deck. A deck-launched session is an explicit request to practice that deck, so it includes all currently studyable cards in the deck rather than silently truncating the session to the remaining daily new-card allowance. Archived, suspended, or currently buried cards remain excluded. If you end a session early, ostudy returns to the screen that launched it; a session started from a deck detail page returns to that same deck.

The active card is presented as the front of a restrained visual stack. The backing layers communicate that more cards remain without adding duplicate card content to the accessibility tree. A labeled progress indicator shows studied and remaining counts.

After revealing an answer, record how difficult recall was:

| Choice | Meaning |
| --- | --- |
| **Wrong (0)** | The answer was not recalled correctly. |
| **Hard (1)** | The answer was recalled with substantial difficulty or uncertainty. |
| **Medium (2)** | The answer was recalled correctly with normal effort. |
| **Easy (3)** | The answer was immediate and clearly known. |

ostudy maps these choices to the FSRS scheduling engine and records an immutable review event together with the new current schedule. Press **Space** or **Enter** to reveal an answer, then **0**, **1**, **2**, or **3** to submit the result when no editor or overlay has focus.

At completion, ostudy shows cards studied, average confidence, remembered-answer percentage, cards needing another look, and a labeled confidence distribution. A one-shot celebration may appear when celebrations and full motion are enabled. Reduced or disabled motion removes the particle effect while preserving the saved result and any available nonvisual feedback.

You can suspend cards that should leave normal circulation, bury them temporarily, reset a schedule, reschedule the next due time, and set daily limits. A **Needs more practice** classification identifies repeatedly missed material without changing the underlying review history.

## 8. Review your progress

Open **Analytics** to examine practice volume, remembered-answer rate, confidence, time spent, due workload, repeated misses, deck and tag performance, recent sessions, and individual reviews. Current inventory and historical review records are treated separately so renaming, moving, archiving, and deleting material have predictable effects.

Use analytics to choose a useful next action, not as a score to maximize. A high review count can mean productive practice or an overloaded schedule; retention, response time, and topic-level patterns provide the surrounding context. Topic-level **Practice** buttons remain in the final table column; the table scrolls horizontally on narrower desktop windows without clipping those actions.

## 9. Customize appearance and accessibility

Open **Settings → Appearance** to preview and save:

- Light, Dark, Console, or Synthwave theme;
- interface and default card fonts, including bundled IBM Plex Sans, Roboto, Ubuntu, and the other listed families;
- interface scale and default card text size;
- full, reduced, system-controlled, or disabled nonessential motion;
- automatic, rich, or simple opaque materials;
- decorative artwork, completion celebrations, and contextual tutorials.

Every theme implements the same semantic color and material contract. Changing a theme does not change information architecture, touch targets, accessibility semantics, or study behavior.

**Reduced motion** removes spatial route movement, card flips, parallax, reorder interpolation, artwork entrance motion, and particles. **Simple materials** use opaque surfaces and no grain. Forced colors and operating-system reduced-transparency preferences take precedence where supported.

Turning off decorative artwork hides mascot and empty-state illustrations only. Functional icons and images authored inside study cards or retained documents are unaffected.

## 10. Configure AI only when needed

Open **Settings → AI/LLM** to configure an AI provider profile. The settings page only exposes providers implemented by the current build; future provider work is tracked in the project roadmap rather than shown as inactive settings controls.

The current provider profile supplies the endpoint, model, and credential boundary used by AI tasks. **Default AI model** selects the default profile for flashcard generation. There is no separate second default-task-profile setting.

ostudy does not install or run a local model server. It connects to the endpoint you provide, tests the profile's capabilities, and uses only the provider selected for the task. There is no silent cloud fallback from a failed local provider.

See [AI providers](ai-providers.md) before entering a credential or exposing a LAN endpoint.

## 11. Generate flashcards from a PDF

Open **Documents** and import a PDF. The compact model tile directly below the Documents heading shows the current model selected for the Flashcard Generation task.

ostudy retains the source in its local document library, extracts usable native text, renders page images for visual evidence, and uses OCR only as a last-resort recovery path when native text extraction cannot provide usable text. It then sends bounded document context to the configured provider and creates an ordinary deck from the generated cards.

The generated deck uses the same editing, scheduling, practice, folder, archive, Search, export, and analytics tools as a manually authored deck. If the AI produces a card you do not want, edit or delete it through the normal deck/card tools.

See [Documents and AI generation](documents-and-ai-generation.md) for processing states, retries, cancellation, and provider requirements.

## A useful first test

A useful first test of the application is:

1. Create two small decks and place one inside a folder.
2. Reorder the folder and decks once with drag-and-drop and once with the keyboard handle shortcuts.
3. Open one deck and use **Add flashcard** and **Bulk add** to add five Basic cards, one Reversible note, and one Fill in the blank note.
4. Add two tags and find the cards through **Flashcards → Search** using a folder scope and one study-state filter.
5. Practice a deck and use more than one confidence response.
6. Inspect the completion summary and then open Analytics.
7. Switch themes, enable reduced motion, and verify that the same workflow remains clear without card flips or particles.
8. Export one deck, create a complete backup, restart ostudy, and confirm the folder, order, cards, and history remain available.
