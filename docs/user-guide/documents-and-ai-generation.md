# Documents and AI-generated learning material

Library's **Documents** view keeps a managed local source and turns it into reusable flashcards or study guides. The two learning artifacts can be generated independently from the same retained source and shared learning plan. Generated cards remain ordinary editable study cards; generated guide content remains separate from learner-owned notes and edits.

## Before you begin

Use material you are allowed to process and choose a provider appropriate for its privacy requirements. Configure and test an implemented model profile in **Settings → AI/LLM**. The current flashcard-generation default also supplies the document learning workflows; there is not a separate required study-guide provider setup.

For a scanned/damaged PDF on desktop, install the required OCR runtime and trained data described in the platform guide. Android packages an English Tesseract model with the application and does not require a separate OCR installation or runtime model download. A missing platform capability can make the Documents workspace unavailable rather than silently sending the original file elsewhere.

The original document is not uploaded to the model as an opaque file. ostudy stages a local copy, parses it, and sends bounded selected source context through the chosen provider. Selecting a local model never authorizes a cloud fallback.

## Import and generate

Open Documents and choose **Upload document**. Before starting, you may expand **Instructions for new document flashcards** and describe the focus, audience, or question style you want, for example “create cards for each cranial nerve and emphasize nursing assessment tests.” The instructions affect flashcard authoring only; they are not treated as source evidence and do not change Study Guide content. The current run captures a normalized copy before provider work starts, so editing the field later cannot mutate an in-flight request and an ambiguous retry reuses the same captured intent.

Select a supported file; format detection is automatic rather than a parser selector. After local/source understanding and the shared learning plan are ready, initial **Flashcards** and **Study Guide** authoring start as sibling tasks instead of waiting on one another. Documents shows separate vertically stacked progress bars backed by committed flashcard windows and durable Study Guide sections. Either artifact can finish first; completion is shown independently. Navigating away does not cancel the native work.

### Upload an image collection

Choose **Upload images**, then **Select images** to select several screenshots or photos at once. Use the **+** button to add files from another location, drag files onto the area below the file list, press **Ctrl+V** while the paste area is focused, or use **Paste images** to read image data from the clipboard. All of those paths add to the same collection, including repeated screenshots pasted one after another. Pasting text or a URL does not fetch an image from the web.

Image collections also expose **Flashcard instructions (optional)**. The same per-run rules apply as for documents: the prompt is captured with the generation request, does not become source evidence, and can guide focus or question style without bypassing native validation.

Processing starts automatically. ostudy retains each original, reads the images locally, resolves verified overlap across the collection, and uses the existing document-learning pipeline to create flashcards and a study guide. You do not need to order images, crop them, label text regions or run a preparation step. Upload order is not used as proof that two passages are equivalent; changed numbers, units, qualifications or source pixels are not silently merged.

You can keep adding images while processing is running. They wait for the current task, then extend the same source and generated deck rather than creating a second deck. Leaving the collection page does not cancel the work. **Stop** stops the active task and prevents the next automatic stage. A genuine failure shows **Retry processing**; successfully saved sources and cards remain available. Failed files can be removed without uploading the successful selections again.

Use **View collection** on an existing image source to add more images or optionally inspect a retained original. Inspection is not required for generation. Previously saved source/filter state remains compatible with complete-library backup and restore; removing the old preparation controls does not alter original image bytes.

Open generated cards from **Library → Decks** and generated guides from **Library → Study guides**. There is no separate generated-card approval queue. Accepted model output must pass native validation before it becomes study content. A replacement study guide is published only after a complete validated revision is ready, so a failed regeneration does not discard the last active revision.

Document flashcard generation can create Basic/Fill in the blank recall plus single-best-answer, select-all, true/false, matching, grouping, and ordering questions when retained source evidence supports the required relationship. Image occlusion remains manually authored; document generation does not invent masks. The type mix is a soft planning target, not a quota: allowed types, evidence fit, topic/component coverage and compound-workload limits outrank percentages. If the source does not support a requested interaction, oStudy uses another supported opportunity or returns fewer cards instead of manufacturing filler.

