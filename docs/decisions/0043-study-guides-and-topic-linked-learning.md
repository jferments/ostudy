# ADR 0043: Shared document learning plans, study guides, and topic-linked cards

- **Status:** Accepted for implementation
- **Date:** 2026-08-22
- **Base:** `7d070e01248cd9930d364edeb93159890834be9e` (merged PR #66)

## Context

PR #63 established app-owned canonical documents, source units, semantic blocks,
assets, source locators, and semantic chunks. PR #64 separated format-neutral
document-learning reasoning from provider transport. PR #66 added production
non-PDF adapters, generic source acquisition and lifecycle handling, truthful
source-unit metadata, canonical retained images, generic backup/restore, and
format-neutral Documents UI behavior.

The current outline remains primarily a compact intermediate used to guide
flashcard generation. It has global summary/objective/concept fields and ordered
source-bounded sections, but no durable topic identity shared by different
learning artifacts. Generated cards are flattened to ordinary card payloads
before commit, so their source-window relationship is not retained as a
relational topic association. There is also no durable study-guide artifact,
resumable guide job, learner annotation layer, or PDF export.

Study guides, flashcards, future practice tests, and interactive tutoring must
not each reinterpret the document independently. They need one source-grounded,
app-owned representation of what the document teaches. AI output may help name,
summarize, and explain topics, but it cannot own topic identity, hierarchy,
source provenance, user edits, or learner mastery.

## Decision

### 1. Canonical evidence remains authoritative

The immutable managed source, `CanonicalDocument`, semantic chunks,
`SourceLocator` values, and canonical assets remain the authority. Study-guide
content and model-generated descriptions are derived learning artifacts. They
never overwrite source text, parser output, source assets, source hashes, or
provenance.

Every topic and generated section is anchored to canonical source locators and
semantic-chunk hashes. The model cannot create document IDs, topic IDs, chunk
IDs, asset IDs, source-unit coordinates, or internal relationships.

### 2. Introduce an app-owned `DocumentLearningPlan`

The shared document-learning domain becomes:

```text
CanonicalDocument / semantic chunks / visual evidence
                         |
                         v
                 bounded outline fragments
                         |
                         v
             native DocumentLearningPlan
                |          |          |
                v          v          v
           study guide  flashcards  later artifacts
```

Provider outline responses remain bounded fragment contracts. Native code
normalizes and assembles them into a `DocumentLearningPlan` containing:

- a whole-document summary;
- global learning objectives and key concepts;
- ordered `LearningTopic` records;
- topic hierarchy where source structure supports it;
- canonical source locators;
- exact semantic evidence identities; and
- a stable plan fingerprint.

A `LearningTopic` contains an app-owned topic ID, optional parent topic ID,
deterministic order, title, summary, objectives, concepts, source locator,
semantic-chunk identities/hashes, and an anchor hash. Initial topic hierarchy may
be shallow when an adapter exposes only deterministic sections. The application
must not fabricate slide/chapter hierarchy that the adapter could not preserve.

The existing concise outline becomes an internal/provider compatibility input to
the learning-plan compiler rather than the long-term identity boundary consumed
by every artifact.

### 3. Topic identity is local and reconcilable

Topic IDs are local stable entity IDs, not hashes returned by a model. Topic
reconciliation preserves an existing ID when the current source locator,
evidence hashes, and normalized structural anchor match. Model wording such as a
title or summary is not sufficient identity.

When source or extraction changes invalidate the semantic evidence, a new plan
revision is compiled. Exact anchors preserve matching topic IDs; unambiguous
source-overlap reconciliation may preserve an ID only under tested deterministic
rules. Ambiguous topics are superseded rather than silently merged.

This allows guide prose to be regenerated while flashcard associations and user
annotations remain attached to the same underlying topic whenever the source
identity still supports that relationship.

### 4. Persist plans, topics, guides, and artifact jobs separately

A new append-only migration introduces normalized app-owned records for:

- document learning-plan revisions;
- stable document topics and their hierarchy/evidence anchors;
- study-guide revisions and active-revision selection;
- ordered study-guide sections linked to topics;
- learner edit/annotation overlays;
- note-to-topic relationships for generated flashcards;
- resumable guide checkpoints; and
- document artifact jobs with explicit artifact kind and progress.

Source lifecycle, outline readiness, artifact-job state, and artifact existence
are distinct dimensions. A document with a completed guide but no generated deck
is a valid ready document. Guide state does not reuse flashcard counters or
`generated_deck_id` as a proxy.

Completed guide revisions and learner overlays are durable learner data.
Provider-window checkpoints are rebuildable acceleration data and may degrade to
recomputation if unavailable.

### 5. Compile study guides from the shared plan

Creating a guide performs these stages:

1. ensure canonical derivatives and semantic chunks;
2. ensure or rebuild the compatible outline fragments;
3. compile/reconcile the current `DocumentLearningPlan`;
4. create a versioned study-guide job and deterministic topic work plan;
5. generate bounded source-grounded section content with relevant visual
   evidence;
6. validate and checkpoint each section locally;
7. assemble sections in deterministic topic/source order; and
8. atomically publish a complete new guide revision.

Requests may complete concurrently where provider/network policy permits, but
durable section order and persistence remain deterministic. Cancellation,
interruption, or provider failure retains validated checkpoints. A failed
regeneration never replaces the currently active guide.

The first guide content schema is a bounded structured document containing
paragraphs, lists, term/definition groups, comparisons, process steps, and visual
explanations. Arbitrary HTML, scripts, provider-created citations, and hidden
reasoning are forbidden.

Visual-only or materially visual topics require a verified image-capable
profile. Text-only operation may continue only when canonical text is sufficient;
it must not silently omit essential diagrams, equations, charts, or tables.

### 6. Link generated notes to topics transactionally

Generated card candidates retain an internal provenance envelope through commit:

- source window and locator;
- source evidence hash;
- resolved learning-topic IDs;
- generation job identity; and
- validated card payload.

Native code resolves topic associations from source overlap and evidence; the
provider may not invent topic IDs. The durable relationship is between a note
and a topic because notes own editable learning content while one note may
produce forward/reverse scheduled cards. A note may have one primary topic and
bounded supporting topics.

The note, card presentations, document/deck provenance, and topic links commit in
one transaction. Existing generated cards are not backfilled from wording or tags
alone. Later section-scoped practice expands linked notes into the ordinary
studyable card presentations.

### 7. Keep generated guide content and learner edits as separate layers

A study-guide revision stores immutable generated base content. User changes are
stored as learner-owned overlays so regeneration does not silently destroy them.

The initial editing model supports:

- guide-level notes;
- topic/section notes;
- section-title or section-body overrides tied to a base content hash; and
- text-quote annotations/highlights anchored to a stable section block when
  possible.

An override records the base section hash it edited. If regeneration changes that
base, the overlay becomes explicitly conflicted until the learner chooses to keep
their version, discard it, or reapply it. The generated base remains inspectable.
Annotations are never sent to an AI provider unless a future explicit user action
and privacy contract says otherwise.

Rich editable overlays use the existing validated versioned rich-document
contract; arbitrary HTML is not persisted.

### 8. Export the resolved guide view to PDF locally

PDF export renders the same resolved guide view used by the application:

```text
generated base revision + non-conflicted learner overlays + source labels
```

The export is produced locally through a native `study_guide_export` boundary.
It includes document title, generation/export timestamps, table of contents,
ordered sections, source-unit labels, learner notes selected for export, and page
numbers. It does not include credentials, provider responses, hidden reasoning,
internal IDs, or routine diagnostic data.

The PDF backend must be pure local code, reviewed for licensing and platform
footprint, support selectable Unicode text, paginate deterministically, and work
on Linux and Android. Export failure must not mutate the guide or source
lifecycle. The platform file-save boundary remains under `src/platform/` and
native IPC remains under `src/ipc/`.

### 9. UI and controller ownership

The Documents workflow exposes independent artifact actions rather than treating
upload as synonymous with card generation:

- create/open/regenerate study guide;
- generate/generate-more flashcards;
- open source;
- archive/restore; and
- delete.

A focused study-guide controller owns reads, generation mutations, stale-read
protection, annotation/edit persistence, export, and reconciliation. React
components render the resolved plan/guide view and communicate user intent.

The interactive guide provides a table of contents, global overview/objectives,
ordered topic sections, source navigation, editing/annotation controls, and PDF
export. It uses natural source-unit labels such as pages, slides, chapters,
sections, segments, or images.

### 10. Provider, security, diagnostics, and versioning boundaries

Study-guide prompts and schemas are provider-neutral domain contracts. Provider
wire lowering remains confined to document-intelligence transport. Source and
model content are never written to routine logs.

Versioned identities include at least:

- learning-plan compiler version;
- outline prompt/schema versions;
- study-guide prompt/schema/assembly versions;
- canonical source and semantic evidence identities;
- selected visual evidence;
- provider/model/capability identity; and
- effective reasoning policy.

Credential rotation and unrelated profile edits do not invalidate semantic
artifacts unless they change the task contract. Prompt/schema/compiler changes
invalidate rebuildable checkpoints but do not delete completed guides, learner
annotations, cards, schedules, or review history.

## Invariants

1. Canonical source/evidence remains authoritative; model output is derived.
2. Native code owns topic IDs, hierarchy, order, locators, evidence, and links.
3. Study guides and flashcards consume the same `DocumentLearningPlan`.
4. Durable writes are deterministic even when provider calls are concurrent.
5. Failed regeneration cannot replace a valid active guide.
6. User overlays are independent durable learner data and survive compatible
   regeneration.
7. A topic-linked note commits atomically with its generated card data.
8. Tags are learner organization, not document-topic provenance.
9. PDF export is local, deterministic, and read-only with respect to guide state.
10. Mastery, difficulty color, embeddings, tutoring, and practice-test behavior
    remain later derived consumers and are not stored in the guide body.

## Compatibility and migration

The public baseline and migrations 0002/0003 remain immutable. A new migration is
append-only. Existing documents, outlines, generated decks, cards, schedules,
review history, backups, and source assets remain readable.

Existing outlines may be recompiled into a learning plan without another
provider request when their prompt/schema/source contract remains compatible.
Existing generated cards remain valid but initially have no topic association.
No association is guessed solely from card text or tags.

Backup/restore includes completed learning plans, topics, guide revisions,
learner overlays, and note-topic relationships. Rebuildable in-progress guide
checkpoints may be excluded from portable backups if the backup contract records
that choice explicitly.

Before release, rollback is branch reversion. After a released migration, code
rollback requires a pre-migration backup; the schema remains forward-only.

## Non-goals

- No mastery/difficulty overlay or section-level analytics in this change.
- No embedding runtime, vector index, semantic-search UI, or topic-map inference.
- No interactive tutoring or practice-test generation.
- No collaborative editing, synchronization redesign, or cloud document service.
- No automatic AI rewriting of learner annotations.
- No speculative backfill of topic links for existing cards without source
  provenance.
- No replacement of the native PDF adapter or PR #66 AnyDoc adapters.

## Qualification requirements

Before review readiness:

1. migration, repair, backup/restore, delete/archive, and startup reconciliation
   tests;
2. learning-plan compilation/reconciliation and stable-topic identity tests;
3. outline/card/guide domain-boundary and provider-schema tests;
4. resumable/cancelable/atomic guide-generation tests;
5. topic-linked note commit, move/copy/delete, and reverse-card behavior tests;
6. annotation/edit conflict and regenerated-base tests;
7. deterministic Unicode PDF export tests and platform save-flow tests;
8. compact phone, tablet, desktop, keyboard, reduced-motion, and Android UI tests;
9. static architecture verification preventing artifact/provider/domain leakage;
10. exact-head static, fast, full Linux/Tauri, Android package, and emulator
    qualification; and
11. human live-provider, multimodal, editing, source-navigation, and exported-PDF
    inspection.

## Consequences

The application gains a shared conceptual learning representation instead of
parallel flashcard and guide interpretations. Study guides become durable,
editable-by-overlay, exportable artifacts, and generated notes retain enough
provenance for section-scoped learning later. The migration and orchestration
surface are broader than a guide-only JSON blob, but they establish the stable
identity needed by future tests, tutoring, semantic search, and mastery views
without making those future features authoritative today.
