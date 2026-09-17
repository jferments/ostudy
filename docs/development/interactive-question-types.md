# Interactive question types: implementation contract

Status: implementation complete on PR #101 through Stages 7–9, pending terminal
exact-head full-platform qualification and the explicitly human visual/physical-device
acceptance items that automation cannot perform. The source implementation,
compatibility paths and maintained documentation are no longer at the Stage 6
test-drive boundary.

Baseline: merged PR #98, main `9af5e204b9cbaa74e955fce8726ffd476f0f11d2`.
Its final source includes schema-3 math/media, MathLive authoring, shared KaTeX,
managed IPC-to-Blob playback and the accepted Android formula-dialog behavior.
Preserve #94 knowledge/privacy, #96 UI ownership, #97 presentation/startup/USB,
and the current self-hosted qualification/storage policy.

## Scope and dependency policy

Include seven new types in this revision: **single-best-answer multiple choice**,
select all that apply, true/false, one-to-one matching, grouping, ordering and
manually authored image occlusion. Basic and existing Cloze remain first-class.
Automatic document generation covers the six non-occlusion types only when the
source supports the required relation. Single-best-answer is not deferred.

Only deliberately selected AI/LLM APIs may depend on an external service.
All application-controlled non-AI functionality uses free/open-source software
and runs locally: authoring, checking answers, scheduling, analytics, document
parsing/OCR, math rendering, media playback, drag interactions, masking, storage,
backup and direct device transfer. No proprietary study SDK, hosted assessment
engine, remote mask service, license-key gate, runtime CDN, or mandatory account.
Optional local/LAN model endpoints remain valid; never silently switch to cloud.
Ordinary operating-system/WebView services and user-requested peer connections
are not a hosted study-provider dependency.

Quizlet, RemNote, Duolingo and other proprietary products are design references
only. Do not embed their services or copy proprietary implementation/assets.
Evaluate any new dependency for need, open-source license compatibility, exact
pin, local packaging, platform behavior and offline operation before adoption.
For AC-22, Android document OCR now uses Tesseract4Android/Tesseract with an
immutably pinned packaged `tessdata_fast` model, and Android pairing uses local
JourneyApps ZXing Embedded/ZXing core. Google ML Kit text recognition and Google
Code Scanner are removed from the runtime dependency graph. See
[ADR 0052](../decisions/0052-open-source-android-imaging.md).

Non-goals: automatic occlusion extraction; dedicated formula OCR; rich-cloze
redesign; arbitrary HTML/JavaScript templates; a plugin/form-builder platform;
new FSRS algorithms; handwriting, recording or speech grading; gamified timers,
hearts or competitive multiplayer; unrelated infrastructure or UI redesign.

## Ownership and invariants

Keep these objects distinct:

```text
editable CardNote
  -> independently scheduled CardPresentation
  -> captured QuestionInstance
  -> LearnerResponse
  -> EvaluationResult
  -> existing scheduling review
```

A permutation is instance state, not another card. One compound matching/grouping
board produces one scheduling review, not one review per tile. An occlusion note
can generate several stable child presentations. No new generic card DTO may
collapse authored source, public study projections and scheduling state.

Rust owns canonical definition/response admission, objective evaluation, durable
attempt transitions, transactions and scheduling. React owns interaction/draft
state. IPC remains in `src/ipc`; UI facades contain no provider, SQL or Tauri calls.
Reuse the existing rich-document validator and renderer for every rich field.
Keep question definition version, rich root, SQLite migration, transfer package
and sync compatibility independently versioned. Do not allocate migration numbers
until implementing against the then-current head. Released migrations stay fixed.

A public study projection contains only visible prompts/items and permitted
assets. Keys, per-option rationales and source passages that disclose answers
remain behind the reveal/check boundary. Keep #94 Tutor hint/source permissions;
an ordinary hint must not obtain the private key through another projection.

## Editor composition: reuse behavior, not one universal form

