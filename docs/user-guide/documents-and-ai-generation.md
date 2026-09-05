# Documents and AI-generated learning material

The **Documents** workspace keeps a managed local source and turns it into reusable flashcards or study guides. The two learning artifacts can be generated independently from the same retained source and shared learning plan. Generated cards remain ordinary editable study cards; generated guide content remains separate from learner-owned notes and edits.

## Before you begin

Use material you are allowed to process and choose a provider appropriate for its privacy requirements. Configure and test an implemented model profile in **Settings → AI/LLM**. The current flashcard-generation default also supplies the document learning workflows; there is not a separate required study-guide provider setup.

For a scanned/damaged PDF on desktop, install the required OCR runtime and trained data described in the platform guide. Android's bundled Latin text-recognition model does not require a separate Tesseract installation. A missing platform capability can make the Documents workspace unavailable rather than silently sending the original file elsewhere.

The original document is not uploaded to the model as an opaque file. ostudy stages a local copy, parses it, and sends bounded selected source context through the chosen provider. Selecting a local model never authorizes a cloud fallback.

## Import and generate

Open Documents and choose **Upload document**. Select a supported file; format detection is automatic rather than a parser selector. Once the source is retained, use its flashcard or **Generate study guide** action. Watch the displayed progress and use the active task's stop control when cancellation is needed.

Open generated cards from Flashcards and generated guides from **Study Guides**. There is no separate generated-card approval queue. Accepted model output must pass native validation before it becomes study content. A replacement study guide is published only after a complete validated revision is ready, so a failed regeneration does not discard the last active revision.

Review source-grounded output for factual errors, weak questions, duplicated concepts, and missing nuance. A source citation and valid schema are useful evidence, not proof of factual correctness.

## Supported source families

The current registry supports PDF; Word DOC/DOCX-family files; PowerPoint PPT/PPTX-family files; OpenDocument ODT/ODP/ODS; EPUB; RTF; Excel XLS/XLSX-family files; and CSV.

PDF uses the native PDF adapter. Supported non-PDF formats use **Xberg**, with immediate normalization into ostudy's own canonical representation. AnyDoc is a retired parser, not a selectable alternative or fallback.

Detection uses source content where a signature/container identifies the format. Legacy OLE formats additionally require an authorized DOC/PPT/XLS filename hint; CSV requires its filename hint because it lacks a unique binary signature. Renaming arbitrary bytes does not make them a supported format. HTML/web-page acquisition, audio transcription, and every format an underlying library might support are not automatically enabled product features.

## Local processing and source labels

The application stores a managed source copy and extracts bounded source units, semantic blocks, supported images, and chunks. Those derivatives support a reusable outline and learning plan. Stable learning-topic identities connect guide sections with cards generated for those topics.

PDF processing can extract native text, render relevant pages, and selectively recover text through OCR. Desktop uses its configured PDFium/Tesseract path; Android uses platform PdfRenderer and bundled ML Kit recognition. OCR is a recovery tool, not a guarantee that every diagram or table was understood.

Current non-PDF canonical provenance is persisted as **sections**. Xberg may expose richer internal slide/sheet boundaries and names, but the app does not promote all such details into exact authored page/slide/chapter promises without a coordinated locator/schema change. Non-PDF visual evidence is supported embedded images, not fabricated whole-slide renders. External image relationships are not fetched.

The guide/source action opens the retained original through the platform. It does not guarantee that an external viewer can jump to an exact page, slide, or section.

## What the provider receives

A request contains a bounded portion of canonical text, relevant outline/plan context, and selected source images only when the verified model supports image input. Large sources are processed through bounded requests rather than an unbounded prompt. Source analysis, topic planning, generation, validation, and curation are distinct stages.

The native provider gateway handles transport. Model output has no direct SQLite, filesystem, shell, or application-command authority. Generated content is checked for structure, limits, allowed rich content, tags, cloze grammar, and duplicates before durable use.

