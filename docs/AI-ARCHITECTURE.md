# AI architecture

ostudy keeps AI integration native-only. React requests narrow domain tasks; it never receives provider credentials, raw provider responses, PDF page bytes, or arbitrary model completions. Provider profiles, credential isolation, endpoint trust policy, model discovery, capability evidence, bounded HTTP/SSE transport, and task defaults are implemented under `src-tauri/src/ai/`.

There is no generic prompt IPC command and no silent provider fallback.

## Document intelligence

Managed PDFs are local durable sources. PDF interpretation is split into independent native capabilities:

- `pdf_backend.rs`: structure-aware extraction through oxidize-pdf;
- `pdf_render_backend.rs`: bounded pure-Rust page rasterization through justpdf/justpdf-render;
- `document_semantics.rs`: ostudy-owned semantic chunks and hard-bounded provider windows;
- `ai/document_intelligence.rs`: provider-enforced local outline/generation/curation tasks, deterministic native hierarchy assembly, and a bounded optional whole-document synthesis.

The extraction boundary maps third-party parser output into an ostudy-owned semantic intermediate representation containing page text, typed elements, geometry, heading relationships, and structure-aware chunks. Semantic metadata is sanitized/bounded before SQLite insertion. The renderer only returns PNG bytes. Neither PDF backend can contact an AI provider or mutate cards.

Derived text, semantic chunks, outline data, and rendered pages are rebuildable from the immutable managed source PDF. SQLite records extraction/render backend identities so retry can rebuild stale derivatives after an implementation upgrade.

### Hard-bounded semantic source windows

A physical PDF page is provenance/layout, not necessarily a semantic reasoning boundary. Document-aware AI therefore consumes overlapping semantic windows derived from structure-aware chunks.

Provider source windows are deliberately conservative:

- initial outline: about **3,200 estimated source tokens**, at most **8 physical pages**, with about 350 tokens of semantic overlap;
- flashcard generation: about **5,000 estimated source tokens**, at most **10 physical pages**, with about 700 tokens of semantic overlap;
- Generate more cards: the same 5,000-token / 10-page generation boundary.

The token estimate is an application-side source bound rather than a claim that it exactly matches every provider tokenizer. The design keeps room for system instructions, JSON Schema/tool definitions, compact outline context, images, and completion tokens. Large documents scale through **more small requests**, not larger individual prompts.

Normal semantic chunks remain intact. If a third-party extractor produces one chunk that is individually larger than the provider boundary, ostudy splits only that pathological chunk into bounded continuation source units. Continuations retain the original chunk identity and page provenance and are marked as segments in the model-facing source. This makes the request bound hard rather than advisory.

### Hierarchical first pass and whole-document view

Before card generation, ostudy asks the configured provider for one concise schema-constrained outline fragment per bounded source window. Each fragment can contain a summary, learning objectives, concepts, and section/chapter-like page spans. Fragment output is deliberately capped so a 3,200-token source window cannot produce an unbounded outline object.

The section hierarchy is assembled **deterministically in Rust**. Native assembly:

- bounds and sanitizes fragment content;
- clamps section ranges to their source window/document;
- repairs reversed page ranges;
- deduplicates objectives and concepts;
- orders sections by physical page position;
- stitches adjacent/overlapping sections with the same normalized title;
- creates a bounded fallback section when a fragment omits sections.

ostudy does **not** send several complete outline objects to another model for a recursive “outline of outlines” merge. That former design could itself overflow or drift on large documents.

The model still receives a global semantic view. After deterministic hierarchy assembly, ostudy builds a bounded whole-document digest that deliberately samples the summary, global objectives/concepts, and section coverage from the beginning through the end of the source. One optional schema-constrained **global synthesis** request turns that digest into a concise whole-document summary, major learning objectives, and key concepts. The provider cannot rewrite section/page hierarchy in this step.

Global synthesis is an enhancement rather than a single point of failure. If that bounded request is rejected, truncated, or otherwise fails validation, ostudy logs the safe reason and continues with the already-valid native whole-document aggregate. This provides global semantic prioritization without making successful document processing depend on one large merge request.

The design is therefore chapter/section aware without making chapter detection a brittle prerequisite. Imperfect or missing section boundaries cannot prevent processing; later tasks still use bounded semantic source windows.

The persisted outline remains reusable document intelligence for flashcards today and later study-guide, quiz, search, and tutoring tasks.

### Outline-guided flashcard generation

Every generation request receives:

