# PDF flashcard generation testing

## Purpose

Stages 4.3–4.6 now use one direct workflow: a selected PDF is copied into the managed Documents library, structurally extracted with oxidize-pdf, rendered to PNG with the pure-Rust justpdf renderer, analyzed into a reusable global outline, processed in overlapping semantic windows, and committed as a normal deck after global relevance curation and canonical card validation.

There is no generated-card approval queue, preview page, or raw JSON screen. Uploading a PDF starts generation directly; the resulting deck and cards are inspected, edited, or deleted afterward through the ordinary Decks and Search/Filter interfaces.

## Automated qualification

Run the ordinary repository gates from the PR branch:

```bash
git fetch origin
git switch agent/stage-4-3-4-6-pdf-flashcard-generation
git pull --ff-only

pnpm install
./scripts/check-fast.sh
./scripts/check.sh
```

On a fresh clone, shell scripts execute directly. Do not run `chmod +x` as normal setup. Project-manifest verification checks every tracked `scripts/*.sh` Git mode.

No Pdfium/Poppler/browser runtime must be installed or provisioned for PDF rendering. `justpdf`/`justpdf-render` is part of the locked Rust dependency graph and runs in-process.

The automated suite covers:

- schema migrations through 18 and immutable migration checksums;
- typed document IPC parity;
- executable Git modes for repository shell scripts;
- oxidize-pdf structured extraction through the backend boundary;
- semantic element/chunk persistence and cross-page window construction;
- pure-Rust PNG rendering through the independent renderer boundary;
- absence of mandatory Pdfium and OCR/Tesseract dependencies;
- page-evidence chunks for pages with no extractable structured text;
- bounded managed source/page paths, signatures, sizes, and SHA-256 values;
- relevance/outline/card schema validation;
- deterministic native Cloze marker construction;
- canonical generated-import rollback;
- document source/page backup and restore;
- retry and deleted-generated-deck behavior;
- responsive Documents navigation and direct upload UI behavior.

Normal CI never uses a real API key or internet AI provider.

## Structural extraction acceptance

Use PDFs with headings, paragraphs, lists, tables, multi-column content, rotated pages, and concepts that cross page boundaries. Confirm:

1. the PDF opens and all pages are counted;
2. extracted page text is meaningful;
3. semantic chunks retain page mappings and heading context;
4. table/list/title elements are represented rather than flattened into an undifferentiated page string where the source permits recovery;
5. semantic windows can span multiple pages and do not cut individual semantic chunks;
6. pages with no recoverable structure remain represented by visual-evidence chunks;
7. source windows remain bounded by both estimated tokens and physical-page count.

The active extraction backend ID should identify `oxidize-pdf-4.2.2-rag`; the active renderer should independently identify `justpdf-render-0.1.2/pure-rust`.

## Study-quality acceptance

A successful JSON response is not sufficient. Inspect generated content for educational quality:

1. Use a textbook or course PDF containing conceptual prose, definitions, mechanisms, cross-page explanations, and a bibliography/reference section.
2. Verify the global outline captures the document/chapter's major learning objectives rather than listing every fact.
3. Verify generated cards concentrate on central concepts, mechanisms, causal relationships, important distinctions, processes, equations/variables, interpretation, and application.
4. Verify concepts developed across page boundaries produce integrated questions rather than being silently split into unrelated page-level fragments.
5. Verify bibliography/reference pages normally produce no cards.
6. Verify copyright, publisher, citation metadata, author biography, index/navigation, figure/page numbering, and similar scaffolding do not become flashcards.
7. Look for hyperspecific dates, names, or numerical trivia. These should be absent unless the value is itself important for understanding or decision-making.
8. Verify cards are self-contained and do not ask where information appeared in the document.
9. Verify the generator uses an appropriate mix when supported: direct questions, important term/definition pairs, multiple choice, application/critical-thinking questions, and Cloze/fill-in-the-blank.
10. Verify the global curation pass removes redundant cards and overrepresented minor concepts while retaining coverage of major outline objectives.

## Real-provider regression acceptance

### llama.cpp / Unsloth