The existing card editor becomes the shared authoring shell. It owns deck, tags,
Template selection, dirty/Back protection, durable draft recovery, asset staging,
Save/Save and add another, duplicate handling, preview coordination and committed
write versus refresh-failure reporting. There is one save/draft lifecycle, not one
copied controller for each type.

Each type owns a small typed form. The shell eventually holds a discriminated
`QuestionDraft`, not a flat object with every possible type's optional fields.
Incomplete drafts are recoverable; native canonical admission is stricter.
Changing type must retain or explicitly discard draft content, never silently
convert it. Unknown kinds must fail explicitly rather than falling through to
Basic. Keep saved templates conceptually separate from question kinds even when
they share the current Template dropdown.

Shared field machinery:

- `RichFieldList` addresses fields by stable IDs, shares asset-draft resources,
  and uses the existing `RichTextEditor` adapter. Changes are field deltas, never
  replacement snapshots of a whole captured form. Row position/label is not ID.
- Choice options share rich content, item ordering, add/delete controls and
  optional explanation fields. Correctness controls/policy remain type-specific.
- Matching pairs, grouping items and ordering steps reuse item-row and move
  controls where their behavior is genuinely identical. Their relation models
  and validation remain different; do not hide them behind configurable flags.
- Reuse existing FormField, Button, Checkbox/radio, menu, Dialog/Sheet and semantic
  tokens. Base UI imports stay in the owned vendor layer. Specialized drag/image
  geometry may have a reviewed focused owner, not a second visual framework.

Purpose-built forms:

| Kind | Authored fields and policy |
| --- | --- |
| Single-best-answer | Prompt, rich options, exactly one correct option, per-option rationales and optional overall explanation. Radio selection in authoring and study. |
| Select all | Same option machinery, a correct-ID set and per-option rationales. Checkbox selection; an empty correct set is valid and deliberate empty submission is representable. |
| True/false | Statement, explicit Boolean key, explanation and optional corrected statement. Unanswered is distinct from false. |
| Matching | Instruction, equal-cardinality paired rows, optional pair rationales. Require one unambiguous bijection. |
| Grouping | Category definitions, item rows, one correct category per item and rationales. Authoring uses a category selector with optional dragging. |
| Ordering | Explicit ordering criterion/direction, rich item list in correct order and explanation. Initial grading supports one total order, not ties or concurrent steps. |
| Image occlusion | Managed image, rectangle workspace and question inspector. Prompts/answers/explanations reuse rich fields; only geometry is specialized. |

Single-best-answer and select-all use separate semantic kinds. An incomplete
single-choice draft may have no selected correct option, but saving requires
exactly one valid ID. Do not represent it as a multi-select question which merely
happens to have a one-element key. No automatic conversion may discard additional
correct answers. Distractor quality and the meaning of "best" require a clear
stem/criterion, not just a syntactically valid radio group.

Keep explanations collapsed until needed and use compact field chrome. Before
lazy editor activation or a shared active toolbar is introduced, prove that field
undo history, selection, formula transactions and pending asset imports survive
focus changes and reordering. Unmounting an editor to save resources is not safe
when it discards an in-progress operation. The first increment intentionally
preserves current editor mounting; no performance gain is claimed.

## Practice, evaluation and attempts

Preserve contextual session creation, progress, scheduling actions, editing,
Tutor entry and the return target. Basic/Cloze retain their reveal behavior.
Interactive bodies are not wrapped in a click-to-flip surface. Player, checkbox,
radio, drag, link and text-selection events must not flip or rate a card.

Use answer -> Check answer -> feedback -> difficulty/continue. Do not auto-submit
on the final tile drop or leak correctness on each placement. True/false supports
t/f shortcuts only outside editing, IME composition and overlays. Drag activities
must also support keyboard and non-drag click/tap alternatives. Reuse mechanics,
not HTML5 desktop-only drag behavior or a new platform-specific product flow.

Objective correctness is separate from partial diagnostics, learner difficulty
and the effective FSRS rating. Single-choice compares one submitted ID; select-all
compares exact sets; matching compares every pair; grouping compares every item;
ordering compares the specified complete order; true/false compares a submitted
Boolean. Reject duplicate/unknown IDs and invalid shapes natively. Item-level
feedback explains selected-wrong and missed-correct alternatives without counting
unselected distractors as extra successful reviews.

