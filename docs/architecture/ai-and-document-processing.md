# AI and document processing

ostudy's document system has four distinct responsibilities: retain and normalize a source; derive reusable learning structure; request bounded provider-assisted content; and validate/publish ordinary learning artifacts. The UI initiates tasks but does not own parser policy, provider transport, credentials, or the database graph.

[The comprehensive architecture](overview.md) gives the system context. [Knowledge representation](knowledge-representation.md) defines evidence, interpretations, coverage and revision safety. [Document AI routing](document-ai-routing.md) covers task contracts/concurrency. [AI infrastructure](ai-infrastructure.md) owns profiles, probes, and transport. [The user guide](../user-guide/documents-and-ai-generation.md) describes the visible workflow.

## Source and package ownership

| Owner | Responsibility |
| --- | --- |
| `documents/` plus current lifecycle/repository service modules | Source metadata, managed source ownership, admission and lifecycle coordination |
| `document_ingestion/` | Application adapter contract, explicit registry/detection, canonicalization and persistence boundary |
| `documents/image_sets/` + `document_ingestion/image_set.rs` | Ordered-image admission/ownership, retained originals/manifests, local OCR cache, conservative overlap/filter projection, revision-safe amendments and source inspection |
| `pdf_backend/` | Semantic/native PDF text extraction and quality/recovery |
| `pdf_document_ingestion/` | PDF pipeline, bounded rendering/OCR preparation, progress and recovery |
| `xberg_document_adapter/` | Isolated non-PDF backend invocation and immediate conversion to application-owned structures |
| `document_canonical*`, `document_store`, `document_semantics` | Canonical units/blocks/chunks/assets and their coherent persisted projections |
| `document_learning*`, `document_topic_links`, `document_card_topics` | Shared topic structure, source-window provenance, and transactional card links |
| `knowledge/authoring`, `knowledge/extraction`, `knowledge/repository` | Shared source-grounded knowledge units, generation catalogs, and exact artifact-to-unit support |
| `ai/document_intelligence/` | Provider-neutral bounded task requests and domain interpretation |
| `document_generation`, `document_initial_generation`, `document_more`, `document_checkpoints` | Task execution, reuse, validation, and ordinary flashcard publication |
| `document_study_guide*` | Generated revisions, learner overlays, library queries, lifecycle, and local PDF export |
| `study_materials/` | Durable document/collection StudyTarget snapshots, coherent bundle receipts, live collection flashcards over the subject's decks, collection-guide revisions, suppression and restart semantics |

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

### Ordered image-set sources

Ordered image sets are a first-class source family rather than another Xberg container type. Native admission validates staged PNG/JPEG/WebP bytes, decoded dimensions/pixels, stable member identities, sequence order and aggregate limits before one manifest becomes authoritative. Each original is retained independently; the source is never defined by a stitched composite.

Local region OCR is cached by compatible normalized-pixel/extractor identity separately from source ownership. The canonical adapter preserves literal text, member/region identities and geometry, and proves screenshot overlap only through exact text/pixel/reading-order evidence. Protected numbers, units, signs, operators, negation, ambiguous repeated headers, missing geometry or conflicting pixels prevent an equivalence merge.

Content filtering is a reversible source projection, not deletion. Specific confident edge controls/notices may be excluded, short attribution can remain metadata, and uncertain educational text remains retained. Manual Include/Exclude/Metadata decisions are revision checked. When a transform changes OCR region IDs, a decision transfers only to one exact-literal match on the same stable member and is rebound atomically when the successor projection commits.

Append, reorder, quarter-turn rotation and source-inclusion amendments use an expected source/filter revision and a bounded managed-file staging journal. An acknowledged edit invalidates the current projection while keeping the prior processing receipt inspectable until local preparation commits a successor. Stale extraction cannot publish into a newer manifest; compatible unchanged OCR/knowledge can be reused.

Image import, source inspection, local OCR preparation, overlap proof, filtering and amendment do not select an AI provider or fetch guessed source URLs. The controlled original preview reads retained owned pixels through a native command. Whole-library backup packages and validates the current manifest plus independently retained originals and persistent filter/pending-source state.

