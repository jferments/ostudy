# PDF flashcard generation testing

## Purpose

Stages 4.3–4.6 add one direct workflow: a selected PDF is copied into the managed Documents library, processed locally page by page, sent through the explicitly configured `flashcard_generation` provider profile, and committed as a normal deck after strict output validation.

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

On a fresh clone, the shell scripts above must execute directly. Do **not** run `chmod +x` as part of normal setup. Project-manifest verification checks that every tracked `scripts/*.sh` file is committed with Git mode `100755` so an accidental executable-bit regression fails CI.

The automated suite covers:

- schema migration 17 and immutable migration checksums;
- typed document IPC parity;
- executable Git modes for repository shell scripts;
- the swappable PDF-backend boundary plus real local text extraction and PNG rendering;
- bounded managed source/page paths, signatures, sizes, and SHA-256 values;
- deterministic llama.cpp multimodal request lowering through a loopback server;
- generation-contract normalization for Basic, definition, multiple-choice, critical-thinking, and Cloze forms;
- deterministic construction of canonical Cloze markers from validated exact blank substrings;
- rejection of malformed output plus one same-provider repair attempt;
- relevance-prompt rules that explicitly discard references/bibliographies and other low-value material;
- canonical generated-import rollback when document provenance cannot commit;
- document source/page backup and restore;
- retry and deleted-generated-deck behavior;
- responsive Documents navigation and direct upload UI behavior.

Normal CI never uses a real API key or internet provider.

## Real-provider regression acceptance

These checks specifically cover failures observed against llama.cpp/Unsloth and DeepSeek during PR #38 development.

### llama.cpp / Unsloth

1. Configure and verify the same llama.cpp / Unsloth profile that previously produced `Mark at least one word or phrase as a blank` import failures.
2. Assign it as the **Flashcard generation** default.
3. Open **Documents** and retry the retained PDF, or upload a different copy in a disposable test library.
4. Confirm the page render count reaches the document page count.
5. Confirm generation creates a deck rather than surfacing card-by-card Cloze syntax errors.
6. Inspect several generated fill-in-the-blank cards and verify their hidden spans are meaningful phrases from a complete sentence.
7. Confirm the final cards behave as ordinary ostudy Cloze cards in practice.

The model-facing Cloze contract no longer asks the model to emit `{{cN::...}}` markers. It supplies a normal sentence and exact blank substrings; Rust validates those substrings and writes the canonical markers itself.

### DeepSeek

1. Configure and verify the DeepSeek profile that previously produced `The model returned flashcard JSON that did not satisfy ostudy's card contract.`
2. Assign it as the **Flashcard generation** default.
3. Retry the retained PDF or upload a representative text-heavy PDF.
4. Confirm generation succeeds using the JSON-object request plus the explicit generation-schema example.
5. If the first response violates the local contract, confirm the same provider/model receives one bounded fresh-output retry rather than a fallback to another provider.
6. If both responses fail, confirm the UI shows one concise safe contract error rather than raw provider output or a large per-card import dump.

The application intentionally does not persist raw provider responses, so when an external model produces a contract failure the exact malformed field is not recoverable from the document ledger. Use a deterministic local fixture when raw wire-level diagnosis is required.

## Study-quality acceptance

A successful JSON response is not sufficient. Inspect generated content for educational quality:

1. Use a textbook or course PDF containing conceptual prose, definitions, mechanisms, and at least one reference/bibliography section.
2. Verify generated cards concentrate on central concepts, mechanisms, causal relationships, important distinctions, processes, equations/variables, interpretation, and application.
3. Verify bibliography/reference pages normally produce zero cards.
4. Verify copyright, publisher, citation metadata, author biography, index/navigation, figure/page numbering, and similar document scaffolding do not become flashcards.
5. Look for hyperspecific dates, names, or numerical trivia. These should be absent unless the value is itself important for understanding or decision-making.
6. Verify cards are self-contained and do not ask where information appeared in the document.
7. Verify the generator uses an appropriate mix when the material supports it: direct questions, important term/definition pairs, multiple choice, short application/critical-thinking questions, and Cloze/fill-in-the-blank.
8. Verify it does not manufacture every card style on every page; zero or fewer cards should be preferred when the source has little durable learning value.
9. For multiple-choice cards, verify all four choices are plausible enough to test discrimination and the explanation teaches why the selected answer is best.
10. For critical-thinking cards, verify the prompt requires applying or connecting ideas rather than recalling an obscure detail.

## Manual local-provider acceptance

