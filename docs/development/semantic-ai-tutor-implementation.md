# Semantic AI tutor implementation contract

Status: maintained acceptance contract for `agent/semantic-ai-tutor`. The PR records qualification for the exact published head; this document is not test-run evidence. Base: `61f0b463f83e08d0485332c95c2bc562441303f3`.

## Product boundary

The tutor explains selected study material, supplies graduated hints, and conducts user-requested conversational quizzes. Documents, guide sections, generated cards, manually authored cards, and imported notes feed one revision-aware semantic representation. Hosted embeddings and local vector retrieval ground answers in the selected material. The base model may supply clearly distinguished additional explanation.

This is a content-understanding system, not a learner-management system. It does not track curriculum, goals, mastery, misconceptions across sessions, or choose the learner's schedule. Tutor responses and quiz answers never submit FSRS reviews or alter schedules. No embedding model is bundled, downloaded, or executed by the application. No custom agent platform, graph database, hosted application backend, or autonomous tool loop is introduced.

The independent Core/Full packaging and first-run provider-onboarding proposal is not silently folded into this tutor change. Existing non-AI study remains available and unchanged; deployment-edition work has a separate acceptance boundary.

## Acceptance criteria

| ID | Required behavior | Verification boundary |
| --- | --- | --- |
| AC-01 | One native semantic content contract represents source evidence, interpretable knowledge units, artifact links, revisions, origin, and support status. Existing content IDs and schedules survive. | Native evidence/authority validation, migration, cross-source dependencies and whole-document revision fixtures. |
| AC-02 | Documents, effective guide sections, generated cards, manual cards, and imported cards use the shared adapters. Saving cards never waits for AI; committed edits make old interpretations stale. | Manual/imported/generated fixtures, deletion and stale-result tests. |
| AC-03 | Native evidence IDs support multi-span knowledge extraction before outline/card/guide authoring. Valid siblings survive targeted repair; partial coverage remains explicit and resumable; cards and guides retain unit links. | Injected-provider orchestration, partial/repair/concurrency tests, atomic overview publication, directionality and source-link tests. |
| AC-04 | Embeddings come from a selected provider profile, with model/dimension/task identity, bounded batches, validation and cancellation. No local embedding runtime. | Provider fixtures including malformed/nonfinite/wrong-sized vectors, unsupported profiles, errors and cancellation. |
| AC-05 | Semantic retrieval uses SQLite vector storage plus exact artifact relationships, not keyword-only search. Scope, revisions, source deletion, index identity and incomplete preparation are explicit. | Retrieval fixtures covering paraphrases, type diversity, stale/deleted entries and incompatible model spaces. |
| AC-06 | A shared StudyContext resolves current material, linked evidence, guide/card context and appropriate conversation history under a bounded request budget. The tutor cannot invent valid source IDs. | Context, scope, citation and token-budget tests. |
| AC-07 | A top-level Tutor and contextual card/guide entry points share persisted chat, selected model, cancellation and recoverable turn states without unmounting study. | Frontend behavior, navigation, keyboard/focus and lifecycle tests. |
| AC-08 | Explain, hint and quiz modes use the common context. Hidden answers and source previews are protected in hint mode. Quiz answer criteria are fixed before evaluation; no scheduler writes. | Hint-leakage fixtures, quiz/restart tests and scheduling-invariance tests. |
| AC-09 | Conversations retain exact messages and revisioned references, with bounded summaries/retrieval and deletion. User answers and model output are never promoted to independent source authority. | Conversation idempotency, interrupted-turn, role/provenance and deletion tests. |
| AC-10 | Settings expose separate tutor/embedding selections using existing native profile/secret ownership. No secret readback, silent provider fallback or implicit library upload. | Settings, consent, provider-policy, redaction and offline fixtures. |
| AC-11 | Backup/recovery, migrations, resource bounds and source deletion preserve authoritative data while vector indexes remain rebuildable. | Backup/restore and deletion/rebuild fixtures. |
| AC-12 | Maintained user/architecture docs, focused/static/fast tests and required Linux/Android qualification reflect the exact published implementation. | GitHub Actions on the final frozen head; live-provider/device boundaries explicitly assigned to manual acceptance. |

## Consumer integration acceptance

| ID | Required behavior | Verification boundary |
| --- | --- | --- |
| AC-13 | Switching consumer models reuses saved interpretations; legacy capacity drafts recover locally; independent cloud windows overlap without changing evidence quality rules. | Producer/consumer receipt, capacity recovery and bounded-concurrency tests. |
| AC-14 | Automatic indexing uses saved knowledge only, and scoped retrieval catches up missing vectors without interpreting the document again. | Read-only index revision tests and the absence of interpretation calls in the index path; live-provider latency remains manual. |
| AC-15 | Documents jobs survive route navigation, retain failures and use one guide generation/stop control. | Application-lifetime controller navigation, failure/refresh, duplicate-intent and cancellation tests. |
| AC-16 | Tutor is a chat/history workspace with a Sources checklist and contextual advanced controls. Multi-source choices are native authorization; legacy scopes, hint secrecy and retry identities remain valid. | React workspace/controller/IPC tests and native selected-union/exact-conversation-scope/revocation fixtures. |

