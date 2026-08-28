# Getting started

ostudy is a local-first study application for creating, organizing, practicing, importing, exporting, and analyzing flashcards. AI features are optional. You can use the ordinary study system without configuring a model provider.

This guide follows a normal first-use path rather than the internal implementation order.

## 1. Learn the main navigation

The main navigation contains five areas:

- **Flashcards** for decks, templates, Search, imports and exports, and card authoring;
- **Study Guides** for generated document outlines, learner notes, exports, and topic-linked study;
- **Documents** for retaining local source documents and generating flashcards from them;
- **Analytics** for current workload, practice history, and topic-level patterns;
- **Settings** for storage, appearance, AI providers, and review scheduling.

Inside **Flashcards**, the four workspaces are **Decks**, **Templates**, **Search**, and **Import/Export**. Open a deck to add or bulk-add cards to that deck. Card creation is intentionally deck-scoped rather than a separate global workspace.

On a narrow mobile window, Flashcards, Study Guides, and Analytics remain directly available while Documents and Settings appear under **More**. Practice opens from contextual study actions instead of appearing as a standing destination. Android Back closes the top overlay or nested editor before leaving the current area. During an active Practice session, Back asks before ending the session; cancel keeps the card and session in place, while confirmation saves the completed exit before returning to Flashcards.

Interactive tutorials never open automatically. New installations leave them off by default. If you want one, enable **Interactive tutorials** under **Settings → Appearance**, then start the related tutorial explicitly from Help. Tutorial completion/dismissal state is device-local presentation state; it is not learner history and is not included in library backups.

## 2. Create and organize decks

Open **Flashcards → Decks** and create a deck for one subject or study goal. A deck can contain Basic cards, reversible Basic cards, Fill in the blank cards, or cards produced from a custom template.

Folders provide one optional level of organization above decks. Folders appear first, followed by **Unsorted decks**. Create a folder from the **Folders** heading. Drag a folder handle to reorder folders. Drag a deck handle to reorder the deck or place it in a different folder; dropping a deck in **Unsorted decks** removes its folder assignment.

Dragging is not required. Focus a folder handle and use **Up**, **Down**, **Home**, or **End** to change its position. Focus a deck handle and use **Up**, **Down**, **Home**, or **End** within its current group, or **Left** and **Right** to move it between folder groups and Unsorted decks. These keyboard operations use the same revision-checked organization commands as pointer and touch dragging.

Use a folder's collapse control to hide its deck rows without changing organization. **Collapse all** closes every folder at once. The control tooltip identifies whether the next action will collapse or expand the folder.

Deleting a folder does not delete its decks. The decks return to Unsorted decks with their cards, schedules, and review history unchanged. Deleting a deck remains a separate, explicit action.

The deck library uses one compact row layout across supported widths. Responsive behavior changes how controls wrap and how navigation is presented; it does not expose a separate Comfortable/Compact density preference.

A deck remains the main unit for practice scope, import/export, archiving, and analytics. Renaming a deck changes its current display name without discarding retained review history.

## 3. Add cards

Open **Flashcards → Decks**, open the destination deck, and choose **Add flashcard**. Select the card type that matches the material:

- **Basic** for a question and answer;
- **Reversible** when both directions are independently useful;
- **Fill in the blank** when a sentence or statement contains one or more useful recall targets;
- a **custom template** when you repeatedly study the same structured fields.

Use **Bulk add** in the same deck when you have a prepared set of question-and-answer pairs. The single-card editor and bulk entry both use the same validation, duplicate analysis, managed-image boundary, and durable draft behavior. The destination deck is fixed by the deck you opened.

Write one main retrieval task per card. A precise question with a focused answer is usually easier to study than a card containing an entire paragraph of unrelated facts.

## 4. Use templates

Open **Flashcards → Templates** to browse reusable layouts or edit one. A template defines named fields and safe placement rules for the question and answer sides. Template layouts are data-only: they do not execute scripts or arbitrary HTML.

Use a template when many cards share the same structure, such as medication name, mechanism, adverse effects, and nursing considerations. Fields may still contain rich text, images, lists, and tables.

## 5. Search, filter, and manage cards

Open **Flashcards → Search**. The Search workspace and an individual deck use the same underlying card-listing framework, so card previews and selection behavior remain consistent. An individual deck shows a simple search field plus deck-authoring actions; the Search workspace shows the advanced search/filter form and does not show **Add flashcard** or **Bulk add**.

The ordinary Search controls include the query, deck/folder scope, result sort, and **All filters**. Open **All filters** for due availability, scheduling status, last-studied date, individual decks, folders, and tags. Changes apply immediately. Active filters return above the results as removable chips so the current scope remains visible.

Card rows present their primary content as separate **Q:** and **A:** lines. Each scheduled presentation appears as its own result, so a forward and reverse card can be selected independently even though note-level actions such as tagging or moving apply to their shared source.

**Select visible** affects only loaded rows. **Select all matching** resolves the complete filtered result set before a bulk action. **Practice selected** remains visible but disabled until at least one card is selected. Exact duplicate detection prevents accidental repetition while allowing an explicit duplicate when the same wording is genuinely needed.

## 6. Import, export, back up, and share

Open **Flashcards → Import/Export** for study-content transfers and complete-library protection.

