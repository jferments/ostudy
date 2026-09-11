# Tutor memory and privacy

Tutor is a content-focused chat and quiz tool. It does not maintain a curriculum, change FSRS schedules, or turn chat answers into card reviews. Saved conversations remain readable without a reachable model provider.

## Chat and sources

Use Tutor's chat window and conversation list. **Sources** opens an optional checklist of documents, flashcard decks and study guides; choose up to 16. No selected material means a general conversation. Selecting sources sends nothing. Changing sources for an existing chat starts a new chat while the previous conversation keeps its original material selection.

Document knowledge is saved during processing and reused by all learning tools. It does not need a separate Tutor understanding stage. Sending a message searches this saved knowledge; missing search vectors are indexed automatically using the selected embedding model. Changing the Tutor model does not repeat interpretation, and changing embeddings only rebuilds the derived search index. Configure these providers under **Settings → AI/LLM**. Sending may transmit relevant saved material to those configured services and incur charges.

A failed message remains available. **Try again** retries the latest confirmed failed response; **Check response** reconciles an uncertain acknowledgement without submitting a duplicate. Reopen Practice for a new hint rather than replaying an old card's hint from chat history. Advanced controls are under **Chat settings**, not in the main chat.

## Conversation memory

Open a saved conversation, open **Chat settings**, and expand **Conversation memory**. These settings are local; reading or changing them does not contact a provider.

**Use earlier turns automatically** controls whether recent eligible messages and the stored rolling summary enter subsequent responses in this conversation. Disabling it clears the current summary and omits automatic history. Re-enabling it permits eligible history again; it is not the same as forgetting earlier context.

**Allow this conversation in semantic retrieval** is off by default, including for conversations created before this setting existed. Enabling it grants permission, but does not embed or upload the transcript. Enable **Search permitted conversations in this material selection** in Chat settings when you want eligible conversations included. Sending then indexes eligible saved turns as needed; changing the permission or checkbox alone sends nothing. Retrieval is restricted to conversations in the same selected material scope; general discussions use automatic history instead. Automatic history and explicit semantic retrieval are separate choices. Revoking retrieval removes the conversation's existing semantic source projections.

**Forget earlier context** advances a permanent cutoff to the latest saved turn. The earlier transcript stays readable, but earlier turns are no longer eligible for future summaries, conversation retrieval, or outstanding quiz answers. Clearing a summary alone would allow it to be recreated; the cutoff prevents that. This operation does not erase existing backups, a provider's retained requests, or independent material already authored from a conversation.

**Stored conversation summary** shows the current summary and its coverage boundary. It is generated conversational continuity, not a verified source or a learner profile. Summary inspection is not exposed inside the hidden-answer hint panel.

Privacy changes and conversation deletion wait until active tutor/preparation requests are finished or canceled. An uncertain change result can be checked with its original operation identity without repeating the action over newer choices.

## Manual and imported cards

Creating, editing, importing, rating, and organizing ordinary cards do not require embedding calls. Tutor index catch-up projects their saved native presentations and embeds the text without another LLM interpretation pass. They remain identified as authored assertions rather than verified textbook claims.

Content changes retire old interpretations and retrieval entries. This includes native question/answer text, rich content, note type, deletion, and relevant source-image changes. Undo restores the original card identity and scheduling history, but does not revive an old interpretation. Search indexing catches up on use; advanced Search index maintenance is available in Chat settings. Ratings and scheduling timestamps alone do not invalidate semantic content.

Inactive or deleted sources are excluded immediately from retrieval. A durable queue removes their device-local vectors on the next index opening, in bounded batches. That cache maintenance makes no provider requests. It is logical application deletion, not a promise of forensic erasure from storage snapshots or backups.

## Full backup versus device sync

A **full-library backup** retains conversations, private quiz data, memory preferences/cutoffs, source records, knowledge units, and artifact links. Treat the backup as sensitive personal data: it is not an application-encrypted chat export. Saved API keys are not included.

Device-local vector caches are not included. Restoration removes the pre-restore cache and marks completed preparation receipts as requiring a rebuild. Compatible stored knowledge is reused when the receiving device processes selected material or sends a grounded chat; any required indexing uses the chosen embedding model. Startup never silently contacts an AI provider. Schema-10 backups can be restored and migrated to schema 11 after migration-ledger validation.

**Device sync** has no conversation-sharing category. Chats, summaries, private quiz data, and semantic copies of messages are removed from the transfer snapshot, not merely ignored by the receiving merge engine. The copied SQLite file is compacted/checkpointed so deleted private text is not left in transmitted free pages or a stale journal view. The live library is not sanitized or erased.

The current sync contract also excludes the new derived knowledge-source/unit/link graph, which does not yet have its own merge policy. Selected documents, canonical processing data, guides, and cards continue through their existing sync domains. The receiving device can index its saved source text on chat use. Because sync currently excludes the authoritative interpretation graph, generating artifacts from transferred sources can require knowledge extraction; this is different from a full backup or reuse of a document processed on the same device. Full backups, unlike device sync, preserve the complete knowledge graph.

New multi-source conversations require a build that understands selection scopes. This extension adds no database migration, but switching to an older binary does not make its scope parser understand those conversations. Preserve a compatible full-library backup before testing unmerged versions.

## Vector backend

Embeddings come from the configured provider. No embedding model is bundled with ostudy. The current implementation stores vectors in ordinary SQLite rows and ranks them with native Rust cosine calculations. It is semantic retrieval, not keyword search, but it is not the planned sqlite-vec implementation. Backend replacement must preserve scope filtering, exact source revisions, model-space isolation, cancellation, and these lifecycle/privacy rules.
