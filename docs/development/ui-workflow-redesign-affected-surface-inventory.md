# UI workflow redesign affected-surface inventory

This is the durable Stage 0 affected-surface map for [ADR 0049](../decisions/0049-learning-workflow-ui-simplification.md) and the [learning-workflow redesign roadmap](ui-workflow-redesign-roadmap.md). It records the ownership topology at commit `f92697fd7b5a61ddc4f67793d9bd9b84edcdc3c3`; later stages must update the owning contracts instead of rediscovering or bypassing them.

The Stage 0 checkpoint changed no product behavior, schema, migration, transfer format, IPC command, smoke path, or ownership boundary. “Future” in the baseline tables identifies a roadmap consumer, not behavior that was already shipped at Stage 0. Current screen-owned IPC orchestration is recorded as topology and does not imply that the controller ownership required by later stages already exists.

## Reading the map

- **Authority** names the state or layer that decides behavior now. React state is transient unless a durable owner is named.
- **Compatibility concern** identifies behavior or data that a later stage must preserve, migrate, or deliberately replace.
- **Roadmap consumer** names the stage that must use the row when it changes the surface. Stage 13 owns final documentation, smoke, and exact-head qualification across all rows.
- `scripts/route-smoke-contract.json` is the existing machine-readable expected inventory. It remains an assertion over application-owned routes and executable smoke evidence; it is not routing configuration.

## Stage 2A navigation delta

Stage 2A adds `study-guides` as a typed, lazy-rendered application section and
visible destination immediately after Flashcards. Wide and medium navigation is
now ordered Flashcards, Study Guides, Documents, Analytics, Settings. Compact
navigation exposes Flashcards, Study Guides, Analytics, and More; More contains
Documents and Settings. The route/smoke contract records this complete,
ordered, disjoint partition and protects direct entry at each layout size.

Practice remains an `AppSection`, an App render branch, and the owner of the
existing setup, active-session, completion, checkpoint, exit-guard, and return
behavior, but it is no longer in any visible navigation collection. Existing
deck/card/topic actions remain its contextual entry points. The current guide
library also remains composed inside Practice until Stage 8; the new route
reuses it rather than introducing the pending route-owned guide controller.

The native `ui-checkpoint-v1` section vocabulary is unchanged. Stage 2A excludes
Study Guides from generic and lifecycle checkpoint writes, so reopening restores
the last supported destination. Practice checkpoint migration and normalization,
including resumable-versus-setup-only behavior, remain pending. The Stage 0
tables below preserve the original affected-surface baseline; this delta is the
maintained description of the implemented navigation checkpoint.

## Stage 2B active-session delta

Stage 2B removes `practice` from `AppSection` and represents it with a separate
typed `AppWorkflow`. The visible section remains the contextual launch origin,
while `PracticeWorkflowState` owns the launch or restore payload and a logical
transient return target. App renders the workflow directly, the shell labels it
Practice, and no visible primary destination is selected while it is active.

Deck, selected deck/tag scope, and exact-card requests converge in
`useAppController`; scheduling-mode policy is normalized there before
`PracticeScreen` dispatches the resulting discriminated request through the
single `PracticeController.start` boundary. That dispatcher retains the two
existing typed native gateways: selected scopes use
`start_selected_practice`, while exact card IDs use `start_card_practice`.
Rust scheduling, review, session, and database ownership are unchanged.

At the Stage 2B checkpoint, `ui-checkpoint-v1`, its `section = "practice"`
compatibility representation, lifecycle saves, and platform Back behavior were
not yet migrated. Stage 2C completes that bounded follow-up as described below.
Stage 3 still owns setup relocation, multi-deck selection, and removal of the
retained setup/recent/guide composition.

## Stage 2C checkpoint lifecycle delta

Stage 2C adds strict `ui-checkpoint-v2.json` storage. Its section vocabulary is
the five maintained visible destinations, including Study Guides; optional
Practice data independently stores a resumable session hint and logical return
target. Rust prefers v2, reads v1 only when v2 is absent, normalizes valid v1
Practice/Cards/Templates/Create values, writes canonical v2, and retires v1.
Invalid v2 never falls through to stale v1, and clear removes both generations.

`useAppController` now constructs ordinary and lifecycle snapshots. It retains
the latest cursor reported by `PracticeScreen`, restores the visible backing
route before reopening Practice, validates saved deck targets against the
current library, and allocates new Flashcards/Analytics request identities.
Platform Back still runs through overlay precedence and the active-session
termination guard. Stage 8 still owns guide detail/controller/deletion state;
Stage 3 still owns the retained Practice setup composition.

## Stage 2E typography delta

Stage 2E makes the existing application-owned `plex` preset the shared initial
interface and card default. The foundation CSS, application controller, Restore
appearance defaults action, and semantic appearance resolver now agree on IBM
Plex Sans followed by the system sans-serif stack. UI and card choices remain
independent, and rich card content continues to consume
`--card-font-family`.

The existing native compatibility read is now an explicit invariant: only a
released `system` row with `updated_at = 0`, or an absent font row, resolves to
`plex`. A real save writes a nonzero timestamp, so an explicit system, bundled,
accessibility, serif, mono, or custom value is preserved independently for
`uiFont` and `cardFont`. No released migration was edited and no durable schema
or backup format changed.

IBM Plex Sans continues through the maintained Google Fonts provisioning cache,
local `@font-face`, and system-family fallback. Production builds verify hashed
local WOFF2 output, CSS references, offline behavior, and a packaged OFL 1.1
notice. A pristine source build may need the one-time provisioning network;
installed application use does not. Native Study Guide PDF typography and
embedding remain Stage 9, and Stage 2 remains otherwise incomplete.

## Stage 2F native window-controls delta

Stage 2F extends the internal native capability snapshot with independent
maximize/restore and full-screen availability. Current desktop adapters report
both available; Android and conservative mobile adapters provide explicit
unavailable reasons. The trusted main window receives only maximize,
unmaximize, and set-fullscreen mutation authority.

