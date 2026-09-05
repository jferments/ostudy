# Study guides

A study guide is a source-grounded learning artifact built from a retained document's learning plan. It brings together a concise overview, learning objectives, key concepts, ordered topic sections, source labels, and links to relevant flashcards. It is not a replacement for checking the original source.

## Create and find a guide

Configure a suitable AI profile in Settings → AI/LLM, then open **Documents**, upload a supported source, and use **Generate study guide** beside it. A guide can exist without a flashcard deck; flashcards can also exist without a guide.

Open **Study Guides** from the main navigation. Its library supports **Search guides** by title, filename, or format; Newest/Oldest/Title sorting; Refresh; and pagination when needed. Library rows show summary information rather than loading every guide body just to list them.

Select a row or **Open** to read its guide. The reader is an inline workspace within the app shell, not a modal covering main navigation. **Close study guide** returns to the library. Selecting the main Study Guides destination also returns to that library rather than leaving you trapped in the current viewer.

The reader deliberately does not display internal revision numbers or a standing Regenerate button. Regeneration belongs to the source document's action in Documents.

## Read source-grounded sections

Use Contents to move through the overview and topic sections. Source labels identify relevant page or section ranges when available. **Open source** opens the retained original through the operating system; an exact deep-link into an external viewer is not guaranteed.

Generated sections favor a high-yield outline rather than a long restatement of the source. The native pipeline bounds section length and validates structure, but a model can still omit, misunderstand, or poorly prioritize information. Correctness requires learner review.

For current non-PDF ingestion, canonical provenance is expressed as sections even when the parser has richer internal slide or sheet information. Do not infer precise authored pagination from a generic section label. [Documents and AI generation](documents-and-ai-generation.md) explains source limitations.

## Add notes without losing the generated base

The guide keeps learner-owned information separate from generated revisions:

| Learner change | Ownership and behavior |
| --- | --- |
| Guide note | Belongs to the source document as a whole |
| Topic note | Belongs to a stable learning topic, not just its current display title |
| Generated-section edit | A non-destructive override tied to the generated base content it edits |

Use **Add guide note** or a topic's **Add note** to open the editor, then **Save**. Existing notes offer edit/remove actions. Clearing an existing note and saving removes it through the same deliberate ownership boundary. Section edits can be reset without deleting the generated source-grounded base.

If regenerated content changes enough that an old section override cannot safely apply, the old edit becomes a visible conflict instead of being silently attached to different material. Review the preserved text and discard an older edit only when that is what you intend. Stable guide/topic notes survive ordinary regeneration.

The current reader's note and section-edit entry is plain-text editing backed by validated rich-document storage. Do not assume every rich editor feature used for flashcards is exposed in the guide UI merely because the shared storage/adapter supports structured content.

## Practice linked cards

When a topic has linked studyable cards, **Study linked cards** starts an exact-card session for that topic. The application resolves current native card identities, not a fuzzy search or a temporary tag.

A topic can have no cards, or have cards that are currently unavailable because of lifecycle/scheduling state. Those are different from a failed projection request. Card provenance links are distinct from editable tags. Existing manually authored cards are not retroactively assigned to topics by guessing from their wording.

Ending the contextual session returns to the logical Study Guides context. Reviews update the same ordinary schedules and history as other practice. See [Practice and analytics](practice-and-analytics.md).

## Export a PDF

Use **Export PDF** in the reader. **Include my notes in PDF** controls annotation inclusion. The application freezes an effective guide snapshot and renders a local selectable-text PDF; export does not require another provider request or regenerate the guide.

The normal prepared-file and platform save boundary controls the final destination. Inspect pagination, fonts, Unicode glyphs, layout, and annotation inclusion before distributing the result. Automated structural/text tests are not a guarantee of perfect visual fidelity for every document or device.

## Regeneration, stopping, and failures

Use the guide-generation action for the source in Documents to regenerate. In an initial-generation reader, the generating control becomes a stop control while work is active. Cancellation is deliberate and cooperative; do not interpret the stop request as an instruction to delete the source or already committed learning artifacts.

A complete new generated revision is validated before atomic publication. Failed/canceled regeneration leaves the previous active guide available. Compatible checkpoints may reduce repeated work; changes to source, backend, prompt, or schema can invalidate them. Restart recovery never silently authorizes a new provider transmission.

## Delete a guide without deleting its source

The library's delete action asks for confirmation. It removes generated guide revisions, guide-specific section overrides/conflicts, saved guide-generation checkpoints/jobs, and prepared PDF exports according to the native lifecycle.

It preserves the source document, shared learning plan, linked flashcards, and stable guide/topic notes so a new guide can be generated later. This is not the same as permanently deleting the source document, which has a broader document-owned cleanup scope. Read the confirmation rather than assuming a generic trash icon always means the same operation.

## Protect and report

Whole-library backup/restore preserves the document, guide, notes, topic relationships, and required managed files together. Current trusted-device synchronization must not be assumed to carry the entire learning-artifact graph. See [Backups, imports, and sharing](backups-imports-and-sharing.md).

For a defect report, distinguish generated base content, learner notes, section overrides/conflicts, source citations, linked-card scope, generation cancellation, and export rendering. Include the exact build and safe reproduction; do not post private source text or credentials unnecessarily.
