# ostudy architecture: current system, decisions, and invariants

This is the comprehensive architectural description of **ostudy**. It explains what the application currently contains, who owns each responsibility, how important workflows cross those boundaries, and why those boundaries exist. It is intended for a contributor or agent who needs to make a change without reconstructing the design from months of conversations.

The documentation refresh was grounded in merged main `2e284a9f084b3bce41c2f7fe40950d1846a61a4b`, after PR #86, and the documentation/isolated-cleanup changes accompanying this guide. This is a source description, not a claim that every platform, external provider, or physical device was newly qualified. Current qualification belongs to the exact commit's PR/Actions evidence. Historical decisions remain in [ADRs](../decisions/); implementation-stage plans are not alternative current specifications.

## Contents

1. [Purpose and philosophy](#purpose-and-philosophy)
2. [System context and deployment](#system-context-and-deployment)
3. [Ownership and dependency direction](#ownership-and-dependency-direction)
4. [Composition and startup](#composition-and-startup)
5. [Frontend architecture](#frontend-architecture)
6. [Navigation and workflow restoration](#navigation-and-workflow-restoration)
7. [Interaction and visual architecture](#interaction-and-visual-architecture)
8. [Native command and error boundary](#native-command-and-error-boundary)
9. [Durable data model](#durable-data-model)
10. [Rich content and authoring](#rich-content-and-authoring)
11. [Transactions, revisions, and retries](#transactions-revisions-and-retries)
12. [Drafts, assets, and deletion](#drafts-assets-and-deletion)
13. [Scheduling and practice](#scheduling-and-practice)
14. [Search, projections, and analytics](#search-projections-and-analytics)
15. [Transfers, backup, and recovery](#transfers-backup-and-recovery)
16. [Trusted-device synchronization](#trusted-device-synchronization)
17. [AI infrastructure](#ai-infrastructure)
18. [Document ingestion and canonical evidence](#document-ingestion-and-canonical-evidence)
19. [Bounded document intelligence](#bounded-document-intelligence)
20. [Study guides and topic-linked learning](#study-guides-and-topic-linked-learning)
21. [Platform and Android boundaries](#platform-and-android-boundaries)
22. [Security and privacy model](#security-and-privacy-model)
23. [Build, dependencies, and delivery](#build-dependencies-and-delivery)
24. [Testing, diagnostics, and development workflow](#testing-diagnostics-and-development-workflow)
25. [Tradeoffs, compatibility, and extension rules](#tradeoffs-compatibility-and-extension-rules)
26. [Source map and glossary](#source-map-and-glossary)

## Purpose and philosophy

ostudy is a local-first learning application, not a hosted account with a thin local client. A learner can create and organize flashcards, practice, inspect analytics, search, import content, and make backups without configuring an AI provider. Optional AI assists the production of learning material; it does not own the learner's records or determine whether ordinary study is available.

**Ownership of data is a product requirement.** The authoritative library is on the device. Portable decks support sharing without donor history; complete backups support recovery; explicit trusted-device offers support a narrower synchronization graph. These are separate contracts rather than three names for copying the database. The application must remain useful when a provider is offline or an external service disappears.

**AI is a bounded authoring capability, not an authority.** A model may propose a source outline, a card, or a guide section. Rust validates that proposal and controls its durable publication. A model does not execute SQL, call arbitrary native commands, choose a different cloud provider, or calculate authoritative schedules. Deterministic code owns scheduling, duplicate policy, source identity, transactions, and analytics. This separation makes failures explainable and allows model providers to change without changing the data's meaning.

**Modularity serves actual independence.** Modules should have high internal cohesion and limited coupling: things that change for the same reason belong together, while unrelated layers communicate through narrow application-owned contracts. A backend query may provide an efficient read model for a screen, but the screen's current button layout must not define the durable schema or a domain operation's meaning. Hiding a menu or changing a responsive layout must not require rewriting review history, credential policy, or source parsing.

**Simplicity means fewer competing owners.** Reuse an existing controller, overlay, transaction boundary, or platform port before creating another. The goal is not the maximum number of interfaces or files. A small explicit facade is useful when it establishes ownership; a new framework that merely obscures a direct call is not. Some current modules are still large and some feature-local hooks combine orchestration. This document identifies those realities instead of pretending the code already realizes an idealized diagram perfectly.

**A calm interface can remain capable.** Common study actions should be shallow; less frequent technical detail should be contextual. Empty libraries should have a useful next action rather than unused toolbars. Reduced motion, accessible labels, keyboard controls, and touch targets are functional requirements, not optional polish. Decorations cannot become dependencies of successful study operations.

**Evidence is more important than claims.** A feature's enum variant, an ADR, a planned stage, a generated patch, and a passing test are different kinds of evidence. Describe implemented features separately from roadmap goals and platform qualification. Preserve exact source identity when publishing or testing changes.

## System context and deployment

The application combines a React/TypeScript WebView frontend with a Tauri 2 Rust native application. Vite builds the frontend. SQLite through `rusqlite` stores structured state, including FTS5 search data. Tiptap/ProseMirror provides the rich-editor implementation behind an application adapter. Native dependencies and platform services handle files, secure storage, document rendering/OCR, and optional network communication. Python and shell code primarily support tests, diagnostics, and build operations; there is no required Python application server.

```text
Learner
  │
  ▼
React interface inside the device's Tauri WebView
  │ typed domain commands and native events
  ▼
Rust application and task services
  ├── local SQLite library
  ├── managed documents/images/derivatives/staging
  ├── operating-system file, secret, lifecycle, and resource services
  ├── explicitly selected local/LAN/cloud AI provider
  └── explicitly accepted trusted-device sync session
```

Linux and Android are the maintained development/qualification paths. They share study semantics rather than implementing separate schedulers or databases. Desktop and mobile still have different operating-system integration and packaging requirements. Windows/macOS need regular target qualification; iOS remains a planned platform, not a completed installable application. A native library compiling on one target does not establish that every platform plugin, file picker, credential service, or packaged runtime works on another.

The stable release application identifier is `io.github.jferments.ostudy`. Android debug uses `io.github.jferments.ostudy.dev`, with separate application data and signing assumptions. A Git checkout or worktree is not a data profile: two desktop builds launched normally can address the same user library. Tests must deliberately isolate storage.

Source authorities: [package manifest](../../package.json), [native manifest](../../src-tauri/Cargo.toml), [Tauri configuration](../../src-tauri/tauri.conf.json), [platform status](../installation/README.md).

## Ownership and dependency direction

The principal call flow is screen → controller → typed IPC → native command → application service → repository/platform service. The frontend's import dependencies include shared UI; data returning from native code travels in the opposite direction. These two arrows should not be confused when reading diagrams.

| Boundary | Owns | Does not own |
| --- | --- | --- |
| Feature screen | Layout, interaction, contextual wording, rendering a read model | SQL, secrets, provider HTTP, authoritative schedules |
| Controller | Workflow sequencing, request identity, stale reads, navigation, reconciliation | Database schema, OS implementation, visual animation timing |
| `src/ipc/` | Typed native command names and payloads/results | Independent business policy or arbitrary storage access |
| Shared `src/ui/` | Interaction primitives, tokens, overlays, focus, presentation effects | Feature IPC, persistence, provider configuration |
| Native command adapter | Deserialize/validate boundary input, delegate, return safe domain results | A second unrelated implementation of the use case |
| Native application service | Use-case policy, admission, revision/operation identity, transaction scope | React component hierarchy or screen widths |
| Repository/projection | Explicit SQL, coherent reads/writes, bounded read models | Provider requests or OS dialog workflows |
| Platform port/adapter | OS operations and capability observations | Scheduling, card semantics, learner-data merge policy |
| Provider adapter/gateway | Wire dialect and safe transport respectively | Direct model authority over library writes |

Direct Tauri `invoke()` calls are confined to [domain IPC modules](../../src/ipc/). Shared UI must not import those modules, feature controllers, or Tauri APIs. Frontend platform adapters isolate OS-facing operations. Native composition selects platform implementations; domain services depend on capabilities, not a proliferation of platform-specific feature forks.

A controller's location is not itself an invariant. General controllers live in `src/controllers/`, while deck-library organization/reorder controllers are colocated with their workspace. Likewise, current native document code has a focused `documents/` facade plus active root `document_*` owners. Do not infer that a facade has already absorbed every implementation file, or delete root modules because their names look old.

## Composition and startup

[Native `lib.rs`](../../src-tauri/src/lib.rs) composes the Tauri application, plugins, managed asset protocol, native command registry, and lifecycle hooks. [Startup](../../src-tauri/src/startup.rs) owns the recovery sequence. [Frontend `main.tsx`](../../src/main.tsx) composes the root providers and the single global CSS foundation; [App](../../src/App.tsx) renders the current destination/workflow.

Startup must not expose an ordinary library before its durable state is ready. It resolves app-private paths and diagnostics, handles a pending whole-library replacement through a recovery guard, opens and migrates SQLite, composes native services, and reconciles persisted records whose process-local work cannot have survived restart. These include stale synchronization sessions, interrupted external-file transfers, drafts/staged assets, pending deletions/purges, document task state, and detached source relationships.

A pending restore is not equivalent to replacing one file while all services continue using the old graph. Its database and managed files are validated and staged, startup applies the replacement, and the guard is committed only after initialization/reconciliation succeeds. Source/code rollback and data recovery consequently require different reasoning.

Schema compatibility is explicit. [Database initialization](../../src-tauri/src/database.rs) and the pre-public transition path recognize supported histories rather than guessing from a similarly named file. Migrations have registered identities/checksums. [Versioned data repairs](../../src-tauri/src/data_repairs.rs) are separate from schema changes and run under their own evidence. Unsupported/corrupt durable state must fail visibly instead of being truncated or silently replaced with an empty library.

The AI runtime can initialize without contacting a model. An unavailable secure store should not prevent non-AI study; the relevant secret operation reports its availability or uses an explicitly requested session-only credential. Startup and foreground restoration never implicitly grant permission to resume transmitting source material to a provider.

When initialization fails, the application presents the startup failure and terminates rather than making a partially initialized library look usable. Lifecycle notifications are opportunities to flush/checkpoint, not guarantees that every shutdown is graceful.

## Frontend architecture

React components render bounded native read models and local interaction state. The current app owns navigation explicitly through its controller and route/request types; contributors should inspect that implementation rather than assume the presence of a routing/state-management library from an earlier proposal.

Controllers separate independent request lanes. A user can change a search or model profile while an earlier native read is still running. That earlier success or failure may arrive, but its obsolete identity must not overwrite the newer state. The native work does not have to be physically canceled for the display to reject a stale result. Independent concerns require independent lanes so one refresh does not accidentally invalidate another unrelated read.

Durable writes are different. Once native code has committed, the UI reports success even if a later list reconciliation fails. It should offer a refresh or contextual informational message, not encourage a new mutation as though nothing happened. The controller preserves the operation identity for ambiguous retries, guards repeated submission, and treats UI dismissal separately from mutation cancellation.

The deck library illustrates useful frontend decomposition: a route composes the library controller, organization/selection models, pointer-session logic, reorder logic, presentational rows/folders, and shared sheets. Pointer motion updates transient feedback, not the database. A drop becomes a semantic move request against an organization revision. The same move policy supports keyboard and menu alternatives.

There are still integration-heavy screens and hooks, including document orchestration and the application controller. Their existence is not authorization to move SQL, saved keys, or scheduling into React. Future extraction should clarify one responsibility at a time with behavior tests, not impose an arbitrary maximum file size or add a second parallel coordinator.

Sources: [request lanes](../../src/controllers/requestLane.ts), [reconciliation](../../src/controllers/reconciliation.ts), [deck-library owner](../../src/screens/flashcards/deck-library/useDeckLibraryController.ts), [application controller](../../src/controllers/useAppController.ts).

## Navigation and workflow restoration

Current standing destinations are **Flashcards, Study Guides, Documents, Analytics, and Settings**. Wide/medium layouts have labeled navigation. Compact layout exposes the first four directly in bottom navigation and Settings in an upper-right gear; there is no mobile More-menu architecture to preserve.

Flashcards contains **Decks** and **Search/Filter**. Templates are a normal Settings tab. Deck import/export is contextual; complete-library backup is in Backup/Sync. Removing those old standing tabs did not remove their domain operations. Selecting Flashcards or Decks returns to the library root; selecting Study Guides returns to its library. Dirty authoring guards still apply before a navigation reset.

Practice is an active internal workflow, not a standing destination. A launch records its semantic scope and logical return target. A direct deck launch becomes extra/deck practice; selected deck/tag scheduled work and exact-card custom work use distinct request contracts. The application keeps a visible destination separate from the active workflow so consuming an asynchronous launch payload does not unmount the session.

The schema-v2 [UI checkpoint](../../src-tauri/src/ui_checkpoint.rs) records a device-local destination plus an optional real resumable-session hint and return target. SQLite remains authoritative for session progress, schedules, and reviews. It does not serialize the whole React tree, the current selection set, secrets, or provider responses. Fresh transition/request IDs are constructed on restoration rather than replaying stale in-memory identities.

The schema-v1 reader remains intentional compatibility. A real old session can normalize without losing its identity/cursor; a setup-only old Practice checkpoint falls back to Flashcards rather than inventing an active session. A deleted deck can invalidate a return target without invalidating otherwise resumable study. The checkpoint is outside library backup/sync. Legacy Templates/Flashcards values similarly normalize to current destinations rather than forcing obsolete tabs back into the interface.

Sources: [Flashcards route model](../../src/navigation/flashcardsRoutes.ts), [Study Guides navigation](../../src/navigation/studyGuidesNavigation.ts), [ADR 0050](../decisions/0050-versioned-ui-checkpoint-workflow-restoration.md).

## Interaction and visual architecture

The shared overlay provider owns stacking, focus containment/restoration, topmost Escape/Back dismissal, inert background behavior, and scroll locking. Features use Dialog, Sheet, Menu, overflow, and confirmation primitives rather than creating competing focus traps or backdrops. Busy/non-dismissible behavior is explicit. Native shutdown protection remains native because a React dialog cannot reliably prevent all process termination.

A Study Guide reader is currently an **inline workspace**, even though its implementation file retains the name `StudyGuideDialog.tsx`. It is not a modal covering the application navigation. A filename must not override the actual accessibility role and composition. Desktop window management uses native OS chrome; the removed in-app maximize/full-screen adapter is not part of current frontend ownership.

Styling has one global entry, `src/ui/foundations/index.css`, which establishes layers, root sizing, local typography, safe areas, and semantic foundations. Feature and component styling uses CSS Modules. Generated reference/semantic/component tokens connect themes to controls without separate feature implementations for Light, Dark, Console, or Synthwave. Named layers make overlay/effect ordering reviewable.

Layout responds to viewport and available container width, pointer precision, safe areas, text scale, reduced motion, and transparency/forced-colors needs. It does not infer product behavior from an OS name or user agent. Medium width is a real labeled-sidebar layout rather than an accidental mixture of phone and desktop breakpoints. Visual compactness does not reduce interactive target requirements.

Interface and card typography/scale are separate. The local IBM Plex Sans default is packaged; installed execution does not need a web-font request. A build may provision uncached font assets, which is a different network boundary. Font fallbacks and print/PDF typography are not identical contracts.

Help topics and optional tutorials use stable semantic targets. Tutorials default off and start only by explicit user action; observers maintain an already-active spotlight rather than auto-launch guidance. Decorative artwork and celebrations can be disabled without hiding educational images, controls, or results. Semantic feedback events are consumed by the experience layer; a review must not wait for a particle animation or haptic effect.

See [UI architecture](ui.md), [appearance guide](../user-guide/appearance-and-guidance.md), and [ADR 0048](../decisions/0048-frontend-state-and-style-ownership.md).

## Native command and error boundary

Tauri commands are narrow application operations: obtain a bounded list, start a particular practice scope, submit one review, commit a draft, prepare an export, acquire an external file, configure a profile, or request a document task. They are not a generic SQL endpoint, filesystem explorer, prompt proxy, or saved-secret read API.

TypeScript DTOs improve call-site correctness, but Rust validation is still required. Imported files, manipulated IPC payloads, provider responses, rich content, identifiers, and file-handle metadata cross trust boundaries regardless of frontend types. Native code checks shape, bounds, revisions, ownership, lifecycle, and safe content before changing data.

`CardNote` and `CardPresentation` are distinct transports. One carries editable source; the other carries a rendered study/search identity. Combining them into a generic card object would couple authoring fields to scheduling/rendering and make operations on reverse/cloze variants ambiguous.

Errors have stable codes and safe user-facing messages. Detailed SQLite or platform failures can be recorded in bounded local diagnostics, but raw credential/provider/source payloads do not become ordinary UI errors. The UI should distinguish unavailable capability, invalid input, stale revision, already committed replay, failed reconciliation, and interrupted work rather than collapsing them into one “try again” action.

The IPC verifier compares frontend command names to the native command registry and rejects direct frontend bypasses. This is a useful structural contract, not proof that every request's semantics are correct. Native and component behavior tests remain necessary.

## Durable data model

The simplest useful conceptual graph is:

```text
folder → decks
           └── notes: editable structured content, tags, source associations
                 └── cards: stable forward/reverse/cloze presentations
                       └── current schedules

practice session → ordered session-card snapshots → review events

source document → canonical units/blocks/chunks/assets → learning plan/topics
                                                        ├── generated note links
                                                        └── guide revisions/sections
learner annotations ── document/topic identity or exact generated base
```

A folder organizes decks and does not own an independent schedule. A note owns source content; one note can produce multiple independently scheduled presentations. A template defines a safe authoring structure but does not remain the mutable owner of every note it once generated. Review/session history is not the same as the current card schedule.

SQLite holds structured entities, relationship tables, optimistic revisions, operation identities, search indexes, historical evidence, document metadata/checkpoints, and settings. [Managed paths](../../src-tauri/src/paths.rs) separate card images, staging/quarantine, retained document sources, renders and versioned assets, and external transfer staging/exports. The database records controlled identities and integrity metadata rather than accepting arbitrary imported absolute paths.

The codebase uses ordinary relational constraints and explicit SQL, not a remote synchronization database or an LLM memory store. It can produce bounded projections tailored to a screen while preserving canonical entities as the write authority. UI summary counts are not independent editable facts.

Database schema version, `.studydeck` format version, backup version, sync protocol/package version, canonical document schema, provider prompt schema, and UI-checkpoint version are distinct namespaces. Updating one does not imply all the others changed. Compatibility must identify which contract is being preserved.

For exact columns and current migration inventory use [migrations](../../src-tauri/migrations/), [database code](../../src-tauri/src/database.rs), and the [card/data-model reference](card-format-and-data-model.md). Do not duplicate every SQL column in prose as a competing schema.

## Rich content and authoring

The canonical rich document is validated, versioned Tiptap/ProseMirror JSON. Native code derives normalized searchable text and validates allowed nodes, marks, links, structure, size, and managed image references. Arbitrary HTML is not the storage contract. The editor adapter isolates Tiptap details so feature screens do not independently invent serialization or validation.

A Basic note usually has a forward card; an enabled reverse presentation has its own stable identity and schedule. Editing the shared source or tags affects both, while review histories remain independent. A cloze note has one stable scheduled card. One-at-a-time and all-together hiding are presentation modes; a session fixes the selected group, preventing a resumed question from silently becoming a different blank.

Templates contain named fields and safe layout rules. Rendering is structural substitution, not evaluation of JavaScript, HTML, SQL, or shell expressions. Inline values and block-rich content have different layout needs; the native renderer validates the resulting document and preview/generation share that renderer. Generated template output becomes ordinary independent notes/cards with fresh schedules. Later template changes do not retroactively rewrite existing study material.

Manual authoring, bulk input, strict generated JSON, and document-generated cards converge on native authoring validation instead of each owning a private card format. Duplicate handling first uses a candidate fingerprint and then verifies normalized equality. Manual entry can require deliberate override; bulk/template/generated input can skip exact duplicates. Empty searchable text alone must not classify unrelated image-only cards as duplicate.

Draft commit is a versioned use case. The frontend flushes an exact draft revision; native code validates that revision, writes canonical content and asset relationships, and retires the draft within the operation's durable transaction. A failed commit leaves recoverable work. A successful commit whose response was lost must not become a second targetless batch.

Sources: [authoring](../../src-tauri/src/authoring/), [rich-content validation](../../src-tauri/src/cards/content.rs), [editor adapter](../../src/editor/adapter.ts), [drafts](../../src-tauri/src/editor_drafts.rs), [rich-editor design](rich-document-editor.md).

## Transactions, revisions, and retries

Three questions require different mechanisms: “Did the data change since I read it?”, “Have I already applied this logical operation?”, and “Did the entire related update commit?” Optimistic revisions, operation identities, and transactions answer them respectively. None substitutes for the others.

A revision-bound edit or organization move refuses to overwrite a newer state. This protects multiple windows and asynchronous workflows even on a single local device. The frontend may preview a move optimistically, but the server-side revision check decides whether it is valid. A refresh after conflict is preferable to silently applying a drop to a differently organized library.

A stable operation/submission ID makes supported replay deterministic. It must identify the same logical request, not simply be regenerated whenever a network/IPC response is ambiguous. New user intent is a new operation; re-observing the result of a committed intent is not. Review submissions also validate the schedule revision seen when interval choices were previewed.

A service owns the full transaction and passes a connection/transaction to helpers rather than allowing hidden commits. Multi-card schedule actions, review submission, validated generated imports, and related authoring updates either satisfy their whole logical unit or leave it unchanged. Durable success is recognized before view refresh or an optional effect.

The database cannot atomically roll back arbitrary filesystem writes, OS secret-service mutations, or external provider calls. Those boundaries need explicit staging, ordering, recovery, and idempotence. For example, export bytes are prepared privately before an OS destination is chosen; document deletion stages a directory before the database mutation; credential updates coordinate a secret store and metadata without falsely claiming a cross-store SQL transaction.

Do not hold a broad database lock or connection across model HTTP, OCR, file-picker interaction, or other unbounded external work. Capture a coherent request snapshot, perform external work, then validate freshness/admission before publication. This keeps normal study responsive and makes cancel/retry semantics tractable.

## Drafts, assets, and deletion

Managed assets are a lifecycle, not just a path in a rich-text node. Staged images can belong to an unfinished draft. Committed images are associated with live content; quarantined/deletion-owned bytes can support Undo; unreachable residue is reconciled after interruption. The asset ledger and actual bytes must agree on identity, relative path, size, MIME and integrity evidence.

Rich content retains `study-asset://<asset-id>` identities. The WebView resolves those through the managed asset protocol and receives only authorized bytes. An internal custom URL is not permission to traverse arbitrary filesystem paths. External file imports similarly use opaque staged handles rather than passing desktop paths or Android content URIs around feature code.

Deleting a folder only changes organization and returns its decks to an unsorted scope. Archiving a deck retains it and changes study availability. Deleting a deck/note/card follows an explicit recovery window and then finalized purge semantics. Undo is a bounded durable recovery operation, not a permanent trash archive.

`review_events` is append-only historical evidence while the owning material is retained. This does not mean personally deleted content must survive forever: finalized deletion deliberately purges its owned history/analytics according to policy. Distinguish that ownership lifecycle from editing or rewriting a retained historical event.

Startup reconciles interrupted file staging, expired delete windows, purge work, and managed references. A successful database operation must not be reported failed merely because best-effort residue cleanup later failed; that cleanup needs diagnostic/recovery evidence. Conversely, dangling required bytes cannot be called a coherent successful restore.

Sources: [asset lifecycle](../../src-tauri/src/asset_lifecycle.rs), [asset protocol](../../src-tauri/src/asset_protocol.rs), [deletion](../../src-tauri/src/deletion.rs), [purge](../../src-tauri/src/deletion_purge.rs), [ADR 0008](../decisions/0008-managed-asset-lifecycle-and-delete-undo.md).

## Scheduling and practice

[Scheduler](../../src-tauri/src/scheduler.rs) is the adapter around `rs-fsrs`. The external package computes scheduling behavior through that owner; it does not define the app's database, IPC, or UI. `card_schedules` is mutable current state, and `review_events` records committed historical outcomes and attribution. Scheduling-control actions are not synthetic successful reviews.

The four visible outcomes, Wrong/Hard/Medium/Easy, map through the native adapter to Again/Hard/Good/Easy ratings. The frontend displays native interval previews. On submission, native code validates session/card identity, previewed schedule revision, and submission identity; appends the event; updates the schedule; and updates session progress transactionally.

A session captures an ordered, stable card identity set and per-presentation context. The frontend receives a bounded first window and requests subsequent windows rather than hydrating an arbitrarily large entire session into every screen. Viewed/revealed state and the selected cloze group remain consistent through resumption. Editing a note during practice uses a dedicated refresh boundary instead of silently discarding the session graph.

Scheduled mode applies scope and daily new/review budgets to studyable due cards. Extra mode is an explicit scope choice beyond the scheduled queue. Exact selected-card practice is custom study over current resolved identities. “Due timestamp has passed” is not sufficient: archival, suspension, burial, deletion, scope, and applicable limits affect studyability. The workload projection explains selected, available, and omitted work rather than treating every omission as missing data.

Local study-calendar semantics are native. Day boundaries, date intervals, timezone observations, DST transitions, and leap days must be handled consistently across practice and analytics rather than with independent fixed-millisecond browser calculations. Capture operation-consistent time facts instead of letting half a transaction use yesterday and the other half tomorrow.

Suspend/resume, bury, reset, and reschedule mutate current availability/schedule under their explicit policies. Reset preserves retained review history; manually rescheduling is not evidence of recall. Review progress and remembered proportions describe recorded outcomes, not a medical/educational guarantee of mastery.

Sources: [practice](../../src-tauri/src/practice.rs), [selection](../../src-tauri/src/practice_selection.rs), [resume](../../src-tauri/src/practice_resume.rs), [studyability](../../src-tauri/src/studyability.rs), [calendar](../../src-tauri/src/study_calendar.rs), [schedule controls](../../src-tauri/src/schedule_controls.rs), [ADR 0003](../decisions/0003-fsrs-review-ledger.md).

## Search, projections, and analytics

FTS5-backed search is a native query capability, not raw SQL supplied by a search textbox. Rust owns allowed filter/query construction, escaping and bound values, sorting, and bounded result projection. A card result is a study presentation, so reverse cards can appear independently and a cloze note's many groups do not become many separately scheduled results.

“Select visible” and “Select all matching” have different identity scopes. Exact-card practice resolves a complete selected set deliberately. Bulk note-level operations deduplicate shared source ownership rather than applying the same edit once for each forward/reverse result. Folder scope resolves current membership; it does not create another durable deck.

Read models prevent list screens from loading full rich documents, entire session histories, or every guide body just to display rows/counts. The app uses focused deck, practice, learning-signal, analytics, and study-guide projections. A query optimized for one screen is legitimate; embedding button availability or pixel layout into durable entities is not.

Analytics derives reproducible summaries from stored sessions/review evidence and current inventory. These are different time perspectives. Renaming an entity may change a current label while historical snapshots retain what was recorded. Moving material preserves its identity and retained evidence under current ownership rules; archived material remains relevant to history; finalized deleted material's owned history is purged.

A current tag in recent reviews can launch Search by stable ID. Historical-only tag text is not a live ID. An unavailable filter should remain visibly unresolved until cleared, not silently widen into the entire library. This is an example of preserving user intent across a presentation/domain boundary.

Charts and counters are renderings, not new sources of truth or LLM judgments. Missing/empty scopes must remain distinguishable from failed queries and stale results. Numerical semantics should be tested in native projections independently of chart-library output.

Sources: [deck-card queries](../../src-tauri/src/deck_cards.rs), [projections](../../src-tauri/src/projections/), [analytics owners](../../src-tauri/src/analytics/), [card browser](../../src/screens/cards/CardBrowserWorkspace.tsx), [ADR 0010](../decisions/0010-study-calendar-historical-attribution-and-projections.md).

## Transfers, backup, and recovery

`.studydeck` is a portable single-deck SQLite interchange graph. It includes content and managed study assets, not the donor's personal schedule/history or unrelated settings. Import validates the graph and creates fresh local scheduling state. It does not transfer the sender's folder as a new authority over receiving organization.

`.ostudy.json` is a strict content-only authoring format. It contains a deck and supported Basic/Reverse/cloze input using restricted Markdown. It cannot provide arbitrary internal IDs, SQL, schedules, review history, settings, or arbitrary executable HTML. The whole file validates before one transaction; exact duplicates are skipped. There is no intermediate generated-card approval queue. Native schema/conformance tests, not a tolerant frontend preview, enforce this contract.

`.ostudybackup` serves personal recovery. It captures a coherent SQLite snapshot with the managed files required by it, including source documents, canonical derivatives, learning plans/topics, guide revisions, annotations, and links. Version 3 additionally covers active/conflicted persistent drafts and their staged images; supported earlier package versions remain readable. Nonsecret provider metadata may be included; credential bytes are not.

A backup has to respect in-flight operations. Active document processing or transient file/asset work can make a point-in-time database plus files incoherent, so admission blocks unsafe capture. Successfully persisted drafts are recoverable library state, not necessarily a reason to block backup. A UI checkpoint/tutorial state outside the backup graph does not become portable simply because it belongs to the same app.

External selection/publication is split from domain transfer. The platform acquires bounded bytes into app-private staging and returns an opaque handle. Native code validates them before importing. For export, native code prepares/validates bytes first, then the platform asks for a save/share destination. Canceling a picker is not a failed content transaction. Multi-deck export can have an explicit partial result after some separate files were saved.

Restore validates and stages a replacement for restart; it is not an append import. A fresh code worktree does not isolate app data, and reinstalling with Android `-r` does not erase it. Backups contain sensitive personal material and are not automatically encrypted archives. Never use live user data for destructive testing.

Sources: [studydeck](../../src-tauri/src/studydeck/), [generated import](../../src-tauri/src/authoring/card_types_and_templates/generated_import/), [library backup](../../src-tauri/src/library_backup.rs), [document backup](../../src-tauri/src/document_backup.rs), [external streaming](../../src-tauri/src/external_file_streaming.rs), [user transfer guide](../user-guide/backups-imports-and-sharing.md).

## Trusted-device synchronization

Synchronization is explicit **one-way offer/receive**, not a cloud account, permanent background relationship, live shared SQLite file, or general CRDT. The sender creates a short-lived invitation; the receiver scans/accepts it; authenticated encrypted transport carries the selected snapshot; the receiver merges it and returns acknowledgement. A reverse transfer is a new deliberate offer from the other device.

The protocol combines stable Ed25519 identity, fresh X25519 agreement material, HKDF-derived keys, ChaCha20-Poly1305 framed TCP traffic, and a one-time pairing secret. Session transcript binding includes the endpoint/identity/expiry/scope. A QR is a short-lived acceptance capability, not a reusable password. Android Google Code Scanner acquires the QR through Play services; the app still validates the decoded protocol and crypto data. Pasted invitation text is the fallback where scanning is unavailable.

Only one unclaimed outgoing offer is kept at a time; overlapping sessions for the same peer are rejected. Waiting/connecting/transferring can be canceled by signaling and shutting down the socket. The final short SQLite merge is intentionally non-cancellable so the receiver never exposes half a committed graph. Restart invalidates process-local sockets and reconciles stale durable session rows.

The allowed study graph includes decks/folders, notes/cards, tags/templates, managed images, schedules, sessions/reviews, and historical attribution. **A schema table is not synchronized merely because it exists.** The newer document/study-guide artifact graph is not currently a promise of complete cross-device transfer. Whole-library backup and sync therefore remain different scopes.

`app_settings` is excluded and stripped from the outgoing snapshot. Ordinary appearance preferences stay local. AI profiles/defaults and credential transfer are off by default for each offer. Explicit sender opt-in can include eligible metadata and available saved keys through a separate secure-store path; credentials are not inserted into a file backup or a general SQLite payload. Probe/operation caches remain ephemeral. Receiving credentials requires an available secure store.

Three-way row merge uses a stored common baseline. One-sided edits can apply, independent fields can merge, concurrent same-field or delete-versus-edit cases become explicit conflicts, and mismatched managed-image identities fail instead of choosing arbitrary bytes. The receiver records its merged baseline; the sender records the exact acknowledged sent snapshot without secretly receiving the other library. Removing trust removes the relationship, not content already imported into the learner's library.

Sources: [sync module](../../src-tauri/src/device_sync/), [merge allowlist](../../src-tauri/src/device_sync/merge.rs), [sync architecture](device-sync.md).

## AI infrastructure

AI has three independently owned infrastructure concerns: **gateway transport**, **durable profiles**, and **probe/capability evidence**. Provider dialects and document-intelligence tasks consume them rather than each creating clients, credentials, and settings tables.

The [gateway](../../src-tauri/src/ai/gateway/) owns HTTP client construction, common request/response bounds, redirect rejection, streaming reconstruction, inactivity timeouts, allowlisted bounded retries, usage normalization, and error redaction. `reqwest` belongs there. It does not query SQLite or know guide sections, cards, or the React settings screen.

[Profiles](../../src-tauri/src/ai/profiles/) own contracts/commands, normalization/policy, optimistic metadata persistence, operation identity, secret-store coordination, projections, and task defaults. Provider family is immutable; built-in cloud endpoints are fixed. Loopback HTTP is distinct from acknowledged private-LAN HTTP, and public cleartext is forbidden. Changes affecting trust, credentials, or model identity invalidate derived verification evidence.

Credential bytes are held behind native OS/session storage after entry. A newly typed key necessarily exists briefly in the UI form and crosses the write boundary; saved keys are never read back into that field or ordinary profile DTOs. SQLite contains references/epochs and nonsecret configuration, not keys. Secret-store mutation and metadata commits have explicit ordering/recovery; they are not magically one atomic datastore.

[Probes](../../src-tauri/src/ai/probes/) own ordered endpoint/auth/model/text/structured-output tests and persistence of typed evidence. A result is committed only when the profile revision, credential epoch, and model still match the tested snapshot. Capabilities distinguish supported, unsupported, and unknown, and declared versus probed evidence. HTTP success alone is insufficient. Provider work is performed without holding the database transaction open.

Implemented [provider adapters](../../src-tauri/src/ai/providers/) are llama.cpp/Unsloth-compatible servers, DeepSeek, and OpenRouter. Other native catalog entries are planned, not usable adapters. Dialect modules isolate endpoint/request/schema/response differences; compatible wire conventions do not imply identical capability semantics. The local server owns model loading and chat templates. The app does not install a model runtime.

Reasoning effort is profile-owned, resolved against the selected model's advertised capability, with requested/effective values in job provenance. A default-model change switches profile identity, not another profile's reasoning/key/privacy state. Migration 0008 moves the supported legacy global preference into profiles and removes the old live global key. Current document learning uses the configured flashcard-generation task default rather than an imaginary independent study-guide selector.

OpenRouter routing/privacy flags are deliberate requests and may reduce endpoint availability; they are not a proof of external provider behavior. No failure, slowness, or local-server outage authorizes silent provider fallback. See [AI infrastructure](ai-infrastructure.md), [model profiles](ai-model-profiles.md), and [ADR 0046](../decisions/0046-modular-ai-infrastructure.md).

## Document ingestion and canonical evidence

A managed document is a durable source, not a temporary file attached only to one generation request. The system retains an immutable source copy and application-owned canonical units, blocks, chunks, images, locators, and provenance. Rebuildable derivatives are separate from learner-owned annotations and generated study artifacts.

The [ingestion registry](../../src-tauri/src/document_ingestion/registry.rs) selects explicit adapters. PDF uses the native PDF path. Current non-PDF DOC/DOCX/PPT/PPTX/ODT/ODP/ODS/EPUB/RTF/Excel/CSV families use **Xberg** behind its private adapter boundary. AnyDoc is retired runtime history, not a fallback. Old migration names, fixtures, and persisted backend identities remain compatibility/provenance evidence.

Content detection is authoritative where the format has a recognizable signature/container. OLE families need an authorized DOC/PPT/XLS filename hint; CSV needs an extension hint because it has no unique binary signature. Unsupported underlying-library formats are not automatically product capabilities. External relationships are not fetched and non-PDF import does not execute document macros or provide a web-acquisition service.

Third-party parser types are converted immediately into ostudy-owned contracts. Current non-PDF persisted units use Section semantics, even when Xberg's internal document model has richer slide/sheet information. Promoting that information into a new public locator requires a coordinated schema/consumer change. Embedded images are real extracted evidence; the adapter must not invent rendered slides it did not produce.

PDF extraction and rendering are intentionally separate. A PDF can render correctly while a broken font-to-Unicode map corrupts extracted text. Native text/semantic extraction uses the PDF backend; quality gates distinguish readable text with bad structure from genuinely unusable pages. A page-text fallback can repair semantics without OCR. On desktop, independent PDFium native-text rescue precedes raster OCR. Android PdfRenderer lacks that same rescue text layer.

Desktop rendering uses an explicitly provisioned, hash-verified PDFium runtime; Android uses PdfRenderer through the Kotlin plugin. Desktop Tesseract and bundled Android ML Kit recognition implement a narrow OCR contract. OCR candidates, worker/bitmap budgets, ordering, and cancellation policy remain shared native decisions. Readable native text must not be replaced merely because it is short or contains a minority of unmapped scientific glyphs.

Resource planning bounds extraction/OCR concurrency and high-resolution queues by workload, CPU, memory, and optional configured caps. PDFium access is serialized; parallelism belongs to safe independent work rather than blindly sharing a renderer handle. Cancellation discards uncommitted native results. Unknown resource observations produce conservative plans, not unlimited allocation.

Sources: [PDF backend](../../src-tauri/src/pdf_backend/), [PDF ingestion](../../src-tauri/src/pdf_document_ingestion/), [Xberg boundary](../../src-tauri/src/xberg_document_adapter/), [canonical representation](../../src-tauri/src/document_canonical.rs), [ADR 0045](../decisions/0045-xberg-non-pdf-document-adapter.md).

## Bounded document intelligence

Parsing answers what the source contains; document intelligence decides how to organize evidence for learning. Those responsibilities must not be folded into a provider adapter or a UI click handler. The task layer exchanges canonical application data with parsing and authoring, and provider-neutral task data with transport.

Large work is **request-bounded**, not rejected solely because the aggregate document cannot fit one context window. Each request budget accounts for source text, instructions/schema, expected output, optional images, and safety reserves. Oversized chunks split into bounded continuations with source provenance. Heading structure remains metadata and does not force inefficient one-heading-per-request execution.

A first pass obtains bounded outline fragments. Rust assembles hierarchy deterministically, sanitizes ranges/counts, deduplicates concepts/objectives, orders source coverage, and uses safe fallback structure when needed. A bounded whole-document digest can support a global summary without recursively returning unbounded complete outlines to a model. Failure of optional synthesis need not invalidate an already valid native aggregate.

The learning plan gives stable topic identities. Flashcard and study-guide generation consume bounded relevant source/outline/topic context. Supported visual evidence is selected by verified model capability. Local/LAN model jobs remain serial under current policy; cloud requests can overlap within bounded concurrency. Completion order does not define final publication order: native assembly/checkpoints and SQL writes remain deterministic.

Provider-enforced structured output is necessary but insufficient. Adapters use their appropriate schema/strict-tool dialects, then Rust checks the stricter domain grammar, limits, source ranges, cloze exact-substring rules, choices, and curation indices. A malformed outer response fails the request; a structurally valid but unusable candidate can be rejected without granting it write authority. Curation selects acceptable candidates and cannot arbitrarily rewrite the library.

Generated cards normalize into the existing authoring model. Definition, scenario, application, or multiple-choice educational forms can use Basic cards; they do not each require a new durable scheduler type. There is no hidden generated-card approval queue. “Generate more” reuses valid retained derivatives/understanding and current coverage rather than rerendering an unchanged source unnecessarily.

Checkpoints carry source/backend/prompt/schema/assembly identity. They are reusable only while compatible. Retry never treats an old derivative as authoritative solely because the file exists. New asset versions use immutable managed paths so a failed rebuild cannot destroy the last committed evidence.

Task cancellation is exact-document scoped across extraction, rendering, OCR, and provider transport. A responsive SSE stream resets its inactivity timer; this is not a total duration limit. A failed stream does not publish partial raw output as success. Desktop close protection warns about active work; Android uses a user-visible foreground execution lease. Process death is recovered from durable state, never a promise that the old socket or WebView callback survived.

Usage/time/cost observations retain their provenance. Missing provider token usage is unknown, not a fabricated exact estimate. Operational diagnostics may contain bounded counts/engine/model IDs without recording raw prompts, reasoning, or source text. See [document AI routing](document-ai-routing.md) and [processing architecture](ai-and-document-processing.md).

## Study guides and topic-linked learning

A guide is generated content with a revision lifecycle, not a long card answer or a blob owned by the Documents screen. It shares a source and learning plan with generated flashcards but can exist independently. Native topic/card links connect exact identities; editable tags and text similarity do not become the source-of-truth mapping.

The guide library uses bounded summary pages, search/sort, counts, and an explicit open request. Opening one item hydrates its guide, not every list row. The inline reader preserves main navigation. Technical revision metadata stays native/diagnostic rather than cluttering the everyday reading surface.

Generated base sections are published as a complete validated revision. Regeneration leaves the prior active revision available until replacement succeeds. Learner guide notes attach to the document; topic notes attach to stable topics; section overrides attach to an exact generated base/content hash. A changed base that cannot safely accept an old override becomes a visible preserved conflict rather than silently losing or misapplying learner text.

The current reader exposes plain-text note/section editing backed by validated rich-document storage. The broader editor adapter and canonical type do not imply that all flashcard formatting controls are exposed in this reader. This distinction prevents documentation from advertising a future UI as implemented.

Topic practice resolves current linked card IDs and studyability. A guide section with no cards differs from one with unavailable cards or a failed projection read. The resulting session uses ordinary practice/scheduling and returns through the application workflow context.

PDF export freezes an effective guide snapshot, optionally including annotations, then renders locally into a prepared-file result. Export does not invoke a model or regenerate content. Text extraction/layout tests and safe font handling protect the native pipeline, but visual output still needs review for representative scripts, pagination, and devices.

Deleting a guide removes generated revisions, guide-specific overrides/conflicts, checkpoints/jobs, and prepared PDF exports while preserving its source, learning plan, linked cards, and stable guide/topic notes. Permanent source deletion is broader; surviving generated cards retain detached readable citation provenance where appropriate. Archiving a source retains its graph. A guide-only document is valid and must not be forced to invent a deck.

Sources: [guide library](../../src-tauri/src/document_study_guide_library.rs), [guide generation](../../src-tauri/src/document_study_guide_generation.rs), [guide/overlay store](../../src-tauri/src/document_study_guides.rs), [topic links](../../src-tauri/src/document_topic_links.rs), [PDF export](../../src-tauri/src/document_study_guide_export.rs), [guide deletion](../../src-tauri/src/document_study_guide_deletion.rs), [ADR 0043](../decisions/0043-study-guides-and-topic-linked-learning.md).

## Platform and Android boundaries

[PlatformRuntime](../../src-tauri/src/platform/mod.rs) composes capability, external-file, secure-secret, lifecycle, clock/timezone, and resource ports. Adapters perform OS work and report observations; shared services choose domain policy. Unsupported capability is explicit, not a successful no-op or a browser substitute for a native guarantee.

External-file ports own acquisition/publication and bounded streaming through app-private storage. Shared code validates reported size/hash/purpose and records the transfer ledger. Android content URIs remain in the platform integration rather than becoming durable deck fields. Crash reconciliation can promote verified copied/renamed staging files or fail a bad transfer instead of leaving a valid import permanently stranded.

The private [ostudy platform plugin](../../plugins/ostudy-platform/) supplies Kotlin bridges for Android file/share/secret/lifecycle operations, document rendering/OCR, QR acquisition, and foreground document execution. The generated Android project contains the shell, manifest/resources, packaging setup, and instrumentation. Do not replace a native boundary by assuming WebView browser APIs have the same permissions, lifecycle, or data visibility.

Google Code Scanner is separate from OCR. Play services owns its scanner UI/camera permission; manual invitation entry remains a fallback. ML Kit Text Recognition supplies the bundled Latin document OCR path. Sharing, file picking, keyboard/safe-area insets, Back navigation, and Activity/process restoration need dedicated Android tests beyond frontend mocks.

A foreground service with a visible notification helps a user-started long task outlive Activity backgrounding; a bounded wake lock is not indefinite execution authority. Resource pressure, service limits, termination, reboot, or user cancellation can still end work. Durable checkpoints and startup reconciliation, not a hidden endless background job, make the result recoverable.

The platform boundary is also the future extension point. A future iOS/Swift adapter must implement the required services and be qualified; a conservative stub is not a finished platform. [Platform services](platform-services.md) and [Android development](../development/android.md) contain the narrower contracts.

## Security and privacy model

The trusted computing base includes the application binary, its dependencies/native runtimes, the OS/WebView, and configured platform services. Rust validation and CSP reduce attack surface, but neither makes a hostile native parser or a compromised user account harmless. The application is not an encrypted operating system or a complete hostile-document sandbox.

The important trust boundaries are external file → staged bytes → validated domain graph; WebView intent → native command; provider output → typed validated task result; peer package → authenticated/validated merge; and managed asset identity → controlled file bytes. Every crossing needs both structural and ownership checks appropriate to its operation.

Packaged [Tauri CSP](../../src-tauri/tauri.conf.json) restricts network connections to self/IPC, disallows embedded objects/frames, and constrains image/font sources. Inline styling remains allowed for editor/presentation needs. Development CSP is a separate configuration and should not be described as identical production hardening. Native provider networking is outside the WebView's direct HTTP surface and stays in the gateway.

Rich documents and templates contain allowed structured data, not executable imported HTML. Search uses bound input. Managed file protocols validate identity/path ownership rather than exposing arbitrary filesystem traversal. Package readers validate versions, graph integrity, paths, size, MIME signatures and hashes. A valid file format still does not prove the educational content is true.

Privacy claims must name the channel. Source text, generated cards/guides, and annotations are intentionally persisted in the local library and included where appropriate in backups or typed domain DTOs. They are excluded from **routine diagnostic logging of raw transport/source bodies**, not categorically excluded from storage. Saved credentials/private keys and raw reasoning are different: they do not become learner-data payloads or ordinary exports.

Newly typed credentials can be present transiently in the entry form; no saved-secret readback exists. Session-only keys have an explicit lifetime. OS secure storage availability is not identical on every target. Local files/backups are not automatically encrypted at rest; use suitable OS/disk/backup protection. Sync's encrypted transport does not change that fact.

Local provider choice is enforced: failures never authorize cloud fallback. Sending to a LAN server still crosses a network boundary; a phone's loopback is not the desktop. Cloud request flags express policy but do not independently verify an external provider's retention practices. Do not expose a private LAN sync or unauthenticated model service publicly as a troubleshooting shortcut.

Diagnostics are local and bounded, with safe event metadata and rotation. Export is deliberate. Even a redacted operational report may contain filenames or model identifiers, so inspect it before sharing. Destructive/restore/provider tests use synthetic sources, temporary libraries, and non-production credentials.

## Build, dependencies, and delivery

Toolchain/dependency facts live in [versions.env](../../versions.env), [package.json](../../package.json), [Rust toolchain](../../rust-toolchain.toml), lockfiles, and the native runtime manifest. Do not repeat transient version numbers throughout product prose. New direct dependencies need explicit need, exact pinning, license review, native/platform compatibility, and qualification.

The short Linux test-drive path on a configured machine is `scripts/build.sh` then `run-app.sh`. The build wrapper provisions/checks the expected environment, installs locked dependencies, performs compile preflight, uses the Tauri build path, and stages a standalone **debug** executable. It does not generate release bundles or prove the full test suite passed.

The launcher checks a build stamp/source fingerprint and staged binary checksum. It does not rebuild or start a Vite server. The staged standalone artifact protects the launched executable from later Tauri dev/other builds reusing Cargo's mutable debug output. `pnpm dev`/`pnpm build` are frontend operations; `pnpm desktop` is native hot reload; neither should be mislabeled as a standalone installed release.

Android's normal install wrapper builds/verifies an ARM64 debug APK, installs non-streaming with `-r`, and launches the debug application. The no-build path selects an existing APK by modification time, not a complete Git-SHA/ABI attestation; use known provenance or a fresh build for review. Replacement install is not fresh data. Debug/release IDs, signing keys, APK versus bundle, and public signed distribution remain distinct concerns.

Native PDFium acquisition uses a reviewed manifest and archive hashes rather than an opaque transitive runtime downloader. External parser/font/native assets have provenance and license obligations. An installed app's offline operation does not imply that initial dependency provisioning or optional live-provider testing is network-free.

Releases coordinate the application manifests, release checks, intended reviewed main commit, target packaging, and explicit signing/distribution. A normal merge does not automatically change the version, tag a release, or make all platforms qualified. [Installation](../installation/README.md) and [local handoff](../development/local-testing-and-handoff.md) give concrete safe commands.

## Testing, diagnostics, and development workflow

The test strategy layers inexpensive structural checks, focused behavior tests, native integration, and platform runtime evidence. Static verification checks documentation links/organization, IPC parity, dependency pins, migration identities, platform/module/style ownership, test policy, and workflow source-read-only contracts. It is not proof of complete correctness or a complete dead-code detector.

Vitest/Testing Library tests protect components, controllers, request lanes, navigation, selection, overlays, and state transitions. Rust tests cover validation, SQL invariants, operation replay, schedules/history, migrations/repairs, transfer graphs, source processing, and provider dialects with deterministic fixtures. Native WebDriver tests exercise persistence and actual Tauri/WebKit interaction. Android compile/package and instrumentation smoke test its separate integration boundary.

Focused reproduction comes before another expensive matrix. A regression test should target the current production owner, not an unmounted predecessor that happens to have extensive tests. During this documentation refresh, unused guide implementations and their exclusive styles/tests were retired while their active-reader coverage was retained and active-library empty-state coverage was added.

Performance and visual baselines preserve provenance and comparison contracts. A recorded old UI is not the current product specification. Wall-clock performance is advisory/manual; deterministic correctness tests must not pass or fail merely because an arbitrary worker ran slower. Removing stale code must not rewrite historical measurements to manufacture a new baseline.

The canonical agent workflow is **GitHub-direct**. The Connector reads exact source and publishes ordinary files through expected-parent Git-data commits and verified non-force ref updates. Exactly one writer owns a branch. Custom developer MCPs/Codex/orchestrators are not normal prerequisites and require an explicit current-task request. A failed requested MCP must not trap a task in repeated connectivity diagnosis while GitHub still works.

Actions execute published source and never author it, including manually dispatched maintenance workflows. Draft automatic Check is fast; explicit gates and ready PRs provide full/platform qualification. `PR Contract` checks metadata independently; a malformed or stale acceptance snapshot must not prevent product qualification. Readiness may trigger full testing and is not a reason to claim pending tests passed.

Qualification belongs to the actual tested SHA, not the branch label. Publication, static/fast/full/platform results, manual acceptance, and human merge are separate states. Failure handling distinguishes product, test contract, environment, Connector, service incident, and human-only boundaries. Same-fingerprint repetition requires a changed strategy, not no-op commits, permission expansion, sleeps, or weaker assertions.

Local diagnostics record bounded JSON Lines and safe run context. CI preserves machine-readable qualification/failure evidence and bounded logs. The human handoff supplies exact checkout/SHA verification, a short native build/run/install path, concrete acceptance observations, unrun checks, and data/rollback cautions. It does not make the human reconstruct an opaque implementation or rerun a full matrix merely to look at the UI.

See [Testing and CI](../development/testing-and-ci.md), [Git workflow](../development/workflow.md), [agent procedure](../agents/GITHUB-DIRECT-DEVELOPMENT.md), and [diagnostics](../development/diagnostics.md).

## Tradeoffs, compatibility, and extension rules

The relational local library favors inspectability, transactional integrity, and offline use over effortless global multi-user replication. Explicit device sync adds merge/conflict complexity but avoids requiring a central account service. Its allowlist must evolve deliberately with new document learning graphs; it cannot inherit every table automatically.

The WebView/Rust split adds serialization and platform integration work but keeps native data/secret policy independent of UI changes. Typed gateways and structural verifiers reduce accidental bypasses. They do not prove that all screens are already pure presentation or justify coupling domain objects to one screen's interaction details.

Separate parser/render/OCR engines add native dependency management but handle distinct evidence problems and allow platform-specific capabilities without rewriting document reasoning. Canonicalization limits how much raw parser detail is exposed; that is intentional, although richer non-PDF provenance will need future coordinated work. Structured model output improves reliability but does not prove grounding or educational quality.

Draft/operation/file ledgers and checkpoint versions add code because interruption and lost acknowledgements are normal, especially on mobile. Removing them merely to shorten a flow would trade visible complexity for data loss/duplicate work. Conversely, unused alternate components and unconsumed internal APIs are not valuable compatibility simply because they once shipped in a development PR.

Retain compatibility only with a named consumer: released migration identity, supported backup/interchange reader, persisted checkpoint, asset/citation identity, or real platform fallback. Historical ADRs and fixtures remain useful evidence. Maintained docs must not keep contradictory old instructions alongside a new appendix. The [cleanup audit](../development/documentation-and-cleanup-audit.md) records proven removals and limits of this review.

New work should enter through the narrowest existing boundary. A provider adds a native dialect plus deterministic contracts/probe/privacy tests. A document format supplies an explicit adapter returning canonical data. A platform implements required ports and packaging/runtime tests. A new learning tool reuses source/topic/authoring services but receives its own narrow task contract, not generic prompt/SQL IPC. A changed durable representation needs migrations/readers and a coordinated backup/sync/privacy review.

A new UI workflow should first specify user intent, current domain operation, loading/error/empty/dirty states, accessibility, and compact behavior. Add a read projection when necessary for scale; do not move the canonical rule into the screen. A durable cross-module decision receives an ADR describing alternatives, consequences, compatibility, and enforcement.

Known limits include incomplete platform/release qualification, model/extraction/OCR variability, partial non-PDF locator richness, the narrower synchronization graph, ordinary unencrypted local-library storage, no complete hostile-document process sandbox, and remaining concentrated orchestration. Tutoring, dedicated new providers, and additional study tools remain roadmap work. These are engineering boundaries to improve explicitly, not hidden features to advertise as completed.

## Source map and glossary

| Area | First source locations to inspect |
| --- | --- |
| Shell, routing, restoration | `src/main.tsx`, `src/App.tsx`, `src/controllers/useAppController.ts`, `src/navigation/`, `src-tauri/src/ui_checkpoint.rs` |
| Shared interaction/visual system | `src/ui/`, `src/platform/`, `src/components/NavigationSidebar.tsx`, `src/components/MobileAppBar.tsx` |
| Flashcards library | `src/screens/flashcards/`, especially `deck-library/` controller/models/views |
| Card browsing/editing | `src/screens/cards/`, `CardEditorScreenBase.tsx`, `src/editor/`, `src/controllers/useDurableDraft.ts` |
| Native composition/data | `src-tauri/src/lib.rs`, `startup.rs`, `database.rs`, `paths.rs`, `data_repairs.rs`, `migrations/` |
| Cards/templates/drafts | `authoring/`, `cards/content.rs`, `editor_drafts.rs`, `decks.rs`, `deck_folders.rs` under native source |
| Learning/schedules | `scheduler.rs`, `practice.rs`, `practice_selection.rs`, `practice_resume.rs`, `schedule_controls.rs`, `study_calendar.rs`, `studyability.rs` |
| Read projections/history | `deck_cards.rs`, `projections/`, `analytics/`, `workload.rs` |
| Assets and deletion | `asset_lifecycle.rs`, `asset_protocol.rs`, `deletion.rs`, `deletion_purge.rs` |
| External files/transfers | `external_files.rs`, `external_file_streaming.rs`, `external_file_recovery.rs`, `studydeck/`, `library_backup.rs`, `document_backup.rs` |
| AI infrastructure | `ai/gateway/`, `ai/profiles/`, `ai/probes/`, `ai/providers/`, `ai/secrets.rs`, `ai/reasoning.rs` |
| Source ingestion | `documents/`, `document_ingestion/`, `pdf_backend/`, `pdf_document_ingestion/`, `xberg_document_adapter/` |
| Canonical learning artifacts | `document_canonical*.rs`, `document_learning*.rs`, `document_topic_links.rs`, `document_citations.rs`, `document_artifacts.rs` |
| Generation and guides | `ai/document_intelligence/`, `document_generation.rs`, `document_checkpoints.rs`, `document_study_guide*.rs`, `src/screens/StudyGuidesScreen.tsx`, `StudyGuideDialog.tsx` |
| Sync | `src-tauri/src/device_sync/`, `src/screens/settings/DeviceSyncSettings.tsx` |
| Android | `plugins/ostudy-platform/`, `src-tauri/gen/android/`, Android scripts/instrumentation |
| Build/evidence/policy | `scripts/`, `.github/workflows/`, `tests/scripts/`, `tests/webdriver/`, co-located frontend and native tests |

**Note:** editable source shared by its card presentations. **Card/presentation:** stable independently studyable identity. **Schedule:** current scheduling state, not the history ledger. **Projection:** derived bounded read model, not a second write authority. **Revision:** freshness/concurrency evidence. **Operation ID:** identity of a logical mutation/replay. **Checkpoint:** versioned restart/reuse hint whose authority is bounded by its owning subsystem. **Canonical document:** application-owned normalized source evidence, not a third-party parser object. **Learning topic:** stable source-plan identity used for artifact/card relationships. **Overlay:** in guide storage, learner-owned annotation/override; in UI, a dialog/menu layer—the contexts are distinct. **Prepared export:** validated app-private bytes awaiting platform publication. **Qualification:** evidence for a named gate and exact source SHA, not a permanent property of a branch.

When changing a subsystem, follow its source owners, tests, focused reference, and relevant ADR together. Update this architecture when ownership or durable meaning changes, not for every new internal helper or transient CI result.
