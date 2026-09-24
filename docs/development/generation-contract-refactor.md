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

Historical rejected proposals retain their rule in `code`. Under AC-03, an
independently admitted core carries a deferred annotation with its precise rule
instead of being a rejected core proposal. The revision-bound SQLite receipt
preserves this distinction. Content-free diagnostics expose deferred annotations
separately; genuinely rejected cores still enter the targeted repair prompt.
Old receipts containing only a broad error remain readable; local re-admission
can determine a current rule from an actual retained draft, never from speculation.
Rule classification does not parse provider prose or human-readable messages.

The `document_` Fast regressions cover distinct validation rules, real SQLite
checkpoint round trips, preserved admitted siblings, legacy receipt serialization
and valid repeated-phrase selection. Strict typed-shape tests retain numeric
signs, strict/inclusive operators, finite values, process edges, evidence
selection and every existing admission limit.

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
still own stage attribution. This instrumentation does not change prompts,
retries or caller fallback policies; AC-03 adds explicit annotation capability
checks at assessment admission, checkpoint reuse and publication.

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

Operation-owner capture is described in AC-02f; non-document owners still need
to adopt its result-aware completion boundary. Candidate admission has the
separate AC-02e record below.

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

## AC-02e: candidate admission outcomes

The real flashcard parser now records `nativeAdmission` once per observed
physical response. Each rejected candidate carries its response-local index,
the static native validation boundary and the original error code. No card text,
knowledge alias, rejected draft or error prose enters this projection. Nested
assessment and content validators remain named boundaries, not a claim that all
their internal failure predicates now have individual rule identifiers.

Declared, examined, admitted, rejected and selected candidate counts are separate.
Valid surplus cards not chosen by the existing selection policy are not rejected
cards. An all-invalid response remains a failure with unknown selection, whereas
a valid empty response has an observed zero selection. Malformed envelopes and
responses over the hard limit do not falsely classify unexamined items as rejected.
The observer preserves original errors, accepted siblings, source associations,
output budgets and final import validation. It does not authorize recovery.

`ai/generation/outcomes/authoring/candidates.rs` collects bounded content-free
metadata and performs one best-effort diagnostic write after the parser returns.
Local-only parsing, absent request accounting and another task stage cannot
fabricate a physical request. A failed diagnostic write never changes admission.
The original result is passed through without erasing the first attempt's record
when a subsequent recovery succeeds.

The normal usage reporter exposes `nativeOutcomes.candidateAdmission` with
coverage counts, response states, failure boundaries, rejection observations and
selection totals. Unknown or inconsistent records stay unobserved. Counts concern
response-local observations, not unique facts, saved cards or factual quality.
An error being eligible under the existing bounded policy does not assert that
resending the task is useful; that policy change remains AC-05.

Eight native regressions exercise the actual parser and SQLite ledger, including
partial/all rejection, valid empty output, envelope and size failures, selection,
recovery linkage, diagnostic-write failure and missing/wrong request scope. Eight
Python regressions verify readable aggregation, strict count validation, legacy
unknowns, privacy, input immutability and unchanged provider/linkage semantics.
No paid provider is involved in these tests.

## AC-02f: request-independent document operation results

Migration 30 adds bounded nullable `generation_operations.outcome_json` through
normal schema conversion. Existing rows remain NULL: neither a previous failure
cause nor a successful artifact publication is reconstructed from request status.
Migrations 1 through 29 and previously paid checkpoint identities are unchanged.

`Operation::finish_result` receives the actual operation owner's Result and writes
its terminal state, time and content-free error code atomically. The document task
wrapper uses this boundary only after its child returns. Individual card/guide
observers cannot finish their parent. Early document generation binds the same
operation before source validation and ingestion, so a managed-source error can
be retained even with no physical request. Diagnostic binding/writing is best
effort and never replaces the original product result or authorizes another call.

Startup records unfinished running operations as interrupted without claiming an
observed return. Finished records cannot be overwritten by startup or a stale
owner. An ordinary dropped legacy owner still reports interruption with unknown
cause; owners that still call boolean-only `finish` retain unknown detailed
outcomes. Other operation owners must explicitly adopt `finish_result`; their
integration is not inferred from this document-task change.

