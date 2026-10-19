# Getting started

ostudy keeps your study library on your device. You can create cards, organize decks, practice, search, and follow your progress without an account or AI provider. Document generation is optional and uses only the provider you configure.

## Find your way around

The main navigation has five destinations: **Home**, **Library**, **Review**, **Tutor**, and **Progress**. On a wide or medium window they sit in a rail down the left side, with **Search** and **Settings** at its bottom. On a phone they are in the bottom bar, and the top bar keeps Search and Settings. There is no separate mobile More menu.

**Home** opens first. It shows how many cards are due today with **Start review**, then your subjects: each subject's tile shows what is due, its sources and cards, and opens the subject's page. A subject's page has its own **Review** for what is due in it, **Manage**, which opens Library with only that subject, the overview guide across its sources, its sources and how far each has been prepared, and its decks. **Library** opens the **Workbench**, where you browse, search, edit, and organize cards; **Documents** and **Study guides** are pages linked from it. **Review** lists what is due today by subject, with a **Start** for each and **Choose decks** for a custom practice. **Progress** shows your study activity, review outcomes, and workload.

The Workbench shows a tree of views, subjects, decks, and tags on the left, the chosen cards in the middle, and the open card's editor on the right; on a phone you see one of these at a time. The header's **Search** opens the Workbench on **All cards** with the search field focused. Card templates are in **Settings → Card templates**. Deck import/export is contextual; complete-library backup and device sync are in **Settings → Backup/Sync**.

Choosing **Library** opens the page you used last. On the Documents and Study guides pages, the switcher's **Workbench** returns to the Workbench, and choosing Study guides returns to the guide library rather than keeping an already-open viewer in the way. Practice opens from a study action and runs inside Review; it is not a navigation tab of its own. On Android, Back returns to Home from any other destination; in the Workbench it first steps back to the tree on a phone, or to **All cards** on a wider screen.

## Create your first deck

A new library opens on Home's welcome, **Your library is empty**, with **Create a subject**, **Import a document**, and **Open Library**. A subject keeps one course's documents, study guides, and decks together; you can also start with a deck and file it later.

Open **Library**. On a wide or medium window, a library with no decks shows **No cards yet** with **New deck** and **Import deck**. To start a deck inside a subject, choose the subject and then **New deck**, or use **New deck here** in the subject's actions menu. On a phone, the tree offers **New deck** while you have no subjects.

Give the deck a name. The Workbench opens it; choose **New card**. Use a Basic question and answer for a first card. Reverse presentations are useful when both directions are worth recalling. Fill in the blank cards hide marked parts of a statement. Choose a reusable template when many cards have the same structure.

**Bulk add…** in the deck's actions menu accepts multiple prepared question/answer pairs in the current deck. The destination is fixed by the deck you opened. Ordinary authoring supports duplicate checks, tags, managed images, and recoverable drafts; a draft is not a committed card until saved successfully.

Keep one focused retrieval task per card. Generated or imported wording should receive the same review as a card you wrote yourself. See [Library Workbench and authoring](flashcards-workspace.md).

## Organize without changing your study history

The **+** beside Subjects creates a subject. **No subject** appears only when some decks or documents are not filed under one. Use a deck's actions menu for **Move to subject…**, **Move up**, and **Move down**.

Collapse/expand changes presentation, not membership. A deck's actions menu also has **Merge into…**, **Export…**, **Archive**, and **Delete deck**. **Merge into…** moves the source deck's cards into another active deck while preserving card/note identities, schedules, review history, tags, media, and source provenance. The emptied source deck is archived after the transaction, and existing destination cards are not deduplicated simply because their wording looks similar. Deleting a subject moves its decks and documents to No subject; deleting a deck is a different action with different consequences. Archiving retains the deck but removes it from normal study availability until restored. See [Subjects and deck organization](flashcards-organization.md).

## Practice a small session

Use **Practice** on an active deck with studyable cards, or choose **Practice with options…** in its actions menu (or **Practice…** on a subject) to open the practice setup. Deck, tag, and exact-card study actions can have different scopes; the setup/workload display shows what will be studied. Scheduled practice applies daily limits, whereas an explicit deck practice action follows its all-studyable-card scope. Suspended, buried, or archived material can make a card unavailable.