`src/platform/windowControls.ts` is the direct Tauri window boundary.
`useWindowControlsController` initializes from native state, reconciles external
window-manager changes on native resize events with a latest-read lane,
serializes UI mutations, and surfaces failures without browser fallbacks. The
shell renders one accessible group in the wide/medium sidebar or compact app
bar. The native close guard remains unchanged. Stage 2G still owns shell, route,
dialog, and guide-workspace full-viewport consumption; overall Stage 2 remains
incomplete.

## Route and navigation topology

Visible destination vocabulary is owned by `AppSection`; protected workflow
vocabulary is owned separately by `AppWorkflow`. Normalization and transient
workflow state live in `src/controllers/useAppController.ts`. Rendering is
selected in `src/App.tsx`; primary navigation is declared in
`src/components/NavigationSidebar.tsx`; compact titles are mirrored in
`src/components/MobileAppBar.tsx`. These lists must change together with the
route/smoke contract.

| Surface | Current repository path and owner | Current authority and checkpoint behavior | Compatibility concern | Roadmap consumer |
| --- | --- | --- | --- | --- |
| Flashcards | `src/controllers/useAppController.ts`, `src/App.tsx`, `src/components/NavigationSidebar.tsx` | `section = "decks"` is a primary destination and owns the `FlashcardsRoute` state. | Remains the first primary destination and becomes the fallback for setup-only old Practice checkpoints. | Stages 2–5 |
| Documents | `src/App.tsx`, `src/screens/DocumentsSection.tsx`, `src/screens/DocumentsScreen.tsx` | `section = "documents"` is a primary destination. | Route remains primary while archive semantics and action density change; document actions later route into Study Guides. | Stages 1, 2, 6–9 |
| Practice | `src/controllers/useAppController.ts`, `src/controllers/practiceController.ts`, `src/App.tsx`, `src/screens/PracticeScreen.tsx` | `AppWorkflow = "practice"` and `PracticeWorkflowState` own the internal launch/restore container independently of the visible `section`; schema v2 stores only a resumable cursor and logical visible return target. One discriminated controller start boundary retains the existing selected-scope and exact-card native gateways. | SQLite remains authoritative; route/request IDs are reconstructed rather than persisted. Stage 3 relocates setup. | Stages 2, 3, 5 |
| Analytics | `src/App.tsx`, `src/screens/AnalyticsScreen.tsx`, `src/controllers/analyticsController.ts` | `section = "analytics"` is a primary destination. `AnalyticsLaunchRequest` can carry range, deck/tag filters, and highlighted session. | Receives workload/recent-practice material and must launch Search with a typed tag-filter request without duplicating native analytics or scheduling rules. | Stage 5 |
| Settings | `src/App.tsx`, `src/screens/SettingsScreen.tsx` | `section = "settings"` is a primary destination; the screen owns settings draft/preview/save state and composes the AI-profile controller. | Navigation order changes; font, window, and per-model reasoning work must preserve dirty-state guards and platform/security boundaries. | Stages 2, 9, 11 |
| Cards alias | `src/controllers/useAppController.ts` (`AppSection`, `normalizeDestination`) | `"cards"` is a compatibility alias normalized to `decks/search`; it is not rendered as a primary destination. | Preserve callers/checkpoints until they are deliberately migrated; do not add a second Search route. | Stages 2, 3, 5 |
| Templates alias | `src/controllers/useAppController.ts` (`AppSection`, `normalizeDestination`) | `"templates"` is a compatibility alias normalized to `decks/templates`; it is not rendered as a primary destination. | Preserve callers/checkpoints and unsaved-template guarding while navigation changes. | Stages 2, 10 |
| Study Guides | `src/screens/StudyGuidesRoute.tsx`, `src/screens/StudyGuideLibrarySection.tsx`, `src/screens/StudyGuideDialog.tsx` | `section = "study-guides"` is a visible schema-v2 checkpoint destination. Topic-linked card IDs launch the internal Practice workflow and retain Study Guides as the durable logical return target. The retained guide library also remains composed in Practice setup until Stage 8. | Stage 8 still owns route-scoped library/reader/deletion architecture, detail restoration, and removal of the legacy nested composition. | Stages 2, 8, 9 |

### Flashcards nested routes

`src/navigation/flashcardsRoutes.ts` owns the nested route type; `src/screens/flashcards/FlashcardsWorkspace.tsx` owns the tabs and renders the workspace; `src/controllers/useAppController.ts` owns route request identity and deck-detail transitions.

| Nested surface | Current route | Authority and compatibility concern | Roadmap consumer |
| --- | --- | --- | --- |
| Decks | `{ workspace: "decks", view: "root" }` | Default Flashcards root. Deck list/folder selection and later bulk selection remain separate from application routing. | Stages 2–5 |
| Templates | `{ workspace: "templates", view: "root" }` | Canonical destination for the `templates` alias and template dirty-state guard. | Stages 2, 10 |
| Search | `{ workspace: "search", view: "root" }` | Canonical destination for the `cards` alias. Later Analytics tag launches must enter this route with controller-owned filter intent. | Stages 3, 5, 10 |
| Import/Export | `{ workspace: "transfer", view: "root" }` | Owns deck/import/backup launch UI, while durable formats remain Rust-owned. | Stages 2, 6, 10 |
| Deck detail | `{ workspace: "decks", view: "deck", deckId }` | `createFlashcardsDeckRoute`, `openFlashcardsDeck`, and `returnToFlashcardsLibrary` preserve request identity and platform-Back behavior. | Stages 2, 3, 5, 6 |

## React and controller ownership