## Ownership and implementation sequence

1. Introduce source references, knowledge units and artifact associations over the current SQLite/card/document model. Preserve the canonical source and existing presentation semantics. Add migrations with registered checksums.
2. Add hosted embedding transport/probes and local revision-keyed retrieval. Keep vectors derived and separate from authoritative library backup data. Query and content embeddings must share a compatible space.
3. Move extraction and generator inputs onto the shared representation. Index manual/imported material from saved text with revision-aware catch-up on use, never by interpretation calls during keystrokes, saves or chat preparation.
4. Implement native conversation/context services and the shared Tutor workspace/panel. UI sends identifiers and revisions; native code resolves scope and owns provider calls and storage.
5. Add explanation, guarded hints and conversational quizzes. Reuse the current overlay stack, request lanes and native authoring/secret boundaries. Keep native review submission untouched.
6. Finish source/documentation review and targeted tests before terminal qualification. Freeze the candidate before full Linux/Android gates. Fix failed boundaries rather than repeatedly rerunning the matrix for unrelated changes.

## Data and trust rules

- Source records establish provenance, not guaranteed truth. Manual cards are user assertions unless linked to supporting evidence; similarity alone cannot supply a citation.
- Generated knowledge units retain native evidence references, source locators and qualifications. The model chooses request-local IDs; Rust supplies exact UTF-8 spans, hashes and image selectors. Location validation does not establish factual entailment. They never replace original evidence. Repeated generated derivatives do not count as independent corroboration.
- Interpretable unit identity is independent of embeddings. Changing Tutor or guide models does not re-interpret the document or rebuild vectors. Changing embedding spaces indexes saved content without re-interpreting documents or altering authored cards.
- Document processing and sending material-scoped chat authorize automatic index catch-up through the selected embedding provider. Navigation and merely selecting checkboxes do not transmit content. Advanced maintenance retains its explicit preview/confirmation flow. Indexing is separate from interpretation and never invokes it. Failure or partial source analysis remains visible. An empty output is not a complete disposition without explicit no-learning source IDs. Successful siblings are saved before repair; malformed or duplicate repair slots stay unresolved without destroying unrelated successes. Checkpoint reads are read-only and concurrent stale writes are rejected. No whole-library transmission at startup or on an ordinary card save.
- Provider/source content is untrusted data, not executable instructions. The model cannot execute SQL, native commands, file paths or arbitrary tools.
- Narrative conversation summaries are acceleration data with a message boundary, not a learner profile. Raw reasoning is not stored as memory or exposed in ordinary logs.
- A card edit invalidates its interpretation; it does not rewrite its original document or every related guide/card. A source deletion invalidates dependent retrieval entries without erasing unrelated user content.
- Hints must not stream unchecked answer text. Automated leakage controls are tested safeguards, not a promise of perfect semantic non-disclosure.

## Required manual acceptance

Use a small non-sensitive document plus manual/imported cards. Verify source-grounded questions, a cross-reference between a guide and its cards, changed-card index catch-up, provider errors/quota exhaustion, no automatic review submission, mobile Back/keyboard behavior and preserved guide/card position. Live provider accounts, actual retention policy, physical Android lifecycle and visual hint quality are not established by deterministic provider fixtures.

## Knowledge-representation regression acceptance

The current [representation contract](../architecture/knowledge-representation.md) separates canonical evidence, fallible interpretations, artifact relationships and derived views. In addition to existing Tutor tests, native tests must exercise the real decoder and SQLite publication through an injected provider boundary, not mocks that bypass admission:

- Two valid units plus one invented evidence ID: retain the valid units, repair only the rejected slot, and preserve explicit partial state when repair fails. Reopening a completed checkpoint must make no provider request or durable rewrite.
- Cross-page support: retain both exact spans and project the combined locator. Editing either source invalidates the interpretation and its receipt. Scope filtering or context eviction cannot leave half of a multi-source claim visible.
- Source changes during analysis or reduction: reject stale publication, including a source yielding zero units, added/deleted chunks, invalidated prepared projections, and concurrent reinterpretation. The final outline and generated metadata commit together.
- Malformed/duplicate repair entries: independent valid corrections survive. Duplicate slots do not resolve by response order. Repeated retries stay within bounded proposal and storage limits.
- Legacy records remain readable. New migration tables and partial drafts follow the existing backup, deletion, device-transfer privacy and vector-rebuild rules; no source reset or library deletion is a recovery step.

A real-provider test should first use a small document spanning at least two pages, inspect citations and partial-state behavior, then retry the previously failing 138-page PDF. Use the diagnostics launcher described in the local-testing runbook. `knowledge_analysis_partial` indicates retained usable knowledge with unresolved work; it is not full coverage. `knowledge_extraction_batch_validated` certifies protocol validation, not truth or exhaustive concept extraction.
