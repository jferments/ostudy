# Generation efficiency, reuse, and usage diagnostics

This document owns the operational contract for diagnosing paid AI work in oStudy.

The goals are:

- generate from bounded, source-grounded inputs without repeatedly transmitting unrelated evidence;
- persist admitted work early enough that sibling failure or application restart does not force repayment;
- reuse unchanged whole guides and unchanged collection windows locally before contacting a provider;
- keep provider prompt caching measurable but distinct from local result reuse;
- account for every physical provider request when usage is available;
- never invent zero token usage for failed or interrupted requests whose provider usage is unknown.

## Generation data flow

Document interpretation remains the source-authoritative path:

```text
canonical source
  -> retained knowledge/evidence
  -> optional collection organization
  -> bounded collection guide windows
  -> admitted section checkpoints
  -> published guide + whole-guide manifest
```

Collection flashcards continue to reuse admitted document flashcards; collection guide work must not weaken or replace document generation quality gates.

A collection guide has three reuse layers:

1. **Reducer receipt** — exact source analysis + effective provider/model/reasoning contract + reducer request. A successful reduction or a completed stable rejection/native fallback is reusable. Cancellations and transient transport failures are not.
2. **Section checkpoint** — exact authoring contract + collection title + stable topic window + relationship context + scoped knowledge/evidence. Source and grounding hashes are revalidated before reuse.
3. **Whole-guide manifest** — exact guide contract + retained collection synthesis + ordered section identities. A matching active guide satisfies a new artifact job without another provider request.

Whole-guide manifests are committed in the same transaction as guide publication. Section checkpoints are persisted immediately after each section is admitted, before sibling sections or whole-guide publication finish.

## Stable collection windows

Collection topics remain in source order, but window boundaries are content-defined rather than fixed offsets. A small insertion therefore changes the local window until the next stable topic anchor instead of shifting every later 18-topic chunk.

Every topic is still represented. Each window is bounded by the configured maximum topic and knowledge-unit limits; this optimization changes partition stability, not coverage.

## What is accounted

The durable generation ledger is intentionally content-free.

Representative stages include:

- `knowledge_extraction`
- `document_synthesis`
- `collection_synthesis`
- `flashcard_authoring`
- `flashcard_curation`
- `guide_authoring`
- `artifact_metadata`
- `embedding_indexing`
- `embedding_query`

One `generation_requests` row represents one physical provider attempt at the instrumented transport boundary. A transient HTTP retry therefore produces another attempt row instead of being hidden inside the first request.

For provider responses, the ledger stores only operational metadata such as provider/model, logical request key, attempt number, timing, finish reason, provider response ID, usage, cost when reported, and content-free payload byte measurements. It never stores prompt text, source excerpts, generated answer text, credentials, or authorization headers.

Embedding requests are non-generative. Their output and reasoning token counts are recorded as zero by protocol; provider input/total/cache/cost fields remain provider-reported. Unknown fields remain null.

## Provider-native prompt caching

Local result reuse is always preferred because it avoids a provider request entirely. When a document task does need OpenRouter, oStudy supplies one content-free `session_id` for the generation operation when the selected profile does not require zero-data-retention routing. This keeps related requests on one provider endpoint so provider-native prompt caches can stay warm across repeated stable prefixes such as system instructions, schemas, global outlines, and curation context.

The session key contains only the opaque oStudy operation ID. It is not source text, a document title, or a user identifier. ZDR-required OpenRouter profiles do not opt into this sticky-routing optimization.

oStudy does **not** enable OpenRouter edge response caching for study-material requests. Local durable checkpoints already provide no-provider-call reuse without introducing a separate remote response-retention layer. Prompt-cache effectiveness is observed through provider usage instead.

Do not infer savings from configuration. Verify actual `cached_input_tokens` and cache-read coverage in the generation usage report.

## Curation efficiency

Native card admission already removes unsupported or malformed candidates before global curation. Exact semantic duplicate candidates are also removed locally **before** curation batches are sent to the provider. This preserves one representative card while avoiding repeated input/output spent asking the curation model to reject duplicates that oStudy can identify deterministically.

The curation model remains a separate quality gate for global relevance, question/answer coherence, learning-objective coverage, and non-exact semantic redundancy. Do not remove that quality pass solely to lower token usage without measured evidence from the stage reporter.

## Exact token percentages versus payload diagnostics

Provider-reported token usage is authoritative for billing attribution.

For stage (s):

```text
input share(s)  = reported input tokens for s / all reported input tokens
output share(s) = reported output tokens for s / all reported output tokens
```

Input and output use independent denominators.

If every request in a stage is missing input or output usage, that percentage is **unknown**, not 0%. If only some requests lack usage, percentages are explicitly shares of reported usage and the missing-request count remains visible.

Reasoning tokens are a subset of output tokens. Cached-input tokens are a subset of input tokens. They are never added to total tokens a second time.

Payload composition is a separate diagnostic. oStudy stores byte counts for application-owned components such as instructions, schemas, knowledge units, grounding evidence, structural evidence, source windows, collection digests, repair drafts, embedding inputs, image URLs, and visible response size. These byte measurements help identify prompt inflation but are **not provider-billed token counts** and must never be presented as exact token allocation inside a prompt.

## Reporter

The reporter opens the active local library read-only and selects the latest generation operation by default:

```bash
python3 scripts/generation-usage.py
python3 scripts/generation-usage.py --json
python3 scripts/generation-usage.py --list
python3 scripts/generation-usage.py --all --json
```