## Canonical document evidence

Canonical representation preserves source units/locators, structured text blocks, bounded chunks, relevant metadata, embedded-image identities, and backend/source/schema provenance. It allows document reasoning to ignore parser-specific types and allows a backend replacement to invalidate only rebuildable derivatives rather than learner notes or canonical card identities.

Current non-PDF persisted locators remain **Section** based. Richer internal slide/sheet divisions from Xberg are not promoted into a new authored locator contract without coordinated schema, migration, citation, backup, and UI changes. Do not advertise exact slide/page/chapter deep links from generic section labels.

Non-PDF visual evidence is actual supported embedded images, not imaginary whole-slide rendering. External image relationships are not fetched. The adapter applies bounded per-image/aggregate limits, safe package processing, MIME/identity validation, deterministic order, and cancellation before canonical publication. Oversized or unsafe evidence is not accepted merely because its parent source opened successfully.

Versioned immutable asset paths keep the previous committed evidence safe during rebuilding. A new parse must not delete images still needed by the prior valid canonical state before the new graph commits. Interrupted or incompatible reuse follows explicit reconciliation.

## PDF extraction is not rendering

The semantic/native extraction path and page-rendering path solve different problems. A PDF can display correctly while a damaged font-to-Unicode map produces nonsense extracted text. A single text extractor should therefore not be assumed to validate what a human sees.

The PDF backend uses native text/semantic extraction with quality checks and page-text fallback where structure is bad but text remains usable. Desktop PDFium native-text rescue can try an independent extraction engine before resorting to OCR. Android PdfRenderer does not provide that same native-text rescue layer.

The layout partitioner sometimes reads two text columns as a table. A table element whose long cells (at least 12 characters, at least half of the table, together at least 200 characters) already appear at least 90% in the same page's other elements is dropped before chunking, so the knowledge extractor does not read the page twice. Short label grids and tables that name values also found in prose stay tables.

Figure labels set in a font the extractor cannot decode come out as runs of replacement and control characters with no letters or digits. Those elements, and headings made only of them, are dropped before chunking. A page then falls back to raw page text only when its remaining structure is damaged or the structure has nothing readable; damage left only in the raw page text no longer replaces clean headings and paragraphs, because the raw text comes from the same decoder.

Ordinary document preparation and higher-resolution selective OCR rendering are separate. The pipeline should not OCR every page merely because a source is large, a page is short, or a few scientific glyphs are unmapped. Credible native text is retained; only unusable pages become rescue candidates. If required pages remain unusable, generation fails with a bounded explanation rather than sending noise and calling it grounded content.

Desktop rendering uses a reviewed PDFium runtime provisioned by the native-dependency installer and verified against a committed manifest/hash. The runtime lookup supports explicit configuration, packaged resources, and installed runtime locations. PDFium access is serialized under its safety boundary; more CPU cores do not authorize concurrent unsafe renderer handles.

Android uses PdfRenderer through the private Kotlin document plugin. Platform adapters return bounded observations/results; Rust keeps source admission, candidate selection, ordering, resource policy, and durable generation ownership.

## OCR and resource budgets

Desktop OCR uses Tesseract with required language data through the narrow OCR engine contract. Missing executable/trained data is reported as a runtime/setup issue at the relevant boundary. Android uses bundled Tesseract4Android/Tesseract with the pinned packaged language data; it does not depend on Google ML Kit text recognition or a network model download.

Shared planning considers source workload, CPU, memory, estimated bitmap size, worker ceilings, and queue capacity. User/environment caps are ceilings, not permission to allocate without regard to observed resources. Unknown observations result in conservative limits. Independent OCR work can overlap, while ordered output and bounded queues protect memory and deterministic evidence.

Cancellation is cooperative across parsing, rendering, OCR, and provider work. Active child processes or mobile callbacks are tied to task identity; stale callbacks must not publish into another document generation. A canceled extraction result does not become partially committed canonical evidence.

## Reusable understanding and learning plans