Default incorrect/incomplete attempts to Wrong. Correct attempts permit
Hard/Medium/Easy. Explicit overrides retain the original evaluation and reason.
Post-feedback retries are remediation, not replacement first attempts or another
scheduling success. Manual occlusion initially uses reveal-and-self-rate.

A captured instance freezes definition revision, keys, item IDs/order, relevant
assets and grading-policy version. Persist acknowledged response drafts with
sequence identity. Checking creates one immutable first evaluation using an
idempotent check identity; rating links it to the existing transactional review
with its submission identity and schedule revision. A crash between check and
rating resumes feedback, not a blank question or duplicate review. An edit cannot
retroactively change a captured key. Retain current post-commit reconciliation,
scheduled/extra/custom semantics and local-calendar attribution.

## Image occlusion

Store normalized rectangles against a managed image with explicit orientation
and geometry. The editor supports rectangle creation, select/move/resize, delete,
undo/redo, zoom/pan, fit and preview. Never persist viewport coordinates.

A mask is geometry; a question is a prompt/answer targeting one or more mask IDs.
Several questions may target the same region and several masks may belong to one
question. Child presentation IDs survive movement; creation/deletion/grouping
must not transfer unrelated review history.

Support hide-target-only and hide-all/answer-target. Generate a bounded local
masked image derivative for pre-answer presentation so the original image is not
accidentally exposed through loading flashes, missing overlay CSS, thumbnails or
Tutor input. This is study masking, not secure redaction of the learner's source.
Inspect zoom/letterboxing/orientation and masking on physical touch devices.

## Document generation and quality

Extend the existing knowledge-first pipeline with bounded assessment opportunities
rather than another source-analysis traversal for each format. Use explicit
relation sets, partitions, properties and process order when the evidence admits
them. A two-column table is not automatically bijective; a numbered list is not
necessarily a required order; a contrast is not necessarily an exclusive grouping.

Single-best-answer needs a clear decision criterion and exactly one supported
best answer. Reject tied/both-valid options, wording that gives away the key,
unsupported distractors and unnecessary compound stems. For select-all, absence
from a non-exhaustive passage is not proof an option is false. For false statements
and distractors, evidence supports the correction/rationale, not the false claim.

Composite tasks require component-level support and a bounded union of native
knowledge/evidence dependencies. Keep ordinary recall atomic instead of raising
its global knowledge-unit limit. Native references prove source identity and
location, not semantic entailment or pedagogical quality. Preserve qualifications,
units and context; human factual review remains necessary.

Allowed kinds, evidence fit, topic coverage and bounded task complexity outrank
format percentages. Initial proposed soft allocation among suitable candidates:
Basic/Cloze 50%, single-best-answer 10%, matching 15%, grouping 10%, select-all 10%,
ordering 3%, true/false 2%. These total 100% and are adjustable product defaults,
not an empirically optimal mix. Reallocate unusable slots to suitable recall or
return fewer cards; never manufacture filler. Track component coverage/workload
as well as presentation counts. Generation mix never overrides due scheduling.

Retain versioned prompts/structured schemas, bounded native admission, targeted
repair/checkpoint reuse, whole-file public JSON validation and transactional
publication. Manual saves make no AI call. Do not add an import-preview queue.
Manual/AI origin and derivative evidence remain distinguishable. Occlusion
creation from documents is explicitly outside this revision.

## Persistence, transfers and analytics

Migrate supported Basic/Cloze and durable drafts without rewriting IDs, schedules
or retained events. Introduce typed payloads and necessary query projections with
one authoritative owner per field, not competing front/back and typed copies.
Version search/duplicate projection semantics where needed.

Every enabled type must round-trip through its applicable .studydeck/.ostudy.json,
full backup and supported device-sync contract. Reject unsupported peers or
packages before mutation. Shared decks get fresh schedules/no donor history;
backups preserve personal evidence; device sync retains its explicit allowlist.
Do not mistake one transfer path for another.

