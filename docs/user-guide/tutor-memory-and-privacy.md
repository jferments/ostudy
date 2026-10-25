# Tutor memory and privacy

Tutor is a content-focused chat tool. It does not maintain a curriculum, change FSRS schedules, or turn chat answers into card reviews. Saved conversations remain readable without a reachable model provider.

## Chat and sources

Tutor shows your chats on the left (behind the menu button on a phone), grouped by project, with **New chat** at the top and archived chats at the bottom. Each chat's **…** menu holds Rename, Chat settings and Delete. Tutor is a frontier model: it answers from its own knowledge, and you never have to pick a source. **Add sources** (it shows your choice once you pick one) opens an optional checklist of documents, a subject's documents together (**Subject documents**), flashcard decks and study guides; choose up to 16. No selected material means a general conversation using the configured chat model; no embedding profile or source preparation is required. Sources provide additional grounding, not permission to answer. Selecting sources sends nothing. Changing sources for an existing chat starts a new chat while the previous conversation keeps its original material selection.

Ask for practice questions in your message and press the send arrow, or choose **Quiz me** on a new chat with sources or a topic typed, or **Quiz me on this** under the latest answer. You can request several questions, ask for one at a time, or discuss your answers. This is conversation, not a separate graded practice-test system, and it does not affect your card schedule. Questions may come from the model's general knowledge without citations. A structured one-at-a-time question keeps its expected answer private until you respond.

While Tutor works on a reply, it says what it is doing (reading the conversation, finding sources, writing, checking). An explanation appears as it is written; its source chips show once the finished answer has been checked, and the checked answer replaces what you saw. Hints, quiz questions and grading appear only after their checks, so a hint or a question never shows its answer early.

New replies and saved conversations render Markdown headings, emphasis, lists, tables and code. You do not need to regenerate old replies. Raw HTML is not executed, and linked images are not loaded automatically. When a figure from one of your documents would clearly help, such as a diagram of the very process being explained, an answer can show it beside the text, drawn from the document's page with its title and page number; most answers show none. Only figures oStudy has read and judged worth studying on the pages the answer draws on are offered, a figure you hide on a document's Figures page stops showing, and Copy leaves figures out. Supported external links open only when clicked. Numbered chips under an answer open the cited source: beside the chat on a wide screen, or in a dialog. **Copy** copies an answer's text. Flashcard hints retain their plain-text, hidden-answer safeguards.

Document knowledge is saved during processing and reused by all learning tools. It does not need a separate Tutor understanding stage. Sending a material-scoped message searches this saved knowledge; missing search vectors are indexed automatically using the selected embedding model. Changing the Tutor model does not repeat interpretation, and changing embeddings only rebuilds the derived search index. Choose them under **Settings → AI**: **Interactive tutor** sets the chat model and its **Reasoning** (Tutor is the only task that reasons), and **Semantic embeddings** sets the search model. Sending may transmit relevant saved material to those configured services and incur charges.

If optional search is not configured or is temporarily unavailable, Tutor can still answer general questions from model knowledge without claiming access to missing sources. Selecting a source does not restrict every question to that source. When you explicitly ask what a source says or request an answer only from selected material, Tutor is instructed to identify gaps instead of silently replacing source content with model knowledge. A source whose text came out unreadable, such as a badly scanned page, is passed over: Tutor answers from its own knowledge and does not comment on the source. Source edits, revoked permissions, cancellation and corrupt storage are not treated as ordinary search unavailability.

Nonfatal processing details stay in diagnostics rather than interrupting chat. A failed message remains available. **Try again** retries the latest confirmed failed response; **Check response** reconciles an uncertain acknowledgement without submitting a duplicate. Reopen Practice for a new hint rather than replaying an old card's hint from chat history. Advanced controls are under **Chat settings** in the chat's **…** menu, not in the main chat.

## Projects and conversation organization

Tutor projects organize related chats without merging them. Create a project from the conversation pane, give it an optional description and Tutor instructions, and choose default sources. New chats started inside that project inherit the project's **current** defaults. Project instructions are learner-authored preferences for explanation style, emphasis, or study approach; they are not evidence and cannot override source, privacy, hidden-answer, quiz, or output-safety rules.

Each conversation still owns its own source selection. Changing project defaults does not rewrite an existing chat, and moving a chat into another project preserves its sources by default. **Use destination project's current default sources** is an explicit opt-in during Move. Changing Sources on a project chat starts a new chat in the same project with that explicit source selection rather than mutating the older transcript.

Conversation and project actions live in their overflow menus. You can Rename, Move, Archive/Restore, or Delete a chat, and Edit, Archive/Restore, or Delete a project. Deleting a project moves its chats to **Ungrouped**; it does not delete their messages or independently created documents, cards, guides, or review history. Deleting a conversation removes only that chat.

Project membership is organization, not retrieval permission. Chats in the same project are not searched merely because they share a project. Cross-chat semantic retrieval still requires the existing per-conversation retrieval permission plus the explicit **Search permitted conversations in this material selection** opt-in, and the historical turn must match the exact captured material scope.

Rename, Move, Archive, and project-organization changes use a separate organization revision from Tutor message revisioning. An already admitted response therefore keeps the project instructions and source scope captured when that turn started even if organization changes while the provider is working.

## Conversation memory

Open a saved conversation, choose **Chat settings** from its **…** menu, and expand **Conversation memory**. These settings are local; reading or changing them does not contact a provider.

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

The search index kept on each device is not included. Restoring a backup clears it, and the saved knowledge is indexed again with the chosen embedding model the next time you send a chat that needs it. Startup never contacts an AI provider by itself. Backups from earlier oStudy builds that hold Tutor data still restore and are brought up to date.

**Device sync** has no conversation-sharing category. Tutor projects, project instructions/defaults, organization-operation receipts, chats, summaries, private quiz data, and semantic copies of messages are removed from the transfer snapshot, not merely ignored by the receiving merge engine. The copied SQLite file is compacted/checkpointed so deleted private text is not left in transmitted free pages or a stale journal view. The live library is not sanitized or erased.

Device sync also leaves out the knowledge ostudy extracted from documents, which does not yet have its own merge rules. Selected documents, their processing data, guides, and cards still travel in their own categories. Because the extracted knowledge does not, making new material from a transferred document on the receiving device can mean extracting it again. A full backup, unlike device sync, keeps all of it.

An older oStudy build cannot open a library a newer one has upgraded. Keep a full-library backup before trying a test build.

## Vector backend

Embeddings come from the configured provider. No embedding model is bundled with ostudy. The current implementation stores vectors in ordinary SQLite rows and ranks them with native Rust cosine calculations. It is semantic retrieval, not keyword search, but it is not the planned sqlite-vec implementation. Backend replacement must preserve scope filtering, exact source revisions, model-space isolation, cancellation, and these lifecycle/privacy rules.