| Workflow | Current React/controller boundary | Authoritative state and mutation/read policy | Compatibility concern | Roadmap consumer |
| --- | --- | --- | --- | --- |
| Application navigation and checkpoints | `src/controllers/useAppController.ts`; shell in `src/App.tsx`; back adapter in `src/platform/backNavigation.ts` | Controller owns visible `section`, internal `practiceWorkflow`, `FlashcardsRoute`, fresh launch/request IDs, logical return targets, dirty guards, canonical v2 snapshots, lifecycle saves, and top-level Back. `deckRefreshLane` prevents obsolete deck reads from replacing current state. | Checkpoint reconciliation is presentation-only; do not let stale reads overwrite route state or treat a failed checkpoint save as a failed durable study mutation. | Stage 2 |
| Deck library and movement | `src/screens/flashcards/DeckLibraryWorkspace.tsx`; current controller `src/screens/flashcards/deck-library/useDeckLibraryController.ts`; reorder adapter `useDeckLibraryReorder.ts`; pure policy `organizationModel.ts`; types in `types.ts` | Rust/SQLite own deck/folder organization and revisions. The controller owns loading, mutation busy state, sheets, collapsed folders, announcements, archive visibility, and reconciliation. Every pointer/keyboard/fallback move becomes `LibraryMoveIntent`; pointer preview is not durable. | Stage 3 must add selection as separate state from open row, folder state, and drag/reorder. Stage 4 must retain one intent/policy and optimistic organization revision checks. A committed move remains successful if refresh fails. | Stages 3, 4 |
| Deck detail | `src/screens/DeckScreen.tsx`, `src/controllers/decksController.ts`, `src/controllers/cardsController.ts`, `src/controllers/cardEditorController.ts` | Controllers own latest-result lanes for domain reads; Rust owns deck/card mutations and revisions. | Header/action redesign must preserve direct practice, export, archive/delete, authoring, Back, and committed-versus-reconciled reporting. | Stages 3, 5, 6, 10 |
| Search/card browser | `src/screens/cards/CardBrowserWorkspace.tsx`, `src/screens/cards/CardFilters.tsx`, `src/controllers/cardsController.ts` | Controller/Rust own bounded searches, list-matching selection resolution, card mutations, and independent request lanes; screen owns filter and selection presentation. `CardPresentation` remains the study/search transport. | Preserve card-ID practice and add a typed Analytics-to-Search tag launch without making React authoritative for query semantics. New rich nodes must render/search safely. | Stages 3, 5, 10 |
| Analytics launches | `src/screens/AnalyticsScreen.tsx`, `src/controllers/analyticsController.ts`, launch state in `useAppController.ts` | SQLite/Rust projections are authoritative; the controller applies latest-read semantics. App controller owns launch range/deck/tag/highlight intent and the session-to-analytics transition. | Tag controls must route to Search; moved workload summaries must not reproduce schedule calculations in React. | Stage 5 |
| Practice setup and session | `src/screens/PracticeScreen.tsx`, `src/screens/practice/PracticeSetup.tsx`, `ActivePractice.tsx`, `PracticeComplete.tsx`, pure scope helper `practiceSetupScope.ts`, `src/controllers/practiceController.ts` | React owns presentation/session cursor and pending UI flags. Rust/SQLite own workload, selected/card sessions, snapshots, reveal previews, transactional reviews, completion, and resume. Controller uses independent latest-read lanes for workload/recent/window/card detail; mutations are not stale-canceled. | Extract setup/workload launch behavior before removing the destination. Preserve scheduled/extra/card scopes, operation/submission identity, restart/resume, window paging, edit refresh, completion analytics, and return targets. | Stages 2, 3, 5, 10 |
| Practice checkpoint and exit | `src/controllers/useAppController.ts`, `src/controllers/practiceExitGuard.ts`, `src/screens/PracticeScreen.tsx` | `registerPracticeExitHandler` exposes only the current active-session termination boundary. `PracticeScreen` writes durable session checkpoints; App controller confirms navigation termination and restores real sessions. | Old Practice checkpoint migration must distinguish resumable session from setup-only state. Reviewed cards remain committed when early exit completes the session. | Stages 2, 3, 13 |
| Documents lifecycle and polling | `src/screens/DocumentsSection.tsx`, `src/screens/DocumentsScreen.tsx` | The screen currently owns list/poll timing, `includeArchived`, progress snapshots, clock, upload/generate/cancel/archive/delete/guide busy IDs, and refresh/error state while calling typed IPC directly. Rust owns durable lifecycle, task identity, cancellation, transactions, managed files, and deletion reconciliation. | This is current screen-owned orchestration, not the intended extracted controller topology. Later extraction must use independent list/progress lanes, scope polling to active jobs, and keep committed deletion/generation distinct from refresh failure. | Stages 1, 6, 7, 8, 12 |
| Study-guide library and reader | `src/screens/StudyGuideLibrarySection.tsx`, `src/screens/StudyGuideDialog.tsx` | These components currently own list/selection, guide load/generate/cancel, overlays, topic projections/practice, and PDF preparation directly through typed IPC. SQLite/Rust own guide revisions, overlay revisions/conflicts, topic links, checkpoints, jobs, and export artifacts. | Stage 8 must introduce a route-owned controller without claiming it exists now; deletion needs optimistic revision and a precise cascade. Stage 9 must keep learner overlays and deterministic native PDF ownership. | Stages 8, 9, 12 |
| Settings and AI profiles | `src/screens/SettingsScreen.tsx`, `src/screens/settings/DefaultAiModelSettings.tsx`, `AiLlmSettings.tsx`, `AppearanceSettings.tsx`, `src/controllers/aiProfilesController.ts` | Settings screen owns draft/preview/dirty/save; `app_settings` owns durable application preferences. AI controller owns independent list/detail/model lanes plus profile/default/credential/probe mutations. SQLite owns profile metadata/task defaults; native secure store owns credential bytes. | Fonts and reasoning migration must preserve explicit user choices, dirty state, capability validation, credentials, and task defaults. Per-profile reasoning does not exist yet. | Stages 2, 9, 11 |
| Rich card/template editor | `src/screens/CardEditorScreen.tsx`, `src/screens/templates/TemplateLayoutEditor.tsx`, `src/screens/card-editor/TemplateEntryForm.tsx`, public editor modules under `src/editor/`, `src/controllers/cardEditorController.ts`, `templatesController.ts`, `useDurableDraft.ts` | `RichDocument` in `src/ipc/common.ts` is the transport. Rust validator and SQLite notes/templates/drafts are authoritative. `RichTextEditor.tsx`, `RichTextView.tsx`, and `StudyRichEditor.tsx` currently own Tiptap `useEditor`, extensions, selection, and toolbar commands directly. | Stage 10 must isolate Tiptap behind an editor-neutral adapter while preserving draft flush/commit, assets, validation taxonomy, schema version, renderers, practice/search, templates, and transfers. | Stage 10 |
| Window and native close behavior | viewport/layout providers under `src/ui/layout/`; capability gateway `src/ipc/platform.ts`; native window adapter `src/platform/windowControls.ts`; controller `src/controllers/useWindowControlsController.ts`; native close guard in `src-tauri/src/lib.rs` with active task state in `src-tauri/src/task_cancellation.rs` | The native snapshot independently reports maximize/restore and full-screen availability. Desktop window operations and resize observation stay behind the platform adapter; the controller owns latest native state and serialized mutations. Native close protection still blocks shutdown during an active document task. | Android/mobile expose explicit unavailable capabilities and no browser fallback. Stage 2G must make layouts consume the resulting viewport without duplicating window policy. | Stages 2, 9, 12 |

