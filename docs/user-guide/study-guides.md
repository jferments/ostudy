# Study guides

A study guide is a source-grounded learning artifact built from retained source knowledge. Individual-document guides use that document's learning plan; collection guides use the exact authorized folder/range snapshot and its compact cross-document synthesis. Both bring together an overview, learning objectives, a small set of high-level key concepts with brief explanatory captions, ordered sections, source labels, and links to relevant flashcards. A guide is not a replacement for checking the original sources.

## Create and find a guide

Configure a suitable AI profile in Settings → AI/LLM. For one retained document, open **Library → Documents**, upload a supported source, and use **Generate study guide** beside it. For a document collection, open the collection's **Study materials** panel and choose **Create study materials** or **Update study materials**; the collection target owns one collection guide alongside one ordinary flashcard deck. Collection-guide topics are authored from the already-retained document knowledge/evidence rather than requiring separate source guides or reprocessing the member documents. A collection is not rejected merely because its retained knowledge contains many distinct topic labels or because the aggregate material is larger than one model context: large topic catalogs are ordered by source position and adaptively grouped into bounded higher-level outline sections. Each provider call is then budgeted independently against the selected model's context window; the retained knowledge catalog for that section is reduced to a balanced core sample when necessary instead of rejecting the whole collection. Every retained topic remains represented in the collection outline plan, while source-owned document knowledge remains authoritative. The generated guide uses the same rich semantic section model as document guides—narrative overview, key points, terminology, explanations, processes, comparisons, tables/examples/checks when supported—and the viewer automatically builds its Contents navigation from those sections. Internal coverage receipts and preparation-gap diagnostics are not shown as study-guide content. A document guide can exist without a flashcard deck, and the two collection artifacts retain independent publication/failure state.

Open **Library** and choose **Study guides**. The guide library supports **Search guides** by title, filename, or format; Newest/Oldest/Title sorting; Refresh; and pagination when needed. Library rows show summary information rather than loading every guide body just to list them.

Select a row or **Open** to read its guide. The reader opens as a page within the app shell, not a modal covering main navigation. **Back to Study Guides** at the top of a document guide, or **Close collection study guide** on a collection guide, returns to the library. Choosing **Library** or its **Study guides** view also returns to that library rather than leaving you trapped in the current viewer.

The reader deliberately does not display internal revision numbers or a standing Regenerate button. Regeneration belongs to the source document's action in Documents.

## Read source-grounded sections

A document guide opens on its **Overview**: the guide's title and source, a short summary under **In one minute**, the learning objectives, the key concepts as a short list of terms, the guide's chapters when it has them, your guide note, and **Start reading**. Each section then has a page of its own. The top of a section page shows its position and an approximate reading time at about 200 words a minute, its title, and a **From** chip naming the source document, its file kind and the pages or slides the section comes from; select the chip to open the source. The section's blocks follow in reading order: **In one minute**, explanations, tables, comparisons, process steps, callouts marked as a key idea, tip, warning or note, numbered figures, worked examples, common misconceptions, and **Check yourself**. **Previous** and **Next** at the end of the page name the neighboring sections.

On a wide window the outline sits beside the text. It lists the overview and every section, grouped under the guide's chapters when it has them, and opens the current section to its own headings so you can jump to a block. The margin on the other side holds **Your note**, the section's **Important terms**, its **Source** with **Open source**, and its cards. On narrower windows the outline opens from **Contents** in the top bar, and the margin's blocks follow the end of the section. On a phone, a bar at the bottom of the screen keeps the previous section, **Practice** and the next section within reach.

Section commands live under that section's **…** menu: **Ask Tutor about this section** when the tutor is available, **Add note** or **Edit note**, **Edit or rewrite this section**, and the matching remove and reset commands. Guide commands live in the top bar: **Ask Tutor** for the whole guide (a chat icon on a phone) and the **Study guide actions** menu with **Export as PDF…** and the guide note. **Open source** opens the retained original through the operating system; an exact deep link into an external viewer is not guaranteed.

A collection guide keeps its single scrolling reader with a Contents list, and each section's **…** menu offers **Open source**, **Add note** and **Edit section**.

Generated sections favor a high-yield outline rather than a long restatement of the source and can include semantic key points, explanations, terms, processes, comparisons, misconceptions, callouts, tables, worked examples, retrieval checks, and source-backed visual explanations when those structures add learning value. Machine-like topic inventories such as long slash-separated keyword lists are compacted into human-readable reader titles; this is display-only and does not rewrite the durable learning-topic identity or source evidence. The native pipeline bounds section length and validates structure, but a model can still omit, misunderstand, or poorly prioritize information. Correctness requires learner review.

For current non-PDF ingestion, canonical provenance is expressed as sections even when the parser has richer internal slide or sheet information. Do not infer precise authored pagination from a generic section label. [Documents and AI generation](documents-and-ai-generation.md) explains source limitations.

## Check yourself

Each retrieval check shows its question with **Show answer**. After you reveal an answer, **Did you get it?** offers **Yes** and **Not yet**, and the section counts how many of its checks you have answered. These answers last only while the guide is open: they are not saved, do not change any card schedule, and start fresh the next time you open the guide.

## Add notes without losing the generated base

The guide keeps learner-owned information separate from generated revisions:

| Learner change | Ownership and behavior |
| --- | --- |
| Guide note | Belongs to the source document as a whole |
| Topic note | Belongs to a stable learning topic, not just its current display title |
| Generated-section edit | A non-destructive override tied to the generated base content it edits |