Before testing, configure and verify a `llama.cpp / Unsloth GGUF` or DeepSeek profile in **Settings → AI/LLM**, then assign it as the **Flashcard generation** default.

For a multimodal llama.cpp test, use a model/server configuration whose `/props` response declares image support. A text-only profile still receives locally extracted page text, but not the rendered page image.

1. Start the configured model server.
2. Run `./run-app.sh`.
3. Open **Documents**.
4. Confirm the intended provider profile, model, and network destination remain the configured Flashcard generation default in Settings.
5. Select **Upload PDF** and choose a small PDF with selectable text and at least one diagram or table.
6. Keep the Documents screen open while processing runs.
7. Verify the document row reports extracted/rendered page progress and eventually reports **Deck created**.
8. Open **Decks** and verify a new deck named from the PDF filename exists.
9. Open several generated cards. Verify the content is editable and the cards have fresh schedules.
10. Open the source through **Open PDF** and verify the operating system opens the managed copy.
11. Delete one generated card and confirm ordinary card deletion works.
12. Delete the generated deck. Return to Documents and verify the document reports **Ready to regenerate** and offers **Retry generation**.
13. Retry and verify a new normal deck is created without re-uploading the PDF.

## PDF-backend compatibility acceptance

The active PR #38 engine is `justpdf`, but the document feature is intentionally insulated from that choice by `src-tauri/src/pdf_backend.rs`. Compatibility testing should therefore be maintained as an engine-neutral corpus rather than encoding justpdf-specific assumptions into document logic.

For each candidate backend (`justpdf`, a future `oxidize-pdf` implementation, a future `pdfium-render` implementation, or a platform-native implementation), use the same corpus and record:

- whether the PDF opens;
- page count;
- whether text extraction succeeds per page;
- whether rendering succeeds per page;
- whether extracted text is meaningfully usable;
- whether every rendered page has a valid PNG signature;
- processing time and peak-memory observations for large files;
- exact safe backend error category for failures.

Include ordinary PDFs plus real-world edge cases such as unusual object layouts, incremental updates, embedded fonts, rotated pages, image-heavy/scanned pages, complex diagrams, and PDFs produced by different authoring tools. If compatibility evidence shows the current engine is inadequate, implement another `PdfBackend` without changing the managed-document database/provider/card contracts.

## Failure and recovery checks

### No task default

Clear the Flashcard generation profile in Settings, then upload a PDF. The managed PDF should remain in Documents with a safe error. Configure a valid profile and use **Retry generation**.

### Provider unavailable

Stop the provider during generation. The document should remain available with a normalized provider error and a retry action. No partial deck should exist.

### Invalid structured output

Use a deterministic test server or intentionally incompatible model that returns malformed card JSON. The task should make one bounded retry against the same profile/model. If the second response is still invalid, it should fail before authoring. No deck, note, card, tag, or schedule rows should be partially committed.

### Application restart

Close ostudy while a document is processing. On the next start, the interrupted document/job should be reconciled to a retryable failed state rather than resuming provider transmission automatically.

### Missing or corrupted rendered page

After a failed generation in a disposable library, remove or alter one managed page PNG and select **Retry generation**. ostudy should rebuild extracted text and page renders from the immutable managed PDF, verify each rebuilt render against its byte-size and SHA-256 ledger values before provider transmission, and then continue generation.

### Duplicate PDF

Upload the exact same PDF bytes twice. The second import should be rejected as an existing managed document. Use the existing document row rather than creating a duplicate source copy.

## Backup and restore acceptance

1. Generate a deck from a PDF.
2. Export a complete `.ostudybackup` from Settings.
3. Confirm no provider credential is present in the package through the existing secret-sentinel tests; never inspect a real credential manually.
4. Restore the backup in a disposable test library.
5. Restart ostudy when prompted.
6. Verify the document row, source PDF, rendered pages, generated deck, cards, schedules, and document-to-deck provenance are present.
7. Verify **Open PDF** works after restore.

A backup is intentionally blocked while a document is actively processing, because the database and managed PDF/page files must be captured as one coherent library state.

## Large-document observations

The implementation processes provider requests sequentially by page and applies hard limits to source size, page count, extracted text, rendered PNG size, cards per page, and total cards. During manual testing with a large document, monitor:

- UI responsiveness while native PDF processing runs;
- application memory;
- provider request latency;
- total generated-card usefulness;
- whether text-only versus multimodal generation materially changes diagram/table cards.

Record the provider/model ID, PDF backend ID, document page count, generated-card count, and any safe error code when reporting a problem. Do not attach API keys, raw Authorization headers, full provider responses, or copyrighted source PDFs to a public issue.
