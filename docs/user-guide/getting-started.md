# Getting started

ostudy keeps your study library on your device. You can create cards, organize decks, practice, search, and inspect analytics without an account or AI provider. Document generation is optional and uses only the provider you configure.

## Find your way around

The main destinations are **Flashcards**, **Study Guides**, **Documents**, **Analytics**, and **Settings**. On a wide or medium window they appear in the labeled sidebar. On a compact phone layout, Flashcards, Study Guides, Documents, and Analytics are directly available in the bottom navigation; the upper-right gear opens Settings. There is no separate mobile More menu.

Flashcards has two tabs: **Decks** and **Search/Filter** (shown as Search in compact presentation). Card templates are in **Settings → Card templates**. Deck import/export is contextual; complete-library backup and device sync are in **Settings → Backup/Sync**.

Selecting Flashcards opens the Decks library root. Selecting Decks while a deck is open returns to the library, subject to unsaved-change protection. Selecting Study Guides returns to the guide library rather than keeping an already-open viewer in the way. Practice opens from a study action, not a standing navigation tab.

## Create your first deck

Open **Flashcards → Decks**. A new library shows “No decks have been created yet” and **Create a new deck**. The import icon above it is another way to begin. Empty folder and unsorted tiles, selection controls, and collapse controls are intentionally absent when there are no decks.

Give the deck a name, open it, and choose **Add flashcard**. Use a Basic question and answer for a first card. Reverse presentations are useful when both directions are worth recalling. Fill in the blank cards hide marked parts of a statement. Choose a reusable template when many cards have the same structure.

**Bulk add** accepts multiple prepared question/answer pairs in the current deck. The destination is fixed by the deck you opened. Ordinary authoring supports duplicate checks, tags, managed images, and recoverable drafts; a draft is not a committed card until saved successfully.

Keep one focused retrieval task per card. Generated or imported wording should receive the same review as a card you wrote yourself. See [Flashcards and authoring](flashcards-workspace.md).

## Organize without changing your study history

Once active decks exist, the **+** beside Folders creates an optional folder. Unsorted decks appears only when some active decks have no folder. Move decks by dragging a non-interactive part of a row, using keyboard reorder controls, or using the selected-deck Move action.

Collapse/expand changes presentation, not membership. Deleting a folder returns its decks to Unsorted decks; deleting a deck is a different action with different consequences. Archiving retains the deck but removes it from normal study availability until restored. See [Folders, ordering, and selection](flashcards-organization.md).

## Practice a small session

Use **Practice** on an active deck with studyable cards, or select decks and open the practice setup. Deck, tag, and exact-card study actions can have different scopes; the setup/workload display shows what will be studied. Scheduled practice applies daily limits, whereas an explicit deck practice action follows its all-studyable-card scope. Suspended, buried, or archived material can make a card unavailable.

Think of the answer before revealing it. Choose **Wrong**, **Hard**, **Medium**, or **Easy** based on recall difficulty. On a keyboard, Space or Enter reveals the answer, and 0–3 selects the four outcomes when no editor or overlay has focus. Reviews update your schedule and history; they are not merely a visual score.

The progress track shows studied, remaining, and percentage. Ending early asks for confirmation and returns to the logical place that launched the session. A real resumable session can be recovered after reopening; an old saved setup with no session does not pretend that a review is in progress. See [Practice and analytics](practice-and-analytics.md).

## Find and manage cards

**Search/Filter** finds material across your library. Use the query, scope, sort, and **All filters** controls. Active filters appear as removable chips. A deck detail has a simpler deck-scoped search plus authoring actions; global Search is not a second card-creation screen.

**Select visible** affects loaded rows. **Select all matching** resolves the complete filtered scope. Forward and reverse presentations can appear separately, while actions that edit their shared note can affect both. Read the confirmation before bulk deletion, moving, or scheduling changes.

## Protect your library

Use the library's **Import deck from file** icon for `.studydeck` or supported generated-card JSON. Export an individual deck or selected decks through contextual export controls. A shared deck has fresh schedules when imported and does not include the sender's personal review history.

Use **Settings → Backup/Sync** for a complete `.ostudybackup` and restoration. A backup contains personal library history and managed files but excludes credentials. Restore replaces the library through a staged restart process; it is not an append operation. Keep backups outside the app's data directory and before testing risky changes. [Backups, imports, and sharing](backups-imports-and-sharing.md) explains the formats and [device sync](device-sync.md) explains explicit trusted-device exchange.

## Add AI only when it is useful

In **Settings → AI/LLM**, choose Add model, select an implemented provider, enter its server URL or required API key, connect/load models, choose a model, and save/test the profile. Advanced settings contain less common network, credential, timeout, and provider-routing policies. A default model selection does not install a local server or guarantee that the server/model supports every document task.

Current adapters are llama.cpp / Unsloth GGUF, DeepSeek, and OpenRouter. Other catalog entries or future integrations are not current adapters. A local provider never silently falls back to cloud. See [AI providers](ai-providers.md).

In **Documents**, upload a supported source and use its flashcard or study-guide generation actions. Parsing happens locally; AI requests send bounded source context and, when supported, selected visual evidence to the configured provider. Review generated material against the source. [Documents and AI generation](documents-and-ai-generation.md) covers formats, progress, cancellation, and privacy.

Open **Study Guides** to browse generated guides, add learner notes, review source links, export a guide, or practice linked cards. Guides and cards are separate learning artifacts: one may exist without the other. See [Study guides](study-guides.md).

## Adjust the interface

**Settings → Appearance** offers Light, Dark, Console, and Synthwave themes, separate interface/card typography, scale, motion, materials, decorative artwork, celebrations, and optional tutorials. Preview changes, then use **Save preferences** when shown. Provider profiles and template edits have their own save paths.

Tutorials default off and never start automatically. Enable them and start one explicitly from related Help when needed. Ordinary help does not require enabling tutorials. Reduced motion and disabled decoration preserve controls and educational content. See [Appearance and guidance](appearance-and-guidance.md).

## A useful first check

Create a small deck, add a few cards, find them through Search, practice them, and inspect Analytics. Restart and confirm the content and reviews remain. Then make a complete backup. For development builds, use an isolated library rather than experimenting on valuable data; the [local testing guide](../development/local-testing-and-handoff.md) shows how.