One bounded semantic analysis traverses the canonical source neighborhoods and publishes shared knowledge. Rust creates addressable exact text spans and attached-image references; the model selects their IDs instead of copying quotations. A unit may reference several spans or pages. Per-unit admission preserves successful siblings before a targeted repair; receipts distinguish complete source dispositions, partial analysis, and explicit no-learning decisions.

The document outline is now a derived view of this knowledge, not a second raw-document outline traversal. Native grouping retains source ranges and whole topic labels. An optional balanced knowledge-only reduction supplies broader context and an advisory core-ID set; it must represent every nonempty input window or fall back locally. Neither local importance nor absence from the bounded core-ID set proves what is pedagogically essential.

Document collections reuse those same source-owned interpretations through an exact range-aware source snapshot and a complete coverage `LearningContext`. Their compact derived synthesis stores topic/unit dependencies and explicit gaps rather than duplicating source facts or embeddings. Native grouping keeps minority topics and separates instructional emphasis from factual support; an explicit generation path may make one bounded provider request only when every topic fits the balanced digest, and provider-authored topic summaries/relationships may reference only admitted unit IDs. If the digest, provider response, or freshness checks fail, the complete native catalog remains authoritative. Tutor relevance retrieval remains a separate bounded policy and is never relabeled as complete generation coverage.

StudyTarget bundling is a publication/orchestration layer above those owners, not another document author. A document target adopts the ordinary deck/guide already produced by the established document pipelines. A collection is a subject, and its flashcards are the decks filed in it, studied live: the target's Flashcards step writes no card and succeeds when the subject has a live card to study. Nothing is copied, so a fact keeps one card and one schedule, and moving a deck or document changes what the collection studies. CollectionSynthesis never invents replacement flashcards. Collection copies made before schema 6 were retired into their original cards ([ADR 0055](../decisions/0055-subjects.md)). Collection guide revisions may consume the current validated synthesis and reconcile learner overlays independently of the flashcard deck.

A whole-document snapshot binds canonical chunks, image revisions, all prepared source projections, unit revisions and analysis receipts, including sources yielding no units. The snapshot is checked before and after provider work, then inside the transaction publishing the outline and generated metadata. Source changes, deletions, additions and competing reinterpretations invalidate stale results. Display renames and progress updates do not change v2 document evidence identity.

The learning plan gives stable topic identities used by both flashcards and guides. Card provenance is a stored relationship to those topics/citations, not a mutable tag label or a later fuzzy match. Existing manually authored cards are not retroactively assigned to a source by guessing from their wording.

Source/canonical evidence identity controls knowledge reuse; producer contracts remain recorded for provenance and repair. Compatible unchanged understanding supports cards, guides and Tutor even when the consuming model changes. Changing the embedding model rebuilds a derived index, not document interpretations. Incompatible evidence/backend/schema semantics can require rebuilding derivatives; presence alone is not validity.

## Knowledge-first card and guide authoring

Both initial/additional flashcard generation and study-guide generation prepare shared knowledge units before writing artifacts. New interpretation uses the selected document-generation profile and canonical chunk adapters, while current saved interpretations are model-independent reusable inputs. Independent cloud windows run with bounded concurrency; local inference remains serial. Ordinary card saves never call AI. After shared knowledge is published, indexing attempts only missing vectors using the configured embedding profile; an indexing failure cannot erase knowledge or require re-extraction. Compatible extracted units are reused across overlapping windows and across guides/cards. Embeddings remain a separate retrieval index over this material, not the definition of a knowledge unit.

The generation catalog contains unit IDs, statements, qualifications and native evidence references. Exact evidence is deduplicated within the catalog. Authoring receives the selected knowledge, its evidence, compact learning context and applicable visual inputs, not a duplicate of the full raw source window. Card output selects one primary target, or two for a necessary comparison; guide output selects the units its section actually explains. Native validation rejects unknown, duplicated, or excessive IDs and supplies all provenance hashes itself. This establishes traceability, not proof that a model explanation is correct.

