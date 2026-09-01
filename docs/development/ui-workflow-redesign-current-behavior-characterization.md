# UI workflow redesign: current-behavior characterization

This document is the Stage 0B pre-change baseline for the learning-workflow redesign in [ADR 0049](../decisions/0049-learning-workflow-ui-simplification.md). It characterizes the implementation on branch `agent/ui-workflow-redesign` at `5ee0cf0ead5c40dc1dc7428f0ba6eeb2d3ce0057`. Later stages must treat statements here as historical baseline evidence, not as permission to preserve a known gap or as evidence that an ADR 0049 target has shipped.

The scope is AC-01 Practice, AC-02 document lifecycle, AC-03 Study Guide lifecycle, AC-04 consistency boundaries, AC-05 path-and-test evidence and explicit absences, AC-06 one durable linked baseline, AC-07 only the three matching roadmap items, and AC-08 documentation-only validation. Product, schema, IPC, smoke, CI, styling, dependency, and Stage 1+ changes are non-goals.

## Evidence method and vocabulary

Claims below come from maintained architecture and ADRs, the bound source tree, migrations `0001` through `0005`, and named tests. Paths and symbols/test names are used instead of line numbers so later code movement does not disguise the baseline.

The following terms are intentionally distinct:

- **Transient UI state** is React state, refs, request tokens, dialog state, and in-memory return targets. It is lost on WebView/process replacement unless separately checkpointed.
- **Device checkpoint state** is the bounded `device-state/ui-checkpoint-v1.json` navigation hint or the separate `localStorage` Practice setup scope. Neither is learner-domain authority and neither is included in `.ostudybackup`.
- **Native authority** is validated Rust behavior and process-local native coordination such as cancellation registries.
- **Durable structured state** is SQLite data: sessions, schedules, review events, documents, revisions, jobs, checkpoints, plans, topics, guides, overlays, and provenance.
- **Managed filesystem state** is application-owned source and derivative bytes under the ostudy data directory. Prepared exports use a separate transfer area with a bounded lifetime.
- **Commit** means the authoritative transaction or file/database boundary succeeded. **Reconciliation** means React, a job projection, or filesystem cleanup caught up with that success.

The ownership model is consistent with `docs/architecture/overview.md`, `docs/architecture/ui.md`, `docs/architecture/card-format-and-data-model.md`, and `docs/architecture/ai-and-document-processing.md`.

## AC-01: Practice

### Entry paths and selection modes

The top-level `practice` section currently contains setup, recent activity, active-session UI, completion UI, and the Study Guide library. Setup defaults to every active deck with at least one generated card, even when nothing is due. Its last deck selection is an optional, sorted `localStorage` value (`ostudy.practice.setup.v1`); invalid or now-ineligible IDs are dropped, and an empty surviving selection falls back to all eligible decks. This setup preference is not the durable Practice session and is not the native UI checkpoint. Evidence: `src/screens/PracticeScreen.tsx`, `src/screens/practice/PracticeSetup.tsx`, `src/screens/practice/practiceSetupScope.ts`; tests `practice setup scope > treats active decks with generated cards as eligible even when nothing is due`, `restores the still-valid portion`, and `round-trips a canonical device-local scope` in `src/screens/practice/practiceSetupScope.test.ts`.

Current launch behavior is deliberately different by origin:

- Setup starts a selected deck/tag scope as `scheduled`, or offers explicit `extra` study when the authoritative workload is empty.
- `practiceDeck` launches one deck as `extra`, bypassing daily scheduled limits while retaining studyability checks.
- `practiceScope` launches deck IDs, tag IDs, or a tag-only scope as `scheduled`. Tags constrain notes; callers need not enumerate decks for a tag-only launch.
- `practiceCards` and Study Guide topic practice pass exact card IDs. Rust records these sessions as `custom`; the launch request's `scheduled` field is not the resulting mode.

