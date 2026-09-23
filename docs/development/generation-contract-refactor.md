# Generation contract refactor

## Scope and sequence

This is the September 23 document-generation reliability implementation plan.
It follows real-provider diagnosis of unnecessary extraction repairs, optional
annotation failures, repeated authoring payloads and predictable guide truncation.
The pipeline keeps native authority, retained evidence, revision-bound checkpoints,
shared understanding and independently published cards/guides. It is not a new
agent framework or a replacement source-processing system.

Implement and qualify coherent increments in this order:

1. **AC-01: coverage-driven recovery.** Completion and same-input recovery use
   the same validated region receipt. Supporting sources and explicit region
   dispositions count. Unexposed evidence and unreadable input remain distinct
   from work that another identical interpretation can resolve.
2. **AC-02: actionable outcomes.** Persist task/request/recovery linkage and
   fine-grained validation reasons, including what a recovery actually improved.
   Ordinary diagnostic exports remain content-free.
3. **AC-03: independent knowledge and annotation admission.** Revalidate core
   knowledge independently of optional structural annotations. Preserve unresolved
   annotations; consumers needing validated quantities or ordering cannot grade
   from an invalid annotation. Do not claim that traceability proves entailment.
4. **AC-04: feasible first-pass tasks.** Align prompt/schema requirements, scope
   teaching tasks and their output budgets, and select evidence by dependency.
   Splitting conserves learning obligations rather than discarding difficult work.
5. **AC-05: targeted recovery and publication.** Repair the smallest coherent
   unresolved object, retain accepted siblings, and reconcile failed local commits
   without buying another model response. Root task identity bounds child recovery.
6. **AC-06: joint quality/cost acceptance.** Compare representative textbook,
   slide, table, numeric, cross-page, assessment, image and collection cases for
   factual support, preserved qualifications, useful coverage, latency and tokens.

## First increment: coverage-driven recovery

`knowledge/coverage.rs` owns the revision-bound evidence receipt. Its
`same_input_repair_sources` projection identifies exposed unresolved sources
that have no admitted interpretation. Every supporting reference counts, not just
an accepted unit's primary source. A source fully accounted for through explicit
metadata/non-learning/unreadable region dispositions does not buy an empty repair.

`knowledge/extraction/decode.rs` uses that projection for paid-repair eligibility.
Legacy receipts without region states still use every admitted grounding source
and explicit whole-source non-learning declarations. Actual rejected proposals
remain repair candidates under the existing bounded policy.

This does not redefine completeness. Unresolved regions within an interpreted
source, omitted attachments and unreadable regions retain their diagnostic state.
Protocol completeness is not exhaustive concept coverage or factual entailment.
In particular, omitted attachments require changed evidence, not another paid
interpretation of the identical input. Later increments will associate such gaps
with explicit source-preparation or learning-objective obligations.

The new `document_`-named regressions run in the existing native Fast test filter.
They cover receipt projections, cross-source admission, legacy compatibility,
region-only dispositions, partial supporting sources, genuine rejection/missing
source controls, and provider-call counts through the actual decoder/checkpoint
loop. No live provider is needed for these regressions.

## AC-02a: durable annotation validation causes

The native typed-payload validator assigns `knowledge_annotation_*` rule IDs
at the failing predicate. Missing phrases, invalid occurrences, malformed shapes,
structural bounds, knowledge-kind mismatch, numeric/operator token splitting,
invalid quantity order/operator/literal, dependency errors and persisted span or
version failures no longer collapse into one indistinguishable cause.

Extraction retains that rule in the existing rejected proposal `code` field.
The revision-bound SQLite analysis receipt and targeted repair prompt preserve it.
Content-free diagnostic reports keep `category = invalid_qualified_structure`
for aggregation and add `rule` for the precise cause. Old receipts containing
only the broad code still deserialize unchanged; the missing historical detail
is not invented. Rule classification never parses provider prose or matches
human-readable error messages, and diagnostic reports do not include draft text.

The `document_` Fast regressions cover distinct rejection rules, actual SQLite
checkpoint round trips, targeted-repair propagation, unchanged admitted siblings,
legacy receipt serialization and valid repeated-phrase selection. Existing typed
round-trip, quarantine and legacy tests now also run under the Fast filter.
Numeric signs, strict/inclusive operators, finite values, process edges, evidence
selection and all admission limits remain enforced.

## AC-02b: request-linked extraction outcomes

`ai/generation/outcomes.rs` provides a producer-scoped observer. Physical request
IDs are captured at request creation, not inferred by querying whichever request
happened to finish most recently. The extraction producer owns the scope; waiting
subscribers do not create duplicate records or turn subscriber cancellation into
a false producer failure. The child future is boxed before task-local wrapping.

The existing content-free `generation_requests.payload_bytes_json` envelope now
also carries versioned native outcome objects. They do not alter payload byte
measurements, provider status, token usage or attempt ordinals:

- `nativeLineage` names the preceding observed physical request and the recovery
  decision leading to this request. A resumed checkpoint can have a recovery with
  no preceding request in the current producer; no historical ID is guessed.
- `nativeRecoveryDecisions` retains allowed and denied decisions separately, keyed
  by the four existing reasons. The same logical-task budget remains authoritative.