Cards carry an internal support envelope through generation checkpoints and selection-only curation. Learner-authored **generation instructions** are a separate per-operation intent value in the effective question policy: they may change focus, audience, detail and supported question-family preference, but they are never source evidence, never a global provider preference, and never allowed to override native schemas or grounding. Normalized instructions participate in request/checkpoint identity, so unchanged retries can reuse compatible work while changed intent cannot accidentally replay an older result. Provider/debug formatting redacts the private text.

That support envelope is removed before the strict public card importer runs. The import's existing transaction attaches unit links to the actual created notes/presentations after duplicate filtering; forward/reverse presentations retain separate scheduling identities. A stale source/unit aborts the entire import rather than leaving cards without trustworthy support.

Guide checkpoints retain content, selected support, and the complete catalog fingerprint. Old content-only checkpoints and changed source/unit revisions are not reused. Guide publication inserts source/unit links in the same transaction as the new revision; a failure also rolls back superseding the previous guide. Reader content and learner overlays keep their existing format. Carried user edits are not labeled as model-verified source claims.

Text-only extraction cannot claim image-only support merely because an image ID was present. Only image references whose bytes were attached enter the model-selectable evidence catalog. The legacy primary `supportingExcerpt` remains a native-filled compatibility projection, not a provider output field. The producing extraction contract is kept with its revision-bound receipt. Merely changing a consumer model does not invalidate that receipt, authored notes, schedules, or review history.

## Bounded generation and provider validation

Large-document support is request-bounded. Each task accounts for source text, instructions/schema, output reserve, optional image context, and safety margin. Oversized source chunks split into explicit continuations with retained provenance. Headings are metadata, not a requirement to waste a separate request on every title.

Provider transport is entirely native. Local/LAN jobs run serially under current policy; cloud work can run through a bounded window. Stable chunk/topic identities, ordered assembly, checkpoint writes, and serialized SQL publication prevent network completion order from becoming application behavior.

The provider dialect requests enforceable structured output: the appropriate JSON-schema or strict-tool representation for that provider. Rust then validates domain shape, limits, allowed fields, choice/cloze grammar, exact cloze excerpt text, source ranges, tags, and curation indices. Successful HTTP and valid JSON do not establish a usable candidate.

Generated cards normalize into the existing native authoring and scheduling model with independent local schedules. Recall-style definitions, scenarios, application prompts, reversible definitions, and cloze prompts use the established recall card families where appropriate. Interactive generation uses native typed questions (`single_choice`, `select_all`, `true_false`, `matching`, `grouping`, and `ordering`) rather than flattening those assessments into Basic cards; legacy text-only `multiple_choice` output is rejected. Image occlusion remains manually authored. Invalid individual candidates are rejected according to the task policy; malformed outer responses fail the request. Curation may select validated candidates, not gain arbitrary library-write authority.

Collection StudyTarget flashcards keep the individual-document generator as the quality authority: they are the documents' own generated cards in the subject's decks. Because there is no copy to select, a collection bundle takes no flashcard instructions, and only the collection guide authorizes provider work.

There is no separate generated-card approval queue. User review remains important after validated publication because syntax and citations cannot prove factual accuracy or useful pedagogy. Generated and manual material use the same ordinary editing/deletion tools.

## More cards, checkpoints, and observability

Generate more uses retained source understanding and existing deck coverage. It should not reparse/rerender an unchanged compatible source simply to ask for additional candidates. Prompt/schema/assembly changes invalidate inappropriate checkpoints rather than silently mixing old and new semantics.

Persisted checkpoints record bounded reusable work; they are not a second unversioned source of truth. Reuse reads saved units without rewriting them. Optimistic analysis revisions prevent a late provider response from overwriting newer work. Regrouping retires overlapping obsolete receipts and marks their other sources for reanalysis. An explicit retry repairs unresolved slots without regenerating successful siblings; duplicate or malformed repair slots cannot discard independent successful repairs. Progress distinguishes observable phases and indeterminate work. Non-PDF extraction is indeterminate until actual source-unit totals exist. Usage observations distinguish provider-reported tokens from estimates/unknowns rather than presenting exact fabricated costs. Planning includes semantic extraction, bounded repair allowance, global reduction and artifact authoring. Uncalibrated estimates are workload allowances, not billing predictions. Complete source disposition means the protocol resolved each source, not that every important idea was extracted.