Evidence: `src/controllers/useAppController.ts` (`practiceDeck`, `practiceScope`, `practiceCards`), `src/screens/PracticeScreen.tsx` (`beginSession`, `beginCardSession`), `src/ipc/practice.ts`, and `src-tauri/src/practice_selection.rs`; tests `auto-starts a deck-origin launch as explicit extra practice`, `starts filtered cards as a custom session`, and `PracticeScreen tag scope > auto-starts a tag-only scope without enumerating decks` in `src/screens/PracticeScreen.test.tsx` and `src/screens/PracticeScreen.tagScope.test.tsx`.

Rust validates up to 100 decks, 32 tags, and 10,000 session cards. Scheduled planning applies the current daily new/review limits and due workload. Extra planning selects studyable cards without the scheduled due/daily-allocation restriction. Exact-card planning preserves the caller's de-duplicated order unless shuffle is requested and rejects the whole start if any requested card is suspended, buried, deleted, archived, or otherwise unavailable. Deck/tag scope IDs must currently exist; a scope may contain decks, tags, or both. Evidence: `src-tauri/src/practice_selection.rs` (`validate_scope`, `validate_current_scope`, `load_exact_planned_cards`), `src-tauri/src/workload.rs`, and `src-tauri/src/projections/practice.rs`.

### Session identity, snapshots, and start failures

Every start carries a frontend-created `operationId`. `PracticeScreen` retains one ID in a ref for the exact canonical start key (kind, IDs, shuffle, and mode) while an outcome is ambiguous. SQLite stores it as `practice_sessions.start_operation_id` with a request fingerprint. Repeating the same ID and payload returns the one committed session; reusing it for another payload or crossing selected/custom start kinds returns `idempotency_conflict`.

Rust prepares and hydrates the entire first response window before opening the session transaction, then atomically inserts the session, ordered session cards, and per-card presentation snapshots. A hydration failure therefore commits no invisible session. If an already committed session cannot be reloaded during an equivalent retry, Rust returns `committed_practice_session_unavailable`; React retains the operation ID and instructs the learner to retry the same start. An unknown transport error is also treated as ambiguous and retains the ID. A typed non-commit error clears it. The ID lives only in the mounted React ref, so a WebView/process loss between an ambiguous commit and retry does not preserve that retry identity.

Evidence: `src/screens/PracticeScreen.tsx` (`operationIdForStart`, `handleStartFailure`), `src-tauri/src/practice_selection.rs` (`practice_session_for_operation`, `create_practice_session`); Rust tests `equivalent_practice_start_retry_returns_one_committed_session`, `response_hydration_failure_occurs_before_session_commit`, `operation_identifiers_cannot_cross_practice_start_kinds`, and `practice_start_persists_snapshot_and_returns_a_bounded_first_window` in `src-tauri/src/practice_selection/operation_tests.rs`; React test `reuses one practice-start operation after an ambiguous transport failure` in `src/screens/PracticeScreen.test.tsx`.

### Review identity, commit, and retry

Viewing and revealing are idempotent timestamp updates on one unrated session card. Reveal also returns an FSRS preview with the authoritative schedule revision. A review submission uses a separate `submissionId` and fingerprints session, card, outcome, response time, and schedule revision. One immediate SQLite transaction validates the open/revealed session card and unchanged schedule revision, applies FSRS, updates `card_schedules`, marks the session card rated, refreshes session counters/completion, appends `review_events`, writes attribution, and commits. Replaying the same submission and payload returns the originally stored result without another schedule transition or review event; changing the payload yields `idempotency_conflict`.

React holds the pending submission and exact score in a ref. An unknown transport failure keeps it and permits only the same score to retry. A typed error clears it; `schedule_changed` and `idempotency_conflict` also hide the answer and discard the stale preview. A pending ambiguous review blocks reveal, card editing/schedule actions, explicit End session, and navigation termination. The pending submission ID is not device-checkpointed, so process loss relies on durable review/session state rather than preserving the exact frontend retry affordance.

