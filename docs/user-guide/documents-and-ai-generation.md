# Documents and AI-generated learning material

Library's **Documents** page keeps a local copy of each source you add and turns it into flashcards and study guides. Both are made from the same saved reading of the source and can be made independently. Generated cards are ordinary editable cards; a generated guide keeps its text separate from your own notes and edits.

## Before you begin

Use material you are allowed to process and choose a provider appropriate for its privacy. In **Settings → AI**, add a provider and choose a model for **Flashcard and study-guide generation**, then use **Test model** (see [AI providers](ai-providers.md)). That one model makes both cards and study guides. Until a model is chosen for that task, **Make study materials**, **Make a study guide**, **Make more cards…** and **Retry** open the AI setup guide in **Settings → AI** instead.

For a scanned PDF on desktop, install the OCR runtime and trained data described in your platform's installation guide. Android packages an English Tesseract model with the application and needs no separate OCR installation or download. A missing platform capability can make the Documents page unavailable rather than sending the original file elsewhere.

The original file is not uploaded to the model. ostudy keeps a local copy, reads it on your device, and sends bounded portions of what it read to the chosen provider. Choosing a local model never permits a cloud fallback.

## Add a document

On Documents choose **Upload → Document…** and select a supported file; the format is detected automatically. With a subject chosen in Library, the document goes straight into that subject. For a PDF, **Import document** then asks whether to **Use images from this document** (on unless you turn it off): with it on, oStudy finds the document's figures once its pages are read (see [Figures](#figures)); with it off, the document is used as text. Once the document has been read and its topics planned, card writing and study-guide writing start side by side. While anything runs, the document's row shows one progress bar with what is happening beside it, such as **Making flashcards and a study guide**; **Generation details** in its **…** menu shows each part on its own bar with the current stage and live request statistics. Either part can finish first. Leaving the page does not stop the work.

Each document is one row: its format, length and when it was added, its reading range if it has one, what it has made so far (**Study guide**, its cards, or **Nothing made yet**), its status while it is being read or after a failure, **Open**, and a **…** menu. **Open** opens the original file through the operating system. The menu holds everything else:

- **Annotate**, for a PDF whose pages have been drawn ([Annotating PDF pages](pdf-annotation.md)).
- **Figures**, for the same PDFs: the figures oStudy found in it ([Figures](#figures)).
- **Reading range…**, for a document in a subject (see [Documents in a subject](#documents-in-a-subject)).
- **Add to a subject…** or **Move to another subject…**, and **Remove from subject**.
- **Make more cards…**, once the document has a deck.
- **Retry**, when card generation failed or the document's deck was deleted.
- **Make a study guide** or **Remake the study guide**, and **Stop study guide** while one is being written.
- **Stop** while the document is being read or its first cards are being made.
- **Rename…**: give the document your own title. A title you choose stays; generated metadata no longer replaces it.
- **Details**: the file, size, length, processing time and token use. While the document is being processed or made into study material it is **Generation details**, first in the menu, and adds each part's progress, the current stage, the live request statistics and, under **Provider and request**, the provider and model serving it and the requested and effective reasoning. **Rename…** waits until processing ends.
- **Delete**.

On a phone, **Open** also moves into the menu.

**Make more cards…** asks for optional **Instructions** describing the focus, audience, or question style you want, for example "For each cranial nerve, make concise recall cards about how nurses test it." Instructions shape the cards only; they are not treated as source material and do not change the study guide. The instructions are captured when the run starts, so editing them later does not change a run already under way. A first upload uses no instructions.

### Upload a set of images

Choose **Upload → Images…**, then **Select images** to pick several screenshots or photos at once. Use the **+** button to add more, drop files onto the paste area, click the paste area and press **Ctrl+V** (Cmd+V on a Mac), or use **Paste images**. All of these add to the same set, including screenshots pasted one after another. Pasting text or a web address does not fetch an image.

Image sets also have **Flashcard instructions (optional)**, with the same rules as **Make more cards…**.

Processing starts automatically. ostudy keeps each original, reads the images on your device, works out where screenshots overlap, and makes flashcards and a study guide. You do not need to order, crop or label images. Overlap is decided from the images themselves, so passages whose numbers, units or wording differ are not merged.

You can keep adding images while processing runs. They wait for the current step, then extend the same source and deck rather than creating a second one. Leaving the page does not stop the work; **Stop** does. A failure shows **Retry processing**; images and cards already saved stay. A file that failed to add can be removed without re-adding the others.

**Open** on an image set returns to it, to add images or **View** a saved original.

### Where the results go

Generated cards appear in **Library → Flashcards**, in the document's own deck, and generated guides in **Library → Study guides**. There is no approval queue for generated cards; everything the model returns must pass ostudy's own checks before it becomes study content. A remade study guide replaces the old one only when the whole new guide is ready, so a failed remake keeps the last guide.

### Generated card formats

Document cards can be Basic or Fill in the blank recall cards, or single-best-answer, select-all, true/false, matching, grouping and ordering questions where the source supports them. Choose which formats AI writes under **Built-in templates** in **Settings → Card templates**: each format has a switch, and **Question mix** sets how strongly each one is preferred and how many parts one question may have. Switches save at once and never change existing cards or running tasks. Image occlusion is always made by hand. The mix is a target, not a quota: if the source does not support a format, ostudy uses another or makes fewer cards rather than filler.

Review generated material for factual errors, weak questions, duplicates and missing nuance. A source label and a valid format are useful evidence, not proof that a card is correct.

## Documents in a subject

A **subject** is one course: its documents, guides, lessons and decks. Library's page picks the subject, and its **Documents** tab lists that subject's documents. **No subject** lists the documents not filed anywhere, each with **Add to…** on its row. Subjects do not nest, and a document is in one subject at most. **Search documents** above the list narrows it to the documents whose title or file name contains what you type.

A document's **…** menu files it: **Add to a subject…** or **Move to another subject…**, and **Remove from subject**. Moving a document moves its reading range and its generated deck with it; removing it leaves the document and its deck in **No subject** and clears its reading range. A generated deck that you filed in a different subject yourself stays where you put it.

**Reading range…** limits what the subject's study materials use from a long source, such as pages 1–3 and 7. The row shows the range, for example "pages 1–3 studied". **Whole document** clears it. When the file changes, the row says the range needs review.

### New study set

A study set is a subject and the materials made for it. The **New study set +** tile at the end of Library's page starts one in one place. Its sheet asks for:

- **Subject**: the new subject's name. A name alone makes an empty subject.
- **Documents**: **Choose files** adds a file each time; **From the library** adds documents you already have, moving them into the subject. A new file is read as if you had uploaded it in the subject.
- **Prompt**: optional with documents, where it says how to use them, such as what to cover, what to use for detail or what to skip, and also guides their first cards. With no documents, describe what to learn and the AI connection used for cards and guides writes study notes on it. The notes become a PDF in the subject's **Documents**, and everything else is made from them like any upload.
- **What kinds of study materials do you want to create?**: **Flashcards**, **Study guide**, **Lessons** (with **Lecture audio**, which starts as **Make lecture audio with new lessons** in **Settings** says and needs a lesson voice) and **Podcast**. A podcast is made from the lessons: once they are written, two hosts talk through the first three.

**Make study set** opens the subject's **Documents**. To add documents and materials to a subject you already have, choose **Add materials…** in its **…** menu on its tile or beside its name; the same sheet opens without the name and ends with **Add materials**. Reading a document always writes its first cards. Once every document is read, the subject's study materials start as if you had chosen them in **Make study materials**: the study guide, the subject's flashcards, and the lessons, then the podcast. If you leave the page first, they start the next time you open the subject's **Documents** while the app is still running.

### Make study materials

A subject's own study materials are made from its documents together. Nothing is made until you choose **Make study materials** beside **Upload**. Its sheet asks:

- **From**: which of the subject's documents to use. Every document is ticked unless you left it out before. An unticked document stays in the subject, but the subject's guide, its lessons and a Tutor chat about the subject skip it. Its row says "left out of study materials", and **Use in study materials** in its **…** menu puts it back. **Leave out of study materials** in the same menu leaves a document out without opening the sheet. Moving a document to another subject puts it back in.
- **Make**: **Study guide**, **Flashcards** and **Lessons**. Everything not yet made is ticked to start.
- **Cards go into** and **Instructions (optional)**: these appear when **Flashcards** will write cards for a ticked document that has none yet. Its new cards go into the chosen deck of the subject, or into a new deck of its own, and the instructions guide what they cover. A document's later **Make more cards…** adds to the same deck.

**Make** first writes those documents' cards, then writes the guide and the lessons side by side, with one **Stop** for both. Lessons appear in the subject's **Lessons** tab; a subject's lessons are written once, so the sheet shows them as **Made** afterwards. Afterwards the subject offers **Study** and **Read guide**, and **Update study materials** when its documents or ranges change (it reads **Retry study materials** after a failure and **Recreate study materials** after you deleted its guide). The subject's flashcards are the cards in its decks: **Study** practices those decks as they are when the session starts, and each card keeps its one schedule and history wherever you study it. Nothing is copied.

Making a subject's materials does not read or OCR an already-processed document again. The subject guide is written from what ostudy already extracted from each document, organized across documents, and each fact still points back to its own source.

Deleting a subject (from the **…** menu beside its name in Library) moves its documents and decks to **No subject**; it deletes neither. The subject's guide and lessons are deleted with it. Deleting only the subject guide keeps it deleted until you choose **Recreate study materials**.

Earlier versions called a subject's documents a "collection" and kept its flashcards as copies in a separate deck. The update folds unedited copies back into their original cards; see [Library upgrades](library-upgrades.md).

## Supported sources

ostudy reads PDF; Word DOC and DOCX; PowerPoint PPT and PPTX; OpenDocument ODT, ODP and ODS; EPUB; RTF; Excel XLS and XLSX; and CSV. Image sets accept PNG, JPEG and WebP screenshots and photos; they are not stitched into one image.

The format is detected from the file's contents. The older Word, PowerPoint and Excel formats also need their usual file extension, and CSV needs its `.csv` extension because it has no signature of its own. Renaming a file does not make it a supported format. Web pages, audio, and other formats an underlying library might read are not supported.

## Figures

For a PDF imported with its images on, oStudy looks for its figures once the pages are read, beside card and guide writing, without contacting a provider. A figure is a picture or a drawing big enough to study. Logos and other marks that repeat in the same place on several pages, pictures in the header or footer band, ruled tables and boxed paragraphs are left out, and so is a page that is one scanned picture.

**Figures** in a document's menu shows each figure drawn from the PDF, with its label and page ("Figure 3.2 · Page 14"), its caption, and the pages that explain it: passages anywhere in the document that name its label, quoted, and, for a figure that fills most of its page or a page with little other text, the pages on either side. **Show captions in the picture** draws each figure with or without its caption. **Hide** moves a figure that is not worth studying into **Hidden**; **Show** brings it back. Searching again keeps a hidden figure hidden while it is found in the same place.

When your AI model can see images, oStudy also reads each figure on its page, beside card and guide writing: a short title, what the figure shows, how the text uses it, and how much it matters (**Essential**, **Key figure**, **Helpful** or **Minor**). It also marks a figure that is not worth studying, such as a decorative photo. **Read figures with AI** in the page's **…** menu reads them for a document imported earlier, and **Read figures again** reads them anew. A model that cannot see images leaves figures with their captions and passages, and the page says so. Searching again keeps what was read of a figure found in the same place.

A document imported before figures existed, or with its images off, shows **Find figures** or **Use images** there. **Stop using images** in the page's **…** menu forgets the figures found, hidden ones included, and **Find figures again** searches the PDF again.

Figures are found on desktop only for now: Android and iOS cannot read a PDF's page objects yet, so their Figures page says so. A study guide shows the figures that best illustrate each of its sections (see [Study guides](study-guides.md#read-and-write-on-the-guides-pages)); hiding a figure keeps it out of the guide. Flashcards and lessons do not use figures yet.

## How sources are read

ostudy keeps a managed copy of the source and extracts its text, structure, images and page renders. What is extracted is interpreted once and saved; flashcards, study guides and Tutor all reuse it, and changing the chat or guide model does not repeat it.

For PDFs, ostudy:

- reads the text with its paragraphs, lists and headings, and reads two-column pages once rather than twice;
- rebuilds tables drawn with cell borders from those borders;
- reads scanned pages, and pages whose text cannot be extracted, with OCR on every such page (Tesseract on desktop and Android);
- sends the rendered page image with the text for pages that hold a numbered table or figure, a grid of text, or OCR text, when the tested model accepts images;
- skips end-of-chapter material such as key terms, chapter reviews, review questions, references and indexes, which only repeat or test the chapter.

OCR and page images help, but they do not guarantee that every diagram or table was understood.

Other formats record their place in the source as **sections**, even where the file has slides or sheets; do not read a section label as an exact page. Their images are the embedded images, not renders of whole slides. Linked external images are not fetched.

**Open source** in a study guide opens the original through the operating system. It cannot always jump to the exact page, slide or section.

## What the provider receives

Each request contains a bounded portion of the extracted text, the relevant outline and plan, and selected page images only when the tested model accepts images. Large sources are split into many bounded requests. When an embedding model is set up, processing also indexes the saved knowledge for Tutor search; the embedding service receives the saved text, not a second request to interpret the document. Ordinary navigation sends nothing. A **New study set** made without documents sends what you typed in **Prompt** in one request for its notes; the notes then come back as a document and are read like any other.

Model output cannot write to the library, files or commands directly. Generated content is checked for structure, size, allowed formatting, tags, Fill in the blank syntax, question rules, source references and duplicates before it is saved. A false statement or distractor in a question is never recorded as something the source asserted, and a single-best-answer question must have one supported best choice.

Diagnostics leave out credentials, prompts, responses, reasoning text, source text, answer keys and image bytes. Still review a diagnostic export before sharing it, since it can contain file names.

## Progress and stopping

Progress shows the steps ostudy can actually observe. Reading a non-PDF can show an indeterminate bar until its length is known. Card progress counts finished batches and study-guide progress counts finished sections; neither bar guesses from elapsed time. **Complete** belongs to each artifact, so one can finish while the other is still running. An indeterminate bar does not mean the task has stalled.

**Stop** and **Stop study guide** ask the work to stop at its next safe point. Work already saved is kept, and a stopped remake leaves the previous guide in place. Failures stay visible when you return to Documents.

If one part of a study guide cannot be written, the guide still publishes, and that section is marked **Not written yet** and lists the facts extracted from that part of the source. Remaking the guide tries it again.

On Android, a long document task uses a foreground service with a visible notification so work can continue while the app is in the background. That does not protect it from the system stopping the app, a reboot, or **Stop**. On reopening, ostudy picks up the saved state.

On desktop, closing while documents are processing warns that the work will be interrupted. Restarting never sends more of a source to a provider by itself; retry deliberately. A whole-library backup cannot be made while document work is running.

## Retry, more cards, and remaking

**Retry** reuses saved reading, outlines and finished card batches when they still match the current source, settings and ostudy version, so only the remaining work is redone. Saved knowledge is reused by any model. Some ostudy updates change how documents are read; those documents are read again the next time their first cards are made. In **Generation details**, **Retries** counts recovery requests during a run, not failed documents or pages.

**Make more cards…** uses the saved reading and the cards the deck already has; an unchanged source is not read again.

Remake a guide from the document's menu. Its row shows a new Study Guide bar straight away: **Preparing** while oStudy plans the sections, then how many sections are completed ("3 / 14 sections completed"), **Organizing the guide** while it adds chapters and the introduction, and **Saving guide**. The reader has no Regenerate button or revision number. A complete new guide replaces the old one in one step. Your notes follow the rules in [Study guides](study-guides.md).

## Analysis details

With diagnostics turned up, logs can show model proposals that were not accepted, rejected question candidates, parts of the source with no accepted result, provider failures and reuse of saved work. These do not necessarily mean a wrong fact or an unread page, and they do not appear as error banners on a successful run. A real failure still says why the operation could not finish. Complete coverage does not guarantee that every important idea was found.

## Sources, cards and deletion

Generated cards remember the learning topic and source they came from, and questions keep private evidence of how they were made. These links do not depend on tags, so renaming or moving a card does not change its source.

Deleting a document removes its managed files, its annotations and its processing state, unless a running task prevents it. Its generated decks and cards stay as ordinary study content, with a text-only note of the source they came from. Deleting a generated deck does not delete its document.

Deleting only a study guide removes the guide, its section edits, its saved work and its prepared PDF exports, and keeps the source, its learning plan, its linked cards, and your guide and topic notes. Read the confirmation.

## Backup, sync, and limits

A whole-library backup includes documents, their managed files, everything extracted from them, image sets, guides, annotations, generated questions and their evidence. Older backups remain readable. Provider keys are never included.

Device sync has separate **Documents** and **Study Guides** categories; **Study Guides** needs **Documents**, and links between documents and cards travel only when **Flashcards** is selected too. Annotations and figures travel with **Documents**. A sync is not a substitute for a whole-library backup. See [Device sync](device-sync.md).

Malformed, protected, unsupported, oversized or poorly recognized sources can fail before generation. OCR quality, model reliability, extraction fidelity, document size, context limits and provider cost remain practical limits. Reading a file locally is not a sandbox for hostile files; process only sources you trust.

To report a problem, give the source format, the step that failed, platform and build, provider and model, the error message, and what happened on retry. Say whether the trouble is in reading or OCR, the provider's response, generated wording, source labels, or PDF export. Use a made-up or redacted source rather than private material.

For implementation details, see [AI and document processing](../architecture/ai-and-document-processing.md), [document AI routing](../architecture/document-ai-routing.md), and the [current architecture](../architecture/overview.md).
