# AI and document processing

ostudy's document system has four distinct responsibilities: retain and normalize a source; derive reusable learning structure; request bounded provider-assisted content; and validate/publish ordinary learning artifacts. The UI initiates tasks but does not own parser policy, provider transport, credentials, or the database graph.

[The comprehensive architecture](overview.md) gives the system context. [Document AI routing](document-ai-routing.md) covers task contracts/concurrency. [AI infrastructure](ai-infrastructure.md) owns profiles, probes, and transport. [The user guide](../user-guide/documents-and-ai-generation.md) describes the visible workflow.

## Source and package ownership

| Owner | Responsibility |
| --- | --- |
| `documents/` plus current lifecycle/repository service modules | Source metadata, managed source ownership, admission and lifecycle coordination |
| `document_ingestion/` | Application adapter contract, explicit registry/detection, canonicalization and persistence boundary |
| `pdf_backend/` | Semantic/native PDF text extraction and quality/recovery |
| `pdf_document_ingestion/` | PDF pipeline, bounded rendering/OCR preparation, progress and recovery |
| `xberg_document_adapter/` | Isolated non-PDF backend invocation and immediate conversion to application-owned structures |
| `document_canonical*`, `document_store`, `document_semantics` | Canonical units/blocks/chunks/assets and their coherent persisted projections |
| `document_learning*`, `document_topic_links`, `document_card_topics` | Shared learning plan/topics and stable links to generated material |
| `ai/document_intelligence/` | Provider-neutral bounded task requests and domain interpretation |
| `document_generation`, `document_initial_generation`, `document_more`, `document_checkpoints` | Task execution, reuse, validation, and ordinary flashcard publication |
| `document_study_guide*` | Generated revisions, learner overlays, library queries, lifecycle, and local PDF export |

Several root `document_*` modules remain active owners behind focused facades. The facade is not evidence that those modules are dead. Cross-module services exchange application-owned contracts; third-party types must not spread into authoring, profiles, or UI.

## Managed source lifecycle

External files first enter app-private staging through an opaque file handle. Native code verifies transfer purpose, size/hash and content, then retains a managed source copy. User-selected external paths/content URIs are not permanent domain identity.

A source can exist independently of a generated deck or guide. Ingestion persists reusable canonical evidence and a learning plan; flashcard and guide generation consume that evidence separately. A guide-only source is valid. Deleting a generated deck must not implicitly delete its document; deleting a source must not silently erase unrelated ordinary flashcards.

Archiving retains source, derivatives, learning graph, and generation history. Permanent source deletion checks task admission, stages the source directory, removes the document-owned graph transactionally, restores staged bytes after failed persistence, and purges residue after success. Already generated surviving cards retain detached readable provenance where their original source is gone.

## Explicit format registry

PDF is routed to the native PDF adapter. Current Word, PowerPoint, OpenDocument, EPUB, RTF, Excel, and CSV source families are routed to **Xberg**. Supported extensions/container variants are defined by the registry/detector; an upstream parser's entire feature list is not automatically a supported ostudy format.

AnyDoc is retired runtime history. Its old persisted backend identities, numbered migration names, and useful fixture provenance remain readable historical evidence, not a second selectable parser or fallback. [ADR 0045](../decisions/0045-xberg-non-pdf-document-adapter.md) supersedes the previous backend choice.

Xberg is pinned with only the reviewed runtime/office/excel features. Its PDF/OCR/layout/network-acquisition/transcription/embedding/server paths are not part of this integration. Third-party parsing results are normalized immediately into ostudy-owned canonical objects.

Detection primarily uses source signatures and container structure. OLE needs a valid DOC/PPT/XLS filename hint to disambiguate supported families; CSV requires its filename hint because it lacks a unique signature. Renaming unknown bytes is not authorization to treat them as a supported source. HTML acquisition and arbitrary external links are not a side effect of document import.

## Canonical document evidence

Canonical representation preserves source units/locators, structured text blocks, bounded chunks, relevant metadata, embedded-image identities, and backend/source/schema provenance. It allows document reasoning to ignore parser-specific types and allows a backend replacement to invalidate only rebuildable derivatives rather than learner notes or canonical card identities.

Current non-PDF persisted locators remain **Section** based. Richer internal slide/sheet divisions from Xberg are not promoted into a new authored locator contract without coordinated schema, migration, citation, backup, and UI changes. Do not advertise exact slide/page/chapter deep links from generic section labels.