After a successful review response, React clears the pending submission and advances. If the next card-window read then fails, the review is still committed and is not resubmitted, but the screen reports a generic window error rather than a distinct committed-but-not-reconciled state. Evidence: `src/screens/PracticeScreen.tsx` (`revealAnswer`, `submitScore`, `advanceOrComplete`), `src-tauri/src/practice.rs` (`submit_review_in_connection`); tests `retries an ambiguous review with the same submission and payload` in `src/screens/PracticeScreen.test.tsx` and `equivalent_review_retry_returns_the_original_committed_result` plus `reused_submission_id_with_a_different_payload_is_rejected` in `src-tauri/src/practice/operation_tests.rs`.

### Checkpoint save and authoritative restoration

While a session is active, React debounces a checkpoint containing `section=practice`, session ID, displayed absolute position, and revealed hint. Rust validates, size-bounds, writes, syncs, and atomically renames `device-state/ui-checkpoint-v1.json`. Invalid files are quarantined. The file intentionally excludes cards, reviews, credentials, and learner history. Evidence: `src/screens/PracticeScreen.tsx`, `src/ipc/uiCheckpoint.ts`, and `src-tauri/src/ui_checkpoint.rs`; tests `checkpoint_accepts_a_bounded_practice_identity` and `checkpoint_rejects_sensitive_or_unbounded_shapes_through_strict_deserialization`.

At startup, `useAppController` sends a Practice checkpoint to `PracticeScreen`; a Practice destination without a session checkpoint is normalized to Flashcards/Decks. `resume_practice_session` ignores the checkpoint's position and revealed values as authority. It queries SQLite for the first session card that is neither rated nor dismissed, derives its position/revealed state and durable counters, and loads a bounded window there. A stale hint therefore cannot skip or duplicate reviews. If that card had already been revealed, React calls reveal again to obtain a fresh schedule preview. Completed/no-pending sessions restore directly to completion. Evidence: `src/controllers/useAppController.ts`, `src/screens/PracticeScreen.tsx`, `src-tauri/src/practice_resume.rs`, and the module-level restoration contract in `practice_resume.rs`.

### Completion, restart, return, guard, and early exit

Rating or dismissing the last session card sets `practice_sessions.completed_at` as part of native counter refresh. The completion view can open Analytics scoped to the session/decks, return to setup, or restart. Restart starts a new session with a new operation ID over the retained exact-card list or selected deck/tag scope and preserves scheduled-versus-extra mode; it does not reopen or reset the completed session.

`complete_practice_session` is an idempotent early termination (`completed_at = COALESCE(...)`). The active-session exit handler calls it before navigation. Main-section navigation first confirms termination; cancellation keeps the learner in Practice, native failure prevents navigation, and already committed reviews remain authoritative. The path deliberately does not show post-review Analytics. The in-session End action also completes first: an external launch returns to its transient `PracticeReturnTarget` (including deck detail or Flashcards workspace when supplied); a top-level/no-target session resets to setup. Return targets are React navigation state, not SQLite or checkpoint content. Evidence: `src/controllers/practiceExitGuard.ts`, `src/controllers/useAppController.ts`, `src/screens/PracticeScreen.tsx`, `src-tauri/src/practice.rs`; tests `returns to an external launch origin when End session terminates early` and `opens analytics for the completed session and its deck scope` in `src/screens/PracticeScreen.test.tsx`, plus both `active practice navigation` tests in `src/controllers/useAppController.practiceExit.test.tsx`.

### Practice boundary matrix

| Concern | Current boundary |
| --- | --- |
| Transient UI | Scope selections, tag selection, launch/return target, loaded card windows, reveal/preview, busy/error state, confidence display counts, pending start/review IDs. |
| Durable authority | SQLite session/card snapshots, `card_schedules`, append-only `review_events`, session counters/completion. Rust owns selection, FSRS, validation, and transactions. |
| Device-only hints | Setup deck IDs in `localStorage`; navigation/session hint in `ui-checkpoint-v1.json`. Neither overrides SQLite resume state. |
| Idempotency | Start `operationId` and review `submissionId` are distinct. Reveal/view and explicit completion are naturally idempotent. Return/navigation and window reads have no mutation identity. |
| Stale reads | `PracticeController` has independent latest-result lanes for workload, card windows, and recent reviews. Resume/launch effects use mounted/request guards, but not a general controller lane. Mutations are not canceled as stale reads. |
| Optimistic revision | Review requires the previewed schedule revision. Practice start validates current scope but does not accept deck revisions. |
| Cancellation | No native cancellation of ordinary Practice reads. UI refs suppress duplicate starts/reveals/reviews/actions. |
| Commit versus reconciliation | Session creation hydrates before commit. Review commits before React advances/loads another window. Early completion commits before navigation. A later window/navigation refresh problem does not roll back the native mutation, though some messages are not specialized for reconciliation failure. |