## Typed IPC and Rust entrypoints

All frontend calls remain under `src/ipc/`; `src-tauri/src/lib.rs` is the registration authority. The command lists below are current command names and must be changed end-to-end if a later stage deliberately changes the bundled contract.

| Typed gateway and commands | Registered Rust command owner | Domain/persistence owner and later-stage concern | Roadmap consumer |
| --- | --- | --- | --- |
| `src/ipc/documents.ts`: `list_documents`, `get_document_generation_progress`, `import_document`, `run_document_flashcard_generation`, `generate_more_pdf_flashcards`, `cancel_document_flashcard_generation`, `open_document_source`, `delete_document`, `set_document_archived` | `src-tauri/src/documents/commands.rs`, `document_progress.rs`, `document_import.rs`, `document_operations.rs`, `document_more.rs` | `documents/` owns contracts/lifecycle/repository/reconciliation; ingestion modules own canonical source; initial/more generation and `document_checkpoints.rs` own task/checkpoint behavior; `document_delete_reconciliation.rs` owns managed-file cleanup. Archive removal, citation snapshots, and generated metadata must remain transactional and revision-checked. Compatibility exports named `*Pdf*` in the gateway are aliases, not separate commands. | Stages 6, 7, 8 |
| `src/ipc/studyGuides.ts`: `generate_document_study_guide`, `cancel_document_study_guide_generation`, `get_document_study_guide`, `list_document_study_guides`, `get_document_learning_plan`, `get_document_artifact_job`, `save_document_study_guide_overlay`, `discard_document_study_guide_overlay`, `get_document_topic_card_projection`, `get_document_topic_card_ids`, `prepare_document_study_guide_pdf` | `src-tauri/src/document_study_guide_commands.rs`, `document_study_guide_export.rs` | `document_learning*.rs`, `document_artifacts.rs`, `document_study_guide_generation.rs`, `document_study_guides.rs`, `document_topic_links.rs`, and `document_card_topics.rs` own plans, guide publication, overlays/conflicts, topic links, jobs, and projections. Export is deterministic native output. There is no `delete_document_study_guide` command yet. | Stages 7–9 |
| `src/ipc/practice.ts`: `get_practice_workload`, `start_selected_practice`, `start_card_practice`, `resume_practice_session`, `get_practice_card_window`, `refresh_practice_after_card_edit`, `mark_practice_card_viewed`, `mark_practice_card_revealed`, `submit_practice_review`, `complete_practice_session` | `src-tauri/src/practice_selection.rs`, `practice_resume.rs`, `practice.rs` | Native selection/projections and scheduler own studyability, stable snapshots, resume, preview, transactional/idempotent review, and completion. Route relocation must not turn these rules into frontend logic. | Stages 2, 3, 5, 10 |
| `src/ipc/uiCheckpoint.ts`: `get_ui_checkpoint`, `save_ui_checkpoint`, `clear_ui_checkpoint` | `src-tauri/src/ui_checkpoint.rs` | Native file owner strictly reads v1/v2 and atomically writes canonical device-local `ui-checkpoint-v2.json`. It stores visible section/workspace/deck, Analytics filters, and optional resumable Practice identity/cursor/return target. It is not a SQLite table. | Stages 2, 8 |
| `src/ipc/settings.ts`: `get_settings`, `save_settings`, `get_app_info` | `src-tauri/src/settings.rs` | `app_settings` owns preferences. Rust validates font presets, motion/material choices, scheduling limits, and global `aiReasoningEffort`; process-local publication currently feeds document-task resolution. IBM Plex compatibility logic already maps untouched timestamp-zero `system` rows to `plex`, but defaults and documentation still require Stage 2 treatment. | Stages 2, 9, 11 |
| `src/ipc/platform.ts`: `get_platform_capabilities`, `set_platform_back_interception` plus lifecycle/Back/insets/network/share event listeners; `src/platform/windowControls.ts`: current-window maximize/restore/full-screen reads, mutations, and resize observation | `src-tauri/src/platform/mod.rs`, platform implementations under `src-tauri/src/platform/`, and Tauri's scoped main-window API | Native platform adapters own capability and Android Back behavior. The frontend platform adapter owns presentation-facing native window operations; `src-tauri/src/lib.rs` separately owns desktop close guarding. | Stages 2, 9, 12 |
| `src/ipc/aiProfiles.ts`: `get_ai_provider_catalog`, `list_ai_provider_profiles`, `get_ai_provider_profile`, `create_ai_provider_profile`, `update_ai_provider_profile`, `delete_ai_provider_profile`, `set_ai_provider_credential`, `clear_ai_provider_credential`, `get_ai_task_defaults`, `set_ai_task_default`, `list_ai_provider_models`, `test_ai_provider_profile`, `get_ai_provider_probe_result` | `src-tauri/src/ai/profiles/commands.rs`, `src-tauri/src/ai/probes/commands.rs` | `src-tauri/src/ai/profiles/` owns contracts/policy/projections/repository/service/defaults; `src-tauri/src/ai/probes/` owns model/capability evidence; `src-tauri/src/ai/secrets.rs` and platform secret services own credentials. `src-tauri/src/ai/document_intelligence/context.rs` currently combines selected profile/capabilities with the process-local global reasoning preference. Profiles have no reasoning field. | Stage 11 |
| `src/ipc/cards.ts`: `list_tags`, `search_cards`, `list_matching_cards`, `get_card_note`, `create_card_note`, `update_card_note`, `find_duplicate_cards`, `analyze_bulk_cards`, `bulk_create_cards`, `transfer_cards`, `bulk_tag_cards`, `delete_card`, `delete_cards`, `undo_deletion` | `src-tauri/src/authoring/card_commands.rs`, `src-tauri/src/authoring/card_types_and_templates.rs`, `src-tauri/src/authoring/transfer.rs`, `src-tauri/src/deletion_commands.rs`, `src-tauri/src/deletion.rs` | Authoring validation, card/note identities, search projections, transactions, delete undo, and rich-document validation remain native. `CardNote` must stay distinct from `CardPresentation`. | Stages 3, 5, 6, 10 |
| `src/ipc/drafts.ts`: `open_editor_draft`, `save_editor_draft`, `discard_editor_draft`, `commit_editor_draft`, `list_editor_drafts` | `src-tauri/src/editor_drafts.rs` | Durable draft revision, payload schema/hash, staged assets, canonical commit, and retirement are native/SQLite-owned. An editor-adapter refactor must preserve exact-revision flush and recovery. | Stage 10 |
| `src/ipc/templates.ts`: `list_card_template_summaries`, `get_card_template`, `create_card_template`, `update_card_template`, `delete_card_template`, `preview_template_cards`, `generate_cards_from_template` | `src-tauri/src/authoring/card_types_and_templates.rs` and `src-tauri/src/authoring/card_types_and_templates/templates.rs` | Template field/rule validation and structural rich-document substitution are native. New editor nodes/marks must round-trip preview and generation without exposing Tiptap types. | Stage 10 |
| `src/ipc/decks.ts`: `list_decks`, `get_deck`, `create_deck`, `update_deck`, `set_deck_archived`, `delete_deck`, `list_deck_card_summaries`, `prepare_deck_export`, `import_deck` | `src-tauri/src/decks.rs`, `src-tauri/src/deck_cards.rs`, `src-tauri/src/deletion_commands.rs`, `src-tauri/src/studydeck.rs` | Deck metadata/revision/lifecycle, card projections, deletion, and `.studydeck` transactions are native. Selection and action-density work must preserve these revision and transfer boundaries. | Stages 3, 5–7, 10 |
| `src/ipc/deckFolders.ts`: `get_deck_library_organization`, `create_deck_folder`, `update_deck_folder`, `delete_deck_folder`, `move_deck_in_library`, `move_deck_folder` | `src-tauri/src/deck_folders.rs` | SQLite `library_organization.revision`, folder/deck revisions, and validated ordering are authoritative. Every input path remains one `LibraryMoveIntent` policy in the frontend and one optimistic native mutation boundary. | Stages 3, 4 |

