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

The automated suite covers:

- schema migration 17 and immutable migration checksums;
- typed document IPC parity;
- real local PDF text extraction and PNG page rendering;
- bounded managed source/page paths, signatures, sizes, and SHA-256 values;
- deterministic llama.cpp multimodal request lowering through a loopback server;
- strict Basic/Cloze response parsing and rejection of malformed output;
- canonical generated-import rollback when document provenance cannot commit;
- document source/page backup and restore;
- retry and deleted-generated-deck behavior;
- responsive Documents navigation and direct upload UI behavior.

Normal CI never uses a real API key or internet provider.

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
9. Open several generated Basic and Cloze cards. Verify the content is editable and the cards have fresh schedules.
10. Open the source through **Open PDF** and verify the operating system opens the managed copy.
11. Delete one generated card and confirm ordinary card deletion works.
12. Delete the generated deck. Return to Documents and verify the document reports **Ready to regenerate** and offers **Retry generation**.
13. Retry and verify a new normal deck is created without re-uploading the PDF.

## Failure and recovery checks

### No task default

Clear the Flashcard generation profile in Settings, then upload a PDF. The managed PDF should remain in Documents with a safe error. Configure a valid profile and use **Retry generation**.

### Provider unavailable

Stop the provider during generation. The document should remain available with a normalized provider error and a retry action. No partial deck should exist.

### Invalid structured output

Use a deterministic test server or intentionally incompatible model that returns malformed card JSON. The task should fail before authoring. No deck, note, card, tag, or schedule rows should be partially committed.

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

Record the provider/model ID, document page count, generated-card count, and any safe error code when reporting a problem. Do not attach API keys, raw Authorization headers, full provider responses, or copyrighted source PDFs to a public issue.