No database path or operation ID is required for the ordinary diagnostic workflow. `--database` and `--operation` remain available as explicit overrides for archived/benchmark libraries or an older retained operation. The text report gives stage-level input/output shares, retry shares, provider cache-read coverage, reasoning share, local reuse, and content-free payload composition. It also reports repeated-attempt provider time in two forms: **cumulative request time** (which double-counts concurrent requests) and **active wall-clock time** (the union of intervals in which at least one repeated attempt was in flight). Neither number is presented as guaranteed wall-clock savings because sibling requests can overlap. JSON additionally exposes request records, per-operation shares, and the largest reported input/output requests.

A useful token-spike investigation is:

1. Find the affected operation or use `--all --json` over the retained ledger.
2. Check `usageComplete`. Do not draw bill totals from an incomplete report without noting missing requests.
3. Identify the stages with the largest `percentOfReportedInput` and `percentOfReportedOutput`.
4. Inspect `largestReportedInputRequests` and repeated-attempt shares.
5. Compare `cacheReadPercentOfCoveredInput` with `cacheReadCoverageRequests`.
6. Check `localReuse`: local reuse means no provider request; provider prompt-cache hits still incur a request.
7. Use `payloadByteDiagnostics` on the expensive stage/request to distinguish source/evidence/schema/repair inflation. Treat these as byte diagnostics only.
8. If retries dominate, diagnose their safe error / finish reason before raising token ceilings or request counts.

## P7 regression and performance acceptance

Performance acceptance compares like-for-like runs. Do not compare a cold first run
with a warm provider-cache run, a checkpoint resume, or an unchanged local-reuse
run and call the difference an optimization.

Generate content-free reports for the baseline and current implementation:

```bash
python3 scripts/generation-usage.py --database /path/to/baseline/library.sqlite3 \
  --operation <baseline-operation-id> --json > baseline-cold.json
python3 scripts/generation-usage.py --database /path/to/current/library.sqlite3 \
  --operation <current-operation-id> --json > current-cold.json
```

The report now includes the durable generation-operation wall-clock when both
`started_at` and `finished_at` are available. Provider token usage remains
authoritative for token comparisons; incomplete usage is not treated as zero.

Copy
`src-tauri/tests/fixtures/document-generation/quality-acceptance-template.json`
and replace its placeholder scores with benchmark or human-review evidence for
the same source/model/provider configuration. Scores are normalized to 0–1.
The applicable quality floor is:

- source fidelity;
- important-topic coverage;
- preserved qualifications and units;
- answer-key correctness;
- useful question-type diversity;
- visual/table interpretation when the benchmark actually contains those
  materials.

Card count is not a quality metric.

Evaluate one run class at a time:

```bash
python3 scripts/generation-acceptance.py \
  --baseline baseline-cold.json \
  --current current-cold.json \
  --run-class cold \
  --quality quality-cold.json \
  --require-ready \
  --json

# Repeat separately for:
#   --run-class warm
#   --run-class resume
#   --run-class unchanged
```

`acceptanceReady` requires successful operations, complete provider usage,
known operation wall-clock, complete applicable quality evidence, and no quality
regression. The approximately 50% reductions in full processing time and reported
token use are reported separately as engineering targets. Missing those targets
does not authorize dropping requested content, weakening grounding, or reducing
the quality rubric.

For the 55-page performance case that motivated this work, compare the same
source with the same model/provider configuration. Preserve the original
approximately 17-minute / 2.3-million-reported-token observation as baseline
evidence; do not recreate or round it into invented precision if the original
content-free report is unavailable.

## Reuse and invalidation invariants

Never reuse a cached result merely because its JSON parses.

Section reuse requires:

- matching versioned authoring/model contract;
- matching exact semantic request identity;
- valid stored-result hash;
- current source and cross-source grounding hashes;
- current active source authorization;
- local Study Guide content and knowledge-support validation.

Whole-guide reuse additionally requires the current active guide, source snapshot, collection analysis, ordered section identities, and valid retained content.

Changing an unrelated collection preparation counter must not invalidate a section. Changing relevant evidence, relationships, source authorization, provider/model/reasoning contract, prompt/schema version, or section composition must.

Reducer fallback receipts are valid only for completed stable output failures. Transport outages and cancellation must remain retryable.

## Privacy and transfer

Generation ledgers, request fingerprints, reducer receipts, section checkpoints, and whole-guide manifests are device-local derived state. Trusted-device synchronization strips them from the copied database. Complete local-library backups retain them under the normal backup contract.

The usage reporter and diagnostic records are designed to be shareable without source content, but still treat operation IDs, provider/model names, response IDs, timing, and costs as diagnostic metadata.

## Acceptance checks

Before calling generation-efficiency work complete, verify:

- one transient physical retry produces two attempt rows;
- failed/interrupted requests with no usage stay unknown;
- embedding indexing/query usage appears in the same triggering operation when applicable;
- an unchanged collection guide reuses the active guide with zero generation-provider requests;
- a sibling section failure leaves already-admitted section checkpoints reusable;
- a small local topic insertion preserves later content-defined window boundaries;
- a completed stable reducer rejection is receipted and is not repaid unchanged;
- changing source evidence or the effective model/prompt contract invalidates the affected reuse;
- device-sync sanitization removes generation diagnostics and local reuse caches;
- the reporter keeps input/output denominators separate and never counts cached/reasoning tokens twice.