## SQLite persistence and migration compatibility

The public schema is append-only. `src-tauri/src/database.rs` registers migration identity, order, SHA-256 checksum, `LATEST_SCHEMA_VERSION`, and its migration tests; `scripts/check-migration-checksums.mjs`, `scripts/current-public-migrations.mjs`, `tests/scripts/test_current_public_migrations.py`, `src-tauri/tests/fixtures/public-baseline-schema.json`, `src-tauri/tests/study_guide_migration5.rs`, the backup tests under `src-tauri/src/library_backup/`, and `scripts/smoke-startup-compatibility.sh` protect fresh/upgrade behavior. This inventory adds no migration and changes no schema.

### Migration lineage

| Migration | Affected durable surface | Compatibility concern and roadmap consumer |
| --- | --- | --- |
| `src-tauri/migrations/0001_public_baseline.sql` | Baseline decks/notes/cards, practice, settings/AI, documents/generation, folders, drafts, transfer ledgers, and analytics ownership described below. | Immutable released baseline. Later stages add new migrations or data repairs and update checksums/fixtures; they never edit this file. Stages 2–7, 10, 11. |
| `src-tauri/migrations/0002_canonical_documents.sql` | Adds canonical schema/adapter/count fields to `documents`; source-unit counts to jobs; locator/visual-asset fields to generation windows/checkpoints; creates `document_source_files`, `document_source_units`, `document_source_blocks`, `document_source_assets`, and `document_semantic_chunk_sources`. | Canonical format-neutral source/provenance is the owner for citation locators and future metadata; page-shaped fields remain compatibility projections. Stages 6, 7, 9. |
| `src-tauri/migrations/0003_anydoc_document_adapters.sql` | Adds authoritative `documents.source_media_type` while retaining the old PDF-constrained `mime_type`; broadens `staged_external_imports` to format-neutral document kinds. | Citation and metadata work must use actual source format without breaking old rows/import staging. Stages 6, 7, 9. |
| `src-tauri/migrations/0004_study_guides_and_learning_topics.sql` | Creates learning plans/topics, artifact jobs, guides/sections/overlays, note-topic links, and guide checkpoints. | Document deletion currently cascades document-bound rows. Guide deletion must define a narrower revision-checked cascade and preserve document, plan-linked flashcards, and stable learner notes as approved. Stages 7–9. |
| `src-tauri/migrations/0005_study_guide_pdf_exports.sql` | Changes topic-parent delete behavior to `SET NULL`; extends `prepared_exports.purpose` with `study_guide_pdf`. | Preserve document cascades and local prepared-export lifecycle while redesigning PDF layout/fonts. Stages 8, 9. |

### Affected tables and fields