- the bounded whole-document summary;
- a bounded subset of global objectives/concepts;
- only outline sections overlapping that semantic source window;
- the bounded structured semantic source window;
- selected rendered pages when the configured llama.cpp profile has current image-input capability evidence.

The full global outline is **not** copied into every request. This prevents prompt size from growing with total document outline size while still letting each local generation decision be ranked against global learning goals.

The task prompt prefers durable concepts, mechanisms, comparisons, applications, and cross-page relationships when supported by the source. It suppresses bibliography/reference lists, legal/publishing scaffolding, navigation/index content, citation metadata, and hyperspecific trivia that does not contribute to understanding.

Generated forms include Basic conceptual questions, definitions, four-option multiple choice with explanation, critical-thinking/application/scenario questions, and Cloze. Definition/MCQ/critical-thinking forms normalize into existing safe Basic cards; Rust constructs Cloze markers from validated exact substrings.

After source windows complete, a bounded curation task selects which cards remain. It cannot rewrite cards. Curation receives a compact global learning summary rather than the complete outline hierarchy.

Selected cards pass through canonical generated-card authoring for restricted Markdown/content validation, Cloze syntax, tags, duplicate fingerprints, fresh schedules, and transactional deck/card creation. There is no generated-card review queue.

`Generate more cards` reuses the persisted outline and current deck coverage. That means a successful Generate-more pass on an already processed document tests the later generation path; it does **not** exercise initial outline construction.

## Provider-enforced structured output

Prompts describe educational intent, but prompt prose is not the structural guarantee.

### llama.cpp / Unsloth

llama-server receives the task JSON Schema through schema-constrained JSON output. Current llama-server converts JSON Schema to a sampling grammar. If an older compatible server rejects the primary response-format representation, ostudy retries once against the exact same endpoint/profile/model using llama-server's top-level `json_schema` compatibility field. The retry remains grammar/schema constrained; it never degrades into prompt-only JSON and never changes provider.

The document SSE gateway accepts the ordinary OpenAI-compatible `choices[].delta.content` stream and a small set of llama-server compatibility forms used by some builds (`choices[].message.content`, completion `text`, or native top-level `content`). A final full-message snapshot is not duplicated if content deltas were already received. Empty final output now reports the provider `finish_reason` when available instead of collapsing immediately to an unexplained “no content” error.

The server owns the model chat template. ostudy does not construct tokenizer-specific chat templates in the WebView or document feature.

### DeepSeek

DeepSeek document tasks use the Beta **strict Function Calling** interface rather than ordinary JSON Output as the schema guarantee. Each request forces one result function, enables `strict: true`, and supplies a DeepSeek-compatible JSON Schema.

DeepSeek strict mode supports a narrower schema subset than ostudy's local contracts. The native adapter converts internal schemas to the supported subset (for example `oneOf` to `anyOf`, every object property required, `additionalProperties: false`, and omission of unsupported string/array length keywords). Native Rust then enforces the tighter length/count/semantic constraints after the provider returns schema-constrained tool arguments.

Initial DeepSeek outline fragments receive a larger completion allowance than llama.cpp while the source window and requested outline shape are kept smaller. This separates source-context pressure from structured-output completion pressure.

### Native validation remains authoritative

Provider structure is necessary but not sufficient. Rust still validates:

- text/tag/card count bounds;
- generated-card direction rules;
- MCQ option count/uniqueness and correct answer index;
- Cloze mode and exact blank placement;
- outline page-range normalization;
- curation indices;
- restricted Markdown and canonical card-document structure;
- duplicate detection and fresh scheduling.

A structurally valid response can contain one semantically unusable card. Such a candidate may be dropped while valid cards from the same bounded window continue. A malformed outer task object still fails the request.

## Long-running requests, finish reasons, and cancellation

Document AI uses streamed Chat Completions. Model discovery and explicit connection probes keep their ordinary bounded request behavior.

For a document task, the selected profile timeout covers request establishment and receipt of successful provider **response headers**. Those headers are the acknowledgement boundary. After acknowledgement, ostudy does not impose a fixed wall-clock first-token/completion deadline; explicit Stop, transport failure, provider termination, and output-size limits still apply.

The SSE gateway accumulates ordinary content and streamed strict-tool arguments. It also preserves provider `finish_reason`, allowing document intelligence to distinguish at least:

- `length` → structured result was truncated (`ai_document_output_truncated`);
- provider content filtering;
- provider inference-resource exhaustion;
- genuine malformed/invalid task output.

