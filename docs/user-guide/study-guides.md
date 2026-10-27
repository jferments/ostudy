# Study guides

A study guide teaches what a source says, in sections sized for one sitting. A document's guide is written from what ostudy extracted from that document; a subject's guide is written from what it extracted from the subject's documents and their reading ranges. Both have an overview, learning objectives, a short list of key concepts, ordered sections, source labels, and links to the flashcards made from each section. A guide does not replace checking the original sources.

## Create and find a guide

Choose a model for **Flashcard and study-guide generation** in **Settings → AI**. For one document, open **Library → Documents**, upload a supported source, and choose **Make a study guide** from its **…** menu. For a whole subject, choose the subject in Library, open **Documents** and choose **Make study materials** with **Study guide** ticked (it can write the subject's flashcards and lessons at the same time), or **Update study materials** once it has them; a subject has one guide alongside its decks. A subject guide is written from what ostudy already extracted from each document, so its documents are not read again and do not need guides of their own. Large subjects are split into many bounded requests rather than refused. A document guide can exist without a flashcard deck.

ostudy aims to teach each core fact once, in the most specific section it belongs to. After a section is written, a short follow-up adds explanations for core facts it left out, and additions that only restate the section are dropped. Key points, terms, tables, steps and callouts appear only when they add something the explanations do not already say. A textbook chapter usually becomes about ten sections, grouped into chapters with headings when the guide is long.

Open **Library** and choose **Study guides**. Each guide is a card with its own title, its source, its number of sections and notes, the date it was made, an **Open** button and a **…** menu with **Ask Tutor**, **Export PDF**, **Remake the study guide**, **Open the source** and **Delete study guide**. Remake writes a new version of a document's guide; the card shows its progress, the old version stays open to read until the new one is saved, and **Stop remaking** in the same menu stops it. While no AI model is set for guides, Remake opens AI setup instead. A subject's guide offers Ask Tutor and Export PDF. A subject's guide sits on a stacked card, since it covers several documents. **Search guides** filters as you type; **Sort** orders guides by Newest, Oldest, Title A–Z or Title Z–A, and **Previous** and **Next** appear when there are many.

Choose **Open** to read a guide. The reader opens as a page inside Library, under the header, with the main navigation still available. **Back to Study Guides** at the top of a document guide, or **Close subject study guide** on a subject guide, returns to the library. Choosing **Library** in the main navigation also returns to that library rather than leaving you trapped in the current viewer.

The reader has no revision numbers or Regenerate button; **Remake the study guide** in the **Study guide actions** menu writes a new version, and the current one stays open until it is saved.

## Read and write on the guide's pages

A document guide opens as its own PDF pages, the same pages **Save a copy as PDF…** writes, laid out on your device in the study guide theme (**Navy**, **Sage**, **Slate** or **Plum**; Sage until you choose another). The theme is one setting for reading and for PDFs: choosing one in the **Page** menu changes **Settings → Appearance → Study guides → Study guide theme**, and the other way round. Every page sits on one canvas with the PDF annotator's tools: the pen, highlighter, shapes, text and sticky notes, the eraser, lasso, undo and zoom (see [Tools](pdf-annotation.md#tools)). The page in view and **Saved** show at the toolbar's right. **Add link** (or **K**), beside the shapes, places a link to a document page, a study guide, a deck or a notes page on the page you are reading; select a link and choose **Open link**, or tap it twice, to follow it, and a deck's link can be a button that starts Practice. Your marks save as you make them.

Beside each section's heading, a **Practice** button starts Practice on the flashcards linked to that section. On a wide window the guide's contents sit beside the pages: the cover, each chapter with its numbered sections, the cram sheet and the self-test. Choose one to scroll the pages to it; the section you are reading is highlighted, and scrolling on past a section counts it read. Each section's **…** menu in the contents holds **Practice this section's flashcards**, **Ask Tutor about this section** when the tutor is available, **Mark as read** and **Edit or rewrite this section**. On narrower windows the contents open from **Contents** in the top bar.

Each section ends with a **Sources** line naming the pages it comes from, when ostudy can tell which passages of the document its key points come from. A section lists each page once: neighbouring pages are one range of up to three pages (**pp. 116–118**), however many key points come from them. Key points carry no marks of their own, so the text stays clean. Choose a page or range on the Sources line to open the section's sources beside the pages: the passages on those pages as the document says them, with the chosen one marked, and **Open page N** to read them in the document. ostudy matches each key point to the one or two passages that hold most of its words itself, without asking the AI, and leaves out a key point it cannot match well enough rather than naming a wrong page. A section you edited or rewrote has no Sources line, and neither do subject guides yet.

**Open in PDF reader** in the top bar opens the guide in your system's PDF viewer, in the current theme and with your guide and section notes, without asking where to save it. Chapters run on from one another rather than each starting a new page, so pages are not left mostly empty; marks are kept with the chapter whose page they are on, and editing or rewriting a section, which lays the guide out again, can move the marks on the pages after it. Marks written on the single designed section pages of earlier versions stay in your library but do not show on these pages.

When a PDF guide's document uses its images and its figures have been found (see [Figures](documents-and-ai-generation.md#figures)), each section shows the source figures that best illustrate its key ideas: a figure that is not too wide sits beside the key points and the text after them, which runs on full width below it, drawn smaller when the text is short; a second narrow figure stacks under the first; and a wide figure, or narrow figures with no text left beside them, goes to the top or foot of the page it fits on, so the section's text fills the rest of the page rather than leaving it empty. A section shows at most two figures and each figure appears once, with its title and a caption. The figure is cropped from its page without the book's own caption: oStudy renders the figure with the caption under it, finds where the caption starts in the rendered image, and cuts above it, so no part of the printed caption is left behind. When the AI has read the figure, the caption states the ideas it carries in the subject's own terms, as a textbook caption would, or otherwise what it shows. A scanned book whose figures could not be read shows them by their number. A figure the AI has read shows when its reading names an idea the section covers, or when it rated the figure as needed to understand it; a figure not yet read shows only when its numbered caption matches the section's words. Hidden figures never show. Choosing them calls no AI and sends no page images.

## Read as plain text

**Plain text** in the **Page** menu (**Read as plain text** in **Study guide actions** on a phone) reads the guide in the app's own style instead, which also suits a screen reader. A guide opens as plain text by itself when its pages could not be laid out. In plain text a document guide opens on its **Overview**: the guide's title and source, a short summary under **In one minute**, the learning objectives, the key concepts as a short list of terms, the guide's chapters when it has them, your guide note, and **Start reading**. Each section then has a page of its own. The top of a section page shows its position and an approximate reading time at about 200 words a minute, its title, and a **From** chip naming the source document, its file kind and the pages or slides the section comes from; select the chip to open the source. The section's blocks follow in reading order: **In one minute**, explanations, tables, comparisons, process steps, callouts marked as a key idea, tip, warning or note, numbered figures, worked examples, common misconceptions, and **Check yourself**. **Previous** and **Next** at the end of the page name the neighboring sections; **Next** is the page's main button.

On a wide window the outline sits beside the text. It lists the overview and every section, grouped under the guide's chapters when it has them, and opens the current section to its own headings so you can jump to a block. The shaded margin on the other side holds **Your note**, the section's **Important terms**, its **Source** with **Open source**, and its cards. On narrower windows the outline opens from **Contents** in the top bar, and the margin's blocks follow the end of the section. On a phone, a bar at the bottom of the screen keeps the previous section, **Practice** and the next section within reach.

Section commands live under that section's **…** menu: **Ask Tutor about this section** when the tutor is available, **Add note** or **Edit note**, **Edit or rewrite this section**, and the matching remove and reset commands. Guide commands live in the top bar: **Ask Tutor** for the whole guide (a chat icon on a phone), **Open in PDF reader** (a document icon on a phone), the **Page** menu, and the **Study guide actions** menu with **Save a copy as PDF…**, which uses the study guide theme, the guide note and **Remake the study guide**. To test yourself on a section's terms, turn on **Hide meanings** beside **Important terms**: each meaning waits behind **Show** until you have said it, and the switch stays on for the next section. A section you edited or rewrote always reads as your text. On a phone the page styles are in **Study guide actions**. **Open source** opens the retained original through the operating system; an exact deep link into an external viewer is not guaranteed.

A subject guide keeps its single scrolling reader with a **Contents** list and an **Export PDF** button. Its **Guide actions** menu adds, edits or removes the guide note, and each section's **…** menu offers **Ask Tutor about this section** when the tutor is available, **Open source**, **Add note** and **Edit section**.

Sections are named from what they teach; a long list of keywords is shortened into a readable title. ostudy limits section length and checks each section's structure, but a model can still omit, misunderstand, or poorly prioritize information. Check what you learn against the source.

If one part of a guide could not be written, its section is marked **Not written yet** and lists the facts ostudy extracted from that part of the source instead. Remaking the guide tries that part again.

Sources other than PDF are labelled by section even when the file has slides or sheets, so do not read a section label as an exact page. [Documents and AI generation](documents-and-ai-generation.md) explains source limitations.

## Check yourself

Each retrieval check shows its question with **Show answer**. After you reveal an answer, **Did you get it?** offers **Yes** and **Not yet**, and the section counts how many of its checks you have answered. These answers last only while the guide is open: they are not saved, do not change any card schedule, and start fresh the next time you open the guide.

## Add notes without losing the generated base

The guide keeps learner-owned information separate from generated revisions:

| Learner change | Ownership and behavior |
| --- | --- |
| Guide note | Belongs to the source document as a whole |
| Topic note | Belongs to a stable learning topic, not just its current display title |
| Generated-section edit | A non-destructive override tied to the generated base content it edits |

In a document guide, **Add note** under **Your note** opens the note dialog: on the overview for the whole guide, and beside or below a section for that topic. The section menu and the **Study guide actions** menu offer the same commands. Write the note, then **Save**. Existing notes offer **Edit note** and **Remove note**. Clearing an existing note and saving removes it through the same deliberate ownership boundary. **Edit or rewrite this section** opens the section's text in the same kind of dialog; your version replaces the generated text in the reader, and **Reset section edit** returns to the generated section, which is never deleted. In a subject guide, the section menu's **Add note** and **Edit section** open the editor below the guide.

If regenerated content changes enough that an old section override cannot safely apply, the old edit becomes a visible conflict instead of being silently attached to different material. Review the preserved text and discard an older edit only when that is what you intend. Stable guide/topic notes survive ordinary regeneration.

Notes and section edits are plain text; the formatting tools of the card editor are not available in guides. Subject-guide notes and edits follow the same rules.

## Practice linked cards

When a section has linked cards, the end of its page says how many cards come from it and offers **Practice N cards**; the same command sits in the margin on a wide window and, while you read, in the bottom bar on a phone. The bar steps aside once the end of the section is on screen, where the page offers the cards and the next section itself. It starts a session with exactly those cards; they are the same cards as in your decks, not copies. A section without linked cards says so instead.

A section's cards are the ones generated for its topic. The link does not depend on tags, and cards you wrote yourself are not assigned to topics by guessing from their wording. A linked card can be unavailable for practice, for example while it is suspended.

Ending the session returns to Study guides. Reviews update the same schedules and history as other practice. See [Practice, scheduling, and Progress](practice-and-analytics.md).

## Export a PDF

Choose **Save a copy as PDF…** in a document guide's **Study guide actions** menu, or **Export PDF** in a subject guide's top bar, then pick where to save the file. The PDF's look is a setting: **Settings → Appearance → Study guides** picks the **Study guide theme**, the same theme the guide is read in (Navy, Sage, Slate or Plum change the colours and typefaces, not the layout; Sage until you choose another), and whether to **Include my guide and section notes**, which adds your notes and section edits. The PDF shows the same content as the reader and is made on your device; export sends nothing to a provider and does not regenerate the guide.

The PDF opens on a cover with the overview, what you will be able to do and a linked contents list, then gives each section its source label, key points, explanations of how or why in boxes of their own, callouts, terms, tables, worked examples and checks, and a **Sources** line, with chapters as slim bands when the guide has them. A box after the key points sits beside the last of them, as a narrow figure does, and two boxes in a row share the width, so the page is not one tall stack. A long overview is set smaller so the cover stays one page. It ends with a one-page **Cram sheet**: each section's key idea and key points under its number, then the terms and warnings, in three columns set as large as still fits the page (a long guide is set smaller, and only then keeps fewer key points per section), and a **Self-test** of the guide's check questions with answers after them. It has bookmarks, running headers and page numbers, repeats table headers across page breaks and keeps headings with the text that follows them. Source images and the source figures the guide chose appear with captions. The first use of each key term in a section is set in bold: the terms the guide defines, its key concepts, the section topic's concepts and what its figures were read to show. Chemical formulas and ions are set with real subscripts and superscripts (HCO₃⁻, Ca²⁺, H₂O) even when the text spells them flat, and abbreviated organism names such as *H. pylori* are italic. Text stays selectable and searchable.

Common mathematical notation, including fractions, roots, sub- and superscripts, Greek letters, operators, functions and small matrices, is typeset in the PDF. If an expression is outside that set or too large to fit, export asks first and then keeps it as readable LaTeX source rather than drawing a misleading formula. The reader in oStudy always shows full typeset math.

You choose where the file is saved through the platform's usual save dialog. Check pagination, fonts, symbols, figures, layout, and whether your notes are included before sharing the result.

## Regeneration, stopping, and failures

To remake a document's guide, choose **Remake the study guide** from the document's **…** menu in Documents. For a subject guide, use **Update study materials** on the subject's Documents tab; it updates the same guide rather than making a second one. While a document's guide is being written, its menu offers **Stop study guide**; if you open that document's guide before it exists, the **Generating study guide…** button stops it too. You can go elsewhere while it runs and come back to its current state. Stopping does not delete the source or anything already made.

A new version is checked in full before it replaces the old one, so a failed or stopped remake leaves the previous guide in place. Failures stay visible after you leave and return. Saved work from the previous attempt is reused where it still matches the source and ostudy version. Restarting oStudy never sends anything to a provider by itself.

## Delete a guide without deleting its source

**Delete study guide** in a guide card's **…** menu asks for confirmation. It removes every version of the generated guide, your section edits and their conflicts, the guide's saved generation work, and prepared PDF exports.

For a document's guide, deletion keeps the source document, its learning plan, its linked flashcards, and your guide and topic notes, so a new guide can be made later. Deleting a subject's guide keeps the subject, its documents and decks, and your schedules and reviews, and the guide stays deleted until you choose **Recreate study materials** on the subject's Documents tab. Neither deletes source material. Read the confirmation.

## Protect and report

A whole-library backup keeps the document, guide, notes, and the files they need together. Device sync carries guides only when the sender selects **Study Guides** (which also selects **Documents**). See [Backups, imports, and sharing](backups-imports-and-sharing.md) and [Device sync](device-sync.md).

For a problem report, say whether it concerns the generated text, your notes, section edits or conflicts, source labels, linked cards, stopping generation, or the exported PDF. Include the exact build and steps to reproduce; do not post private source text or credentials.