| Durable owner | Current affected fields/relationships | Current behavior and compatibility concern | Roadmap consumer |
| --- | --- | --- | --- |
| `documents` | Identity/title/original filename/source MIME and format/path/hash/size; `lifecycle_state`, `archived_at`, `revision`; page/source-unit/extracted/rendered counts; extraction/render/adapter backends and canonical schema version; semantic/outline/window/OCR progress; generated note/card/deck; safe errors; processing duration and token usage. | Rust document lifecycle is authoritative. Archive is a checked lifecycle state today. Stage 6 must migrate existing archived rows to a valid terminal state before removing archive APIs/filters; do not discard processing/provenance metrics. | Stages 6, 7, 12 |
| `decks` and document provenance | `decks.name`, `description`, `revision`, archive/delete state; `document_decks(document_id, deck_id, job_id)`; `documents.generated_deck_id`; `document_note_topics` source locators/evidence. | Current deletion preservation in `src-tauri/src/document_operations.rs` rewrites `decks.description` with a human-readable deleted-source citation. No structured citation-snapshot table or transfer contract exists. Soft-deleted decks are not preservation targets. | Stages 6, 7, 9 |
| Document generation/checkpoints | `document_generation_jobs` provider/model/profile/credential/network/state/progress/deck/error/time plus source counts; generation windows/units/checkpoints; outline/checkpoints; canonical source files/units/blocks/assets/chunks. | Durable completed work is reusable; process-local cancellation is not. Source locators already support page/slide/chapter/section-style units and should feed citation snapshots rather than filename prose. | Stages 6, 7, 9, 12 |
| Learning plans/topics | `document_learning_plans` revision/compiler/fingerprints/summary/objectives/concepts/state; stable `document_learning_topics.anchor_hash`; ordered `document_learning_plan_topics` hierarchy, content, locators, and evidence. | Shared plan is authoritative input for flashcards and guides. Subject metadata must extend this bounded native contract and protect user edits. | Stages 7–9 |
| Study guides | `document_artifact_jobs`; `document_study_guides` revision/schema/prompt/assembly/state; sections content/hash/locator; overlays kind/base hash/rich doc/plain text/state/revision; `document_study_guide_checkpoints`; `document_note_topics`. | Guide base revisions, conflicts, learner overlays, checkpoints, topic-linked notes, and job history are separate durable concerns. Current document deletion cascades all document-bound guide data. There is no guide-only delete use case yet. | Stages 8, 9 |
| Practice sessions | `practice_sessions` selected deck/tag IDs, mode, shuffle, counts, start operation, timestamps; `practice_session_cards` position/view/reveal/rating/disposition/variant; card/tag snapshot tables with captured note revision, owner deck, rich docs, text/hash; review events/submission IDs. | SQLite session identity/snapshots and review ledger are authoritative. A UI checkpoint refers to a resumable session but does not contain its cards. Route changes must preserve idempotent starts/reviews and early-exit completion. | Stages 2, 3, 5, 10 |
| Settings/fonts/reasoning | `app_settings` key/value/timestamp rows including `uiFont`, `cardFont`, UI scale/motion/material settings and `aiReasoningEffort`. | `src-tauri/src/settings.rs` currently treats untouched timestamp-zero `system` font rows as `plex`; explicit choices remain. Reasoning is global and also published process-locally. Stage 11 must migrate it to profiles before removing the key. | Stages 2, 9, 11 |
| AI profiles and task defaults | `ai_provider_profiles` provider/base URL/model/auth/privacy/timeout/credential markers/revision; `ai_task_defaults(task_kind, profile_id)`; capability/probe tables. | No per-profile reasoning field exists. Credential bytes are outside SQLite. Model capability evidence can advertise reasoning efforts, but task resolution currently reads the global setting. | Stage 11 |
| Rich notes/cards/templates | `notes.content_format = "tiptap-json"`, `content_schema_version = 1`, `front_doc`, `back_doc`, derived text, revision/type/reverse/cloze fields; `cards` stable presentation identity; template layout docs; draft `payload_schema_version`/JSON/hash and draft assets. | Canonical schema version 1 and native allowlist/validation taxonomy are compatibility surfaces. Stage 10 must version and round-trip new nodes/marks through notes, templates, drafts, practice snapshots, search/rendering, and transfers. | Stage 10 |
| Library organization | `deck_folders` identity/parent/sort/revision; `decks.folder_id`/sort; singleton `library_organization.revision` and revision triggers. | Bulk selection is not durable organization state. All moves remain optimistic revision-checked and use the same ordering policy. | Stages 3, 4 |
| Prepared/staged file ledgers | `prepared_exports` purpose/state/path/hash/expiry including `study_guide_pdf`; `staged_external_imports` purpose/kind/state/path/hash/expiry. | Platform pick/share/publish adapters own external movement; product screens receive opaque staged/prepared identities. Do not bypass this for PDF, deck, document, or backup workflows. | Stages 6, 9, 10 |

## Transfer, backup, checkpoint, and recovery ownership

| Contract | Current owner and contents | Explicit boundary to preserve or extend | Roadmap consumer |
| --- | --- | --- | --- |
| `.studydeck` | `src-tauri/src/studydeck.rs` and `src-tauri/src/studydeck/`; exports deck notes/cards/relationships/assets including `content_format`, `content_schema_version`, `front_doc`, and `back_doc`; imports create fresh schedules and no review history. | It has no document provenance/citation snapshot field today. Stage 6 must decide/version any portable citation addition; Stage 10 must round-trip every supported rich node/mark. | Stages 6, 10 |
| `.ostudybackup` | `src-tauri/src/library_backup.rs`, `library_backup/`, and `document_backup.rs`; packages a coherent SQLite snapshot plus validated managed assets, retained source documents, source assets, and renders in a manifest/ledger. | The SQLite snapshot already carries document/guide/settings/profile/draft state. Managed file hashes/paths must stay coherent. Credential bytes are excluded. Any new citation/profile-reasoning schema is preserved through SQLite migration/restore coverage. | Stages 6, 8–11 |
| Prepared Study Guide PDF | `src-tauri/src/document_study_guide_export.rs` writes a local deterministic artifact registered in `prepared_exports` with purpose `study_guide_pdf`; platform export publishes the prepared ID. | Export is transient delivery, not a restorable guide source. Preserve selectable text, cleanup/expiry, and native renderer ownership while adding fonts/layout/citations. | Stage 9 |
| Device UI checkpoint | `src-tauri/src/ui_checkpoint.rs` and `src/ipc/uiCheckpoint.ts`; canonical state is atomically stored as app-private `ui-checkpoint-v2.json`, with a released v1 compatibility reader. | It is deliberately outside SQLite and therefore excluded from `.ostudybackup`. Stage 2C normalizes Practice and adds visible Study Guides restoration; Stage 8 still owns guide detail state if required. | Stages 2, 8 |
| Credentials | Native secure-store boundary under `src-tauri/src/ai/secrets.rs` and platform secret services. | Never stored in SQLite, checkpoints, `.studydeck`, `.ostudybackup`, or prepared PDF. | Stage 11 |
| Device synchronization | `src-tauri/src/device_sync/`, `src/ipc/deviceSync.ts`, `docs/architecture/device-sync.md` | Protocol v2 always strips `app_settings`; AI profiles/task defaults are opt-in, SQLite credential markers are stripped, and credential bytes travel separately only through an explicit encrypted live transfer. Font/global reasoning settings therefore remain device-local today; profile reasoning would follow the opt-in profile boundary, not `app_settings`. | Stages 2, 11 |

