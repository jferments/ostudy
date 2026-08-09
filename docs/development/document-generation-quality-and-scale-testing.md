# Document generation quality, scale, second-pass, and Practice-exit testing

Use this checklist for the PR #38 follow-up motivated by real DeepSeek / Unsloth testing.

## Automated qualification

```bash
git fetch origin
git switch agent/stage-4-3-4-6-pdf-flashcard-generation
git pull --ff-only

pnpm install
./scripts/check-fast.sh
./scripts/check.sh
./run-app.sh
```

## Provider-enforced structured output

Document generation must not depend on prose alone to produce parseable card objects.

### llama.cpp / Unsloth

The normal request uses llama-server's schema-constrained JSON response format. The JSON Schema is lowered by llama-server into its constrained sampling grammar. If an older llama-server rejects that request shape, ostudy retries once against the **same provider and model** using the server's top-level JSON-Schema compatibility field. The retry remains grammar/schema constrained; it must never fall back to unconstrained JSON plus prompt instructions.

Retest the same kind of ~11-page PDF that previously failed with `The AI provider rejected the request.` and also the large ~138-page source that previously reached generation but ended with an ostudy contract error.

Confirm:

- the server accepts either the normal schema request or the one schema-constrained compatibility retry;
- no provider/model fallback occurs;
- the model cannot add unknown card fields or unsupported card types outside the supplied schema;
- if an individual candidate is structurally valid but fails a narrower ostudy semantic rule (for example an invalid Cloze blank), that candidate is dropped instead of causing the entire source window to be regenerated twice;
- canonical card authoring still performs its independent local validation before persistence.

### DeepSeek

Document tasks use DeepSeek's Beta **strict Function Calling** mode rather than ordinary JSON Output. The function is forced, `strict: true` is enabled, and ostudy supplies a DeepSeek-compatible JSON Schema. The strict schema adapter converts the internal schema to DeepSeek's supported subset (including `anyOf` variants and all-properties-required objects) while local Rust validation retains the tighter length/count rules that the provider's strict schema does not support.

Retest the large ~138-page source that previously failed with `The AI provider twice returned flashcards that did not match ostudy's contract.`

Confirm:

- outline, card-generation, and curation calls return forced structured tool arguments;
- the task proceeds through multiple generation windows without prose/Markdown responses leaking into the JSON path;
- one bad semantic candidate does not invalidate otherwise useful cards from the same window;
- no provider fallback occurs.

## Bounded hierarchical large-document analysis

The initial outline pass is now intentionally different from the old large-request architecture.

For the ~138-page document, verify that ostudy performs **many small outline fragments** rather than one or a few very large prompts. Each outline source window is capped at about 5,000 estimated source tokens and 10 pages. There should be no separate LLM call that sends several complete outline fragments back to the model for a global merge.

The global document outline is assembled natively from those fragment objects. Chapter/section names and page spans should survive when the model identifies them, but bad or missing chapter boundaries must not make the document unprocessable. Later flashcard generation also uses roughly 5,000-token / 10-page semantic windows and receives only the global summary plus outline sections relevant to that page range.

If outline generation fails, the surfaced error and diagnostic log should now identify the exact source window, total window count, PDF page range, approximate source-token count, provider error code, and bounded validation detail. A provider `finish_reason=length` should be reported as **`ai_document_output_truncated`**, not as a generic schema/contract mismatch.

When comparing the 11-page and 138-page cases, remember that **Generate more cards** reuses a persisted outline. Its success is therefore a generation-path test, not a test of first-pass outline construction.

## Card direction acceptance

Use a successful text-heavy PDF generation with either DeepSeek or llama.cpp / Unsloth.

Inspect at least 20 generated cards and verify:

- ordinary conceptual questions show the question first and the explanation/answer second;
- multiple-choice and critical-thinking/application cards are one-way;
- a descriptive answer is never shown as the Practice question merely because the card was marked reversible;
- only straightforward term/definition cards produce reverse presentations;
- definition cards read naturally in both directions (term → definition and definition → term).

A provider cannot set Basic `reverseEnabled`; Rust owns the final direction rule.

## Small Unsloth compatibility acceptance

1. Start the configured llama.cpp / Unsloth server and verify the saved profile/model.
2. Upload the PDF.
3. Confirm the server accepts the streamed request and generation begins.
4. If the server rejects the normal schema request as unsupported, confirm ostudy retries once against the **same** provider/model with another schema-constrained llama-server request shape.
5. Confirm the task either generates a valid deck or produces a specific safe compatibility/model error rather than the previous generic rejection.
6. Verify no cloud/provider fallback occurred.

The provider timeout ends when successful response headers arrive; slow first-token and completion times after that are allowed.

## Large 138-page / complex-PDF acceptance

Retest the same large PDF that previously ended with `The local database operation failed.` and later with the generic outline-contract message, using both providers.

During local processing verify the Documents row displays a muted phase label such as:

