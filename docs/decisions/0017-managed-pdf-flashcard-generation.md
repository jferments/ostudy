# ADR 0017: Managed PDF ingestion and direct flashcard-deck generation

- **Status:** Accepted
- **Date:** 2026-08-08
- **Scope:** AI Stages 4.3–4.6

## Context

Stages 4.1 and 4.2 established durable AI provider profiles, credential isolation, model discovery, capability evidence, a native provider gateway, and explicit task defaults. ostudy can now use a selected model without exposing credentials or arbitrary provider networking to React.

The next user-visible workflow must take a PDF from the local filesystem, retain it in a document library, extract its contents, render its pages, ask the configured provider for flashcards, and create an ordinary deck. The workflow must remain local-first and must not add model-server installation, a generic prompt command, provider fallback, or a separate unsafe card-writing path.

The product decision for this stage is intentionally simple: generated cards are not held in a review queue. Model output is treated as study-content input after strict structural validation. A learner who dislikes the result can use the existing deck/card editing and deletion tools.

## Decision

### One direct workflow

The Documents screen exposes one primary action: **Upload PDF**. Selecting a file starts a native task that:

1. validates the PDF signature, filename, and size;
2. copies the source into managed immutable storage;
3. extracts bounded text from every page locally through the PDF backend boundary;
4. renders every page to a bounded PNG locally through the same boundary;
5. loads the configured `flashcard_generation` provider profile;
6. asks the provider for zero or more high-value study cards from each page;
7. validates and normalizes every response through the generation-contract boundary;
8. assembles one strict internal `ostudy-flashcards` version-1 payload;
9. commits a new deck and its document provenance through canonical generated-card authoring.

There is no generated-card preview, approval queue, candidate editor, raw-JSON page, or separate commit button.

### Managed document identity and files

Each exact PDF byte stream is identified by SHA-256 and stored once at:

```text
documents/<document-id>/source.pdf
```

Rendered pages are stored at:

```text
documents/<document-id>/pages/page-000001.png
```

The source file is immutable. Renaming or replacing source bytes is outside this stage. Exact re-imports are rejected as duplicates. Documents may be archived without deleting their generated deck.

SQLite records document metadata, per-page extracted text and render hashes, durable generation jobs, per-page status/count units, safe errors, and generated-deck provenance. It does not retain duplicate copies of generated question/answer bodies in job rows. Startup reconciliation marks interrupted work as retryable failure and removes stale staging files.

### Swappable local PDF processing

Document lifecycle code no longer imports a concrete PDF library. Native extraction/rendering is isolated behind `src-tauri/src/pdf_backend.rs` and the `PdfBackend` interface. The backend receives only an immutable source path and render DPI and streams `(page number, extracted text, PNG bytes)` back to the document pipeline. It has no database, managed-file, AI-provider, credential, or frontend authority.

The active implementation in this stage is pinned `justpdf` because it is pure Rust, keeps the current package free of an external PDF runtime, and already supports the extraction/rendering path required by the current Linux build. This is an implementation choice rather than a permanent architecture decision. The boundary is deliberately shaped so a future `oxidize-pdf`, `pdfium-render`, or platform-native backend can replace it without changing document identity, file layout, provider requests, backup format, or canonical card authoring.

A future PDFium implementation would also need an explicit packaging strategy for the Pdfium native library. That distribution/runtime decision is kept out of the document feature layer instead of being entangled with it.

No PDF content is sent to a provider during import or extraction. No OCR engine, Python runtime, Tesseract installation, Pdfium binary, or JavaScript PDF viewer is added by this stage.

The document pipeline—not the backend—owns source size, page count, extracted-text limits, rendered-page limits, hashing, managed paths, persistence, and retry behavior. This keeps those safety and durability contracts identical across PDF engines.

A retry rebuilds extracted text and rendered pages from the immutable managed source PDF before contacting the provider. The rebuilt page files are checked against their recorded byte sizes and SHA-256 values immediately before transmission, so deleted or corrupted derivatives do not survive a retry.

### Task-specific provider use

The native document task owns the prompt, source delimiters, output schema, page limits, and local validation. PDF text is untrusted study material and cannot redefine the system task.

Requests are page-bounded. Each request contains:

- the document title;
- the physical PDF page number;
- bounded extracted text;
- the rendered page PNG only when the selected llama.cpp profile has current supported image-input evidence.

DeepSeek and other text-only profiles receive extracted text only. ostudy never silently substitutes another profile when a task fails.

### Relevance-first study-content prompt

The generation prompt explicitly optimizes for learning value rather than card count. Before returning JSON, the model is instructed to silently identify the page's subject and likely learning objectives, rank candidate knowledge by educational importance, discard low-value candidates, and emit only the strongest material. This internal selection is not requested as chain-of-thought output and is never persisted.

The prompt prioritizes:

- central concepts and definitions;
- mechanisms and causal relationships;
- processes and ordered steps;
- equations together with the meaning of their variables;
- clinically or technically important distinctions;
- interpretation and application;
- common misconceptions;
- prerequisite facts needed to understand later material.