Non-PDF visual evidence is actual supported embedded images, not imaginary whole-slide rendering. External image relationships are not fetched. The adapter applies bounded per-image/aggregate limits, safe package processing, MIME/identity validation, deterministic order, and cancellation before canonical publication. Oversized or unsafe evidence is not accepted merely because its parent source opened successfully.

Versioned immutable asset paths keep the previous committed evidence safe during rebuilding. A new parse must not delete images still needed by the prior valid canonical state before the new graph commits. Interrupted or incompatible reuse follows explicit reconciliation.

## PDF extraction is not rendering

The semantic/native extraction path and page-rendering path solve different problems. A PDF can display correctly while a damaged font-to-Unicode map produces nonsense extracted text. A single text extractor should therefore not be assumed to validate what a human sees.

The PDF backend uses native text/semantic extraction with quality checks and page-text fallback where structure is bad but text remains usable. Desktop PDFium native-text rescue can try an independent extraction engine before resorting to OCR. Android PdfRenderer does not provide that same native-text rescue layer.

Ordinary document preparation and higher-resolution selective OCR rendering are separate. The pipeline should not OCR every page merely because a source is large, a page is short, or a few scientific glyphs are unmapped. Credible native text is retained; only unusable pages become rescue candidates. If required pages remain unusable, generation fails with a bounded explanation rather than sending noise and calling it grounded content.

Desktop rendering uses a reviewed PDFium runtime provisioned by the native-dependency installer and verified against a committed manifest/hash. The runtime lookup supports explicit configuration, packaged resources, and installed runtime locations. PDFium access is serialized under its safety boundary; more CPU cores do not authorize concurrent unsafe renderer handles.

Android uses PdfRenderer through the private Kotlin document plugin. Platform adapters return bounded observations/results; Rust keeps source admission, candidate selection, ordering, resource policy, and durable generation ownership.

## OCR and resource budgets

Desktop OCR uses Tesseract with required language data through the narrow OCR engine contract. Missing executable/trained data is reported as a runtime/setup issue at the relevant boundary. Android uses bundled ML Kit Text Recognition v2 Latin, not an implicit network model download or Tesseract dependency.

Shared planning considers source workload, CPU, memory, estimated bitmap size, worker ceilings, and queue capacity. User/environment caps are ceilings, not permission to allocate without regard to observed resources. Unknown observations result in conservative limits. Independent OCR work can overlap, while ordered output and bounded queues protect memory and deterministic evidence.

Cancellation is cooperative across parsing, rendering, OCR, and provider work. Active child processes or mobile callbacks are tied to task identity; stale callbacks must not publish into another document generation. A canceled extraction result does not become partially committed canonical evidence.

## Reusable understanding and learning plans

A bounded outline pass extracts source-grounded section fragments and coverage evidence. Rust assembles hierarchy deterministically: normalize ranges, order coverage, deduplicate useful concepts/objectives, reconcile overlaps, and preserve fallbacks when optional synthesis fails. A short top-level digest can request a whole-document summary without resending unbounded complete outlines.

The learning plan gives stable topic identities used by both flashcards and guides. Card provenance is a stored relationship to those topics/citations, not a mutable tag label or a later fuzzy match. Existing manually authored cards are not retroactively assigned to a source by guessing from their wording.

Source/canonical/backend/prompt/schema identity controls checkpoint reuse. Compatible unchanged understanding can support more cards or another guide without repeating extraction/rendering/OCR. New backend or prompt semantics can require rebuilding derivatives; presence alone is not validity.

## Bounded generation and provider validation

Large-document support is request-bounded. Each task accounts for source text, instructions/schema, output reserve, optional image context, and safety margin. Oversized source chunks split into explicit continuations with retained provenance. Headings are metadata, not a requirement to waste a separate request on every title.

Provider transport is entirely native. Local/LAN jobs run serially under current policy; cloud work can run through a bounded window. Stable chunk/topic identities, ordered assembly, checkpoint writes, and serialized SQL publication prevent network completion order from becoming application behavior.

The provider dialect requests enforceable structured output: the appropriate JSON-schema or strict-tool representation for that provider. Rust then validates domain shape, limits, allowed fields, choice/cloze grammar, exact cloze excerpt text, source ranges, tags, and curation indices. Successful HTTP and valid JSON do not establish a usable candidate.