### Practice behavior absent at the baseline

There is no internal-only active Practice route, no migration of setup-only Practice checkpoints beyond startup fallback to Decks, and no relocation of setup into Flashcards. Return targets and ambiguous retry refs are not durable. Not every async screen effect uses an independent request lane, and committed-versus-refresh failure is not consistently presented as a distinct UI state. These are findings, not Stage 2/3 implementation. Evidence: `src/controllers/useAppController.ts`, `src/screens/PracticeScreen.tsx`, and the affected-surface inventory.

## AC-02: Document lifecycle

### Active processing and cancellation

Import first establishes a durable document ID, then generation runs inside one process-local cancellation scope keyed by document ID. The same registry gates document work, provider streams, and supported OCR/render work. React polls live progress for documents whose durable lifecycle is `processing` and holds upload, busy, deleting, flashcard-canceling, guide-generating, and guide-canceling IDs only in component state.

Stop requests cancel the exact current native task/stream; they are not durable cancellation handles and do not prove provider-side billing termination. Rust records canceled generation/job state and settles the document to `ready` when an active generated deck or Study Guide remains, `failed` when neither remains, or `archived` when archived. Validated generation/outline/guide checkpoints already committed remain available for compatible retry. On startup, `processing` documents are settled with an interruption error and active generation/artifact jobs become `interrupted`; network transmission is never resumed automatically. Evidence: `src/screens/DocumentsScreen.tsx`, `src/ipc/documents.ts`, `src-tauri/src/document_operations.rs`, `src-tauri/src/task_cancellation.rs`, `src-tauri/src/documents/lifecycle.rs`, `src-tauri/src/document_artifacts.rs`; tests `offers an exact stop action while a document generation task is processing` in `DocumentsScreen.test.tsx`, `cancellation_preserves_ready_state_for_a_guide_only_document`, and `startup_interruption_preserves_ready_state_for_a_guide_only_document` in `documents/lifecycle.rs`.

### Archive and restore

Archive is reversible metadata, not deletion. The UI hides archived rows unless requested and disables archive/restore while it sees processing or another local operation. Native `set_document_archived` validates document ID and expected document revision, derives the settled lifecycle, sets/clears `archived_at`, increments the revision, and reloads the row. Archived source bytes, canonical/semantic derivatives, outlines, jobs, deck provenance, guides, overlays, and generated content remain intact. Restore derives `ready` when an active generated deck or Study Guide exists and otherwise `failed`; it does not pretend a retained source alone is a completed learning artifact. A stale revision returns `document_changed`.

The Rust archive command itself is revision-checked but does not include a native active-task/lifecycle predicate; the current React workflow is what suppresses archive during visible processing. Evidence: `src/screens/DocumentsScreen.tsx` (`toggleArchived`), `src/ipc/documents.ts`, `src-tauri/src/documents/lifecycle.rs` (`set_document_archived`, `settled_state`); test `settled_state_truth_table_preserves_available_learning_artifacts` in `documents/lifecycle.rs` and the manual archive-versus-delete contract in `docs/development/document-streaming-and-deletion-testing.md`.

### Permanent deletion and managed-file reconciliation

Permanent deletion accepts document ID plus expected revision. Rust refuses an active process-local document task, refuses durable `processing`, validates the stored revision twice (before file movement and inside the transaction), and verifies the managed source relative path against document identity/format. It renames the whole managed document directory into `documents/trash/<document>-<time>.delete` before beginning an immediate SQLite transaction.

