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

This is the validation-cause part of AC-02, not the whole outcome ledger. Durable
physical-request/recovery linkage, before/after recovery gains and terminal-error
export remain the next boundary. AC-03's independent optional-annotation admission
has not been implemented by this change: an invalid annotation still leaves its
proposal pending rather than silently being dropped or admitted as correct.

## Compatibility and non-goals

These increments do not change stored schema, prompt versions, provider
selection, completion ceilings, evidence identities or checkpoint contracts. They
neither clear retained source knowledge nor invalidate already-paid results.
Existing [parallel publication lifecycle corrections](parallel-artifact-lifecycle-regression.md)
remain necessary and separate from the contract work.

AC-02 is partially implemented; its linked outcome ledger and AC-03 through AC-06
remain unfinished. There is no measured
real-provider saving for this increment until a new representative run is compared.
Do not weaken assertions, drop qualifications, guess evidence or label partial
material complete to reduce retry counts. No automatic merge is authorized.