Routine logs contain safe counts, phase/timing/cancellation outcomes, backend/model identifiers, and redacted errors. Source text and generated content are intentionally stored in their domain graph; raw prompts/responses/stream buffers, authorization headers, credentials, and reasoning are excluded from routine diagnostics. Those are channel-specific privacy rules, not a blanket claim that learning text never enters SQLite.

## Study guide revisions and learner ownership

Guide generation consumes the shared plan/topic evidence. Documents task state is owned above navigation routes, so leaving the page does not cancel a guide. Refreshes retain failures, and the generating guide button becomes its stop control. A complete validated generated revision is published atomically; failed/canceled regeneration leaves the old active guide readable. Guide notes attach to a source; topic notes attach to stable topics; section overrides attach to an exact generated base hash. Incompatible old overrides become preserved visible conflicts, not silent data loss.

The current inline reader has contextual generation/stop, note/override entry, linked-card practice, source opening, and PDF export. Document and collection guides share the semantic content renderer rather than maintaining separate presentation vocabularies. Per-section actions live in one overflow menu. Reader-facing title/overview compaction can turn legacy machine-like topic inventories into concise prose without mutating the durable learning topic or generated base. Current note entry is plain text backed by validated rich storage; source/topic notes remain independent of generated revisions, while title/body overrides stay bound to an exact generated base hash and become conflicts rather than silently rebasing.

Topic practice resolves native card IDs and availability. PDF export freezes the same effective generated-plus-annotation meaning and renders locally without another model call. The print renderer owns cover/hierarchy, linked contents/bookmarks, source labels, headers/footers, tables and pagination, verified canonical source figures/captions, Unicode text, and bounded non-executing native math. Unsupported math is disclosed and preserved as source notation. The renderer never receives arbitrary model-chosen filesystem paths: figures resolve through canonical locator asset IDs and integrity-checked managed bytes. Prepared export/file publication stays behind the native platform boundary.

Deleting only a guide removes generated revisions, section edits/conflicts, generation checkpoints/jobs, and prepared exports while preserving source, learning plan, linked flashcards, and stable guide/topic notes. Source deletion is a broader operation and must not be confused with that narrower guide action.

## Lifecycle, backup, and platform execution

Desktop close interception warns when document work is active. Android user-started tasks use a foreground service/notification and bounded wake lock to continue through ordinary Activity backgrounding. Neither path promises survival through process termination, reboot, or resource pressure.

Startup reconciles persisted task states and reusable checkpoints. It does not silently issue another provider request. Retry and more-generation are deliberate actions. A setting/profile change cannot silently swap the selected provider in a running or retried task.

Whole-library backup includes a coherent document, canonical, learning-topic, guide-revision, annotation, citation, and managed-file graph, and reads supported older representations. Backup admission blocks in-flight operations that could violate coherence. Credentials remain excluded. Current trusted-device sync is narrower and must not be assumed to transfer all learning-artifact tables/files.

## Testing and limits

Static boundary/dependency checks protect backend isolation and allowed features. Native fixtures test format detection, package defenses, tables/images/locators, PDF quality/rescue/OCR, ordering, budgets, cancellation, checkpoints, candidate validation, guide overlays/deletion/export, and backup relationships. Frontend tests cover active controllers/readers and task control; Android instrumentation covers native plugin/lifecycle integration.

Tests using deterministic provider fixtures do not qualify every live model. Text/schema PDF checks do not guarantee every visual export. Local parsing is not a complete hostile-file process sandbox. Extraction quality, OCR variability, incomplete non-PDF locator richness, context limits, cost, and model quality remain explicit limitations.

For focused commands use [document stabilization](../development/document-platform-stabilization-testing.md), [generation quality/scale](../development/document-generation-quality-and-scale-testing.md), [AI infrastructure tests](../development/ai-infrastructure-testing.md), and [Testing and CI](../development/testing-and-ci.md). Historical parser/stage plans are evidence, not current runtime selection instructions.
