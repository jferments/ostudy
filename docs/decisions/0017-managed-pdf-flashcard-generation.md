# ADR 0017: Managed PDF ingestion and direct flashcard-deck generation

- **Status:** Accepted
- **Date:** 2026-08-08
- **Scope:** AI Stages 4.3–4.6

## Context

Stages 4.1 and 4.2 established durable AI provider profiles, credential isolation, model discovery, capability evidence, a native provider gateway, and explicit task defaults. ostudy can now use a selected model without exposing credentials or arbitrary provider networking to React.

The next user-visible workflow must take a PDF from the local filesystem, retain it in a document library, extract its contents, render its pages, ask the configured provider for flashcards, and create an ordinary deck. The workflow must remain local-first and must not add model-server installation, a generic prompt command, provider fallback, or a separate unsafe card-writing path.

The product decision for this stage is intentionally simple: generated cards are not held in a review queue. Model output is trusted as study content after strict structural validation. A learner who dislikes the result can use the existing deck/card editing and deletion tools.

## Decision

### One direct workflow

The Documents screen exposes one primary action: **Upload PDF**. Selecting a file starts a native task that:

1. validates the PDF signature, filename, and size;
2. copies the source into managed immutable storage;
3. extracts bounded text from every page locally;
4. renders every page to a bounded PNG locally;
5. loads the configured `flashcard_generation` provider profile;
6. generates zero or more Basic/Cloze cards from each page;
7. validates every response against a strict task schema;
8. assembles one strict internal `ostudy-flashcards` version-1 payload;
9. commits a new deck and its document provenance through canonical generated-card authoring.

There is no generated-card preview, approval queue, candidate editor, or separate commit button.

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

### Local PDF processing

PDF parsing, text extraction, and page rendering use the pinned pure-Rust `justpdf` dependency. No PDF content is sent to a provider during import or extraction. No OCR engine, Python runtime, Tesseract installation, Pdfium binary, or JavaScript PDF viewer is added.

Processing is bounded by source size, page count, extracted text size, render size, and total generated-card count. Page images and full PDF bytes never cross the frontend IPC boundary.

A retry rebuilds extracted text and rendered pages from the immutable managed source PDF before contacting the provider. The rebuilt page files are checked against their recorded byte sizes and SHA-256 values immediately before transmission, so deleted or corrupted derivatives do not survive a retry.

### Task-specific provider use

The native document task owns the prompt, source delimiters, output schema, page limits, and local validation. PDF text is untrusted study material and cannot redefine the system task.

Requests are page-bounded. Each request contains:

- the document title;
- the physical PDF page number;
- bounded extracted text;
- the rendered page PNG only when the selected llama.cpp profile has current supported image-input evidence.

DeepSeek and other text-only profiles receive extracted text only. ostudy never silently substitutes another profile when a task fails.

### Structured output and canonical authoring

Provider output must be a bounded object containing Basic and/or Cloze card inputs. Unknown fields, unsupported card types, empty fields, oversized content, invalid tag shapes, and invalid cloze modes are rejected before authoring.

After task-level validation, ostudy reuses the existing generated-import path in memory. That path remains authoritative for:

- restricted Markdown conversion and validation;
- cloze marker validation;
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
- Later tutoring, document search, citations, page-range controls, and alternative provider adapters can reuse the document/page/job data without changing this direct-commit contract.

## Non-goals

This stage does not add OCR, embeddings, vector search, an in-app PDF viewer, page-range configuration, arbitrary prompt editing, automatic provider fallback, custom-template generation, generated-card review, or model-server installation.

## Validation

The decision is protected by migration checks, path/hash/signature tests, PDF extraction/render fixtures, provider contract tests, strict card-schema tests, canonical generated-import rollback tests, direct frontend workflow tests, deleted-deck regeneration tests, backup/restore round trips, typed IPC parity, repository policy checks, and opt-in real-provider testing.