- `nativeCheckpoint` contains bounded before/after counts and rejection-rule
  histograms. `analysis_store` records it only after its transaction commits.
  Signed deltas expose progress and regression rather than clamping failures to
  zero. Duplicate final saves preserve the pre-request baseline and original gain.
- `nativeTask` distinguishes running, continued, returned, failed, interrupted and
  lost-linkage states. Returning partial knowledge is not a complete artifact or
  proof of entailment. Terminal error codes are durable; error messages and draft
  content are excluded. Startup reconciles an unfinished observed producer even
  when its last provider response had already arrived.

The normal `python3 scripts/generation-usage.py --json` and diagnostic archive
already include these fields in each request's `payloadBytesJson`. They remain
available after application-log rotation. They are excluded from device transfer
with the existing operation-ledger sanitization. No schema migration is needed.

A missing request-accounting write explicitly breaks linkage; a subsequent
checkpoint cannot be attributed to the preceding paid request. Pure checkpoint
reuse creates no fabricated request or usage. Native progress measures changes in
admitted units, rejected proposals, unaccounted sources and unresolved regions;
it does not establish improved factual accuracy, exhaustive recall or better
teaching. Unknown legacy region counts and missing outcome records stay unknown.

Native regressions cover interleaved scopes, allowed/denied recovery, durable
before/after gains, zero gain, resume/reuse, interrupted producers, startup repair,
accounting gaps, unchanged usage and privacy. Production extraction tests run the
real shared producer, decoder, bounded repair and SQLite checkpoint path, including
a source-revision race whose failed commit must never claim saved knowledge.

## AC-02c: authoring and curation outcomes

`ai/document_intelligence/observed.rs` wraps the existing card, document-guide
and collection-guide entry points through `ai/generation/outcomes/authoring.rs`.
Curation attaches the same observer to each independently keyed batch, not the
whole concurrently executing curation operation. Original request constructors
still own stage attribution; prompts, schemas, native admission, retries and
caller fallback policies are unchanged.

The final observed request retains `nativeAuthoring` with a returned/failed state,
its item kind (`cards`, `guide_sections` or `curation_selections`) and a returned
item count. Failure leaves that count unknown; a valid empty selection reports
zero. This boundary is explicitly `persistence = not_observed`: returned cards
are not an imported deck, a returned guide section is not a committed guide, and
successful curation is not an independent certificate of factual correctness.
Existing extraction `nativeCheckpoint` measurements remain separate.

Transport responses and native failures can coexist on the same request. A
curation batch whose caller later retains validated candidates remains a failed
curation task, not a fabricated successful provider selection. Recovery decisions
link to the physical requests inside these scopes. Missing request accounting or
pure local reuse never fabricates a request. The observer boxes its child before
constructing its async state machine and passes the original result through even
if its diagnostic write fails.

Native Fast regressions exercise physical-attempt recording, repaired output,
parallel card/guide isolation, separately keyed curation batches, valid empty
selection versus failure, pre-request results, interruption, diagnostic-write
failure, stage attribution and bounded wrapper size. These tests exercise the
observer and real SQLite ledger with synthetic native results, not a live model
or end-to-end factual-quality assessment.

AC-02 remains partial: operation-level failures before/outside authoring and
per-candidate authoring rejection details still need their owning integrations. AC-03's independent optional-
annotation admission has not been implemented: an invalid annotation still
leaves its proposal pending rather than being silently dropped.

## AC-02d: readable recovery evidence

The normal `python3 scripts/generation-usage.py` now renders native task states,
linked recovery counts and their reported input/output usage, native failures,
returned authoring item counts, and the leading checkpoint rejection rules.
`--json` adds `nativeOutcomes` while preserving the existing raw request records,
usage totals and byte diagnostics. `scripts/generation_outcomes.py` owns this
read-only projection and never opens the library or contacts a provider itself.

A recovery's before/after committed counts distinguish progress, no measured
progress, regression and mixed progress. A missing baseline is unobserved, not
zero gain. Authoring output remains explicitly not observed as persisted; empty
curation can be a valid return. Reported token cost belongs to the linked follow-up
request only, not its predecessor, and unknown usage is never imputed as zero.

The reader checks supplied predecessors against operation, stage and logical task
identity. Missing, duplicate, cross-task and interrupted accounting links are
reported rather than reconstructed by timestamps. A resumed checkpoint may have
recovery on physical attempt one. Legacy or unknown-version native records remain
unobserved. Rejection histograms are labeled observations, not unique proposals:
the same unresolved proposal can appear in successive committed snapshots.

Behavioral tests cover these distinctions plus independent provider/native status,
JSON re-import, unchanged input data, and a real read-only SQLite/CLI round trip
from outside the repository directory. This report exposes the existing evidence;
it does not change generation policy or establish factual or teaching quality.

## Compatibility and non-goals

These increments do not change stored schema, prompt versions, provider
selection, completion ceilings, evidence identities or checkpoint contracts. They
neither clear retained source knowledge nor invalidate already-paid results.
Existing [parallel publication lifecycle corrections](parallel-artifact-lifecycle-regression.md)
remain necessary and separate from the contract work.

The remaining AC-02 integrations and AC-03 through AC-06 are unfinished. There is
no measured real-provider saving until a representative run is compared.
Do not weaken assertions, drop qualifications, guess evidence or label partial
material complete to reduce retry counts. No automatic merge is authorized.