The normal usage report includes separate `operationRecords` and
`operationOutcomes`, including zero-request operations in `--all`. Reads use the
same SQLite snapshot as the request report. Older schema-27-through-29 libraries
remain readable without alteration. JSON re-import retains operation evidence.
Missing/ambiguous/unsupported records remain unobserved, and recorded request
counts are not proof of complete provider accounting. Neither operation completion
nor a native return certifies an artifact's publication or teaching quality.

Native regressions cover zero-request errors, the actual document task wrapper,
terminal immutability, interrupted startup, diagnostic-write failure, sanitized
codes, device-transfer removal, and migration/legacy preservation. Reporter
regressions include a read-only SQLite/CLI round trip and zero-request export.
No live provider is needed. Remaining non-document owner integrations are separate
from the AC-03 behavioral change below.

## AC-03: independent core and annotation admission

The extraction decoder first validates core content with no annotation: statement,
qualifications, source authority and every evidence reference retain the existing
checks. An invalid core is still rejected. Only after those checks pass does the
strict typed-shape validator run. A known optional-annotation failure creates a
native deferred record, not a rejected core and not a validated shape. Neither the
statement nor its quantities, signs, qualifications or evidence is rewritten.

`typed_payload/state.rs` keeps validated v1 payload serialization unchanged. Its
disjoint deferred representation stores the original failure rule and binds the
draft to the admitted statement and knowledge kind. Drafts up to 12,000 bytes are
retained in the ordinary source-owned knowledge/checkpoint storage. Larger drafts
retain an explicit fingerprint and byte count, not a fake structured payload.
Stored bounded drafts must reproduce the recorded failure; corrupt validated
payloads and unsupported versions cannot be reinterpreted as deferred state.

A receipt with deferred annotations remains explicitly partial. That state alone
does not authorize another whole-window provider request. Existing rejected core
proposals and genuinely unaccounted sources still use bounded recovery. Historical
annotation-rejected drafts can be re-admitted locally against the same current
ordered evidence catalog; all core checks run again. Already valid knowledge and
published learner artifacts are not reset.

Authoring receives the original core and qualifications plus a compact deferred
capability marker, never the failed annotation draft. Recall and guide explanation
remain available. Conservatively, interactive assessment bindings cannot use a
deferred unit: request schemas exclude those aliases without renumbering other
units, and native new-response admission, checkpoint reuse and final transactional
publication enforce the same rule. Unaffected units keep all permitted question
families. A policy with no supported family has an explicit empty-result schema;
policy-aware repartitioning beyond the implemented guide planner remains pending.

Committed request diagnostics retain deferred annotations separately from rejected
core proposals. Each newly observed extraction snapshot records a count and static
rule histogram; an explicit zero is distinct from older snapshots that never
measured this field. The reporter validates counts against admitted units and
keeps malformed annotation diagnostics unknown without discarding usable core
counts. Annotation resolution can register native progress without adding another
core unit, while increasing debt remains a regression even alongside other gains.
Both comparisons require measured annotation counts; no historical value is
invented. The readable report labels repeated snapshots as observations, not
unique outstanding tasks or factual-quality measurements. Failed checkpoint saves
cannot claim committed annotations, and no draft text enters these diagnostics.

Regression coverage includes no-retry extraction, retained-draft local recovery,
independent core/evidence rejection, checkpoint round trips and tamper rejection,
unchanged validated serialization, strict numeric/process rules, prompt privacy,
stable schema aliases, recall availability, forged-current-hash cache rejection,
and the final publication boundary. Existing recovery-outcome fixtures now use an
invalid core rather than an optional annotation to continue testing paid repair.

This is not automatic factual entailment checking, semantic deduplication, or a
new annotation repair scheduler. Targeted repairs of deferred annotations and
broader representation simplification remain AC-05. Live-provider quality/cost
acceptance remains AC-06; mock call-count reductions are not billing measurements.

## AC-04: first-pass feasibility and lossless collection planning

Guide first passes receive the headroom previously reserved for repair: 3,200
visible tokens for document topics and up to 6,144 for collection sections. The
joint writing target and complete-JSON instructions are supplied initially, and
required key points are reflected in the provider schema. This reallocates
existing maximum allowances rather than introducing a higher recovery ceiling.
Guide field repair preserves independently valid fields under native ownership.