Review source-grounded output for factual errors, weak questions, duplicated concepts, and missing nuance. A source citation and valid schema are useful evidence, not proof of factual correctness.

### Document collections and coherent study materials

A document collection is a **subject**: one grouping for a course's documents, guides and decks. The folders in **Library → Decks** are the same subjects, so a folder you create there also appears here as a collection, and the other way round. Collections are a single level. Create a collection for a week, unit, exam, or other study set, then add existing documents with **Add source** or drag a document card onto the collection. Open a collection to see its sources and reading ranges.

A document is in at most one collection. Adding it to another collection moves it there with its reading ranges and its generated flashcard deck. Removing it from a collection returns the document and its generated deck to no collection and clears its reading ranges. A generated deck that you filed in a different folder yourself stays where you put it.

The **Study materials** panel is a local status read until you explicitly choose **Create study materials** or **Update study materials**. A collection's flashcards are the cards in the decks filed in it, studied live: **Study collection flashcards** practices those decks as they are when the session starts, and each card keeps its one schedule and history wherever you study it. Nothing is copied. If the collection has no cards yet, generate flashcards for its documents or move a deck into it. A ready Study Guide opens the shared semantic guide reader, and its local PDF export uses the same effective guide meaning rather than a second collection-specific renderer.

Adding an already-processed document to a collection does **not** parse, OCR, extract, or semantically interpret that document again. The collection builds a higher-level synthesis from the retained per-document knowledge: it can organize shared topics, relationships, emphasis, and explicit gaps, while exact factual support remains attached to the original document evidence. Collection Study Guides author readable narrative topic sections through the same validated rich-section contract used by document guides, selecting only retained knowledge units from the authorized collection. Internal coverage/gap receipts remain diagnostic metadata and are not learner-facing guide sections.

Deleting a collection takes its documents out of every collection and moves its decks to **Unsorted decks**; it does not delete them. The collection study guide is deleted with it. Deleting the collection guide on its own is explicit suppression, so an ambiguous retry cannot silently recreate it; choosing **Recreate study materials** is a new deliberate action.

Libraries from before the subjects update kept collection flashcards as copies in a separate deck. The update folds unedited copies back into their original cards; see [Library upgrades](library-upgrades.md).

## Supported source families

The current registry supports PDF; Word DOC/DOCX-family files; PowerPoint PPT/PPTX-family files; OpenDocument ODT/ODP/ODS; EPUB; RTF; Excel XLS/XLSX-family files; and CSV.

Image collections are a separate first-class Documents source workflow for supported PNG/JPEG/WebP screenshots and photos. They are not routed through Xberg and are not converted into one stitched image.

PDF uses the native PDF adapter. Supported non-PDF formats use **Xberg**, with immediate normalization into ostudy's own canonical representation. AnyDoc is a retired parser, not a selectable alternative or fallback.

Detection uses source content where a signature/container identifies the format. Legacy OLE formats additionally require an authorized DOC/PPT/XLS filename hint; CSV requires its filename hint because it lacks a unique binary signature. Renaming arbitrary bytes does not make them a supported format. HTML/web-page acquisition, audio transcription, and every format an underlying library might support are not automatically enabled product features.

## Local processing and source labels

The application stores a managed source copy and extracts bounded source units, semantic blocks, supported images, and chunks. Knowledge is interpreted once from those sources and saved for flashcards, study guides and Tutor. Changing the chat or guide model does not repeat that interpretation. Those derivatives support a reusable outline and learning plan. Stable learning-topic identities connect guide sections with cards generated for those topics.

PDF processing can extract native text, render relevant pages, and selectively recover text through OCR. Desktop uses its configured PDFium/Tesseract path; Android uses platform `PdfRenderer` and packaged open-source Tesseract. OCR is a recovery tool, not a guarantee that every diagram or table was understood.

