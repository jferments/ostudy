# Getting started

ostudy keeps your study library on your device. You can create cards, organize decks, practice, search, and follow your progress without an account or AI provider. Document generation is optional and uses only the provider you configure.

## Find your way around

The main navigation has five destinations: **Home**, **Library**, **Review**, **Tutor**, and **Progress**. On a wide or medium window they sit in a rail down the left side, with **Search** and **Settings** at its bottom. On a phone they are in the bottom bar, and the top bar keeps Search and Settings. There is no separate More menu. **Review** shows a badge with the number of cards due today.

**Home** opens first. Its Today strip shows how many cards are due today and about how long they take, with **Start review**; on a wider window the arrow beside it lets you choose which subjects to review. Below it are your subjects: each tile shows what is due, its sources and cards, and opens the subject's page. A subject's page has **Manage**, which opens Library with only that subject, **Review N due** for what is due in it, the overview guide across its sources, its sources and how far each has been prepared, and its cards and decks with **Browse cards** and **Practice a mix**.

**Library** opens the **Workbench**, where you browse, search, edit, and organize cards; **Documents** and **Study guides** are the other tabs at the top of Library. **Review** shows what is due today with **Start review**, the same cards by subject with a **Start** for each, and **Choose decks** for a custom practice. **Tutor** holds conversations with the AI model you set up, grounded in your study material. **Progress** shows your study activity, how well you remembered, card strength, and upcoming workload.

The Workbench shows a tree of subjects, decks, and tags on the left, the chosen cards in the middle, and the open card's editor on the right; on a phone you see one of these at a time. The header's **Search** opens the Workbench on **All cards** with the search field focused. Card templates are in **Settings → Card templates**. Deck import/export is contextual; complete-library backup and device sync are in **Settings → Backup and sync**.

Choosing **Library** opens the page you used last. Library's **Flashcards** tab returns to the Workbench, and choosing Study guides returns to the guide library rather than keeping an already-open viewer in the way. Practice opens from a study action and runs inside Review; it is not a navigation tab of its own. On Android, Back returns to Home from any other destination, and from a subject's page to Home's subject list. In the Workbench it first steps back to the tree on a phone, or to **All cards** on a wider screen; during practice it returns to where the session started.

## Create your first deck

A new library opens on Home's welcome, **Your library is empty**, with **Create a subject**, **Import a document**, and **Open Library**. A subject keeps one course's documents, study guides, and decks together; you can also start with a deck and file it later.

Open **Library**. On a wide or medium window, a library with no decks shows **No cards yet** with **New deck** and **Import deck**. To start a deck inside a subject, choose the subject and then **New deck**, or use **New deck here** in the subject's actions menu. On a phone, the tree offers **New deck** while you have no subjects.

Give the deck a name. The Workbench opens it; choose **New card**. Use a Basic question and answer for a first card. Reverse presentations are useful when both directions are worth recalling. Fill in the blank cards hide marked parts of a statement. Choose a reusable template when many cards have the same structure.

**Bulk add…** in the deck's actions menu accepts multiple prepared question/answer pairs in the current deck. The destination is fixed by the deck you opened. Ordinary authoring supports duplicate checks, tags, managed images, and recoverable drafts; a draft is not a committed card until saved successfully.

Keep one focused retrieval task per card. Generated or imported wording should receive the same review as a card you wrote yourself. See [Library Workbench and authoring](flashcards-workspace.md).

## Organize without changing your study history

The **+** beside Subjects in the Workbench, or **New subject** on Home, creates a subject. **No subject** appears only when some decks or documents are not filed under one. Use the menu beside a deck in the tree for **Move to subject…**, **Move up**, and **Move down**.

Collapse/expand changes presentation, not membership. A deck's actions menu also has **Merge into…**, **Export…**, **Archive**, and **Delete deck**. **Merge into…** moves the source deck's cards into another active deck while preserving card/note identities, schedules, review history, tags, media, and source provenance. The emptied source deck is archived after the transaction, and existing destination cards are not deduplicated simply because their wording looks similar. Deleting a subject moves its decks and documents to No subject; deleting a deck is a different action with different consequences. Archiving retains the deck but removes it from normal study availability until restored. See [Subjects and deck organization](flashcards-organization.md).

## Practice a small session

Use **Start review** on Home or in Review for everything due today, **Practice** on a deck, or **Practice with options…** in a deck's actions menu (or **Practice…** on a subject) to open the practice setup. There you choose **Scheduled study**, which keeps to today's schedule and daily limits, or **Extra study**, and **Cards available to study** shows what the session will include. Suspended, buried, or archived cards are not studied.