- **Extracting text / rendering pages**;
- **Rendering pages**;
- **Building outline / generating cards**.

The page counter may remain `0/—` during the first blocking extraction/render stage; the phase label is the truthful progress signal for that interval.

Confirm:

- the PDF does not fail merely because parser metadata contains long heading paths/font names or unusual/non-finite geometry;
- local semantic metadata is bounded before SQLite persistence;
- the job proceeds into many bounded outline fragments when local processing succeeds;
- no model-generated outline-of-outlines merge occurs;
- provider output stays inside the enforced structured-output boundary during long multi-window generation;
- later generation requests use only relevant outline sections instead of embedding the entire global outline;
- a real PDF rendering/extraction failure, if one occurs, reports a PDF-specific safe error instead of an opaque database error;
- provider output truncation reports the dedicated truncation code/detail instead of `document outline did not match contract`;
- Stop generation remains available once the document is in an active cancellable generation task.

If the exact source still fails, capture the safe error text plus the `document_outline_window_failed` diagnostic event. That event should identify the failing pages/window without including raw PDF text, provider response bodies, or credentials.

## Parallel rendering acceptance

Retry the large PDF with the new renderer backend. Because the renderer backend identity changed, existing page derivatives should be rebuilt automatically.

Compare the local extraction/render phase with the previous serial build:

1. Observe overall CPU utilization while pages are rasterizing.
2. Confirm several CPU cores are busy rather than one core doing all page rasterization.
3. Record elapsed time from upload/retry until page rendering completes.
4. Confirm the finished document still reports every physical page rendered exactly once and in the correct order.
5. Open the managed PDF and inspect several generated cards/images after processing to ensure concurrency did not affect page identity.
6. Exercise Stop/failure/retry afterward to confirm ordered persistence and derivative integrity remain intact.

The rasterizer uses at most `min(logical CPU availability, page count, 16)` workers. PNG rasterization is concurrent, while the existing page-file/SQLite persistence visitor remains ordered. Do not expect exactly 100% CPU usage on every PDF: image decoding, memory bandwidth, filesystem work, and the complexity of individual pages can still limit scaling.

## Documents UI acceptance

The Documents header should stay deliberately compact:

- it displays only **Current Flashcard Generation model** and the configured model name;
- it does not repeat provider kind, network scope, or provider-transmission explanatory text;
- selecting a PDF shows the toast **Large documents can take several minutes.**;
- the document row does not contain the old persistent long-running-generation paragraph;
- the small processing phase label remains visible during active work.

## Generate more cards acceptance

After a document has a successful generated deck:

1. Record the initial card count and inspect representative topics already tested.
2. Select **Generate more cards**.
3. Confirm Documents changes to Processing and shows **Generating additional cards**.
4. Confirm the second pass reuses the retained outline/source rather than uploading/importing another PDF.
5. When it completes, verify cards were appended to the **same deck**, not a replacement deck.
6. Inspect the new cards and confirm they preferentially cover important information not already meaningfully tested.
7. Confirm paraphrases/exact duplicates are limited; canonical duplicate handling should skip exact duplicates.
8. Run **Generate more cards** a second time and verify the later pass considers the expanded deck coverage as well.
9. If no worthwhile extra material remains, confirm ostudy reports that as informational rather than corrupting the existing deck.
10. Start another additional pass and press **Stop generation**; verify the prior deck remains unchanged and usable.

If multiple active decks share the generated deck's name, additional generation must fail safely rather than guessing a destination.

## Active Practice navigation acceptance

1. Start a Practice session with several cards.
2. Rate at least two cards so their reviews/schedules are durably committed.
3. Click a sidebar destination such as **Decks**.
4. Confirm a dialog asks **Terminate this practice session?** and explains that completed reviews are retained and post-review analytics will not be opened automatically.
5. Choose Cancel/No and verify the Practice session remains active.
6. Try leaving again and choose **Yes, terminate**.
7. Confirm navigation proceeds only after native session completion succeeds.
8. Verify no Practice-complete analytics/results screen opens as part of that navigation.
9. Re-open the practiced cards/analytics later and verify the reviews completed before termination still affected review history and scheduling.
10. If a rating is in a pending/uncertain retry state, verify navigation is blocked until that review is safely resolved.

The ordinary **End session** button still uses the normal Practice-completion experience and may show the usual results/analytics flow.

## Regression checks

Also re-run the existing acceptance cases for:

- long-running streamed generation beyond the configured initial-response timeout;
- Stop generation;
- Open PDF;
- Archive versus Delete PDF;
- detached textual source citation after PDF deletion;
- pre-delete and post-delete backup/restore;
- image-capable llama.cpp and image-only PDFs;
- bibliography/trivia suppression and cross-page conceptual quality.

See `document-streaming-and-deletion-testing.md` and `pdf-flashcard-generation-testing.md` for those detailed steps.