Current non-PDF canonical provenance is persisted as **sections**. Xberg may expose richer internal slide/sheet boundaries and names, but the app does not promote all such details into exact authored page/slide/chapter promises without a coordinated locator/schema change. Non-PDF visual evidence is supported embedded images, not fabricated whole-slide renders. External image relationships are not fetched.

The guide/source action opens the retained original through the platform. It does not guarantee that an external viewer can jump to an exact page, slide, or section.

## What the provider receives

A request contains a bounded portion of canonical text, relevant outline/plan context, and selected source images only when the verified model supports image input. Large sources are processed through bounded requests rather than an unbounded prompt. Source analysis, topic planning, generation, validation, and curation are distinct stages. When an embedding model is configured, document processing also indexes saved knowledge for search. The embedding service receives saved text/knowledge, not a second request to interpret the document. Missing vectors can be indexed when you send a chat; ordinary navigation sends nothing.

Interactive generation is knowledge-first rather than prompt-only. Native assessment opportunities point to the retained knowledge/evidence components that support each answer, correction, pairing, grouping membership or ordering relation. False statements and distractors are not recorded as if the source asserted them; their correction/rationale carries the support. Single-best-answer output must have one supported best choice, not two merely plausible choices. Native admission rejects ambiguous or unsupported relation shapes before publication.

The native provider gateway handles transport. Model output has no direct SQLite, filesystem, shell, or application-command authority. Generated content is checked for structure, limits, allowed rich content, tags, cloze grammar, typed question semantics, evidence identities and duplicates before durable use.

Diagnostics are designed to keep credentials, raw prompts/responses, reasoning text, source text, answer keys and image bytes out of ordinary logs. Still review any diagnostic export before sharing it, especially when it contains filenames or operational metadata.

## Progress and cancellation

Progress reports the phases the application can actually observe. Non-PDF native extraction may be indeterminate until the parser returns known source-unit totals. Once shared understanding is complete, Flashcards progress uses committed generation windows and Study Guide progress uses durable completed sections; neither bar guesses from elapsed time. A green **Complete** state belongs to the individual artifact, so one can complete while the other remains active. Do not interpret an indeterminate indicator as a stalled or failed task merely because a percentage is unavailable.

Stop controls request cancellation of their task. Study guides use a single generating/stop button rather than a second permanent button. Failures remain visible when you return to Documents and are not cleared by routine refreshes. Cancellation is cooperative: local extraction, network work, validation, and publication have explicit boundaries. Already committed reusable derivatives are retained, and a canceled guide regeneration leaves the previous published guide available.

On Android, a user-started long document task uses a foreground service, visible notification, and bounded wake-lock support so work can continue when the Activity is backgrounded. This is not immunity to process termination, resource pressure, device reboot, or explicit stop. Reopening relies on durable task/checkpoint reconciliation, not a surviving WebView promise.

On desktop, closing while document processing is active warns about interrupting in-flight work. Restart does not automatically authorize sending more source data to a provider. Retry deliberately after reviewing the state. Whole-library backup is blocked while active processing could make files and database snapshots incoherent.

## Retry, more cards, and regeneration

Retry reuses valid source/outline/generation checkpoints only when their versioned identities still match. Current knowledge remains reusable across different consuming models. Changed parser, prompt, schema, generation policy or source evidence can require rebuilding derivatives. A stale checkpoint is not trusted simply because it exists. The live **Retries** diagnostic is recovery traffic, not a count of documents or failed pages: it distinguishes contract repair, output compaction, transient transport retry, and provider-compatibility retry. Successful admitted knowledge with merely unresolved internal coverage no longer automatically buys another provider request; paid repair is reserved for actual rejected proposals or a supplied source that received no admitted/non-learning disposition.

Card generation checkpoints are window-scoped. Completed source windows remain checkpointed after a later window/provider failure, so retry resumes the remaining compatible windows instead of discarding already admitted work. Generated result summaries report what was requested, admitted, rejected and covered without turning a soft type mix into a hard quota.