## Maintained smoke and characterization paths

| Path | Current coverage authority | Later preservation/extension owner |
| --- | --- | --- |
| `scripts/route-smoke-contract.json`, `scripts/verify-route-smoke-contract.mjs`, `tests/scripts/test_route_smoke_contract.py` | Machine-readable parity among `AppSection`, rendered shell, primary navigation, Flashcards workspaces, and executable native/Android smoke evidence. The current expected inventory is five primary destinations, two aliases, four Flashcards workspaces, and protected active Practice. | Stage 2 must change Practice/Study Guides atomically with executable evidence; Stage 13 verifies the final manifest. |
| `src/App.test.tsx`, `src/components/NavigationSidebar.test.tsx`, `src/screens/flashcards/FlashcardsWorkspace.backNavigation.test.tsx`, `src/controllers/useAppController.practiceExit.test.tsx` | Frontend shell rendering, navigation, nested workspace Back, and active-practice exit behavior. | Stages 2, 3, 8. |
| `src/screens/PracticeScreen.test.tsx`, `src/screens/PracticeScreen.tagScope.test.tsx`, `src/screens/practice/ActivePractice.test.tsx`, `src/screens/practice/PracticeComplete.test.tsx`, `src/controllers/practiceController.ts`, `src/controllers/useAppController.practiceExit.test.tsx` | Practice setup/card/tag scopes, active review, completion, restoration, and exit characterization. | Stages 2, 3, 5, 10. |
| `src/screens/flashcards/DeckLibraryWorkspace.test.tsx`, `src/screens/flashcards/deck-library/useDeckLibraryController.test.tsx`, `src/screens/flashcards/deck-library/useDeckLibraryReorder.test.tsx`, `src/screens/flashcards/deck-library/organizationModel.test.ts` | Deck/folder actions, revision/reconciliation, and shared move-intent inputs. | Stages 3, 4, 5. |
| `src/screens/DocumentsScreen.test.tsx`, `src/screens/DocumentsScreen.ocrFailure.test.tsx`, `src/screens/DocumentsProgress.test.tsx` | Current archive/delete/generate/cancel/poll/progress and OCR presentation behavior. | Stages 6, 7, 8, 12. |
| `src/screens/StudyGuideLibrarySection.test.tsx`, `src/screens/StudyGuideDialog.test.tsx`; Rust guide/overlay/export/migration/backup tests | Current nested library, guide revision/regeneration, overlays/conflicts, topic practice, export, deletion cascade, and backup. | Stages 8, 9. |
| Editor/card/template frontend tests plus native authoring, `studydeck`, and backup tests | Canonical rich docs, Tiptap behavior, draft recovery, templates, practice rendering, and transfer compatibility. | Stage 10. |
| Settings/profile tests under `src/screens/SettingsScreen*.test.tsx`, `src/screens/settings/*test.tsx`, `src/controllers/aiProfilesController.test.tsx`; Rust settings/profile/probe/backup tests | Font choices/default compatibility, dirty/save behavior, model lists/capabilities, global reasoning, profiles/defaults, and credential markers. | Stages 2, 9, 11. |
| `scripts/smoke-native.sh` → `tests/webdriver/durable_library_smoke.py` (which runs `tests/webdriver/smoke.py`) | Creates deck/card, accepts a review, verifies Practice/Analytics and restart/checkpoint persistence. | Stages 2, 3, 5, 10, 13. |
| `scripts/smoke-native.sh` → `tests/webdriver/ui_platform_smoke.py` | Wide/medium/compact navigation, Flashcards tabs/Search, overlay focus, deck-scoped authoring, Settings, and overflow. | Stages 1–5, 8–13. |
| `scripts/smoke-native.sh` → `tests/webdriver/card_listing_followup_smoke.py` | Deck and Search card-listing parity plus authoring. | Stages 3, 5, 10, 13. |
| `scripts/smoke-native.sh` → `tests/webdriver/deck_toolbar_layout_smoke.py` | Compact Flashcards tabs, deck toolbar containment, and horizontal-overflow bounds. | Stages 1–5, 12, 13. |
| `scripts/smoke-startup-compatibility.sh` | Built-binary startup/adoption of the exact historical schema lineage while preserving deck/card/review/attribution data. | Every migration stage, especially 2, 6–11; final Stage 13. |
| `scripts/android-test.sh` | Android Rust/Kotlin compile/package and instrumentation contract qualification. | Any Stage 2–12 Android-shared or generated-project impact; final Stage 13. |
| `scripts/android-emulator-smoke.sh`, `src-tauri/gen/android/app/src/androidTest/java/io/github/jferments/ostudy/AndroidSmokeTest.kt` | Launch/test bridge; seeded library/practice/analytics; restart persistence; Documents route and Android Back; nested deck/editor Back; interrupted-practice Activity/process restoration; picker cancellation. | Stages 2, 3, 5, 6, 8, 10, 13. Add first-class Study Guides route and changed Practice entry coverage before deleting current assertions. |
| `src-tauri/gen/android/app/src/androidTest/java/io/github/jferments/ostudy/AndroidPlatformRoutingTest.kt` | Rust platform snapshot reaches native document and QR routes. | Stages 2, 6, 9, 11, 13; preserve the typed platform boundary when window behavior changes. |

