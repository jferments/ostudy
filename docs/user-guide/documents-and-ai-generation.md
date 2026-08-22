# Documents and AI-generated flashcards

The Documents workspace turns a retained local source into an ordinary oStudy
deck. It supports PDF plus Word, PowerPoint, OpenDocument, EPUB, RTF, Excel, ODS,
and CSV sources. Source parsing and AI generation are assisted-authoring tools:
unusual formatting, malformed files, OCR, provider differences, and imperfect
model output can still require review.

Generated cards use the normal deck/card workflow. Edit inaccurate cards and
delete material that is redundant or not useful.

## Before importing a document

Confirm that:

- the source is material you are allowed to process;
- the selected AI profile verifies successfully;
- a cloud provider is appropriate for the source's privacy requirements;
- the model has enough context and structured-output capability for the task;
- on desktop, Tesseract and the needed trained data are available when a PDF may
  require OCR;
- on Android, no separate PDF OCR installation is required because ML Kit Text
  Recognition v2 is bundled with the app;
- a current library backup exists before testing an unfamiliar development
  build.

The original document is not sent to the provider as an opaque uploaded file.
oStudy copies it into the managed local library and parses it locally before
bounded source context is sent through the selected profile.

## Import and generation workflow

1. Open **Documents** and choose **Upload document**.
2. Select a supported file. There is no parser/type selector; oStudy detects the
   source automatically.
3. Start generation with the configured AI profile/options.
4. Watch progress or use **Stop generation** to cancel that document deliberately.
5. When generation succeeds, open the created deck and manage its cards like any
   other deck.

There is no separate approval queue. Valid cards are written transactionally
after generation and curation complete.

## Supported source families

Current document sources include:

- PDF;
- Word (`.doc` and DOCX-family files);
- PowerPoint (`.ppt` and PPTX-family files);
- OpenDocument text/presentation/spreadsheet (`.odt`, `.odp`, `.ods`);
- EPUB;
- RTF;
- Excel (`.xls` and XLSX-family files);
- CSV.

Signature-bearing formats are detected from their content rather than trusted
from a renamed extension. CSV is the deliberate exception because it has no
unique binary signature; a `.csv` filename is required for CSV parsing.

PDF continues through oStudy's native PDF adapter. The other formats use a pinned
in-process AnyDoc parser and are normalized immediately into the same oStudy-owned
canonical document model consumed by AI generation.

## What happens locally

For every source, oStudy:

- stores an immutable managed copy;
- detects and records the document format;
- extracts bounded canonical source units, semantic blocks, and chunks;
- retains supported embedded source images as multimodal evidence when available;
- builds a reusable document outline and versioned generation checkpoints;
- records safe progress and failure state in the local library.

For non-PDF AnyDoc sources, current public provenance is expressed as
deterministic **sections**. oStudy does not label inferred boundaries as exact
slides or chapters when the parser's generic model cannot prove those authored
boundaries. Presentation and EPUB imports can therefore be marked as having
inferred/degraded section segmentation while still preserving their structured
content.

PDF has additional local processing. oStudy extracts page text, renders pages
when visual recovery is needed, and selectively runs OCR on pages whose native
text cannot be trusted. Desktop uses the qualified PDFium/Tesseract path; Android
uses the platform `PdfRenderer` and bundled ML Kit Text Recognition v2.

These PDF-specific details matter because a visible page and its embedded text
layer can disagree. They are not applied to Word, PowerPoint, EPUB, or spreadsheet
sources.

## What is sent to the provider

Each request contains a bounded portion of canonical source content, relevant
outline context, and selected canonical images only when the verified model
supports image input. Large documents are handled through multiple bounded
requests instead of one unbounded prompt.

oStudy does not silently switch providers, and diagnostics do not log
credentials, authorization headers, raw prompts, raw responses, reasoning
content, source text, or source image bytes.

## Generated card forms

The document task may produce:

- conceptual Basic questions;
- definitions;
- multiple-choice questions represented as safe Basic cards with an explanation;
- application or scenario questions;
- Fill in the blank cards.

Accepted output passes through the same native content, tag, duplicate, cloze,
and scheduling validation used by ordinary authoring. Model output has no direct
database access.

## Progress, stopping, backgrounding, and restart

Document records show the major processing/generation phases, canonical source
unit progress, request/token activity, and safe error information. PDF records
also show page rendering/OCR progress when those stages are applicable.

**Stop generation** cancels the active task without deleting already committed
reusable source derivatives.

On Android, a user-started long-running document task acquires a foreground
service with a visible processing notification and a bounded partial wake lock.
The Activity may move to the background while local parsing/rendering/OCR,
provider work, and durable commits continue.

Android can still terminate the entire process under exceptional resource
pressure or after a restart. Durable document/job checkpoints remain in SQLite
so reopening the app can resume or retry from saved state rather than relying on
an untracked WebView promise.

On desktop, closing the main window while document work is active warns that the
in-flight request will be interrupted. Startup reconciliation can mark the
durable state retryable. Provider transmission is never resumed automatically
after restart.

Complete-library backup is blocked while document work is active so the SQLite
snapshot and managed source files cannot be captured at different points in the
operation.

## Retry and Generate More

**Retry generation** reuses valid extraction/outline/generation checkpoints when
their versioned contracts still match. It rebuilds stale or invalid derivatives
rather than trusting output from a changed backend contract.

For AnyDoc embedded images, retries write changed image content to immutable,
versioned managed paths; a failed retry does not delete the last committed visual
evidence first.

**Generate more cards** uses retained document understanding and current deck
coverage to request additional material. An unchanged source should not be
reparsed or rerendered merely because more cards are requested.

A failed generation remains in the document library so its error and retry state
are not lost.

## Archiving, deleting, backup, and restore

Archiving is reversible and retains the source, canonical derivatives, outline,
generation history, and deck provenance.

Permanent document deletion removes the managed source and document-owned
processing state when no task is active. Decks already created from that
document remain ordinary study content; their source relationship is preserved
as a text citation instead of a live file link where applicable.

Deleting a generated deck does not automatically delete its source document. The
deck and document have separate lifecycles.

Complete-library backups include generic managed document sources, PDF page
renders, and retained canonical source images. Restore remains compatible with
older backups that stored document sources as PDF-specific rows.

## Current limitations

- AnyDoc-backed presentations and EPUB files currently use deterministic section
  provenance rather than claiming exact authored slide/chapter boundaries.
- Whole-slide rendering is not fabricated; non-PDF multimodal evidence currently
  comes from embedded images exposed by the parser.
- External images/relationships in packaged documents are not fetched.
- Desktop PDF OCR currently expects Tesseract and appropriate trained data.
- Android PDF OCR currently uses the bundled ML Kit Latin text recognizer;
  additional script-specific models are not yet packaged.
- OCR quality depends on scan resolution, layout, language, and source quality.
- Encrypted/protected, malformed, unsupported, or resource-limit-exceeding source
  files may fail locally before AI generation.
- Local-model quality and structured-output reliability vary by model/server.
- A model can return structurally valid but educationally weak material. Native
  validation prevents malformed writes, not every factual or pedagogical error.
- Very large documents can require many bounded provider requests and substantial
  local processing time.

When reporting a defect, include the displayed phase, source format, provider
family/model, platform, a redacted safe error message, and whether retry
reproduced the result. For PDFs, also note whether OCR was used. Do not attach
private source material or credentials to a public issue.

The implementation design is documented in [AI and document
processing](../architecture/ai-and-document-processing.md) and [the AnyDoc
adapter contract](../architecture/anydoc-adapter-implementation.md).