The shared authoring evidence projection follows exact native text ranges and
linked structural context. Whole tables, captions, footnotes and ambiguous
structures retain their required context; the stored source is never clipped.

`study_materials/composition/planning.rs` now compiles collection windows before
section-cache identities, whole-guide manifests and progress totals are allocated.
It uses the same complete request constructor as authoring. A context-budget
failure bisects the unit sequence into nonempty children, preserving every unit,
qualification, evidence dependency and topic role. For N units, binary planning
visits at most 2N-1 candidates. The previous 75-percent truncation both dropped the
tail and failed to decrease two- or three-unit inputs; that loop is removed.

An already-fitting window keeps its existing block identity. Split keys bind the
original block and the child's exact evidence fingerprint. Only an indivisible
unit may try the smaller existing completion plans. If it still cannot fit, the
operation fails explicitly before dispatch rather than silently omitting it.
Cancellation and non-context errors are not treated as reasons to split. Successful
children use the existing durable section checkpoints and final publication path.

Seven native document regressions cover finite planning, exact conservation,
qualifications and structural context, stable child keys, explicit singleton
failure, cancellation and empty inputs. These establish task conservation, not
that a model necessarily explains every supplied unit. Generalized document/card
repartitioning and objective-level teaching coverage are not claimed by this
collection-planning increment.

## AC-05a: parent-scoped recovery for planned child tasks

`ai/generation/recovery.rs` registers native-owned recovery families before any
child request starts. The collection planner uses the original section request
key as root and the exact future child request keys as members. Children retain
separate physical requests, attempt ordinals, usage and outcome linkage, but draw
from the original root's existing maximum of two distinct recovery categories,
with each category usable once. Existing parent reservations remain spent.

Registration is atomic and idempotent for the same family. A used child cannot be
reparented to a fresh allowance; a registered root cannot become a child. One lock
serializes concurrent reservations. Unrelated roots, stages and operations retain
independent allowances. Denial does not admit invalid output, mark a partial guide
complete, delete accepted siblings or authorize another source interpretation.

Each new physical request carries a content-free `recoveryScope` in its existing
payload diagnostics, including version, root task key and applicable limits. The
ordinary JSON reporter preserves it. Historical missing records stay unobserved.
This scope metadata is durable; the allowance counter remains operation-local,
like the pre-existing policy. This is not a cross-restart budget ledger or a learned
estimate that every permitted retry will improve quality.

Seven native regressions cover spent parents, atomic registration, reparenting,
concurrency, root/stage/operation isolation, invalid inputs and a real SQLite
request/usage round trip. The corresponding Documents-screen test fixtures now
explicitly supply the independent collection and artifact-job reads and reset
mock implementations between tests; retry still has to run through its real UI
and invoke the native start command. No product assertion or gate is skipped.

Demand-driven deferred-annotation repair, recovery decisions based on measured
benefit beyond these hard limits, broader representation simplification and the
remaining local-publication recovery paths are unfinished AC-05 work. The existing
targeted guide-field repair is a foundation, not completion of that broader stage.

## Compatibility and non-goals

AC-01 through AC-02e do not change schema. AC-02f adds migration 30, so a library
opened by this build requires a schema-30-capable build afterwards; do not
manually downgrade its schema metadata. Preserve a backup before testing schema
changes. AC-03 adds a versioned deferred annotation representation within existing
JSON storage, not a SQL migration. Older binaries do not understand that new state;
do not downgrade an already-updated library. Validated annotation bytes, absent
annotations and previously compatible source receipts remain readable unchanged.

AC-04 changes guide request planning as described above; provider/model selection,
source identities and extraction checkpoint keys remain unchanged. Assessment
schemas project annotation capabilities; native admission remains authoritative.
Retained source knowledge is not cleared. Changes to an admitted unit's actual
content naturally invalidate its dependent authored checkpoint through the existing
content-hash check. Recovery scope adds no migration or provider request.
Existing [parallel publication lifecycle corrections](parallel-artifact-lifecycle-regression.md)
remain necessary and separate from the contract work.

Remaining non-document AC-02 integrations, generalized document/card planning,
the rest of AC-05 and AC-06 quality/cost acceptance are unfinished. There is no
measured real-provider saving until a representative run is compared. Do not
weaken assertions, drop qualifications, guess evidence or label partial material
complete to reduce retry counts. No automatic merge is authorized.