Think of the answer before revealing it. Choose **Wrong**, **Hard**, **Medium**, or **Easy** based on recall difficulty. On a keyboard, Space or Enter reveals the answer, and 0–3 selects the four outcomes when no editor or overlay has focus. Reviews update your schedule and history; they are not merely a visual score.

The progress track shows studied, remaining, and percentage. Ending early asks for confirmation and returns to the logical place that launched the session. A real resumable session can be recovered after reopening; an old saved setup with no session does not pretend that a review is in progress. See [Practice and analytics](practice-and-analytics.md).

## Find and manage cards

The Workbench's search box finds cards within the chosen view, subject, deck, or tag; choose **All cards** to search the whole library. Use **Sort** and, inside a subject, deck, or tag, the **All**, **Due**, **New**, and **Suspended** filters to narrow the list.

**Select shown** affects loaded rows. **Select all N** selects every matching card. Forward and reverse presentations can appear separately, while actions that edit their shared note can affect both. Read the confirmation before bulk deletion, moving, or scheduling changes.

## Protect your library

Use **Import deck…** under More in the Workbench's tree for `.studydeck` or supported generated-card JSON. Export a deck with **Export…** in its actions menu. A shared deck has fresh schedules when imported and does not include the sender's personal review history.

Use **Settings → Backup/Sync** for a complete `.ostudybackup` and restoration. A backup contains personal library history and managed files but excludes credentials. Restore validates and replaces the library in the running application, then refreshes the interface; it does not require restarting oStudy and is not an append operation. Keep backups outside the app's data directory and before testing risky changes. [Backups, imports, and sharing](backups-imports-and-sharing.md) explains the formats and [device sync](device-sync.md) explains explicit trusted-device exchange.

## Add AI only when it is useful

In **Settings → AI/LLM**, choose Add model, select an implemented provider, enter its server URL or required API key, connect/load models, choose a model, and save/test the profile. Advanced settings contain less common network, credential, timeout, and provider-routing policies. A default model selection does not install a local server or guarantee that the server/model supports every document task.

Current adapters are llama.cpp / Unsloth GGUF, DeepSeek, and OpenRouter. Other catalog entries or future integrations are not current adapters. A local provider never silently falls back to cloud. See [AI providers](ai-providers.md).

In **Library → Documents**, **Upload document** starts flashcard generation from a supported source. Expand the optional flashcard-instructions field first when you want a specific focus or style; the prompt guides flashcard authoring but is never treated as factual source evidence. For screenshots or photos, choose **Upload images**, select several files, and let ostudy create the flashcards and study guide automatically. Add more through **+**, drag files into the area below the file list, press **Ctrl+V** in the paste area, or choose **Paste images**. No ordering, cropping, region labeling or preparation step is required. Original inspection is optional through **View collection**. Document collections expose the same optional flashcard guidance when creating or updating their study materials. Parsing and OCR happen locally; AI requests use only your configured provider. Review generated material against the source. [Documents and AI generation](documents-and-ai-generation.md) covers formats, image collections, progress, cancellation and privacy. **Annotate** beside a PDF opens its pages for handwriting, highlighting, shapes and typed notes; see [Annotating PDF pages](pdf-annotation.md).

Open **Library → Study guides** to browse generated guides, add learner notes, review source links, export a guide, or practice linked cards. Guides and cards are separate learning artifacts: one may exist without the other. See [Study guides](study-guides.md).

## Adjust the interface

**Settings → Appearance** offers Light, Dark, Console, and Synthwave themes, separate interface/card typography, scale, motion, decorative artwork, celebrations, and optional tutorials. Preview changes, then use **Save preferences** when shown. Provider profiles and template edits have their own save paths.

Tutorials default off and never start automatically. Enable them and start one explicitly from related Help when needed. Ordinary help does not require enabling tutorials. Reduced motion and disabled decoration preserve controls and educational content. See [Appearance and guidance](appearance-and-guidance.md).

## A useful first check

Create a small deck, add a few cards, find them through Search, practice them, and look at Progress. Restart and confirm the content and reviews remain. Then make a complete backup. For development builds, use an isolated library rather than experimenting on valuable data; the [local testing guide](../development/local-testing-and-handoff.md) shows how.
