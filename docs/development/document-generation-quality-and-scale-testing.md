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

Retest the same kind of ~11-page PDF that previously failed with `The AI provider rejected the request.`

1. Start the configured llama.cpp / Unsloth server and verify the saved profile/model.
2. Upload the PDF.
3. Confirm the server accepts the streamed request and generation begins.
4. If the server rejects the normal JSON-Schema request as unsupported, confirm ostudy retries once against the **same** provider/model in JSON-object compatibility mode.
5. Confirm the task either generates a valid deck or produces a specific safe compatibility/model error rather than the previous generic rejection.
6. Verify no cloud/provider fallback occurred.

The provider timeout ends when successful response headers arrive; slow first-token and completion times after that are allowed.

## Large 138-page / complex-PDF acceptance

Retest the same large PDF that previously ended with `The local database operation failed.` with both providers.

During local processing verify the Documents row displays a muted phase label such as:

- **Extracting text / rendering pages**;
- **Rendering pages**;
- **Building outline / generating cards**.

The page counter may remain `0/—` during the first blocking extraction/render stage; the phase label is the truthful progress signal for that interval.

Confirm:

- the PDF does not fail merely because parser metadata contains long heading paths/font names or unusual/non-finite geometry;
- local semantic metadata is bounded before SQLite persistence;
- the job proceeds into outline/generation when local processing succeeds;
- a real PDF rendering/extraction failure, if one occurs, reports a PDF-specific safe error instead of an opaque database error;
- Stop generation remains available once the document is in an active cancellable generation task.

If this exact source still produces a database error, capture the **ostudy diagnostic log entry/code around the failure**; do not include provider credentials or the copyrighted source file in a public report. That would identify which remaining SQLite boundary is involved.

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