Asset references include all prompts, options, rationales, targets, drafts and
captured instances. Delete/Undo and cleanup must preserve live owners without
creating an undeletable shadow archive. Routine logs exclude learning text,
answers, raw responses and private media. Derivatives stay rebuildable.

Analytics keeps historical review evidence intact and separates self-assessment,
objective first-attempt results, assistance, overrides and remediation. Per-item
results are diagnostic details, not extra schedule events or a mastery claim.

## Incremental delivery on this branch

Use one draft PR and branch for this revision, with coherent published increments.
Do not create competing per-type branches or change infrastructure to implement it.

| Stage | Deliverable |
| --- | --- |
| 0 | Reconcile final #98 and extract the shared field boundary from existing live forms. |
| 1 | Native typed definitions, keys/responses, strict validation, projections, migrations and compatibility protection. |
| 2 | Shared editor/draft shell and durable captured-attempt/session/check/review integration. |
| 3 | Complete single-best-answer, select-all and true/false create/save/reopen/study/feedback/transfer paths. |
| 4 | Matching editor/board, reusable drag adapter and equivalent input alternatives. |
| 5 | Grouping and ordering editors, interactions and deterministic evaluation. |
| 6 | Manual occlusion editor, safe derivatives, stable targets and lifecycle. |
| 7 | Assessment opportunities, component grounding, typed generation and publication. |
| 8 | Mix/coverage controls, generation summaries and Tutor/guide integration. |
| 9 | Analytics, full compatibility/failure/platform qualification and human acceptance. |

Persistence, transfers, hidden answers and tests accompany each enabled type;
they are not postponed to Stage 9. Finish source/tests/docs and audit the complete
candidate before terminal all-full qualification. During implementation use the
smallest regression first, then the normal draft static/fast gate. No expensive
matrix until implementation is complete; no incidental cleanup after the freeze.

### Current implementation boundary

Stages 0–6 remain as described by the native/editor/Practice/occlusion design
above: all seven manual question types share the editor shell, canonical Rust
validation, stable presentations, captured first attempts and the existing
transactional scheduler. Image occlusion keeps the source raster behind the
native masked-derivative authorization boundary and uses stable child schedules.

**Stage 7 is implemented.** The knowledge-first generator publishes bounded
assessment opportunities instead of doing a second extraction traversal for each
format. Native opportunity/evidence contracts model supports/contradicts
components, evidence roles, partitions, ordered relations, contrast pairs and
other type-fit cues. Generated single-choice, select-all, true/false, matching,
grouping and ordering definitions pass type-specific schemas and semantic
admission before publication. Evidence records retain revision/policy/opportunity
identity, and generation checkpoints remain retry-safe. Image occlusion stays
manual-only. DeepSeek schemas that fit its strict beta tool subset still use that
path; nullable/composed generated-question schemas use the provider's JSON-object
mode and then the same authoritative Rust admission, preventing strict-schema HTTP
400 failures without weakening validation.

**Stage 8 is implemented.** Generation policy persists an allowed-type set,
requested workload, maximum compound size and scarce-evidence threshold. The
planner scores evidence fit and topic/component coverage, applies the proposed mix
as a soft target rather than a quota, reallocates unsuitable opportunities to
supported recall or returns fewer cards, and records immutable generation reports.
No filler question is manufactured just to hit a percentage. Tutor focused-attempt
context reuses the canonical question/knowledge projection: pre-answer prompts do
not expose private keys/rationales; assistance is marked only when usable help is
actually exposed; failed/cancelled/late hints cannot replace first-attempt evidence.
Study-guide/Tutor relations use the shared knowledge graph rather than repeating
source extraction.

