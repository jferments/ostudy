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

The normal request uses llama-server's schema-constrained JSON response format. The JSON Schema is lowered by llama-server into constrained sampling. If an older llama-server rejects that request shape, ostudy retries once against the **same provider and model** using the server's top-level JSON-Schema compatibility field. The retry remains schema constrained; it must never fall back to unconstrained JSON plus prompt instructions.

The document SSE reader primarily accepts OpenAI-compatible `choices[].delta.content`, but it also accepts safe compatible llama-server variants used by some versions/wrappers: a final `choices[].message.content`, completion `text`, or native top-level `content`. If a full final snapshot follows already-streamed deltas, ostudy must not duplicate it.

Retest the ~11-page PDF that previously failed with:

`The model returned no final document-task content.`

Confirm:

- the server accepts either the normal schema request or one same-model schema compatibility retry;
- a streamed structured result is recognized whether the server emits deltas or a supported final-message/native-content form;
- if the provider still closes without usable output, the new safe error includes the observed `finish_reason` when available;
- no provider/model fallback occurs;
- canonical card authoring still performs independent local validation before persistence.

### DeepSeek

Document tasks use DeepSeek's Beta **strict Function Calling** mode rather than ordinary JSON Output. The function is forced, `strict: true` is enabled, and ostudy supplies a DeepSeek-compatible JSON Schema. The strict schema adapter converts the internal schema to DeepSeek's supported subset while local Rust validation retains tighter count/length/semantic rules.

Initial outline source windows are smaller than before (about 3,200 estimated source tokens / 8 pages), the requested fragment itself is concise/bounded, and DeepSeek receives a 6,144-token outline completion allowance. These changes target the real failure:

`The AI provider reached its completion limit before finishing the structured document result.`

Confirm:

- outline, card-generation, curation, and bounded global-synthesis calls return forced structured tool arguments;
- the outline proceeds through multiple small fragments without hitting `finish_reason=length` on ordinary textbook windows;
- if a request does hit `length`, ostudy reports `ai_document_output_truncated` with the exact source window rather than a generic schema mismatch;
- one bad semantic card candidate does not invalidate otherwise useful cards from the same window;
- no provider fallback occurs.

## Bounded hierarchical large-document analysis

For the ~138-page document, initial outline windows are capped at about **3,200 estimated source tokens and 8 pages**, with small overlap. Later initial and Generate-more card generation uses about **5,000 tokens / 10 pages**.

The authoritative section hierarchy is assembled natively from validated local outline fragments. Chapter/section names and page spans should survive when the model identifies them, but bad or missing chapter boundaries must not make the document unprocessable.

There is no recursive LLM request containing several complete outline objects. Instead, after native hierarchy assembly, ostudy constructs a compact whole-document digest that reserves space for section coverage from the beginning through the end. One bounded global-synthesis request may refine only the whole-document summary, major learning objectives, and key concepts. It cannot rewrite section/page hierarchy, and a global-synthesis failure is non-fatal.

Later flashcard requests receive that bounded global learning context plus only the outline sections relevant to the local source window.

If local outline generation fails, the surfaced error and diagnostic log should identify exact source window, total window count, PDF page range, approximate source-token count, provider error code, and bounded validation detail.

When comparing the 11-page and 138-page cases, remember that **Generate more cards** reuses a persisted outline. Its success is a later-generation test, not a test of first-pass outline construction.

## Whole-document view acceptance

After a successful large-document first pass:

1. Inspect application diagnostics for `document_outline_assembled` and either `document_global_outline_synthesized` or `document_global_outline_synthesis_skipped`.
2. If synthesis succeeds, generated cards from later chapters should still be ranked against whole-document objectives rather than only local headings.
3. If synthesis is deliberately made unavailable/fails, native outline assembly should remain valid and generation should continue rather than failing the document.
4. Confirm the model-facing whole-document digest covers sections near both the beginning and end of the source; the automated unit test protects this invariant for a 1–138-page example.
5. Confirm no request contains the entire raw PDF or several complete outline JSON objects.

## Card direction acceptance

Use a successful text-heavy PDF generation with either DeepSeek or llama.cpp / Unsloth.

Inspect at least 20 generated cards and verify:

- ordinary conceptual questions show the question first and answer/explanation second;
- multiple-choice and critical-thinking/application cards are one-way;
- a descriptive answer is never shown as the Practice question merely because a card was marked reversible;
- only straightforward term/definition cards produce reverse presentations;
- definition cards read naturally in both directions.

A provider cannot set Basic `reverseEnabled`; Rust owns the final direction rule.

## Large 138-page / complex-PDF acceptance

Retest the same large PDF that previously reached local processing but failed during outline generation, using both providers.

During local processing verify the Documents row transitions through truthful state:

- initially **Extracting text** while total page count is not yet known;
- once structured extraction determines the count, the metric changes to `0/<total> rendered` and phase becomes **Rendering pages**;
- while rasterization continues, the metric advances through `x/<total> rendered` instead of remaining `0/—` until completion;
- after rendering, generation/outline phases take over.

Confirm:

- the PDF does not fail merely because parser metadata contains long headings/font names or unusual/non-finite geometry;
- local semantic metadata is bounded before SQLite persistence;
- the job proceeds into many bounded outline fragments;
- provider output stays inside the enforced structured-output boundary;
- bounded global synthesis provides whole-document semantic context without becoming a required merge step;
- later generation requests use only relevant outline sections instead of embedding the entire hierarchy;
- a PDF rendering/extraction failure reports a PDF-specific safe error rather than an opaque database error;
- provider output truncation reports the dedicated truncation code/detail;
- Stop generation remains available once the document is in an active cancellable provider task.

If the exact source still fails, capture the visible safe error plus the `document_outline_window_failed` diagnostic event. For llama.cpp empty-output failures, also capture the `finish_reason` included in the updated error. Do not attach raw source text, provider bodies, API keys, or Authorization headers.

## Parallel rendering and live-progress acceptance

Retry the large PDF with the parallel renderer backend.

1. Observe CPU utilization while pages rasterize; several cores should be active.
2. Record elapsed time from upload/retry until rendering completes.
3. Confirm total page count appears before rendering finishes.
4. Watch the document metric advance incrementally, for example `0/138`, `8/138`, `27/138`, ... `138/138`.
5. Confirm every physical page is ultimately rendered exactly once and page ordering remains correct.
6. Open the managed PDF and inspect representative generated content afterward.
7. Exercise Stop/failure/retry to confirm derivative integrity remains intact.

The rasterizer uses at most `min(logical CPU availability, page count, 16)` workers. PNG rasterization is concurrent, while managed-file/SQLite persistence remains ordered. The UI polls active documents, so it may advance by more than one page between visible refreshes even though the database counter is updated after each ordered committed render.

## Document deletion / citation acceptance

### Surviving generated deck

1. Generate a deck from a PDF and leave the deck active.
2. Delete the PDF from Documents.
3. Confirm the generated deck remains.
4. Confirm exactly that surviving deck receives the detached text source citation.
5. Confirm the deletion toast reports citation preservation.

### Generated deck deleted first

1. Generate a deck from a PDF.
2. Delete the generated deck through ordinary deck deletion.
3. Return to Documents and delete the PDF.
4. The confirmation copy must say that no active generated deck remains and no citation will be preserved.
5. The success toast must simply say **PDF deleted from Documents.**
6. No soft-deleted deck description should be modified, and backend `preservedDeckCitationCount` must be zero.

This protects the distinction between durable provenance rows and surviving user-visible decks.

## AI Settings layout acceptance

Open **Settings → AI/LLM** at desktop width.

- **Default AI model** must appear at the top of the left column.
- **Provider profiles** must follow immediately beneath it with the normal small column gap; there must not be a large blank vertical region between the two cards.
- The provider editor remains in the right column.
- At narrow widths the blocks should stack without horizontal overflow.

## Documents UI acceptance

The Documents header should remain compact:

- it displays only **Current Flashcard Generation model** and the configured model name;
- it does not repeat provider kind/network/transmission explanatory copy;
- selecting a PDF shows **Large documents can take several minutes.** as a toast;
- no old persistent long-document paragraph returns;
- small processing phase labels remain visible during active work.

## Generate more cards acceptance

After a document has a successful generated deck:

1. Record initial card count and representative topics.
2. Select **Generate more cards**.
3. Confirm Documents changes to Processing and shows **Generating additional cards**.
4. Confirm the second pass reuses retained outline/source.
5. Verify cards append to the **same deck**.
6. Inspect new cards for important material not already meaningfully tested.
7. Confirm exact duplicates are skipped by canonical authoring.
8. Run another pass and verify it sees expanded deck coverage.
9. If no worthwhile material remains, confirm an informational result rather than data corruption.
10. Stop an additional pass and verify existing cards remain intact.

Generate-more now uses the same 5,000-token / 10-page generation bound as the first generation pass.

## Active Practice navigation acceptance

1. Start a Practice session with several cards.
2. Rate at least two cards.
3. Click a sidebar destination such as **Decks**.
4. Confirm **Terminate this practice session?** appears and explains that completed reviews remain durable while post-review analytics will not open automatically.
5. Cancel and verify the session remains active.
6. Try again and choose **Yes, terminate**.
7. Confirm navigation occurs only after native session completion succeeds.
8. Verify no normal Practice-complete results modal is opened by navigation termination.
9. Later verify submitted reviews still affected history/scheduling.
10. If a rating is in an uncertain retry state, navigation should remain blocked until it is resolved.

The ordinary **End session** button still uses the normal Practice-completion flow.

## Regression checks

Also re-run:

- long-running streamed generation beyond the configured initial-response timeout;
- Stop generation;
- Open PDF;
- Archive versus Delete PDF;
- detached textual source citation after PDF deletion;
- pre-delete and post-delete backup/restore;
- image-capable llama.cpp and image-only PDFs;
- bibliography/trivia suppression and cross-page conceptual quality.

See `document-streaming-and-deletion-testing.md` and `pdf-flashcard-generation-testing.md` for related detailed steps.
