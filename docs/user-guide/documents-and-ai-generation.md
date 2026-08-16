# Documents and AI-generated flashcards

The Documents workspace turns a retained local source into an ordinary ostudy deck. PDF support is functional but still benefits from review: unusual fonts, scanned pages, very large files, provider differences, and imperfect OCR or model output can expose source-specific failures that ordinary flashcard authoring does not encounter.

Use this feature as assisted authoring. Generated cards should still be reviewed through normal deck management, corrected when necessary, and deleted when they are not useful.

## Before importing a document

Confirm that:

- the PDF is material you are allowed to process;
- the selected AI profile verifies successfully;
- a cloud provider is appropriate for the document's privacy requirements;
- the model has enough context and structured-output capability for the task;
- on desktop, Tesseract and the needed trained data are available when scanned or corrupted text may require OCR;
- on Android, no separate OCR installation is required because ML Kit Text Recognition v2 is bundled with the app;
- a current library backup exists before testing an unfamiliar development build.

The original PDF is not sent to the provider as an opaque uploaded file. It is copied into the managed local library and processed locally before bounded source context is sent through the selected profile.

## Import and generation workflow

1. Open **Documents** and select a PDF.
2. Choose the configured provider profile and generation options.
3. Start generation.
4. Watch progress or use **Stop generation** to cancel that document deliberately.
5. When generation succeeds, open the created deck and manage its cards like any other deck.

There is no separate approval queue. Valid cards are written transactionally after the generation and curation passes complete. If a card is wrong, redundant, or poorly phrased, edit or delete it through the ordinary card and deck interfaces.

## What happens locally

Before contacting a model, ostudy:

- stores an immutable managed copy of the PDF;
- extracts page text and document structure;
- checks whether the extracted text is trustworthy;
- renders pages that require visual recovery;
- runs OCR only on pages whose native text cannot be trusted;
- organizes the source into bounded semantic chunks and a reusable document outline;
- records progress and recoverable checkpoints in the local library.

Desktop builds use the qualified PDFium rendering/Tesseract OCR path. Android uses the platform `PdfRenderer` for bounded page rasterization and bundled ML Kit Text Recognition v2 for selective OCR. The shared Rust pipeline decides which pages need rendering or OCR and retains ownership of limits, cancellation, checkpoints, generation, and SQLite transactions.

These details matter because a PDF's visible page and its embedded text layer can disagree. A page may look correct while its font mapping produces unusable characters. Rendering and OCR provide a visual recovery path instead of sending corrupt source text to the model.

## What is sent to the provider

Each request contains a bounded portion of the structured source, relevant outline context, and selected rendered page images only when the verified model supports image input. Large documents are handled through more bounded requests rather than one unbounded prompt.

ostudy does not silently change providers, and it does not log credentials, authorization headers, raw prompts, raw responses, reasoning content, or source text in diagnostic exports.

## Generated card forms

The document task may produce:

- conceptual Basic questions;
- definitions;
- multiple-choice questions represented as safe Basic cards with an explanation;
- application or scenario questions;
- Fill in the blank cards.

All accepted output passes through the same native content, tag, duplicate, cloze, and scheduling validation used by ordinary authoring. Model output has no direct database access.

## Progress, stopping, backgrounding, and restart

Document records show the major processing and generation phases, page and OCR progress where available, and safe error information. **Stop generation** cancels the active task for that document without deleting already committed reusable source derivatives.

On Android, a user-started long-running document task acquires a foreground service with a visible processing notification and a bounded partial wake lock. The Activity can move to the background while native rendering, selective ML Kit OCR, normalization, provider work, and durable commits continue.

Android can still terminate the whole application process under exceptional resource pressure or after a restart. ostudy does not pretend process death is transparent: durable document/job checkpoints remain in SQLite so reopening the app can resume or retry from saved state rather than starting from an untracked WebView promise.

On desktop, attempting to close the main window while document work is active warns that the in-flight request will be interrupted. Closing leaves durable state that startup reconciliation can mark retryable. Provider transmission is never resumed automatically after restart.

Complete-library backup is blocked while a document task is active so that the SQLite snapshot and managed files cannot be captured at different points in the operation.

## Retry and generate more

**Retry** should reuse valid extraction, OCR, outline, and completed generation checkpoints when their versioned contracts still match. It rebuilds stale or invalid derivatives instead of trusting data created by an older backend contract.

**Generate more cards** uses the retained document understanding and current deck coverage to request additional material. It should not rerender or OCR an unchanged source merely because more cards are requested.

A failed generation remains in the document library so that its error and retry state are not lost. Retrying the same failure repeatedly without correcting the provider, OCR, source, or application defect is unlikely to help.

## Archiving and deleting documents

Archiving a document is reversible and retains the source, derivatives, outline, generation history, and deck provenance.

Permanent document deletion removes the managed PDF and document-owned processing state when no task is active. Decks already created from that document remain ordinary study content. Their source relationship is detached rather than deleting useful cards unexpectedly.

Deleting a generated deck does not automatically delete its source document. The deck and document have separate lifecycles.

## Current limitations

- PDF is the active document source format. Other source adapters are planned, not implemented.
- Desktop OCR currently expects Tesseract and appropriate trained data.
- Android document OCR currently uses the bundled ML Kit Latin text recognizer; additional script-specific models are not yet packaged.
- OCR quality depends on scan resolution, layout, language, and source quality.
- Some PDFs with unusual encryption, malformed objects, complex scripts, damaged fonts, or extreme page dimensions may fail extraction or rendering.
- Local model quality and structured-output reliability vary substantially by model and server configuration.
- DeepSeek and llama.cpp/Unsloth are the currently implemented provider adapters; the other catalog entries remain planned.
- A model can return a structurally valid but educationally weak card. Native validation prevents unsafe or malformed writes, not every factual or pedagogical mistake.
- Large documents can require many bounded provider requests and substantial local processing time; a desktop may still be the more practical device for exceptionally large jobs.

When reporting a defect, include the phase shown by the application, whether the PDF was text-based or scanned, the provider family and model identifier, whether OCR was used, the platform, a redacted error message, and whether a retry reproduced the result. Do not attach private source material or credentials to a public issue.

The implementation design is documented in [AI and document processing](../architecture/ai-and-document-processing.md). Focused acceptance plans live under [`../development/`](../development/).
