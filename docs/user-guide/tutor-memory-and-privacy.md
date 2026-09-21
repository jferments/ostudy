# Tutor memory and privacy

Tutor is a content-focused chat tool. It does not maintain a curriculum, change FSRS schedules, or turn chat answers into card reviews. Saved conversations remain readable without a reachable model provider.

## Chat and sources

Use Tutor's chat window and conversation/project list. **Sources** opens an optional checklist of documents, document folders, flashcard decks and study guides; choose up to 16. No selected material means a general conversation using the configured chat model; no embedding profile or source preparation is required. Sources provide additional grounding, not permission to answer. Selecting sources sends nothing. Changing sources for an existing chat starts a new chat while the previous conversation keeps its original material selection.

Ask for practice questions in your message and press **Send**; no separate quiz button is needed. You can request several questions, ask for one at a time, or discuss your answers. This is conversation, not a separate graded practice-test system, and it does not affect your card schedule. Questions may come from the model's general knowledge without citations. A structured one-at-a-time question keeps its expected answer private until you respond.

New replies and saved conversations render Markdown headings, emphasis, lists, tables and code. You do not need to regenerate old replies. Raw HTML is not executed, and linked images are not loaded automatically. Supported external links open only when clicked. Flashcard hints retain their plain-text, hidden-answer safeguards.

Document knowledge is saved during processing and reused by all learning tools. It does not need a separate Tutor understanding stage. Sending a material-scoped message searches this saved knowledge; missing search vectors are indexed automatically using the selected embedding model. Changing the Tutor model does not repeat interpretation, and changing embeddings only rebuilds the derived search index. Configure these providers under **Settings → AI/LLM**. Sending may transmit relevant saved material to those configured services and incur charges.

If optional search is not configured or is temporarily unavailable, Tutor can still answer general questions from model knowledge without claiming access to missing sources. Selecting a source does not restrict every question to that source. When you explicitly ask what a source says or request an answer only from selected material, Tutor is instructed to identify gaps instead of silently replacing source content with model knowledge. Source edits, revoked permissions, cancellation and corrupt storage are not treated as ordinary search unavailability.

Nonfatal processing details stay in diagnostics rather than interrupting chat. A failed message remains available. **Try again** retries the latest confirmed failed response; **Check response** reconciles an uncertain acknowledgement without submitting a duplicate. Reopen Practice for a new hint rather than replaying an old card's hint from chat history. Advanced controls are under **Chat settings**, not in the main chat.

## Projects and conversation organization

Tutor projects organize related chats without merging them. Create a project from the conversation pane, give it an optional description and Tutor instructions, and choose default sources. New chats started inside that project inherit the project's **current** defaults. Project instructions are learner-authored preferences for explanation style, emphasis, or study approach; they are not evidence and cannot override source, privacy, hidden-answer, quiz, or output-safety rules.

Each conversation still owns its own source selection. Changing project defaults does not rewrite an existing chat, and moving a chat into another project preserves its sources by default. **Use destination project's current default sources** is an explicit opt-in during Move. Changing Sources on a project chat starts a new chat in the same project with that explicit source selection rather than mutating the older transcript.

Conversation and project actions live in their overflow menus. You can Rename, Move, Archive/Restore, or Delete a chat, and Edit, Archive/Restore, or Delete a project. Deleting a project moves its chats to **Ungrouped**; it does not delete their messages or independently created documents, cards, guides, or review history. Deleting a conversation removes only that chat.

Project membership is organization, not retrieval permission. Chats in the same project are not searched merely because they share a project. Cross-chat semantic retrieval still requires the existing per-conversation retrieval permission plus the explicit **Search permitted conversations in this material selection** opt-in, and the historical turn must match the exact captured material scope.

Rename, Move, Archive, and project-organization changes use a separate organization revision from Tutor message revisioning. An already admitted response therefore keeps the project instructions and source scope captured when that turn started even if organization changes while the provider is working.

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

A **full-library backup** retains Tutor projects, project defaults/instructions, conversation organization, conversations, private quiz data, memory preferences/cutoffs, source records, knowledge units, and artifact links. Treat the backup as sensitive personal data: it is not an application-encrypted chat export. Saved API keys are not included.

Device-local vector caches are not included. Restoration removes the pre-restore cache and marks completed preparation receipts as requiring a rebuild. Compatible stored knowledge is reused when the receiving device processes selected material or sends a grounded chat; any required indexing uses the chosen embedding model. Startup never silently contacts an AI provider. Supported historical Tutor backups from schema 10/11 remain admissible through migration-ledger validation and migrate forward through the current public schema; restoring them does not require pretending the current library is still schema 11.

**Device sync** has no conversation-sharing category. Tutor projects, project instructions/defaults, organization-operation receipts, chats, summaries, private quiz data, and semantic copies of messages are removed from the transfer snapshot, not merely ignored by the receiving merge engine. The copied SQLite file is compacted/checkpointed so deleted private text is not left in transmitted free pages or a stale journal view. The live library is not sanitized or erased.

The current sync contract also excludes the new derived knowledge-source/unit/link graph, which does not yet have its own merge policy. Selected documents, canonical processing data, guides, and cards continue through their existing sync domains. The receiving device can index its saved source text on chat use. Because sync currently excludes the authoritative interpretation graph, generating artifacts from transferred sources can require knowledge extraction; this is different from a full backup or reuse of a document processed on the same device. Full backups, unlike device sync, preserve the complete knowledge graph.

New multi-source conversations require a build that understands selection scopes. Tutor projects add schema 25, including project/default organization state, per-conversation organization revision, and a frozen source-scope field for historical turns. An older binary cannot safely interpret or downgrade that schema. Preserve a compatible full-library backup before testing unmerged versions.

## Vector backend

Embeddings come from the configured provider. No embedding model is bundled with ostudy. The current implementation stores vectors in ordinary SQLite rows and ranks them with native Rust cosine calculations. It is semantic retrieval, not keyword search, but it is not the planned sqlite-vec implementation. Backend replacement must preserve scope filtering, exact source revisions, model-space isolation, cancellation, and these lifecycle/privacy rules.