Inside that transaction, ostudy preserves citations for surviving generated decks, then deletes the `documents` row. Foreign-key cascades remove document-owned source files/units/blocks/assets, pages/renders, semantic elements/chunks, outlines, outline/generation/guide checkpoints, flashcard and artifact jobs, document/deck and note/topic provenance, learning plans/topics, Study Guide revisions/sections/overlays, and related document derivatives. The deck, notes, cards, schedules, and review events do not belong to the document row and survive.

If the database operation fails, Rust renames the staged directory back and returns the failure. Once SQLite commits, deletion is authoritative: final trash removal is best effort. Startup first restores a staged directory when the document row survived an interrupted pre-commit delete, then ordinary trash reconciliation removes unreachable residue when the row is gone. Evidence: `src-tauri/src/document_operations.rs`, `src-tauri/src/document_delete_reconciliation.rs`, `src-tauri/migrations/0001_public_baseline.sql`, `0002_canonical_documents.sql`, `0004_study_guides_and_learning_topics.sql`, and `0005_study_guide_pdf_exports.sql`; tests `restores_a_staged_directory_when_the_document_row_survived` in `document_delete_reconciliation.rs` and `deleting_document_cascades_through_parent_child_learning_topics` in `src-tauri/tests/study_guide_migration5.rs`.

### Generated content survival and textual citation preservation

Before deleting provenance, Rust loads every non-soft-deleted deck linked through `document_decks`. It constructs a format-neutral sentence containing document title, original filename, format label, and source-unit count/kind. It replaces the known automatic `AI-generated flashcards from <filename>.` text when present, otherwise appends the sentence. If the 2,000-character deck-description limit would be exceeded, an existing title/filename mention is accepted as a serviceable human-edited reference; otherwise deletion fails without truncating user text. Soft-deleted decks are ignored.

Citation updates and document deletion share the same transaction. The surviving deck revision increments only when its description changes. The document/deck provenance row then cascades, so the surviving deck/card graph is ordinary detached study content. There is no durable structured citation snapshot, per-note citation record, retained `SourceLocator`, or live file link after deletion; the only current preservation path is human-readable deck-description text. Evidence: `src-tauri/src/document_operations.rs` (`deleted_document_citation`, `preserve_deleted_document_citation`, `load_surviving_generated_decks`); tests `deletion_citation_replaces_the_generated_file_reference`, `deletion_never_truncates_a_user_description_that_already_names_the_source`, and `soft_deleted_generated_decks_are_not_counted_for_document_citation_preservation`; React tests `permanently deletes a document only after confirmation and preserves deck citations` and `does not claim citation preservation after the only generated deck was deleted` in `DocumentsScreen.test.tsx`.

### Document boundary matrix

| Concern | Current boundary |
| --- | --- |
| Transient UI | List/loading/error, live progress, local busy/cancel/delete IDs, archive filter, confirmation and toast state. |
| Durable authority | SQLite document revision/lifecycle, jobs/checkpoints/derivatives/provenance and managed source paths; managed filesystem owns source/derivative bytes. |
| Idempotency | Import staging has its own operation contract, but archive/delete use optimistic revision rather than replay identity. Repeating a successful delete returns not found; it does not duplicate deletion. |
| Stale reads | Initial list effects use an `active` mounted flag. The screen does not use focused independent request lanes for every list/progress/refresh concern, so overlapping refreshes are not universally latest-result protected. |
| Optimistic revision | Archive/restore and delete require document revision. Delete rechecks after staging. |
| Cancellation | Process-local exact-document signal; durable terminal/interrupted job state and reusable checkpoints remain after the signal disappears. |
| Commit versus reconciliation | Pre-commit file staging is rolled back on database failure. SQLite commit makes deletion final; trash cleanup is reconcilable. React waits for both document and deck refreshes before showing delete success, so a post-commit refresh rejection enters its generic failure path even though native deletion already committed. Archive similarly awaits refresh before success copy. |

### Document behavior absent at the baseline

Archive/restore still exists. There is no durable citation snapshot independent of deck description, no document citation renderer across deck/card/search/export surfaces, and no preservation of exact source locators after document deletion. The delete cascade already removes document-bound Study Guide data, but no guide-only deletion is implemented. These absences are Stage 6/8 targets, not current behavior.