**Stage 9 source integration is implemented.** Native analytics and the Analytics
UI separate Basic/Cloze self-assessment from per-type objective first checks and
report assistance, overrides and remediation as distinct evidence. `.studydeck`
version 3 round-trips all seven typed kinds, presentation keys and managed assets
with fresh recipient schedules/no donor history. Trusted-device synchronization
now transfers the admitted typed graph, attempts/evidence and managed assets under
its explicit scope; checkpoint/evidence rows merge atomically and malformed graphs
fail before file installation or merge commit. Full `.ostudybackup` has an explicit
typed-definition/presentation/attempt/generation-evidence round-trip regression.
Editor drafts/write receipts remain excluded from sync while complete backup keeps
its documented recovery scope.

AC-22 is implemented through [ADR 0052](../decisions/0052-open-source-android-imaging.md):
Android QR pairing uses local ZXing Embedded/core and document OCR uses
Tesseract4Android/Tesseract with an immutable packaged English `tessdata_fast`
model. The Android verification scripts reject reintroduction of Google Code
Scanner/ML Kit text recognition, verify dependency/model pins, require the model
inside the APK and retain bounded permission/native-library checks.

Automated exact-head qualification evidence belongs to Actions. The implementation
is not declared release-accepted until the ready-PR `all-full` run completes on
the self-hosted runner. Physical camera decoding, USB accessory re-enumeration,
real-touch occlusion/drag ergonomics and final desktop/Android visual inspection
remain explicit human acceptance items because CI cannot truthfully substitute
for those physical observations.

See [Stage 6 build, test and manual acceptance](image-occlusion.md) for the
occlusion-specific physical checklist and [Android development architecture](android.md)
for the local ZXing/Tesseract packaging and emulator boundary.

## Stable acceptance criteria

Retain the accepted plan's AC-01 through AC-20; append the two clarified criteria.

| ID | Required result |
| --- | --- |
| AC-01 | Existing Basic/Cloze content, IDs, drafts, schedules/history and Practice behavior survive. |
| AC-02 | Every kind has strict native definition/response validation; unknown kinds never become Basic. |
| AC-03 | Dedicated editors use the current Template entry point and shared form/UI infrastructure. |
| AC-04 | Rich text/images/math/media survive typed-field authoring, preview, save and asset collection. |
| AC-05 | Matching enforces a bijection and grouping enforces one membership per item. |
| AC-06 | Select-all supports exact sets/empty submissions; true/false distinguishes unanswered. |
| AC-07 | Ordering enforces its explicit total order without claiming support for ties/alternative orders. |
| AC-08 | Drag, keyboard and non-drag pointer alternatives yield equivalent results. |
| AC-09 | Occlusion supports rectangles, both mask modes, multi-mask targets and stable child identities. |
| AC-10 | Pre-answer study/Tutor/media/image views do not disclose hidden keys or source images. |
| AC-11 | First responses/evaluations are frozen; remediation and overrides stay distinct. |
| AC-12 | Resume and retries preserve acknowledged answers/permutations without duplicate checks/reviews. |
| AC-13 | One compound board produces one review; occlusion questions have independent schedules. |
| AC-14 | Preview revisions, practice modes, calendar and post-commit semantics remain intact. |
| AC-15 | Generated components retain revision-checked evidence and reject unsupported relation shapes. |
| AC-16 | Allowed types, fit, coverage, limits and checkpoint reuse outrank quotas; no cloud fallback. |
| AC-17 | All applicable imports/exports/backup/sync handle types or reject incompatibility before mutation. |
| AC-18 | Delete/Undo and cleanup protect drafts/snapshots without retaining an undeletable archive. |
| AC-19 | Analytics separates self-assessment, correctness, assistance, overrides and remediation. |
| AC-20 | Offline/platform/failure/performance checks, maintained docs and assigned human acceptance are complete. |
| AC-21 | Single-best-answer is included now, with exactly one correct option, radio input and option rationales. |
| AC-22 | No non-AI hosted/proprietary dependency; all added software passes open-source/offline packaging review. |

Source owners: [UI](../architecture/ui.md), [rich editor](../architecture/rich-document-editor.md),
[card model](../architecture/card-format-and-data-model.md),
[knowledge](../architecture/knowledge-representation.md),
[math/media](native-math-media.md), [testing](testing-and-ci.md),
[local acceptance](local-testing-and-handoff.md).