1. Configure and verify the same llama.cpp / Unsloth profile that previously produced Cloze import failures.
2. Assign it as the **Flashcard generation** default.
3. Retry the retained PDF or upload a representative copy.
4. Confirm structured extraction and rendering reach the document page count.
5. Confirm outline construction succeeds before card generation.
6. Confirm generation creates a deck rather than surfacing Cloze syntax errors.
7. Inspect fill-in-the-blank cards and verify hidden spans are meaningful exact phrases from complete sentences.
8. With an image-capable model, verify selected page renders are supplied to semantic windows for visual/table/diagram grounding.

### DeepSeek

1. Configure and verify the DeepSeek profile that previously produced a card-contract failure.
2. Assign it as the **Flashcard generation** default.
3. Retry the retained PDF or upload a text-heavy PDF.
4. Confirm outline, semantic-window generation, and global curation use JSON-object responses.
5. If a response violates a local contract, confirm the same provider/model receives one bounded fresh-output retry, with no provider fallback.
6. If both responses fail, confirm the UI shows one concise safe contract error rather than raw provider output.

## Cross-page reasoning test

Use a source where one concept is introduced on one page and completed on the next two pages, such as a physiological mechanism with causes, compensatory responses, and consequences. Verify generated cards can test the full relationship. A good result should not require each page to independently contain a complete question/answer pair.

## Image-only PDF test

Use a small scanned/image-only PDF with no usable text layer.

1. With a text-only profile selected, upload or retry it. The task must fail safely with `document_requires_image_input`; it must not invent source text or create a partial deck.
2. Select a llama.cpp profile whose current capability probe verifies image input.
3. Retry the retained document.
4. Confirm every physical page remains represented in semantic-window provenance even though extracted text is empty.
5. Confirm card content is grounded in rendered-page evidence and no OCR transcript is stored as extracted source text.
6. Confirm the generated deck still passes the same canonical card validation as text-derived decks.

## Renderer portability and resource checks

The default renderer has no separately provisioned native runtime. Validate builds on each supported target as those platform pipelines become available: Linux x86_64/aarch64, Windows x86_64, macOS x86_64/aarch64, Android, and iOS.

For representative mobile hardware or emulators, observe peak memory while importing a multi-page PDF. Rendering is page-at-a-time, output is bounded per page, and page images are encoded at 144 DPI. Confirm a large document does not require all rendered bitmaps to reside in memory simultaneously.

Keep a small compatibility corpus containing difficult fonts, transparency, clipping, rotated pages, vector diagrams, scanned pages, and complex tables. If a document renders incorrectly under justpdf-render, record it as renderer-compatibility evidence. Do not replace the portable default solely because one difficult desktop document exists; the renderer boundary permits a future optional compatibility backend.

## Failure and recovery checks

- **No task default:** the managed PDF remains available with a safe error; configure a profile and Retry.
- **Provider unavailable:** no partial deck is committed; retry after restoring the provider.
- **Invalid structured output:** one bounded same-provider repair attempt; no partial authoring.
- **Application restart:** interrupted jobs become retryable and do not resume network transmission automatically.
- **Missing/corrupt render:** retry rebuilds derived extraction/render data from the immutable managed source.
- **Backend upgrade:** a document whose stored extraction/render backend IDs differ from the active IDs is rebuilt on retry.
- **Duplicate PDF:** exact duplicate bytes are rejected by SHA-256.

## Backup and restore acceptance

1. Generate a deck from a PDF.
2. Export a complete `.ostudybackup`.
3. Restore it in a disposable library.
4. Verify source PDF, rendered pages, semantic elements/chunks, document outline, generation provenance, deck, cards, and schedules survive.
5. Verify **Open PDF** works after restore.

A backup remains blocked while a document is actively processing so database and managed files are captured coherently.

## Large-document observations

For a large textbook chapter or paper, record:

- extraction backend and renderer IDs;
- page count, semantic element count, and semantic chunk count;
- number and page spans of outline/generation windows;
- outline quality;
- generated-card count before/after global curation;
- peak memory and provider latency;
- whether cross-page questions improve over page-isolated generation.

Do not attach API keys, Authorization headers, raw provider responses, or copyrighted source PDFs to public reports.