A `.studydeck` file moves or shares one deck's study content and managed card images. It does not carry the original learner's schedule or review history.

A complete `.ostudybackup` preserves the entire SQLite library snapshot plus managed assets and document files. This includes deck folders and ordering. Provider credentials are deliberately excluded.

Platforms that provide a native share sheet can share a prepared deck export directly. Every file operation crosses the platform file boundary; feature screens do not exchange unrestricted filesystem paths.

See [Backups, imports, and sharing](backups-imports-and-sharing.md) for the exact distinctions and restore procedure.

## 7. Run a practice session

Practice is an internal study workflow rather than a standing navigation destination. Start it from an existing contextual action such as **Practice** on a deck, **Practice selected** in Search, a topic-level Practice action, or **Study linked cards** in Study Guides. When the retained Practice setup view is open, it restores your last manually chosen deck scope when those decks are still eligible; that setup choice is device-local presentation state rather than learner history.

While a session is active, the compact app bar identifies **Practice** but no
primary destination is selected. Deck detail, Search, Study Guides, Documents,
and Analytics launches retain their logical origin so ending early can return
to the right context.

If ostudy closes while a real session is resumable, reopening restores the
internal Practice workflow and its logical return destination. An older saved
Practice setup with no session opens Flashcards instead. This device-local
checkpoint stores only navigation/session identity hints; the native practice
session, submitted reviews, schedules, and review history remain authoritative.

The workload panel is authoritative for the current selection. While it is loading, unavailable, or has no selected scope, ostudy shows that state explicitly instead of displaying made-up zeroes. A real loaded workload of zero remains a real zero. The exact same deck/tag scope used to calculate the workload is passed to **Start scheduled practice**.

Change the deck checkboxes when you want a different scope. The workload statistics show the scheduled reviews, new cards, currently ready cards, and priority material for that scope. Scheduled practice applies your daily new-card and review limits and prioritizes cards according to the current schedule.

You can also press **Practice** directly on a deck. A deck-launched session is an explicit request to practice that deck, so it includes all currently studyable cards in the deck rather than silently truncating the session to the remaining daily new-card allowance. Archived, suspended, or currently buried cards remain excluded. Contextual deck/tag/card launches do not silently replace the remembered manual Practice setup scope for next time.

If you end a session early, ostudy returns to the screen that launched it; a session started from a deck detail page returns to that same deck.

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
- decorative artwork and completion celebrations;
- whether explicitly started interactive tutorials are available.

Every theme implements the same semantic color and material contract. Changing a theme does not change information architecture, touch targets, accessibility semantics, or study behavior.

**Reduced motion** removes spatial route movement, card flips, parallax, reorder interpolation, artwork entrance motion, and particles. **Simple materials** use opaque surfaces and no grain. Forced colors and operating-system reduced-transparency preferences take precedence where supported.

Turning off decorative artwork hides mascot and empty-state illustrations only. Functional icons and images authored inside study cards or retained documents are unaffected.

## 10. Configure AI only when needed

Open **Settings → AI/LLM** and choose **Add model**. The normal setup is deliberately short:

1. choose the provider;
2. enter the local **Server URL** or required cloud **API key**;
3. select **Connect and load models**;
4. choose the model returned by the provider;
5. select **Save profile**, then **Test connection**.

Provider-specific policy such as the inactivity timeout, credential persistence, optional llama.cpp server authentication, private-LAN HTTP acknowledgement, and OpenRouter zero-data-retention routing lives under **Advanced settings** rather than cluttering the basic path.

The settings page only offers providers implemented by the current build. **Default AI model** selects the default profile for flashcard generation. ostudy does not install or run a local model server and never silently falls back from a failed local provider to a cloud service.

See [AI providers](ai-providers.md) before entering an API key or exposing a LAN endpoint.

## 11. Generate flashcards from a PDF

Open **Documents** and import a PDF. The compact model tile directly below the Documents heading shows the current model selected for the Flashcard Generation task.

ostudy retains the source in its local document library, extracts usable native text, renders page images for visual evidence, and uses OCR only as a last-resort recovery path when native text extraction cannot provide usable text. It then sends bounded document context to the configured provider and creates an ordinary deck from the generated cards.

The generated deck uses the same editing, scheduling, practice, folder, archive, Search, export, and analytics tools as a manually authored deck. If the AI produces a card you do not want, edit or delete it through the normal deck/card tools.

See [Documents and AI generation](documents-and-ai-generation.md) for processing states, retries, cancellation, and provider requirements.

## A useful first test

A useful first test of the application is:

1. Create two small decks and place one inside a folder.
2. Reorder the folder and decks once with drag-and-drop and once with the keyboard handle shortcuts.
3. Open one deck; add five Basic cards with **Add flashcard**, then add prepared Q/A pairs with **Bulk add**.
4. Add two tags and find the cards through **Flashcards → Search** using a folder scope and one study-state filter.
5. Open one deck and choose **Practice** to start a contextual session.
6. Inspect the completion summary and then open Analytics.
7. Switch themes, enable reduced motion, and verify that the same workflow remains clear without card flips or particles.
8. If you want guided help, enable Interactive tutorials and start one explicitly from Help; verify it does not relaunch on its own.
9. Export one deck, create a complete backup, restart ostudy, and confirm the folder, order, cards, and history remain available.
