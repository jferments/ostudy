# Stage 4.7 large-document generation testing

## Automated qualification

```bash
git fetch origin --prune
git switch agent/stage-4-7-large-document-resilience
git pull --ff-only

pnpm install
./scripts/check-fast.sh
./scripts/check.sh
./run-app.sh
```

## Small-document DeepSeek regression

Use the representative ~11-page PDF that previously completed at roughly 45,000 aggregate DeepSeek tokens.

Confirm:

- first-pass generation succeeds;
- `document_ai_scale_plan` reports `scale=standard`;
- card quality/direction remain correct;
- actual aggregate usage remains in the same broad range rather than unexpectedly multiplying;
- no repeated completion-limit retries occur.

## 48-page llama.cpp / Unsloth regression

Use the representative ~48-page source and Qwen 3.6 27B configuration.

Confirm:

- the document remains on the standard/conservative local policy;
- extraction/rendering and generation succeed;
- 96-DPI page evidence is adequate for diagrams/tables/equations when image input is enabled;
- local inference never silently falls back to a cloud provider.

## 138-page DeepSeek acceptance

Before starting, record the provider's usage counter.

Inspect the safe `document_ai_scale_plan` diagnostic and record:

- `pages`;
- `source_tokens`;
- `outline_windows`;
- `generation_windows`;
- `card_capacity`;
- `estimated_job_tokens`;
- `ceiling`.

Expected behavior:

- `scale=large`;
- semantic partitions are chapter-scale, targeting roughly 40–60 pages, but provider windows within them remain bounded;
- there is **no 120-card document cap**;
- first-pass card capacity is approximately generation-window count × 16, so a ~138-page document can have capacity for several hundred candidates;
- estimated aggregate tokens remain below the selected DeepSeek safety ceiling before provider work begins;
- a source-window truncation can trigger at most one compact retry for that window;
- a failed/invalid outline fragment in a multi-window source may use a native semantic fallback instead of repeatedly spending provider tokens;
- final curation failure is non-fatal to already validated candidates.

After completion, compare actual aggregate DeepSeek usage with the previous ~1.45-million-token failed run. Record generated card count and elapsed time as well.

If the preflight produces `document_ai_budget_exceeded`, record its estimate and source/window counts. The intended user action is to split the PDF at a chapter/section boundary rather than spend through an obviously excessive automatic run.

## Interruption and retry checkpoint acceptance

Use a document large enough to create multiple outline and generation windows.

1. Start generation and let several outline windows complete.
2. Stop generation or stop the provider.
3. Retry the same retained document without re-uploading it.
4. Verify logs contain `document_outline_window_reused` for completed matching outline windows.
5. During generation, interrupt again after multiple card windows complete.
6. Retry.
7. Verify logs contain `document_generation_window_reused` for those completed validated windows.
8. Confirm source PDF extraction, page rasterization, and already completed matching provider work are not repeated unnecessarily.

Change the provider/model/profile configuration and retry. Matching checkpoints must **not** be reused across a changed contract.

## Generate-more reuse

After a successful first pass:

1. note the generated card count;
2. choose **Generate more cards**;
3. confirm the existing `document_outlines` row and semantic chunks are reused rather than rebuilding extraction/initial outline work;
4. confirm new cards append to the same deck and focus on coverage gaps;
5. run another pass and verify the expanded deck is used as the new coverage constraint.

Generation checkpoints from the initial pass need not remain after the canonical deck commits because the generated cards themselves are now durable. The expensive reusable preprocessing consists of the managed source, structured extraction/semantic chunks, and final persisted outline.

## Retry-spend acceptance

For a deterministic provider fixture or a model likely to truncate one generation window:

- verify a normal request is made;
- verify at most one `document_generation_window_compact_retry` (or additional-pass equivalent) is logged for that window;
- verify there is no loop of successively smaller provider requests;
- after the compact retry fails, surface the safe error and preserve all previously committed checkpoints.

A schema-invalid/empty result may use the adapter's separate single contract-repair attempt. That path must also remain bounded to one repair.

## Explicit automatic limits

Verify a document beyond the automatic page/source ceiling is retained in Documents but generation fails locally with one of:

- `document_generation_page_limit`;
- `document_generation_source_limit`;
- `document_ai_budget_exceeded`.

No expensive outline sequence should begin after the whole-job budget guard rejects the plan.

## 96-DPI visual derivative

Use a PDF containing small labels, equations, a table, and a diagram with a verified image-capable llama.cpp profile.

Verify:

- the original PDF opened through **Open PDF** remains unchanged;
- derived page renders are rebuilt under the new renderer backend identity;
- relevant page images remain legible enough for grounding;
- multimodal local generation remains useful.

DeepSeek text-only generation should show no behavior dependency on the raster DPI.

## Merge acceptance

Before squash-merging PR #39:

- automated `./scripts/check.sh` is green;
- 11-page DeepSeek regression passes;
- 48-page llama.cpp regression passes;
- representative 138-page DeepSeek either completes below the new budget or is rejected locally before an excessive provider run;
- interruption/retry proves checkpoint reuse;
- Generate-more proves preprocessing/outline reuse;
- no recursive retry behavior is observed;
- PDF deletion/citation, Stop generation, Practice navigation, and backup/restore regressions from PR #38 still pass.