Think of the answer before choosing **Show answer**. Then choose **Wrong**, **Hard**, **Medium**, or **Easy** based on how well you recalled it. On a keyboard, Space or Enter reveals the answer, and 0–3 chooses a rating when no editor or overlay has focus. Each rating is saved as a review and updates the card's schedule; it is not merely a visual score.

The top bar shows how many cards you have studied and how many remain. When the last card is rated, **Session complete** sums up the session: how many you remembered, cards that became solid, focused time, and any missed cards you can practice again. **End session** stops early and returns to where you started; every rating you already gave stays saved. A session that was interrupted can resume after you reopen ostudy. See [Practice, scheduling, and Progress](practice-and-analytics.md).

## Find and manage cards

The Workbench's search box finds cards within the chosen subject, deck, or tag; choose **All cards** to search the whole library. **Show** narrows the list to **All cards**, **Due today**, **New**, **Often missed**, or **Suspended**, and **Sort** changes its order.

**Select shown** affects loaded rows. **Select all N** selects every matching card. Forward and reverse presentations can appear separately, while actions that edit their shared note can affect both. Read the confirmation before bulk deletion, moving, or scheduling changes.

## Protect your library

Use **Import deck** beside Library's tabs for a `.studydeck`, a compatible Anki `.apkg`, or a generated `.ostudy.json` file. Export a deck with **Export…** in its actions menu. A shared deck has fresh schedules when imported and does not include the sender's personal review history.

Use **Settings → Backup and sync** for a complete `.ostudybackup` and restoration. A backup contains personal library history and managed files but excludes credentials. Restore validates and replaces the library in the running application, then refreshes the interface; it does not require restarting oStudy and is not an append operation. Keep backups outside the app's data directory and before testing risky changes. [Backups, imports, and sharing](backups-imports-and-sharing.md) explains the formats and [device sync](device-sync.md) explains explicit trusted-device exchange.

## Add AI only when it is useful

In **Settings → AI**, choose **Add a service**, pick a service, enter its **Server address** or API key, and choose **Connect**. Then, under **Default for each task**, choose the service and model for each task: cards and guides, the Tutor, and search. **Test model**, in each row's menu, checks that a model can do the work before a long job. Setting up a provider does not install a local server.

The available providers are llama.cpp / Unsloth GGUF (a local or LAN server), DeepSeek, OpenRouter, and Claude. Other names in the catalog are not yet usable. A local provider never silently falls back to a cloud one. See [AI providers](ai-providers.md).

In **Library → Documents**, **Upload → Document…** makes flashcards from a supported source. A document's **…** menu holds its other actions; **Make more cards…** there takes optional instructions for a specific focus or style, which guide the new cards but are never treated as facts about the source. For screenshots or photos, choose **Upload → Images…**, add several images, and let ostudy create the flashcards and study guide automatically; you can drop files, paste screenshots, or choose **Paste images**. No ordering, cropping, or preparation step is required. Parsing and OCR happen on your device; AI requests go only to the provider you chose. Review generated material against the source. [Documents and AI generation](documents-and-ai-generation.md) covers formats, image collections, progress, cancellation and privacy. **Annotate** in a PDF's **…** menu opens its pages for handwriting, highlighting, shapes and typed notes; see [Annotating PDF pages](pdf-annotation.md).

Open **Library → Study guides** to browse generated guides, add learner notes, review source links, export a guide, or practice linked cards. Guides and cards are separate learning artifacts: one may exist without the other. See [Study guides](study-guides.md).

## Adjust the interface

**Settings → Appearance** offers Light, Dark, Console, and Synthwave themes, separate interface and card fonts and sizes, motion, decorative artwork, celebrations, and optional tutorials. Changes preview immediately; use **Save preferences** to keep them. AI providers and card templates have their own save buttons.

Tutorials are off by default and never start on their own. Turn on **Interactive tutorials**, then start one from a related Help topic. Help itself works without tutorials. Reduced motion and turning off decorative artwork keep every control and all study content. See [Appearance and guidance](appearance-and-guidance.md).

## A useful first check

Create a subject and a small deck, add a few cards, find them through Search, practice them, and look at Progress. Restart and confirm the content and reviews remain. Then make a complete backup. For development builds, use an isolated library rather than experimenting on valuable data; the [local testing guide](../development/local-testing-and-handoff.md) shows how.