## AC-03: Study Guide lifecycle

### Learning-plan revisions and stable topics

The active `DocumentLearningPlan` is compiled from canonical semantic evidence. An identical active plan fingerprint returns the same plan. A changed plan creates the next append-only revision and marks the former active plan `superseded` in one transaction. Stable `document_learning_topics` are document-owned identities keyed by a native anchor derived from source locator and semantic evidence, not model wording. Exact/equivalent evidence anchors reuse topic IDs across presentation revisions; changed source evidence creates a new identity. Plan-topic rows retain hierarchy, deterministic order, source locator, evidence indices/hashes, title, summary, objectives, and concepts.

Evidence: `src-tauri/src/document_learning.rs` (`ensure_document_learning_plan`, `load_active_document_learning_plan`), migration `0004_study_guides_and_learning_topics.sql`; tests `presentation_revision_reuses_stable_topic_identity`, `changed_source_evidence_creates_a_new_topic_identity`, `semantic_chunk_reindexing_preserves_topic_identity`, and `source_selection_prefers_the_narrowest_overlapping_topic` in `document_learning.rs`.

Generated notes link transactionally to one primary and bounded supporting stable topics through `document_note_topics`; card projections expand linked notes into currently studyable card presentations. These links are provenance, not tags. Existing cards without provenance are not inferred from wording. Evidence: `src-tauri/src/document_card_topics.rs`, `src-tauri/src/document_topic_links.rs`, migration `0004`, and ADR 0043.

### Guide revisions, generation jobs, checkpoints, and regeneration

Creating or regenerating a guide ensures canonical chunks, outline, and the compatible active learning plan, then creates a durable `document_artifact_jobs` row with provider/profile/model snapshot and topic total. Only one active artifact job of any kind may run for a document. Job transitions are revision-like compare-state updates (`queued -> preparing -> generating -> committing -> succeeded`, with terminal failure/interruption/cancellation paths).

Each validated topic section is stored best effort in `document_study_guide_checkpoints` under a contract key containing plan fingerprint, provider/profile/model/reasoning, and guide schema/prompt/assembly versions. Compatible checkpoints are hash-validated and reused; invalid/missing checkpoint storage degrades to provider recomputation. Cloud topics may run four at a time, local/LAN topics serially, but durable resolution and publication follow plan order.

Publication validates a complete section set, then one immediate transaction marks the prior active guide `superseded`, inserts the next immutable guide revision and all sections, reconciles content-bound overlays, and marks the owning document ready/archived. A malformed/incomplete replacement or generation failure leaves the prior active guide available. Guide publication is the user-visible commit; failure to mark the already-published job `succeeded` is logged as a lagging projection and does not hide/delete the guide. Obsolete checkpoint cleanup is also best effort.

The generation command has no caller-supplied operation/idempotency ID. The active-job constraint prevents concurrent duplicate work, but an ambiguous response after a completed publication can be followed by a new regeneration and a further valid revision. Evidence: `src-tauri/src/document_study_guide_commands.rs`, `src-tauri/src/document_artifacts.rs`, `src-tauri/src/document_study_guide_generation.rs`, `src-tauri/src/document_study_guides.rs`; tests `artifact_job_state_machine_rejects_skipping_generation`, `execution_requires_plan_ordered_evidence`, `checkpoint_contract_name_is_versioned`, and `publishing_guide_atomically_marks_guide_only_document_ready`.

Study Guide cancellation uses the same process-local exact-document task registry as flashcard generation. Cancellation closes current work, transitions the guide job to `canceled` when possible, preserves compatible checkpoints and any prior active guide, and returns `document_study_guide_canceled`. Startup changes active artifact jobs to `interrupted`; it does not silently restart provider work. Evidence: `src-tauri/src/document_study_guide_commands.rs`, `src-tauri/src/document_study_guide_generation.rs`, and `src-tauri/src/document_artifacts.rs`.

### Learner overlays and conflict behavior

Generated guide revisions are immutable; learner changes are separate validated rich-document overlays:

- guide notes belong to the document and have neither guide nor topic ID;
- topic notes belong to a stable topic and have no guide ID;
- title/body overrides and highlights belong to an active guide/topic and exact base-content hash.

Create has no expected revision because the ID is new. Update and discard require the overlay's current revision and increment it; stale writes return `study_guide_overlay_changed`. Content-bound save also proves the referenced guide is active and its section still has the exact base hash, otherwise `study_guide_overlay_base_changed` is returned.

During regeneration, a content-bound active overlay moves to the new guide and stays active only when the topic and base content hash are unchanged. Remaining active content-bound overlays move to the new guide as `conflicted`, preserving the learner text and old base hash. Document/topic notes remain revision-independent. The current dialog displays conflicted text and supports discard. It does not provide the ADR's complete keep/discard/reapply conflict-resolution workflow: there is no reapply command or explicit “keep as active against the new base” action.

Evidence: `src-tauri/src/document_study_guides.rs` (`publish_document_study_guide`, `save_study_guide_overlay`, `discard_study_guide_overlay`), `src/screens/StudyGuideDialog.tsx`; Rust tests `content_bound_overlay_rejects_a_hash_outside_the_active_section` and `regeneration_keeps_compatible_edit_active_then_surfaces_changed_base_as_conflict` in `document_study_guides/overlay_regression_tests.rs`; React tests `saves topic notes independently of the generated guide revision` and `shows preserved conflicted section text and lets the learner discard it` in `StudyGuideDialog.test.tsx`.

### Library, reader, and topic Practice links

The library is currently rendered inside Practice setup, lists at most 250 active guide revisions (including archived sources by default), and opens the selected document's guide in a fullscreen-presented dialog. It is not a top-level application section. The dialog directly owns load/generate/cancel/save/export/editor state; there is no focused Study Guide controller. Initial loads and topic projection effects use mounted `active` flags, but ordinary `refresh()` calls have no request-lane/latest-result token, so overlapping reads are not comprehensively protected from stale overwrite.

For each guide section, the dialog reads a durable note/card/studyable-card projection. “Study linked cards” asks Rust for exact current card IDs for the stable topic and passes them to the app's direct-card Practice capability, producing a `custom` Practice session. No separate Study Guide scheduler or mastery state exists. Evidence: `src/screens/PracticeScreen.tsx`, `src/screens/StudyGuideLibrarySection.tsx`, `src/screens/StudyGuideDialog.tsx`, `src/ipc/studyGuides.ts`, `src-tauri/src/document_study_guide_commands.rs`, `src-tauri/src/document_topic_links.rs`; tests `lists generated guides inline and opens the selected guide` in `StudyGuideLibrarySection.test.tsx` and `routes exact topic-linked cards through the app practice capability` in `StudyGuideDialog.test.tsx`.

### PDF preparation, publication, and artifact lifetime

PDF preparation is local and read-only with respect to the source guide. It loads the active guide, resolves generated content plus active overlays, optionally includes guide/topic notes and highlights, excludes conflicted/discarded overlays, renders deterministic selectable Unicode PDF bytes, and registers them in `prepared_exports` with purpose `study_guide_pdf`, operation ID, hash, relative transfer path, `prepared` state, and seven-day expiry. Reusing the same explicit operation ID returns the existing verified prepared export; the frontend normally creates a fresh ID per export action.

The dialog then publishes the opaque prepared export through the platform boundary and releases it in `finally`; release marks the row `released` and removes ostudy's prepared bytes. Startup marks missing files failed, releases expired exports, and cleans unreferenced transfer files. `prepared_exports` has no document/guide foreign key, so an already prepared artifact does not participate in the owning-document cascade; it lives until frontend release/startup expiry, while any user-published destination copy is outside document storage. Evidence: `src-tauri/src/document_study_guide_export.rs`, `src-tauri/src/external_files.rs`, migration `0005_study_guide_pdf_exports.sql`, `src/screens/StudyGuideDialog.tsx`; tests `renders structured source-grounded sections and exports through native prepared files` in `StudyGuideDialog.test.tsx`, `study_guide_pdf_is_a_valid_prepared_export_purpose` in `study_guide_migration5.rs`, and `unicode_pdf_round_trips_as_selectable_text` in `document_study_guide_export.rs`.

