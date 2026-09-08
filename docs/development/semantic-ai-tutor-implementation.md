# Semantic AI tutor implementation contract

Status: implementation in progress on `agent/semantic-ai-tutor`. This document defines acceptance, not a claim that the application already implements every item. Base: `61f0b463f83e08d0485332c95c2bc562441303f3`.

## Product boundary

The tutor explains selected study material, supplies graduated hints, and conducts user-requested conversational quizzes. Documents, guide sections, generated cards, manually authored cards, and imported notes feed one revision-aware semantic representation. Hosted embeddings and local vector retrieval ground answers in the selected material. The base model may supply clearly distinguished additional explanation.

This is a content-understanding system, not a learner-management system. It does not track curriculum, goals, mastery, misconceptions across sessions, or choose the learner's schedule. Tutor responses and quiz answers never submit FSRS reviews or alter schedules. No embedding model is bundled, downloaded, or executed by the application. No custom agent platform, graph database, hosted application backend, or autonomous tool loop is introduced.

The independent Core/Full packaging and first-run provider-onboarding proposal is not silently folded into this tutor change. Existing non-AI study remains available and unchanged; deployment-edition work has a separate acceptance boundary.

## Acceptance criteria

| ID | Required behavior | Verification boundary |
| --- | --- | --- |
| AC-01 | One native semantic content contract represents source evidence, interpretable knowledge units, artifact links, revisions, origin, and support status. Existing content IDs and schedules survive. | Native validation, migration, provenance and revision fixtures. |
| AC-02 | Documents, effective guide sections, generated cards, manual cards, and imported cards use the shared adapters. Saving cards never waits for AI; committed edits make old interpretations stale. | Manual/imported/generated fixtures, deletion and stale-result tests. |
| AC-03 | Knowledge extraction precedes card/guide authoring and preserves source evidence, conditions and clear question targets; cards and guides retain unit links. | Structured-provider fixtures, atomicity/directionality and source-link tests. |
| AC-04 | Embeddings come from a selected provider profile, with model/dimension/task identity, bounded batches, validation and cancellation. No local embedding runtime. | Provider fixtures including malformed/nonfinite/wrong-sized vectors, unsupported profiles, errors and cancellation. |
| AC-05 | Semantic retrieval uses SQLite vector storage plus exact artifact relationships, not keyword-only search. Scope, revisions, source deletion, index identity and incomplete preparation are explicit. | Retrieval fixtures covering paraphrases, type diversity, stale/deleted entries and incompatible model spaces. |
| AC-06 | A shared StudyContext resolves current material, linked evidence, guide/card context and appropriate conversation history under a bounded request budget. The tutor cannot invent valid source IDs. | Context, scope, citation and token-budget tests. |
| AC-07 | A top-level Tutor and contextual card/guide entry points share persisted chat, selected model, cancellation and recoverable turn states without unmounting study. | Frontend behavior, navigation, keyboard/focus and lifecycle tests. |
| AC-08 | Explain, hint and quiz modes use the common context. Hidden answers and source previews are protected in hint mode. Quiz answer criteria are fixed before evaluation; no scheduler writes. | Hint-leakage fixtures, quiz/restart tests and scheduling-invariance tests. |
| AC-09 | Conversations retain exact messages and revisioned references, with bounded summaries/retrieval and deletion. User answers and model output are never promoted to independent source authority. | Conversation idempotency, interrupted-turn, role/provenance and deletion tests. |
| AC-10 | Settings expose separate tutor/embedding selections using existing native profile/secret ownership. No secret readback, silent provider fallback or implicit library upload. | Settings, consent, provider-policy, redaction and offline fixtures. |
| AC-11 | Backup/recovery, migrations, resource bounds and source deletion preserve authoritative data while vector indexes remain rebuildable. | Backup/restore and deletion/rebuild fixtures. |
| AC-12 | Maintained user/architecture docs, focused/static/fast tests and required Linux/Android qualification reflect the exact published implementation. | GitHub Actions on the final frozen head; live-provider/device boundaries explicitly assigned to manual acceptance. |

## Ownership and implementation sequence

1. Introduce source references, knowledge units and artifact associations over the current SQLite/card/document model. Preserve the canonical source and existing presentation semantics. Add migrations with registered checksums.
2. Add hosted embedding transport/probes and local revision-keyed retrieval. Keep vectors derived and separate from authoritative library backup data. Query and content embeddings must share a compatible space.
3. Move extraction and generator inputs onto the shared representation. Integrate manual/imported material by explicit preparation and revision-aware queued work, never by model calls during keystrokes or save transactions.
4. Implement native conversation/context services and the shared Tutor workspace/panel. UI sends identifiers and revisions; native code resolves scope and owns provider calls and storage.
5. Add explanation, guarded hints and conversational quizzes. Reuse the current overlay stack, request lanes and native authoring/secret boundaries. Keep native review submission untouched.
6. Finish source/documentation review and targeted tests before terminal qualification. Freeze the candidate before full Linux/Android gates. Fix failed boundaries rather than repeatedly rerunning the matrix for unrelated changes.

## Data and trust rules

- Source records establish provenance, not guaranteed truth. Manual cards are user assertions unless linked to supporting evidence; similarity alone cannot supply a citation.
- Generated knowledge units retain source locators and qualifications. They never replace original evidence. Repeated generated derivatives do not count as independent corroboration.
- Interpretable unit identity is independent of embeddings. Changing tutor models does not rebuild vectors. Changing embedding spaces does not reparse documents or alter authored cards.
- Indexing requires explicit selected scope/provider consent. Failure or partial preparation is visible. No whole-library transmission at startup or on an ordinary card save.
- Provider/source content is untrusted data, not executable instructions. The model cannot execute SQL, native commands, file paths or arbitrary tools.
- Narrative conversation summaries are acceleration data with a message boundary, not a learner profile. Raw reasoning is not stored as memory or exposed in ordinary logs.
- A card edit invalidates its interpretation; it does not rewrite its original document or every related guide/card. A source deletion invalidates dependent retrieval entries without erasing unrelated user content.
- Hints must not stream unchecked answer text. Automated leakage controls are tested safeguards, not a promise of perfect semantic non-disclosure.

## Required manual acceptance

Use a small non-sensitive document plus manual/imported cards. Verify source-grounded questions, a cross-reference between a guide and its cards, changed-card re-preparation, provider errors/quota exhaustion, no automatic review submission, mobile Back/keyboard behavior and preserved guide/card position. Live provider accounts, actual retention policy, physical Android lifecycle and visual hint quality are not established by deterministic provider fixtures.