Generated cards normalize into the existing native authoring model with independent local schedules. Definition/scenario/application/multiple-choice educational forms can use Basic cards rather than inventing a scheduler type per prompt style. Invalid individual candidates are rejected according to the task policy; malformed outer responses fail the request. Curation may select validated candidates, not gain arbitrary library-write authority.

There is no separate generated-card approval queue. User review remains important after validated publication because syntax and citations cannot prove factual accuracy or useful pedagogy. Generated and manual material use the same ordinary editing/deletion tools.

## More cards, checkpoints, and observability

Generate more uses retained source understanding and existing deck coverage. It should not reparse/rerender an unchanged compatible source simply to ask for additional candidates. Prompt/schema/assembly changes invalidate inappropriate checkpoints rather than silently mixing old and new semantics.

Persisted checkpoints record bounded reusable work; they are not a second unversioned source of truth. Progress distinguishes observable phases and indeterminate work. Non-PDF extraction is indeterminate until actual source-unit totals exist. Usage observations distinguish provider-reported tokens from estimates/unknowns rather than presenting exact fabricated costs.

Routine logs contain safe counts, phase/timing/cancellation outcomes, backend/model identifiers, and redacted errors. Source text and generated content are intentionally stored in their domain graph; raw prompts/responses/stream buffers, authorization headers, credentials, and reasoning are excluded from routine diagnostics. Those are channel-specific privacy rules, not a blanket claim that learning text never enters SQLite.

## Study guide revisions and learner ownership

Guide generation consumes the shared plan/topic evidence. A complete validated generated revision is published atomically; failed/canceled regeneration leaves the old active guide readable. Guide notes attach to a source; topic notes attach to stable topics; section overrides attach to an exact generated base hash. Incompatible old overrides become preserved visible conflicts, not silent data loss.

The current inline reader has contextual generation/stop, note/override entry, linked-card practice, source opening, and PDF export. It is not the old modal or a second duplicated reader implementation. Current note entry is plain text backed by rich storage; reusable schema capabilities do not advertise a toolbar that is not exposed there.

Topic practice resolves native card IDs and availability. PDF export freezes the effective generated-plus-annotation snapshot and renders a selectable-text PDF locally without another model call. Prepared export/file publication stays behind the native platform boundary.

Deleting only a guide removes generated revisions, section edits/conflicts, generation checkpoints/jobs, and prepared exports while preserving source, learning plan, linked flashcards, and stable guide/topic notes. Source deletion is a broader operation and must not be confused with that narrower guide action.

## Lifecycle, backup, and platform execution

Desktop close interception warns when document work is active. Android user-started tasks use a foreground service/notification and bounded wake lock to continue through ordinary Activity backgrounding. Neither path promises survival through process termination, reboot, or resource pressure.

Startup reconciles persisted task states and reusable checkpoints. It does not silently issue another provider request. Retry and more-generation are deliberate actions. A setting/profile change cannot silently swap the selected provider in a running or retried task.

Whole-library backup includes a coherent document, canonical, learning-topic, guide-revision, annotation, citation, and managed-file graph, and reads supported older representations. Backup admission blocks in-flight operations that could violate coherence. Credentials remain excluded. Current trusted-device sync is narrower and must not be assumed to transfer all learning-artifact tables/files.

## Testing and limits

Static boundary/dependency checks protect backend isolation and allowed features. Native fixtures test format detection, package defenses, tables/images/locators, PDF quality/rescue/OCR, ordering, budgets, cancellation, checkpoints, candidate validation, guide overlays/deletion/export, and backup relationships. Frontend tests cover active controllers/readers and task control; Android instrumentation covers native plugin/lifecycle integration.

Tests using deterministic provider fixtures do not qualify every live model. Text/schema PDF checks do not guarantee every visual export. Local parsing is not a complete hostile-file process sandbox. Extraction quality, OCR variability, incomplete non-PDF locator richness, context limits, cost, and model quality remain explicit limitations.

For focused commands use [document stabilization](../development/document-platform-stabilization-testing.md), [generation quality/scale](../development/document-generation-quality-and-scale-testing.md), [AI infrastructure tests](../development/ai-infrastructure-testing.md), and [Testing and CI](../development/testing-and-ci.md). Historical parser/stage plans are evidence, not current runtime selection instructions.