This matters because output truncation or context exhaustion should not be mislabeled as a generic schema mismatch.

Outline failures identify the bounded source-window number, total window count, PDF page range, approximate source-token count, safe error code, and a bounded final validation detail. The `document_outline_window_failed` diagnostic event records the same safe metadata. It does not record raw PDF text, raw provider response bodies, credentials, Authorization headers, or model reasoning.

The running document task owns a process-local cancellation signal keyed by document ID. **Stop generation** cancels only that task and drops its exact HTTP response stream. A stopped document remains durable/retryable and the durable job is recorded as canceled rather than succeeded.

Cancellation state is intentionally not restored after application restart. Startup reconciliation marks interrupted work retryable and never resumes provider transmission automatically.

## Portable multicore page rendering and progress

The active raster backend uses justpdf 0.1.2 / justpdf-render 0.1.2 at bounded 144 DPI. It is an in-process Rust dependency rather than a platform-specific Pdfium/Poppler/browser runtime.

CPU-heavy page rasterization uses a bounded worker pool:

```text
min(available logical CPU parallelism, PDF page count, 16)
```

Different physical pages render concurrently. Results are sent to a coordinator and reordered by page number before the existing managed-file/SQLite persistence visitor executes. This raises multicore utilization while keeping durable page persistence deterministic and avoiding concurrent SQLite/file-ledger mutation.

The structure extraction step determines the physical page count before rasterization begins. ostudy writes that total into the document projection immediately, then updates `rendered_page_count` as ordered rendered pages are committed to managed storage. The Documents screen polls active jobs, so the UI advances through values such as `0/138`, `17/138`, and `138/138` instead of remaining `0/—` until the end.

The active renderer identity is versioned as `justpdf-render-0.1.2/pure-rust-parallel-v1`. Retry rebuilds derivatives when a document records an older render backend identity.

The renderer boundary remains swappable. A future compatibility backend could be introduced without changing semantic extraction, durable storage, provider contracts, backup format, or card authoring.

## Image-only pages and OCR policy

Pages with no recoverable text/structure remain represented by explicit page-evidence chunks. A text-only profile cannot treat those markers as source facts. A fully image-only PDF requires a verified image-capable llama.cpp task profile; rendered pages are then the source evidence.

ostudy does not enable oxidize-pdf OCR/Tesseract integration. The dependency uses a restricted feature set and dependency qualification fails if `rusty-tesseract` enters the graph. Model-derived transcription is not silently written into the extracted source-text store.

## Managed document lifetime

Archiving a document is reversible and keeps source PDF, derivatives, and generated deck provenance. Permanent deletion is supported when no generation task is active.

Deletion removes the managed PDF, rendered pages, structured extraction, outline, generation jobs, and document-library provenance. Generated decks/cards remain ordinary study content. Before document deletion, ostudy preserves a human-readable detached source citation **only in surviving generated decks whose `deleted_at` is still null**. Soft-deleted decks may still have provenance rows until normal purge/cascade cleanup, but they are not treated as citation destinations and do not increase `preservedDeckCitationCount`.

The delete workflow stages the managed document directory into application-controlled trash before the SQLite transaction. A database failure restores the directory. After a successful transaction, filesystem cleanup is best-effort and startup reconciliation removes unreachable delete-staging residue.

## Privacy and trust

PDF import, extraction, semantic chunking, native outline assembly, bounded global synthesis, outline persistence, and page rendering are local except for the explicitly configured provider requests. No provider fallback is automatic. Source material is treated as untrusted data and cannot replace system/task instructions.

Credentials, Authorization headers, raw provider responses, and reasoning content are not persisted. Provider requests run outside SQLite transactions. The original PDF is not uploaded as an opaque file; bounded source context and selected page images are transmitted according to the configured provider/capability.

The WebView never receives a managed PDF path merely to open it. `open_document_source` validates document identity/path in Rust and invokes the operating-system opener natively, so the frontend does not require generic filesystem opener authority.

## Backup

Complete library backups preserve the coherent SQLite snapshot, managed card images, retained source PDFs, rendered pages, and document-intelligence state. AI credentials remain excluded.

Backups are blocked while a document is actively processing so the database and managed files are captured as one coherent library state.

## Extension rule

Future study guides, quizzes, tutoring, document search, and learning-analysis features should reuse the managed document/semantic/outline platform rather than adding feature-specific PDF parsers or generic prompt IPC. New providers must implement the native provider boundary and explicit capability/contract tests; task code must never silently substitute another provider.
