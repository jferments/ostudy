# Document-generation admission and recovery

This is the live document-generation boundary, shared by DeepSeek, OpenRouter
and local providers. It is distinct from importing a user-selected `.ostudy.json`
file. See [the interactive-question contract](interactive-question-types.md) and
[AI and document processing](../architecture/ai-and-document-processing.md).

## Reject candidates, not their valid siblings

The response must be complete JSON with the expected outer `cards` array. An
invalid root, unknown root fields, more than 128 returned candidates, a transport
failure, cancellation, or truncated JSON is not a partially successful response.
Do not salvage a prefix of malformed JSON or invent closing syntax.

Within a valid bounded array, each candidate envelope is deserialized separately.
Malformed envelopes, invalid assessment shapes, unknown types, disabled families,
invalid question keys, unsupported rich content, bad cloze/tag content and invalid
source bindings reject only that candidate. Surviving cards retain their own
content and source associations; there is no positional reassignment of evidence.

Both normalized recall and typed interactive cards pass the same pure native
preparation used by the final generated-file importer before they enter selection.
This prevents a superficially valid string containing unsupported Markdown from
surviving generation and later aborting the final publication transaction.
Checkpoint reuse repeats native content preparation as well as policy/evidence
checks. An earlier checkpoint with valid source IDs but invalid content is not
trusted. Compatible valid checkpoints remain reusable under their existing
versioned identities.

## What remains mandatory

Every graded choice option, true/false statement and false-statement correction,
matching pair, grouping membership, and adjacent ordering relation needs valid
component evidence with the appropriate supports/contradicts role. Optional prompt,
category-label and isolated ordering-item context bindings must also validate when
present. A missing graded binding, fabricated source ID, changed source revision,
invalid role, or missing required rationale is never repaired by inventing evidence.
Source identity validation establishes traceability, not semantic entailment or
factual correctness. Human review of generated material remains necessary.

## Repair and publication

If at least one candidate survives, soft-mix/coverage selection operates on those
admitted candidates. The pipeline does not regenerate successful siblings merely
to fill a quota. If a nonempty response has no admitted candidate, it remains a
validation error and can take the existing bounded repair path; it is not reported
as successful empty generation. An explicit valid empty array remains meaningful
when the source supports no suitable question.

Transport failures, cancellation and application/invariant failures are not
converted into card-level success. The existing smaller-output retry for truncated
responses is separate from per-card rejection. This change does not promise that
every model will always produce a complete or admissible response.

After generation/curation, the existing transaction publishes retained notes,
presentations, schedules, source relationships and generation reports together.
Database or stale-source failures still roll back publication. The public
`.ostudy.json` file importer remains whole-file atomic: an invalid card in a file
selected by the user does not silently create a partial imported deck.

## Counts and diagnostics

The persisted generation summary describes the admitted window output entering
curation and the cards actually saved. It is not a count of every rejected raw
candidate across failed requests. Native diagnostics distinguish those events:

- `document_generation_candidates_rejected` reports returned, admitted and rejected
  candidate counts for a partially accepted response.
- `document_generation_all_candidates_rejected` reports a nonempty response with
  no usable candidate, which may require repair.
- Debug event `document_generation_candidate_rejected` reports the zero-based
  candidate position and safe error code, without its content.
- `assessment_required_evidence_missing` reports the question kind and component
  counts, not the answer, source text or provider-selected identifiers.

Use `./run-app.sh --debug=ai,document` for a reproduction. Read the native log path
from **Settings → Diagnostics → Current diagnostic state**. Keep the build SHA and the
last `document_generation_failed` event with request/retry/token counts. Do not
post API keys, raw provider responses or private source content to a public issue.

## Regression coverage

`src-tauri/src/ai/document_intelligence/flashcards/admission_tests.rs` exercises
malformed envelopes and evidence types, native question/rich-content errors,
disabled families, source-association preservation, hard response bounds,
all-invalid versus empty outcomes, native checkpoint preparation, real mixed-output
transactional publication and the unchanged whole-file import contract.

The existing assessment tests continue to require graded bindings and reject
fabricated/wrong-role evidence while accepting valid optional context. The normal
static/fast gate checks these changes before terminal Linux/Android qualification.
The [canonical runbook](../agents/COMMAND-RUNBOOK.md) defines build and test entry
points; a successful compile is not live-provider or physical-device acceptance.