It explicitly rejects bibliography/reference lists, citation metadata, publisher/legal/copyright text, acknowledgements, author biographies, indexes/navigation, page or figure numbers, incidental anecdotes, isolated names/dates, and hyperspecific numerical factoids unless a number is itself important to understanding or decision-making.

Returning zero cards for a page is defined as better behavior than manufacturing low-value cards. The prompt also forbids cards about where information appeared in the document or about document layout.

### Generation forms versus canonical storage types

The provider-facing generation contract supports several pedagogical forms:

- `basic` — direct conceptual question/answer;
- `definition` — important term/definition pair;
- `multiple_choice` — exactly four plausible choices, one best answer, and an explanation;
- `critical_thinking` — short application, mechanism, interpretation, or narrative/scenario reasoning;
- `cloze` — a complete sentence plus one to four exact substrings to hide.

These are generation forms, not new durable card types. The native normalizer converts `definition`, `multiple_choice`, and `critical_thinking` results into the existing safe Basic representation. Cloze remains the existing canonical Cloze type. This provides pedagogical variety without expanding scheduling/search/rendering storage semantics just to encode how a question was authored.

The model does **not** generate ostudy `{{cN::...}}` syntax. For a Cloze card it supplies ordinary text plus an explicit `blanks` array. Native code verifies every blank is a unique, exact, non-overlapping substring of the text and then inserts canonical ostudy markers itself. This removes a fragile serialization responsibility from the model and ensures canonical Cloze validation receives syntactically valid blank markers.

### Provider-specific structured output

llama.cpp uses its strict JSON-schema response format when the configured endpoint supports the existing request contract. DeepSeek uses its JSON-object response format plus an explicit concrete example of the required object shape. Both responses then pass through the same strict native deserializer and normalizer.

Unknown fields, unsupported generation types, empty or oversized content, invalid option counts, invalid answer indexes, duplicate options, invalid tags, invalid cloze modes, and invalid cloze blank selections are rejected before authoring.

If a provider returns empty or contract-invalid output, ostudy makes one bounded repair attempt against the **same configured provider and model**. The invalid provider response is neither persisted nor echoed back. The retry receives the source material and a short instruction to produce a fresh response matching the contract. There is still no provider fallback. If the second response is invalid, the document remains retryable and a concise safe error is shown instead of a page-sized list of downstream card-validation failures.

### Canonical authoring remains authoritative

After task-level normalization, ostudy reuses the existing generated-import path in memory. That path remains authoritative for:

- restricted Markdown conversion and validation;
- canonical cloze marker validation;
- tag validation;
- duplicate fingerprints and duplicate skipping;
- forward/reverse/cloze card directions;
- fresh scheduling rows;
- all-or-nothing database commit.

Deck/card creation, the document-to-deck relationship, document completion state, and generation-job completion state are written in the same SQLite transaction. A provenance write failure therefore rolls back the newly generated deck and cards rather than leaving an orphaned partially recorded result.

The model cannot select an existing deck, preserve scheduling data, overwrite cards, delete data, attach arbitrary files, or bypass duplicate and safe-content rules.

### Backup and restore

Library backup format version 2 packages:

- one coherent SQLite snapshot;
- every ready managed card image;
- every managed source PDF;
- every rendered PDF page.

Each document file is checked against its SQLite path, size, MIME type, signature, and SHA-256 before export and restore. Version-1 backups remain readable when they contain no managed documents. Credentials remain excluded.

### User recovery

A failed generation leaves the PDF in Documents with a safe error and **Retry generation**. Successful cards appear immediately in a normal deck. The learner uses ordinary deck/card interfaces to edit or delete generated content.

Deleting the generated deck does not delete the managed PDF. Document projections treat a soft-deleted deck as absent and offer **Retry generation** so a new normal deck can be created from the retained source.

## Consequences

- The common PDF-to-deck action requires only one user decision: choosing the PDF.
- There is no extra candidate data model or review UI to maintain.
- Poor but structurally valid generations can enter the library; users correct this through normal card/deck management.
- Provider quality remains important, but provider output cannot bypass structural and authoring validation.
- Large PDFs create many bounded sequential provider requests rather than one unbounded document prompt.
- Managed source PDFs increase backup size but preserve a complete local library.
- PDF-engine compatibility can be evaluated independently from the document/provider/card pipeline.
- Later tutoring, document search, citations, page-range controls, alternative provider adapters, and alternative PDF engines can reuse the durable document/page/job model without changing the direct-commit contract.

## Non-goals

This stage does not add OCR, embeddings, vector search, an in-app PDF viewer, page-range configuration, arbitrary prompt editing, automatic provider fallback, durable new card types solely for MCQ/definition/narrative formatting, generated-card review, or model-server installation.

## Validation

The decision is protected by migration checks, executable-script mode checks, PDF-backend extraction/render fixtures, path/hash/signature tests, provider contract tests, generation-form normalization tests, relevance-prompt tests, cloze-blank normalization tests, canonical generated-import rollback tests, direct frontend workflow tests, deleted-deck regeneration tests, backup/restore round trips, typed IPC parity, repository policy checks, and opt-in real-provider testing.