### Owning-document deletion cascade

Every plan, stable topic, guide revision, guide section, overlay, artifact job, note-topic link, and guide checkpoint is document-owned through `ON DELETE CASCADE` (with secondary guide/plan cascades). Permanent document deletion therefore removes the complete document-bound Study Guide graph, including learner notes and conflicts. The generated deck/note/card graph survives, but its `document_note_topics` links disappear. Prepared PDF rows/bytes are separate transfer artifacts and do not cascade from the document row. The complete backup includes active/superseded plans, topics, guides, overlays, and topic links while the document exists. Evidence: migrations `0004` and `0005`; test `deleting_document_cascades_through_parent_child_learning_topics` in `study_guide_migration5.rs` and `study_guide_topic_graph_survives_complete_backup_restore` in `src-tauri/src/library_backup/study_guide_backup_tests.rs`.

### Study Guide boundary matrix

| Concern | Current boundary |
| --- | --- |
| Transient UI | Selected library row/dialog, loading/generating/canceling/saving/exporting flags, editor text, annotation-export toggle, topic projections, toasts/errors. |
| Durable authority | SQLite plans/topics, guide revisions/sections, overlays/revisions/conflicts, note-topic links, artifact jobs, checkpoints, and prepared-export rows; managed transfer filesystem owns prepared PDF bytes. |
| Idempotency | Plan compilation reuses an identical active fingerprint. PDF prepare is operation-ID idempotent. Guide generation/publication and overlay create do not accept caller mutation IDs; updates/discards use optimistic revisions/state transitions. |
| Stale reads | Component mount guards exist, but no Study Guide controller or independent request lanes protect all list/detail/projection refreshes. |
| Optimistic revision | Overlay update/discard checks revision; content edits check active guide plus base hash. Guide publication compares active job state but does not accept an expected guide/document revision from React. |
| Cancellation | Process-local exact-document signal; durable canceled/interrupted job and reusable topic checkpoints. Prior published guide remains. |
| Commit versus reconciliation | Complete guide publication is atomic. Job-success projection and obsolete-checkpoint cleanup are post-commit/best effort. Overlay mutation commits before dialog refresh, but the current UI reports refresh failure through the same generic catch path. PDF registration is separate from platform publication and release. |

### Study Guide behavior absent at the baseline

There is no `study-guides` `AppSection`, navigation item, screen/controller, route checkpoint, guide-only delete command, guide-delete revision contract, persistent reader position, mastery/difficulty overlay, or full conflict reapply workflow. There is no durable association from a prepared PDF row back to the document/guide. Document deletion—not a guide-specific use case—is the only current way to cascade the guide graph. Evidence: `src/controllers/useAppController.ts`, `src/App.tsx`, `src/ipc/studyGuides.ts`, `src-tauri/src/lib.rs`, and `docs/development/ui-workflow-redesign-affected-surface-inventory.md`.

## Cross-layer baseline conclusions

1. Practice uses durable native session/review identity plus two weaker device/UI hint layers. Later navigation work must not promote a hint into authority or lose the two distinct idempotency keys.
2. Document archive is reversible and non-destructive. Permanent deletion has an explicit filesystem-staging/SQLite-commit/reconciliation boundary; surviving generated content is detached and today retains only a textual deck-description citation.
3. Study Guides already have durable revisions, stable topics, learner overlays, jobs, checkpoints, topic-linked Practice, and prepared PDF export. They do not yet have first-class routing or guide-only deletion.
4. Several current screens use mounted flags or selected request lanes rather than complete controller-owned stale-read policy, and some committed mutations are followed by refreshes inside one generic UI error path. Later stages must preserve the native commit while improving reconciliation semantics, not reinterpret current gaps as intended target behavior.

This document completes only the three current-behavior characterization items in Stage 0. Visual baselines, timing/profiling scenarios, and measurable performance budgets remain open.
