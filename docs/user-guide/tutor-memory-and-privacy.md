# Tutor memory and privacy

Tutor is a content-focused chat and quiz tool. It does not maintain a curriculum, change FSRS schedules, or turn chat answers into card reviews. Saved conversations remain readable without a reachable model provider.

## Conversation memory

Open a saved conversation and expand **Conversation memory**. These settings are local; reading or changing them does not contact a provider.

**Use earlier turns automatically** controls whether recent eligible messages and the stored rolling summary enter subsequent responses in this conversation. Disabling it clears the current summary and omits automatic history. Re-enabling it permits eligible history again; it is not the same as forgetting earlier context.

**Allow this conversation in semantic retrieval** is off by default, including for conversations created before this setting existed. Enabling it grants permission, but does not embed or upload the transcript. Explicitly prepare conversations through the selected material's preparation controls before searching them. Retrieval is restricted to conversations in the same selected material scope; general discussions use automatic history instead. Automatic history and explicit semantic retrieval are separate choices. Revoking retrieval removes the conversation's existing semantic source projections.

**Forget earlier context** advances a permanent cutoff to the latest saved turn. The earlier transcript stays readable, but earlier turns are no longer eligible for future summaries, conversation retrieval, or outstanding quiz answers. Clearing a summary alone would allow it to be recreated; the cutoff prevents that. This operation does not erase existing backups, a provider's retained requests, or independent material already authored from a conversation.

**Stored conversation summary** shows the current summary and its coverage boundary. It is generated conversational continuity, not a verified source or a learner profile. Summary inspection is not exposed inside the hidden-answer hint panel.

Privacy changes and conversation deletion wait until active tutor/preparation requests are finished or canceled. An uncertain change result can be checked with its original operation identity without repeating the action over newer choices.

## Manual and imported cards

Creating, editing, importing, rating, and organizing ordinary cards do not require embedding calls. Explicit semantic preparation converts their current native presentations into the same source/unit contracts used by the tutor. They remain identified as authored assertions rather than verified textbook claims.

Content changes retire old interpretations and retrieval entries. This includes native question/answer text, rich content, note type, deletion, and relevant source-image changes. Undo restores the original card identity and scheduling history, but does not revive an old interpretation. Prepare the selected material again when needed. Ratings and scheduling timestamps alone do not invalidate semantic content.

Inactive or deleted sources are excluded immediately from retrieval. A durable queue removes their device-local vectors on the next index opening, in bounded batches. That cache maintenance makes no provider requests. It is logical application deletion, not a promise of forensic erasure from storage snapshots or backups.

## Full backup versus device sync

A **full-library backup** retains conversations, private quiz data, memory preferences/cutoffs, source records, knowledge units, and artifact links. Treat the backup as sensitive personal data: it is not an application-encrypted chat export. Saved API keys are not included.

Device-local vector caches are not included. Restoration removes the pre-restore cache and marks completed preparation receipts as requiring a rebuild. Compatible stored knowledge can be reused when the receiving device explicitly prepares its material; startup never silently contacts an AI provider. Schema-10 backups can be restored and migrated to schema 11 after migration-ledger validation.

**Device sync** has no conversation-sharing category. Chats, summaries, private quiz data, and semantic copies of messages are removed from the transfer snapshot, not merely ignored by the receiving merge engine. The copied SQLite file is compacted/checkpointed so deleted private text is not left in transmitted free pages or a stale journal view. The live library is not sanitized or erased.

The current sync contract also excludes the new derived knowledge-source/unit/link graph, which does not yet have its own merge policy. Selected documents, canonical processing data, guides, and cards continue through their existing sync domains. Explicit preparation on the receiving device rebuilds its semantic graph/index. Full backups, unlike device sync, preserve the complete knowledge graph.

## Vector backend

Embeddings come from the configured provider. No embedding model is bundled with ostudy. The current implementation stores vectors in ordinary SQLite rows and ranks them with native Rust cosine calculations. It is semantic retrieval, not keyword search, but it is not the planned sqlite-vec implementation. Backend replacement must preserve scope filtering, exact source revisions, model-space isolation, cancellation, and these lifecycle/privacy rules.