**Generate more cards** uses retained understanding and current deck/component coverage. An unchanged source should not be reparsed/rerendered simply to request additional cards. Changed embedded assets receive immutable versioned managed paths, avoiding deletion of the last committed evidence before a replacement succeeds.

Regenerate a guide from the relevant document action. The reader does not need a permanent regenerate button or internal revision badge. Compatible topic work can be reused; a complete new validated guide replaces the active generated revision atomically. Learner annotations follow the separate rules in [Study guides](study-guides.md).

## Analysis diagnostics

Debug logs can report unresolved knowledge proposals (model-proposed concepts or relationships that were not admitted), rejected assessment candidates, source dispositions (supplied material that still lacks an accepted analysis outcome), provider transport failures, and checkpoint reuse. It does not necessarily mean a false fact or an unprocessed page. Valid knowledge is saved and remains usable. Nonfatal analysis details are retained in diagnostics, not shown as ordinary error banners or warning text on successful generation. A true failure still explains why the requested operation could not finish. Complete protocol coverage likewise does not guarantee every important concept was found.

Older builds limited a page to 16 primary knowledge units. Saved proposals rejected only for that quota are revalidated locally on document retry without asking the model to generate them again. Other invalid or unsupported proposals remain pending. Retry document generation to repair remaining analysis; Tutor uses the saved valid knowledge and does not start a separate understanding stage.

## Source relationships and lifecycles

Generated notes can carry primary/supporting learning-topic provenance and citations. Interactive generated questions additionally retain private component-level generation evidence tied to the note revision and generation policy. These are stable native relationships, not guesses inferred later from text or mutable tag labels. Renaming or moving a card does not turn a tag name into its source identity.

Archiving a document retains its source, derivatives, learning plan, guide/annotation graph, and generation history. Restoring it can return it to ready state when a live artifact still exists. A guide-only document is valid; it does not need an artificial flashcard deck.

Permanent source deletion is separate from deleting generated cards. It removes document-owned files, page annotations and processing/artifact state when no active task conflicts. Already generated decks/cards remain ordinary study content, with text-only citation provenance where a live source link can no longer exist. Deleting a generated deck does not automatically delete its source document.

Deleting only a study guide has a narrower scope: it removes generated guide revisions, guide-specific edits/conflicts, checkpoints, and prepared exports while preserving the source, learning plan, linked cards, and stable guide/topic notes. Read the explicit confirmation.

## Backup, sync, and limitations

Whole-library backups include the coherent document/learning-artifact database graph and required managed sources/images/renders, including ordered image-set manifests, each retained original image, pending image-source snapshots, Content-filter decisions, typed question definitions, retained attempt/review evidence and generation evidence. Supported older backup representations remain readable. Provider credentials are excluded.

Trusted-device synchronization has explicit sender-selected scopes for Flashcards, Documents, Study Guides and provider settings/credentials. Study Guides require Documents; cross-document/card provenance is included only when both domains are selected. Typed interactive questions and their admitted retained evidence are supported within the Flashcards graph. A selected sync scope is still not a disaster-recovery substitute for the complete-library backup.

Malformed, encrypted/protected, unsupported, oversized, or poorly recognized sources can fail before generation. OCR quality, model reliability, extraction fidelity, document size, context limits, and provider cost remain practical limits. A local parse is not a full sandbox for hostile files; process only sources you intend to trust with the application.

Report a defect with the source format, phase, platform/build, provider family/model, safe error, and retry result. Distinguish extraction/OCR trouble from provider request rejection, generated wording, citations, overlays, or PDF export. Use a synthetic or redacted source rather than publishing private material.

For implementation details, see [AI and document processing](../architecture/ai-and-document-processing.md), [document AI routing](../architecture/document-ai-routing.md), and the [current architecture](../architecture/overview.md).
