# Study guides

A study guide teaches what a source says, in sections sized for one sitting. A document's guide is written from what ostudy extracted from that document; a subject's guide is written from what it extracted from the subject's documents and their reading ranges. Both have an overview, learning objectives, a short list of key concepts, ordered sections, source labels, and links to the flashcards made from each section. A guide does not replace checking the original sources.

## Create and find a guide

Choose a model for **Flashcard and study-guide generation** in **Settings → AI**. For one document, open **Library → Documents**, upload a supported source, and choose **Make a study guide** from its **…** menu. For a whole subject, choose the subject in Library, open **Documents** and choose **Make study materials** with **Study guide** ticked (it can write the subject's flashcards and lessons at the same time), or **Update study materials** once it has them; a subject has one guide alongside its decks. A subject guide is written from what ostudy already extracted from each document, so its documents are not read again and do not need guides of their own. Large subjects are split into many bounded requests rather than refused. A document guide can exist without a flashcard deck.

ostudy aims to teach each core fact once, in the most specific section it belongs to. After a section is written, a short follow-up adds explanations for core facts it left out, and additions that only restate the section are dropped. Key points, terms, tables, steps and callouts appear only when they add something the explanations do not already say. A textbook chapter usually becomes about ten sections, grouped into chapters with headings when the guide is long.

Open **Library** and choose **Study guides**. Each guide is a card with its source, its number of sections and notes, the date it was made, an **Open** button and a **…** menu with **Open the source** and **Delete study guide**. A subject's guide sits on a stacked card, since it covers several documents. **Search guides** filters as you type; **Sort** orders guides by Newest, Oldest, Title A–Z or Title Z–A, and **Previous** and **Next** appear when there are many.

Choose **Open** to read a guide. The reader opens as a page inside Library, under the header, with the main navigation still available. **Back to Study Guides** at the top of a document guide, or **Close subject study guide** on a subject guide, returns to the library. Choosing **Library** in the main navigation also returns to that library rather than leaving you trapped in the current viewer.

The reader has no revision numbers or Regenerate button; remake a guide from its document in Documents.

## Read source-grounded sections

A document guide opens on its **Overview**: the guide's title and source, a short summary under **In one minute**, the learning objectives, the key concepts as a short list of terms, the guide's chapters when it has them, your guide note, and **Start reading**. Each section then has a page of its own. The top of a section page shows its position and an approximate reading time at about 200 words a minute, its title, and a **From** chip naming the source document, its file kind and the pages or slides the section comes from; select the chip to open the source. The section's blocks follow in reading order: **In one minute**, explanations, tables, comparisons, process steps, callouts marked as a key idea, tip, warning or note, numbered figures, worked examples, common misconceptions, and **Check yourself**. **Previous** and **Next** at the end of the page name the neighboring sections; **Next** is the page's main button.

On a wide window the outline sits beside the text. It lists the overview and every section, grouped under the guide's chapters when it has them, and opens the current section to its own headings so you can jump to a block. The shaded margin on the other side holds **Your note**, the section's **Important terms**, its **Source** with **Open source**, and its cards. On narrower windows the outline opens from **Contents** in the top bar, and the margin's blocks follow the end of the section. On a phone, a bar at the bottom of the screen keeps the previous section, **Practice** and the next section within reach.

Section commands live under that section's **…** menu: **Ask Tutor about this section** when the tutor is available, **Add note** or **Edit note**, **Edit or rewrite this section**, and the matching remove and reset commands. Guide commands live in the top bar: **Ask Tutor** for the whole guide (a chat icon on a phone) **Export PDF** (an export icon on a phone), which uses the theme set in **Settings → Appearance → Study guide PDF**, and the **Study guide actions** menu with the guide note. The **Page** menu beside them sets how sections read: **Navy**, **Sage**, **Slate** or **Plum** show each section as a designed page in that PDF theme, with its terms in the margin and **Check yourself** still answerable below the page. The guide's first page reads as an **At a glance** page in the same theme, with its overview, objectives, key concepts, key ideas and warnings, and the chapters and sections still linked below it. **Plain** shows the text in the app's own style. A section you edited or rewrote always reads as your text. On a phone the page styles are in **Study guide actions**. On a designed page, **Write on this page** in the section's **…** menu opens it with the PDF annotator's pen, highlighter, shapes and notes; your marks show over the page whenever you read it in a theme, and stay when you switch themes, though a theme with different typefaces can move the text under them a little. **Open source** opens the retained original through the operating system; an exact deep link into an external viewer is not guaranteed.

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

Choose **Export PDF** in a document or subject guide's top bar, then pick where to save the file. The PDF's look is a setting: **Settings → Appearance → Study guide PDF** picks the **Theme** (Navy, Sage, Slate or Plum change the colours and typefaces, not the layout; Sage until you choose another) and whether to **Include my guide and section notes**, which adds your notes and section edits. The PDF shows the same content as the reader and is made on your device; export sends nothing to a provider and does not regenerate the guide.

The PDF opens on a cover with the overview, what you will be able to do and a linked contents list, then gives each section its source label, key points, callouts, tables, worked examples and checks, with chapters as slim bands when the guide has them. It ends with a **Cram sheet** of the key concepts, key ideas, terms and warnings, and a **Self-test** of the guide's check questions with answers after them. It has bookmarks, running headers and page numbers, repeats table headers across page breaks and keeps headings with the text that follows them. Source images the guide uses appear with captions. Text stays selectable and searchable.

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