The current route/smoke contract is a checkpoint, not complete future coverage. Later guide deletion/reader/export, rich editor nodes, document citation, AI reasoning, and maximize/full-screen behavior need focused tests in their owning stages and executable native/Android coverage where they become critical user flows.

## Maintained documentation impact

| Documentation group | Maintained paths | Current ownership note and roadmap consumer |
| --- | --- | --- |
| Architecture overview/UI/platform | `docs/architecture/overview.md`, `docs/architecture/ui.md`, `docs/architecture/platform-services.md` | Update route ordering/internal Practice, controller/action-density ownership, responsive/full-screen platform commands, editor adapter, and measured performance after implementation. Stages 2, 8, 10, 12, 13. |
| Card/data/transfer architecture | `docs/architecture/card-format-and-data-model.md`, `docs/decisions/0009-coherent-transfer-backup-and-repairs.md`, `docs/development/transfer-backup-and-repair-testing.md` | Update structured citation ownership and any versioned transfer field; update rich schema/round-trip and backup behavior. Stages 6, 10, 13. |
| Document/AI architecture | `docs/architecture/ai-and-document-processing.md`, `docs/architecture/document-ai-routing.md`, `docs/architecture/ai-infrastructure.md`, `docs/architecture/anydoc-adapter-implementation.md` | Update archive removal/delete cascade/citations, subject metadata, guide lifecycle/PDF, and profile-owned reasoning resolution while preserving provider/security boundaries. Stages 6–9, 11, 13. |
| Device sync architecture | `docs/architecture/device-sync.md` | Update only if profile reasoning changes the opt-in profile transfer fields; keep `app_settings` excluded and credentials separately scoped. Stage 11. |
| Active ADRs | `docs/decisions/0034-semantic-visual-system-and-flashcards-workspace.md`, `0039-root-activated-flashcards-routes-and-responsive-layout.md`, `0043-study-guides-and-topic-linked-learning.md`, `0046-modular-ai-infrastructure.md`, `0048-frontend-state-and-style-ownership.md`, `0049-learning-workflow-ui-simplification.md` | These define the route/workspace, guide, AI, controller/style, and approved redesign decisions. Later implementation must update or supersede only if a durable decision actually changes. Stages 2–12. |
| Development/testing | `docs/development/guide.md`, `docs/development/testing-and-ci.md`, `docs/development/ui-modernization-testing.md`, `docs/development/pdf-flashcard-generation-testing.md`, `docs/development/document-streaming-and-deletion-testing.md`, `docs/development/document-generation-quality-and-scale-testing.md`, `docs/development/document-platform-stabilization-testing.md`, `docs/development/large-document-budgeted-generation-testing.md`, `docs/development/ai-infrastructure-testing.md`, `docs/development/ai-provider-contract-testing.md`, `docs/development/scheduling-analytics-query-testing.md`, `docs/development/transfer-backup-and-repair-testing.md`, `docs/development/dependency-audit.md`, `docs/development/ui-workflow-redesign-roadmap.md`, and this inventory | Keep commands, route/smoke expectations, document/guide/editor/settings focused checks, font provisioning/licensing, timing scenarios/budgets, and exact-head qualification current. Stages 0, 2, 6, 8–13. |
| User: entry/navigation/practice | `docs/user-guide/getting-started.md`, `docs/user-guide/flashcards-workspace.md`, `docs/user-guide/flashcards-organization.md` | Remove Practice-as-destination instructions only when replacement launches ship; document deck selection/movement, Search/Analytics launches, active progress, and Study Guides navigation. Stages 2–5, 8, 13. |
| User: documents/guides/AI | `docs/user-guide/documents-and-ai-generation.md`, `docs/user-guide/ai-providers.md` | Document no-archive lifecycle, deletion/citation semantics, generated subject metadata, Study Guides create/open/delete/annotation/export, PDF fonts, and per-model reasoning. Stages 6–9, 11, 13. |
| User: appearance/backup/sync | `docs/user-guide/interface-appearance-and-guidance.md`, `docs/user-guide/appearance-and-guidance.md`, `docs/user-guide/backups-imports-and-sharing.md`, `docs/user-guide/device-sync.md` | Document IBM Plex UI/card default and fallback, full-screen/window behavior where user-facing, rich-format transfer compatibility, citation backup, and reasoning sync boundaries. Stages 2, 6, 9–11, 13. |
| In-product help/tutorial registry | `src/ui/help/helpRegistry.ts`, `src/ui/tutorials/tutorialRegistry.ts` | Update navigation/orientation, deck-folder movement/selection, Practice launch/daily-limit help, Documents lifecycle, Study Guides, editor formats, fonts, and AI reasoning at the stage that changes the UI. These registries are product source and are not changed by this inventory. Stages 2–4, 6, 8–11. |
| Product/index pages | `README.md`, `docs/README.md`, `docs/roadmap.md` | Keep shipped-versus-roadmap language truthful and link any new maintained user/architecture pages. Stage 13. |

## Stage handoff rules

1. Start a consuming stage from the current code and this map; if ownership has moved, update the relevant maintained architecture and this active roadmap rather than retaining a parallel path.
2. Change route declarations, checkpoint normalization, shell rendering, navigation, frontend tests, `scripts/route-smoke-contract.json`, and executable smoke evidence as one coherent route checkpoint.
3. Treat migrations, registered checksums, fixtures, backup/transfer behavior, and focused migration tests as one compatibility unit. Never edit migrations `0001`–`0005`.
4. Keep typed IPC command names, `src-tauri/src/lib.rs`, Rust request validation, controller callers, tests, and documentation in parity.
5. Preserve durable mutation success when view reconciliation fails; use stale-read lanes only for replaceable reads.
6. Do not mark later Stage 0 characterization, visual baseline, timing/profiling, or performance-budget items complete from this inventory.