Diagnostics are designed to keep credentials, raw prompts/responses, reasoning text, source text, and image bytes out of ordinary logs. Still review any diagnostic export before sharing it, especially when it contains filenames or operational metadata.

## Progress and cancellation

Progress reports the phases the application can actually observe. Non-PDF native extraction may be indeterminate until the parser returns known source-unit totals. Do not interpret an indeterminate indicator as a stalled or failed task merely because a percentage is unavailable.

Stop controls request cancellation of their task. The current generating control can become the stop action rather than adding a second permanent button. Cancellation is cooperative: local extraction, network work, validation, and publication have explicit boundaries. Already committed reusable derivatives are retained, and a canceled guide regeneration leaves the previous published guide available.

On Android, a user-started long document task uses a foreground service, visible notification, and bounded wake-lock support so work can continue when the Activity is backgrounded. This is not immunity to process termination, resource pressure, device reboot, or explicit stop. Reopening relies on durable task/checkpoint reconciliation, not a surviving WebView promise.

On desktop, closing while document processing is active warns about interrupting in-flight work. Restart does not automatically authorize sending more source data to a provider. Retry deliberately after reviewing the state. Whole-library backup is blocked while active processing could make files and database snapshots incoherent.

## Retry, more cards, and regeneration

Retry reuses valid source/outline/generation checkpoints only when their versioned identities still match. Changed parser, prompt, schema, or source evidence can require rebuilding derivatives. A stale checkpoint is not trusted simply because it exists.

**Generate more cards** uses retained understanding and current deck coverage. An unchanged source should not be reparsed/rerendered simply to request additional cards. Changed embedded assets receive immutable versioned managed paths, avoiding deletion of the last committed evidence before a replacement succeeds.

Regenerate a guide from the relevant document action. The reader does not need a permanent regenerate button or internal revision badge. Compatible topic work can be reused; a complete new validated guide replaces the active generated revision atomically. Learner annotations follow the separate rules in [Study guides](study-guides.md).

## Source relationships and lifecycles

Generated notes can carry primary/supporting learning-topic provenance and citations. These are stable native relationships, not guesses inferred later from text or mutable tag labels. Renaming or moving a card does not turn a tag name into its source identity.

Archiving a document retains its source, derivatives, learning plan, guide/annotation graph, and generation history. Restoring it can return it to ready state when a live artifact still exists. A guide-only document is valid; it does not need an artificial flashcard deck.

Permanent source deletion is separate from deleting generated cards. It removes document-owned files and processing/artifact state when no active task conflicts. Already generated decks/cards remain ordinary study content, with text-only citation provenance where a live source link can no longer exist. Deleting a generated deck does not automatically delete its source document.

Deleting only a study guide has a narrower scope: it removes generated guide revisions, guide-specific edits/conflicts, checkpoints, and prepared exports while preserving the source, learning plan, linked cards, and stable guide/topic notes. Read the explicit confirmation.

## Backup, sync, and limitations

Whole-library backups include the coherent document/learning-artifact database graph and required managed sources/images/renders. Supported older backup representations remain readable. Provider credentials are excluded.

Do not assume trusted-device sync transfers every source document, study-guide revision, overlay, or topic relationship. Its supported merge graph is narrower than whole-library backup and requires a separate versioned extension for the new learning-artifact graph.

Malformed, encrypted/protected, unsupported, oversized, or poorly recognized sources can fail before generation. OCR quality, model reliability, extraction fidelity, document size, context limits, and provider cost remain practical limits. A local parse is not a full sandbox for hostile files; process only sources you intend to trust with the application.

Report a defect with the source format, phase, platform/build, provider family/model, safe error, and retry result. Distinguish extraction/OCR trouble from generated wording, citations, overlays, or PDF export. Use a synthetic or redacted source rather than publishing private material.

For implementation details, see [AI and document processing](../architecture/ai-and-document-processing.md), [document AI routing](../architecture/document-ai-routing.md), and the [current architecture](../architecture/overview.md).