In a document guide, **Add note** under **Your note** opens the note dialog: on the overview for the whole guide, and beside or below a section for that topic. The section menu and the **Study guide actions** menu offer the same commands. Write the note, then **Save**. Existing notes offer **Edit note** and **Remove note**. Clearing an existing note and saving removes it through the same deliberate ownership boundary. **Edit or rewrite this section** opens the section's text in the same kind of dialog; your version replaces the generated text in the reader, and **Reset section edit** returns to the generated section, which is never deleted. In a collection guide, the section menu's **Add note** and **Edit section** open the editor below the guide.

If regenerated content changes enough that an old section override cannot safely apply, the old edit becomes a visible conflict instead of being silently attached to different material. Review the preserved text and discard an older edit only when that is what you intend. Stable guide/topic notes survive ordinary regeneration.

The current reader's note and section-edit entry is plain-text editing backed by validated rich-document storage. Section overrides remain tied to the exact generated content hash they edited, and collection-guide notes/edits use the same non-destructive ownership model. Do not assume every rich editor feature used for flashcards is exposed in the guide UI merely because the shared storage/adapter supports structured content.

## Practice linked cards

When a section has linked cards, the end of its page says how many cards come from it and offers **Practice N cards**; the same command sits in the margin on a wide window and, while you read, in the bottom bar on a phone. The bar steps aside once the end of the section is on screen, where the page offers the cards and the next section itself. It starts an exact-card session for that topic. A section without linked cards says so instead. The application resolves current native card identities, not a fuzzy search or a temporary tag.

A topic can have no cards, or have cards that are currently unavailable because of lifecycle/scheduling state. Those are different from a failed projection request. Card provenance links are distinct from editable tags. Existing manually authored cards are not retroactively assigned to topics by guessing from their wording.

Ending the contextual session returns to the logical Study Guides context. Reviews update the same ordinary schedules and history as other practice. See [Practice and analytics](practice-and-analytics.md).

## Export a PDF

In a document guide, choose **Export as PDF…** from the **Study guide actions** menu; **Include my notes** controls annotation inclusion, and **Export PDF** prepares the file. A collection guide keeps its **Export PDF** button with **Include my notes in PDF**. Document and collection guides freeze the same effective semantic meaning used by the reader and render it locally; export does not require another provider request, re-interpret the source, or regenerate the guide.

The native PDF includes a designed cover and section hierarchy, linked table of contents, PDF bookmarks/named destinations, source labels, running headers/footers, semantic callouts/tables/examples/checks, repeated table headers across page breaks, and keep-with-next handling for wrapped headings. Verified canonical source images may be embedded with selectable captions when the generated visual explanation actually used that evidence. Text remains selectable/searchable rather than rasterizing the React view.

Common mathematical notation is laid out locally with a bounded non-executing PDF math subset, including fractions, roots, scripts, common Greek/operators/functions, and small matrix forms. If an expression is unsupported or cannot fit at the bounded readable size, the PDF preserves its original LaTeX source and asks for confirmation rather than executing TeX or silently producing a misleading formula. The in-app reader keeps its full local KaTeX presentation.

The normal prepared-file and platform save boundary controls the final destination. Inspect pagination, fonts, Unicode glyphs, figures, layout, and annotation inclusion before distributing the result. Structural/text tests qualify the document contract but are not a claim of pixel-perfect visual fidelity on every platform; rendered-page/accessibility review remains a separate human acceptance step.

## Regeneration, stopping, and failures

For an individual document, use its guide-generation action in Documents to regenerate. For a collection guide, use the folder's **Update study materials** workflow so the target reconciles its current exact source snapshot without creating a second guide identity. While document-guide work is active, the **Generating study guide…** button includes a red stop-sign icon; click that same button to stop. There is no separate permanent stop button in Documents. You can navigate to other sections while it runs and return to its current state. Cancellation is deliberate and cooperative; do not interpret the stop request as an instruction to delete the source or already committed learning artifacts.

A complete new generated revision is validated before atomic publication. Failed/canceled regeneration leaves the previous active guide available. Nonfatal recoveries remain in diagnostics; terminal failures remain visible after refresh or navigation. Compatible checkpoints may reduce repeated work; changes to source, backend, prompt, or schema can invalidate them. Restart recovery never silently authorizes a new provider transmission.

## Delete a guide without deleting its source

The library's delete action asks for confirmation. It removes generated guide revisions, guide-specific section overrides/conflicts, saved guide-generation checkpoints/jobs, and prepared PDF exports according to the native lifecycle.

For an individual-document guide, deletion preserves the source document, shared learning plan, linked flashcards, and stable guide/topic notes so a new guide can be generated later. For a collection target, deleting its guide records explicit suppression and does not delete the collection, member documents, ordinary target deck, learner schedules, or reviews; **Recreate study materials** is a new deliberate publication request. Neither action is the same as permanently deleting source material. Read the confirmation rather than assuming a generic trash icon always means the same operation.

## Protect and report

Whole-library backup/restore preserves the document, guide, notes, topic relationships, and required managed files together. Current trusted-device synchronization must not be assumed to carry the entire learning-artifact graph. See [Backups, imports, and sharing](backups-imports-and-sharing.md).

For a defect report, distinguish generated base content, learner notes, section overrides/conflicts, source citations, linked-card scope, generation cancellation, and export rendering. Include the exact build and safe reproduction; do not post private source text or credentials unnecessarily.
