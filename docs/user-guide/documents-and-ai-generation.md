# Documents, study guides, and AI-generated flashcards

The Documents workspace turns a retained local source into reusable learning
artifacts. It supports PDF plus Word, PowerPoint, OpenDocument, EPUB, RTF, Excel,
ODS, and CSV sources. From one managed source, oStudy can create an interactive
study guide, ordinary flashcards, or both. Source parsing and AI generation are
assisted-authoring tools: unusual formatting, malformed files, OCR, provider
differences, and imperfect model output can still require review.

Generated cards use the normal deck/card workflow. Edit inaccurate cards and
delete material that is redundant or not useful. Study guides keep generated
source-grounded content separate from learner-owned notes and edits so a later
regeneration cannot silently overwrite your annotations.

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
3. Use the actions beside that document to create the learning artifact you want.
   **Generate study guide** creates or regenerates the document's guide;
   flashcard generation creates or extends its ordinary study deck. The two
   artifacts share the same source-grounded learning plan but can be generated
   independently.
4. Watch progress or use the relevant stop action to cancel the active document
   task deliberately.
5. Open **Study Guides** to read a generated guide, annotate or edit it,
   export it, or start topic-linked card practice. Open a generated deck to manage
   its cards like any other deck.

There is no separate approval queue. Valid cards are written transactionally
after generation and curation complete. A study guide is published only after a
complete validated revision is ready, so a failed regeneration does not replace
the last active guide.

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
- builds a reusable document outline and source-grounded learning plan;
- gives learnable topics stable local identities with source locators and evidence
  fingerprints;
- uses that shared learning plan to organize study-guide sections and link newly
  generated flashcards to the topics they teach;
- records safe progress, checkpoints, and failure state in the local library.

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
outline/learning-plan context, and selected canonical images only when the
verified model supports image input. Large documents are handled through multiple
bounded requests instead of one unbounded prompt.

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

When reliable source provenance is available, newly generated notes are linked to
one primary learning topic and may also have supporting topic relationships. The
relationship is native provenance, not an editable tag. Moving or renaming the
card does not turn a tag label into its source identity, and existing cards are
not assigned topics by guessing from their text.

## Interactive study guides

A study guide is a durable, source-grounded **high-yield outline** over the
document's shared learning plan. The guide begins with the document-level
overview, learning objectives, and key concepts, then presents ordered topic
sections with natural source labels such as page or section ranges. Newly
generated topic sections deliberately favor a one- or two-sentence overview and
roughly three to seven prioritized key points over long narrative prose. The
provider receives a much smaller completion budget than the source-analysis
passes, and native validation rejects overlong section fields or total section
content so the single repair attempt has to condense rather than merely rephrase
an essay. Mechanism explanations, terms, processes, comparisons,
misconceptions, and visual explanations are included only when they add distinct
learning value rather than repeating the source.

Open generated guides from the top-level **Study Guides** destination. Use the
table of contents to move through a guide and **Open source** to open the managed
original document.

The guide stores generated base content as immutable revisions. Learner-owned
changes are separate overlays:

- **guide notes** belong to the document as a whole;
- **topic notes** belong to a stable learning topic and therefore survive ordinary
  guide regeneration;
- **section edits** are non-destructive overrides tied to the exact generated base
  content they edited.

If regeneration changes the underlying generated section enough that an old
section override can no longer be applied safely, oStudy preserves the old edit
as an explicit conflict for review rather than silently attaching it to different
content. The generated source-grounded revision remains recoverable underneath
the learner overlay.

Each section also shows the count of linked cards when topic-linked flashcards are
available. **Study linked cards** starts practice with the exact card IDs currently
linked to that topic; it does not create a temporary tag-based approximation. A
study guide can exist without a generated flashcard deck, and a document with a
published guide remains a valid ready document.

### PDF export

Choose **Export PDF** from an open study guide to produce a local selectable-text
PDF. The export is rendered locally from a frozen effective guide snapshot; it
does not send another provider request and does not mutate the guide lifecycle.
You can choose whether learner annotations are included. The resulting file uses
the normal prepared-export/platform save boundary, so the operating system owns
the final save/share destination.

The automated export contract verifies selectable Unicode text. Pagination, font
appearance, complex source-specific typography, and final visual fidelity should
still be inspected on the target device before relying on an exported guide for
high-stakes distribution.

## Progress, stopping, backgrounding, and restart

Document records show the major processing/generation phases, canonical source
unit progress, request/token activity, and safe error information. PDF records
also show page rendering/OCR progress when those stages are applicable.

**Stop generation** cancels flashcard work. A running guide exposes its own
**Stop study guide** action beside the document. Either cancellation leaves
already committed reusable source derivatives intact. A canceled or failed guide
regeneration leaves any previously published guide available, and valid per-topic
checkpoints may be reused on retry when their versioned contract still matches.

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

Click **Generate study guide** again beside a document to regenerate its guide.
Regeneration reuses the shared outline/learning plan and compatible topic
checkpoints, while prompt/schema version changes intentionally invalidate stale
AI derivatives. Publication is atomic: incomplete replacement content cannot
supersede the active guide.

A failed generation remains in the document library so its error and retry state
are not lost.

## Archiving, deleting, backup, and restore

Archiving is reversible and retains the source, canonical derivatives, learning
plan/topics, study-guide revisions and learner overlays, generation history, and
deck provenance. Restoring an archived document returns it to ready state when a
live learning artifact still exists.

Permanent document deletion removes the managed source and document-owned
processing/learning-artifact graph when no task is active. Generated decks/cards
that already exist remain ordinary study content; their source relationship is
preserved as a text citation instead of a live file link where applicable.

Deleting a generated deck does not automatically delete its source document. The
deck and document have separate lifecycles. A guide-only document therefore does
not need a generated deck merely to remain usable.

Complete-library backups include the coherent SQLite snapshot containing learning
plans, topics, guides, overlays, and note-topic provenance, together with generic
managed document sources, PDF page renders, and retained canonical source images.
Restore reinstalls those relationships and managed files together. Older backups
that stored document sources as PDF-specific rows remain compatible under their
supported backup versions.

## Current limitations

- The guide retains exact source-unit locators and displays natural page/slide/
  chapter/section labels, but the generic cross-platform **Open source** boundary
  opens the canonical file rather than guaranteeing an exact deep-link to that
  page, slide, or section in an external application.
- AnyDoc-backed presentations and EPUB files currently use deterministic section
  provenance rather than claiming exact authored slide/chapter boundaries when
  the parser cannot prove those boundaries.
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
- Native study-guide generation now enforces a concise outline shape and bounded
  section length, but a model can still produce factually weak or poorly
  prioritized content that requires learner review.
- Very large documents can require many bounded provider requests and substantial
  local processing time.
- Study-guide/topic state is included in complete-library backup/restore. Peer
  device synchronization of the new learning-artifact graph is not part of the
  current study-guide release and requires a separate versioned sync change.

When reporting a defect, include the displayed phase, source format, provider
family/model, platform, a redacted safe error message, and whether retry
reproduced the result. For PDFs, also note whether OCR was used. For study-guide
issues, note whether the problem is generated base content, a learner overlay, a
topic-card link, or PDF export. Do not attach private source material or
credentials to a public issue.

The implementation design is documented in [AI and document
processing](../architecture/ai-and-document-processing.md), [the AnyDoc adapter
contract](../architecture/anydoc-adapter-implementation.md), and [ADR 0043:
study guides and topic-linked learning](../decisions/0043-study-guides-and-topic-linked-learning.md).
