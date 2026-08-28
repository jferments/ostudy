# Learning workflow and UI simplification roadmap

This is the implementation ledger for ADR 0049. It starts from `main` at `6ccf6574c801654fe1328ee830fae5d948e3b0b7`, immediately after the zero-legacy CSS completion in PR #76.

The work is intentionally staged. Each stage must end in a bounded commit, run its focused tests, and leave the branch in a usable state. Full Linux/Tauri, Android, emulator, and human visual qualification occurs only on the final exact head.

## Product goals

The redesign should make ostudy feel quieter and faster without hiding ordinary tasks. It should:

- reduce persistent button and metadata noise;
- make Flashcards, Study Guides, Documents, and Analytics correspond to user goals rather than implementation workflows;
- keep common actions directly reachable and move secondary actions into compact, accessible affordances;
- improve multi-deck practice and movement on desktop and mobile;
- preserve source grounding after document deletion;
- make study guides first-class, attractive, and useful on large screens and in PDF;
- extend rich cards without binding the application permanently to Tiptap;
- reorganize AI-model settings around the model being configured;
- establish measurable responsiveness budgets instead of relying on subjective performance impressions.

## Architectural constraints

- Keep the direction `Rust domain -> typed IPC -> controllers -> feature screens -> shared UI`.
- Shared UI must not import IPC, invoke Tauri commands, or know document/deck domain rules.
- Continue using the existing deck organization model and `LibraryMoveIntent`; do not create a second movement policy.
- Continue using the shared document learning plan for document-derived cards and study guides.
- Preserve operation identity, post-commit response boundaries, optimistic revision checks, and checkpoint recovery.
- Keep one global CSS import and zero authored feature-global CSS.
- Use semantic tokens and CSS Modules for every new component.
- Preserve keyboard, screen-reader, reduced-motion, forced-colors, safe-area, IME, pointer, and touch behavior.
- Do not overwrite user-edited deck metadata or learner annotations during AI regeneration.
- Do not qualify an implementation head and then modify source files without rerunning qualification.

## Stage 0 — Baseline, characterization, and contracts

- [x] Record ADR 0049 and this durable roadmap.
- [x] [Inventory every affected route, controller, IPC command, database field, smoke path, and documentation page](ui-workflow-redesign-affected-surface-inventory.md).
- [x] [Characterize current practice checkpoint restoration, direct card practice, deck practice, tag practice, and early-exit behavior](ui-workflow-redesign-current-behavior-characterization.md#ac-01-practice).
- [x] [Characterize current document archive/delete behavior and the existing preserved deck-citation path](ui-workflow-redesign-current-behavior-characterization.md#ac-02-document-lifecycle).
- [x] [Characterize study-guide revision, overlay, export, and document-delete cascades before adding guide deletion](ui-workflow-redesign-current-behavior-characterization.md#ac-03-study-guide-lifecycle).
- [x] [Capture representative visual baselines for wide, medium, and compact layouts in all supported themes](ui-workflow-redesign-visual-baseline.md).
- [x] [Add reproducible User Timing/React Profiler scenarios for navigation hover, document-list interaction, deck drag, deck selection, guide open, and active-practice progress](ui-workflow-redesign-performance-baseline.md#scenario-contract).
- [x] [Record initial interaction traces and define budgets for avoidable React commits, pointer-frame work, and visible hover/drag latency](ui-workflow-redesign-performance-baseline.md#evidence-and-budgets).
- [x] Add a route/smoke inventory assertion so removing Practice and adding Study Guides cannot silently reduce coverage.

## Stage 1 — Shared compact-action and metadata primitives

- [x] Add an accessible `Tooltip` primitive for delayed pointer hover and immediate keyboard focus without affecting layout.
- [x] Add `IconButton` with required accessible labels, familiar semantic icons, loading/disabled states, and compact/wide touch-target rules.
- [x] Extend the icon vocabulary with information, calendar/schedule, study-guide/book, overflow, maximize/full-screen, restore, and any missing standard actions.
- [x] Add `ResponsiveActionGroup` so primary actions stay visible while lower-priority actions move into an overflow menu by layout mode.
- [x] Add an accessible `OverflowMenu` with roving focus, Escape dismissal, outside-click dismissal, destructive grouping, and compact-sheet presentation.
- [x] Add `SelectionToolbar` for multi-item count, primary bulk action, secondary bulk actions, and clear-selection behavior.
- [x] Add `MetadataDialog` / `InfoButton` for secondary operational details that should not remain permanently visible.
- [x] Define an action-priority policy: primary text action, familiar icon action, contextual selection action, and overflow-only action.
- [x] Migrate representative delete, edit, archive, schedule, and information controls to prove the primitives before feature-wide adoption.
- [x] Add component tests for accessible names, tooltip focus behavior, keyboard navigation, touch targets, loading states, destructive styling, and all appearance modes.
- [x] Keep all new styling within shared CSS Modules and semantic tokens; add ownership checks for the new primitives.

## Stage 2 — Navigation, branding, fonts, and desktop window behavior

- [x] Add a first-class `study-guides` application section immediately below Flashcards in wide, medium, and compact navigation. (Stage 2A)
- [x] Remove the top-level Practice navigation item and normalize compact-navigation primary/More groupings. (Stage 2A)
- [x] Split the old Practice destination into an internal active-session state plus launch surfaces owned by Flashcards, Search, Study Guides, and Analytics. (Stage 2B)
- [x] Migrate saved UI checkpoints: resumable sessions reopen internally; setup-only Practice checkpoints return to Flashcards. (Stage 2C)
- [x] Preserve platform back behavior, active-session exit guards, lifecycle checkpoints, and return targets after the route change. (Stage 2C)
- [ ] Replace the CSS-drawn brand mark with the supplied transparent raccoon mascot asset, with correctly generated desktop/mobile sizes and no baked background.
- [ ] Remove the “Learn with intent” motto from the sidebar and any other product copy.
- [x] Make IBM Plex Sans the default UI and card family for new installs.
- [x] Add a settings migration that maps the old `system` defaults to IBM Plex Sans without overriding a user-selected font.
- [x] Reuse the existing local-font provisioning and fallback system; document packaging/licensing and test missing-font fallback.
- [x] Add platform-adapter commands for native maximize/restore and full-screen where supported, with keyboard-accessible controls and no direct Tauri calls from shared UI. (Stage 2F)
- [ ] Ensure the shell, routes, dialogs, and guide workspace consume the full resulting viewport on desktop. (Stage 2G)
- [ ] Update navigation, brand, checkpoint, font, and desktop-window tests plus native/UI smoke coverage.

Stage 2B separates visible destination state from the in-memory Practice
workflow, centralizes deck, exact-card, guide-topic, and Analytics launches, and
keeps direct active-session rendering, exit guarding, and logical transient
return targets explicit. It does not change `ui-checkpoint-v1`: checkpoint
migration, platform Back/lifecycle semantics, and persisted return-target work
remain Stage 2C items in the unchecked checkpoint/Back line items. Branding,
fonts, window controls, viewport work, and the Stage 3 practice-setup relocation
also remain pending.

Stage 2C introduces the strict `ui-checkpoint-v2` representation while retaining
and retiring released v1 files through a compatibility reader. All five visible
destinations now save and restore; resumable Practice state carries a logical
return target independently, lifecycle saves flush the latest in-memory cursor,
platform Back retains the existing termination guard, and restored routes use
new request identities. Stage 3 setup relocation and the remaining Stage 2
branding, font, window, and viewport items are still pending; overall Stage 2 is
not complete.

Stage 2F adds independently reported maximize/restore and full-screen desktop
capabilities, least-authority main-window permissions, a Tauri-only platform
adapter, controller-owned native state reconciliation, and action-accurate
keyboard controls in the responsive shell chrome. Android, other mobile targets,
and browser-only execution report no window controls and receive no simulated
fallback. Stage 2G still owns full viewport consumption by the shell, routes,
dialogs, and guide workspace. Branding and motto work also remain pending, so
overall Stage 2 is not complete.

## Stage 3 — Deck selection and practice setup relocation

- [ ] Add pure deck-selection state and actions separate from hierarchy ordering, drag state, open-row state, and folder selection.
- [ ] Replace decorative move affordances on deck rows with deck checkboxes and a clear selected state.
- [ ] Keep ordinary row click/open behavior available without toggling selection accidentally.
- [ ] Add Select visible, Select all in current scope, and Clear selection behavior with deterministic folder/filter semantics.
- [ ] Add a contextual toolbar with Practice selected as the primary action and Move, Archive, Delete, and other valid bulk operations as secondary actions.
- [ ] Define mixed-state validation and confirmation behavior for archived, empty, generated, or stale-revision deck selections.
- [ ] Extract the reusable practice-scope/workload logic from the old Practice setup screen into a controller/pure-model boundary.
- [ ] Open practice setup as a responsive sheet/dialog from selected decks, with scheduled versus extra study, shuffle, due/new counts, and optional tag refinement.
- [ ] Preserve direct single-deck practice as a one-step common action.
- [ ] Preserve card-ID practice launched from Search, Documents, editor flows, and study-guide topics.
- [ ] Preserve tag-based study through Search and Analytics without recreating a top-level setup dashboard.
- [ ] Remove the old setup/recent/study-guide composition from `PracticeScreen` once all launch paths have replacements.
- [ ] Put deck and folder actions into an accessible overflow menu on compact layouts while keeping Practice directly reachable.
- [ ] Add controller, selection-model, route-launch, compact-menu, stale-revision, and UI smoke tests.

## Stage 4 — Deck and folder drag/drop reliability and performance

- [ ] Keep all movement inputs routed through the existing `LibraryMoveIntent` and organization policy.
- [ ] Allow drag initiation from any non-interactive area of a deck or folder row rather than a tiny move icon.
- [ ] Exclude buttons, links, inputs, checkboxes, text selection, and overflow menus from drag initiation.
- [ ] Add a small pointer activation threshold and pointer capture so clicks remain clicks and deliberate drags reliably “catch.”
- [ ] Make the full visible folder row/card a valid drop target, including its padded area rather than only a narrow label region.
- [ ] Add clear valid/invalid drop styling, insertion location, destination folder name, and screen-reader announcements.
- [ ] Add hover-to-expand and edge auto-scroll with bounded timers and cancellation.
- [ ] Keep keyboard move controls and the touch/mobile Move sheet as equal, tested alternatives.
- [ ] Separate transient drag-preview state from persisted organization state and avoid reloads or IPC during pointer movement.
- [ ] Throttle pointer/layout work to animation frames and cache row/folder geometry for a drag session.
- [ ] Memoize deck/folder row rendering and prevent drag hover from committing the entire library tree.
- [ ] Add large-library drag traces, interaction tests for anywhere-to-anywhere movement, and regressions for invalid descendants and stale revisions.
- [ ] Meet the recorded pointer-frame and visible-feedback budgets before considering the stage complete.

## Stage 5 — Deck detail, active-practice progress, and analytics navigation

- [ ] Rebuild the deck-detail header as one vertical flow: breadcrumb, full-width title, description, then actions.
- [ ] Reduce the deck title scale slightly and remove the narrow left-column compression visible on wide desktop layouts.
- [ ] Use `ResponsiveActionGroup` for Practice, Export, Archive, Delete, and mobile overflow behavior.
- [ ] Remove remaining non-functional move icons from deck detail and card/deck presentation.
- [ ] Extend the shared Progress primitive with an accessible inline-content variant.
- [ ] Put “studied · remaining” inside a thicker active-practice progress track with contrast that remains readable over filled and unfilled regions.
- [ ] Vertically center the percentage with the progress track and provide equivalent `aria-valuetext`.
- [ ] Remove the “Needs more practice” chip from the active practice header while retaining scheduling outcomes and analytics data.
- [ ] Make tag names in Analytics → Recently practiced interactive controls.
- [ ] Add a typed navigation request that opens Flashcards → Search with the selected tag filter already applied.
- [ ] Move useful due-topic, workload, and recent-practice summaries from the removed Practice setup destination into Analytics without duplicating scheduling calculations.
- [ ] Add deck-header, progress, analytics-to-search, keyboard, responsive, and UI smoke tests.

## Stage 6 — Document lifecycle simplification, compact cards, and durable citations

- [ ] Define and test a database migration for existing archived documents that restores each row to a valid non-archived terminal state without deleting data.
- [ ] Remove document archive/restore commands, IPC wrappers, controller state, lifecycle labels, `includeArchived`, and “Show archived documents.”
- [ ] Retire or normalize `archived_at` and the archived lifecycle state only after backup/import compatibility and migration tests are in place.
- [ ] Keep live processing stage, progress, elapsed time, token/request activity, and cancellation visible while work is active.
- [ ] Hide completed format, size, source-unit, page/render, processing-time, token, and updated metadata behind an information icon and `MetadataDialog`.
- [ ] Keep the persistent document card focused on subject title, filename/source citation, state, generated-card count, and common learning actions.
- [ ] Rename “Open source” to “View.”
- [ ] Replace the persistent Delete document text button with a red trash icon, accessible label, tooltip, and explicit destructive confirmation.
- [ ] Apply responsive action priority so View and generation remain direct while lower-priority actions collapse cleanly.
- [ ] Promote the existing preserved deck citation behavior into a versioned source-citation snapshot independent of the `documents` row.
- [ ] Include source title, original filename, format, available `SourceLocator` page/slide/chapter/section data, source identity, and “source removed” state in the snapshot.
- [ ] Render citations consistently from deck detail, card source views, search results where appropriate, and exports.
- [ ] Preserve generated decks/cards and their citation snapshots when a document is deleted.
- [ ] Keep document deletion’s current cascade for the managed source, derivatives, document-bound study guides, annotations, and generation jobs, and state that behavior precisely in confirmation copy.
- [ ] Add migration, delete/citation, backup/restore, compact-card, info-dialog, processing-state, and UI smoke tests.

## Stage 7 — Subject-centered generated titles and descriptions

- [ ] Extend the shared document learning-plan/generation contract with bounded, schema-validated artifact metadata.
- [ ] Generate a concise subject-centered document display title, deck title, and educational deck description from the learning plan rather than the filename.
- [ ] Keep full book/document title, original filename, and source details exclusively in the citation/provenance area.
- [ ] Prohibit boilerplate such as “AI-generated,” “generated from,” and repeated source filenames in user-facing deck descriptions.
- [ ] Add deterministic fallback metadata derived from validated outline headings/key concepts when the provider omits or fails metadata generation.
- [ ] Normalize whitespace, punctuation, unsafe control characters, and enforce explicit title/description length limits.
- [ ] Persist metadata origin, generator/prompt version, and source fingerprint so regeneration can distinguish generated values from user edits.
- [ ] Never overwrite a title or description after the user changes it.
- [ ] Add a narrowly matched repair for legacy values that exactly follow known filename-derived generator templates; leave all ambiguous values untouched.
- [ ] Ensure Generate more cards and guide regeneration reuse the existing user-facing deck metadata rather than renaming the deck.
- [ ] Add schema, prompt, fallback, migration/repair, user-edit-protection, and end-to-end generation tests.

## Stage 8 — First-class Study Guides workspace and deletion lifecycle

- [ ] Add a lazy-loaded `StudyGuidesScreen` and controller owned by the new top-level route.
- [ ] Move the existing guide library out of the old Practice screen.
- [ ] Replace the library’s archived-source concepts with current document/citation state after document archives are removed.
- [ ] Add bounded list projection, loading, empty, error, refresh, search, and sort behavior without hydrating full guide bodies.
- [ ] Open a guide through route/controller state so back navigation and compact presentation behave predictably.
- [ ] Route document Generate/Open study guide actions to the new workspace and refresh the list after generation.
- [ ] Add a revision-checked `delete_document_study_guide` domain command and typed IPC boundary.
- [ ] Define deletion to remove generated guide revisions, guide-specific overrides/conflicts, checkpoints, and prepared exports while preserving the source document, learning plan, linked flashcards, and stable document/topic learner notes.
- [ ] Use a destructive confirmation that explicitly describes what is preserved and what is removed.
- [ ] Refresh list/detail state transactionally after deletion and handle stale revisions without duplicate side effects.
- [ ] Use compact icon/overflow actions and keep Open as the direct row action.
- [ ] Add complete UI smoke coverage for route entry, list, open, generate/regenerate, annotate, topic practice, export, delete, empty state, and compact navigation.
- [ ] Update checkpoint, back-navigation, controller, IPC, migration, and user-guide tests.

## Stage 9 — Study-guide reader, semantic emphasis, and designed PDF export

- [ ] Replace the small fixed dialog constraint with a responsive guide workspace that can occupy nearly the full desktop viewport.
- [ ] Keep readable line length inside a wide shell using a contents rail, flexible article column, and optional contextual/source rail rather than shrinking the entire window.
- [ ] Use a full-screen/sheet layout on compact devices and preserve focus restoration, deep section links, and keyboard navigation.
- [ ] Rework guide actions through the shared responsive action system.
- [ ] Add a validated semantic emphasis vocabulary/inline segmentation so key terms can render in bold without accepting raw provider HTML.
- [ ] Apply the same emphasis logic to the on-screen guide and native PDF renderer.
- [ ] Refactor the Rust PDF code into a reusable print-layout layer with semantic block styles instead of one stream of plain text calls.
- [ ] Embed/package IBM Plex Sans Regular, Medium/SemiBold, Bold, and Italic through the existing font provisioning path, with deterministic fallback and licensing documentation.
- [ ] Add a clear title page/header hierarchy, horizontal rules, shaded learning-objective/key-concept/note panels, section numbering, source labels, page numbers, and restrained running headers/footers.
- [ ] Add styled terms, comparisons, processes, misconceptions, annotations, tables, and callout blocks with consistent spacing.
- [ ] Improve pagination with bounded keep-with-next, widow/orphan avoidance, repeated table headers, and safe splitting of long blocks.
- [ ] Include a polished source/citation section using the durable citation snapshot.
- [ ] Preserve selectable/searchable text and do not rasterize the React view.
- [ ] Add structural PDF tests, font-embedding assertions, deterministic text extraction checks, representative rendered-page visual baselines, and large-guide pagination tests.
- [ ] Add reader accessibility, wide/compact visual, theme, export, and UI smoke tests.

## Stage 10 — Editor adapter plus subscript, superscript, tables, and answer checklists

- [ ] Define an editor-neutral `EditorAdapter` / `EditorCapabilities` / `EditorCommand` contract around canonical `RichDocument`, assets, selection state, and change events.
- [ ] Keep the public editor surface and feature screens free of Tiptap `Editor`, extension names, and command-chain calls.
- [ ] Move the current Tiptap setup into a private adapter/provider and generate toolbar groups from declared capabilities.
- [ ] Version and validate the canonical rich-document schema for every new node, mark, and attribute.
- [ ] Add Subscript and Superscript marks with toolbar controls, shortcuts, renderer styles, and mutual-exclusion behavior.
- [ ] Add TableKit with an insert-table dialog and controls for header row, add/remove row/column, merge/split where safely supported, and delete table.
- [ ] Make tables keyboard reachable, horizontally scrollable on compact cards, readable in practice, and supported in templates, previews, search snippets, backup/import/export, and study-guide/PDF rendering where reused.
- [ ] Add ordinary TaskList/TaskItem editing with persisted checked state.
- [ ] Add an answer-key checklist mode with stable item IDs: the question presents disabled empty boxes, while the answer reveals the author-marked correct boxes.
- [ ] Add a “copy checklist to answer” authoring action and validation for missing, duplicate, or mismatched checklist item IDs.
- [ ] Ensure ordinary non-answer-key task lists retain their actual checked state rather than being forced into quiz behavior.
- [ ] Expose low-noise advanced formatting menus for useful StarterKit features already present but not surfaced: undo/redo, strike, blockquote, horizontal rule, inline code, and code block.
- [ ] Evaluate mathematics/KaTeX, typography, text color, character count, find/replace, and placeholders; document which are deferred and why rather than silently expanding scope.
- [ ] Keep high-frequency controls visible and place advanced insertion/formatting controls in compact menus so the toolbar does not become another clutter source.
- [ ] Add adapter contract tests that can be reused by a future non-Tiptap implementation.
- [ ] Add round-trip, schema migration, sanitizer, renderer, card-template, practice, import/export, backup, table, checklist, keyboard, and UI smoke tests.

## Stage 11 — AI Models settings information architecture and per-model reasoning

- [ ] Move reasoning effort from global `AppSettings` into the persisted AI provider/model profile.
- [ ] Preserve `automatic` plus model-advertised effort validation and mandatory-reasoning constraints on each profile.
- [ ] Migrate the existing global value to every existing configured profile so model switching does not silently change behavior.
- [ ] Remove the obsolete global reasoning setting only after settings backup/restore and migration compatibility are verified.
- [ ] Keep the top block as Default model/task assignment.
- [ ] Place the AI models heading and model list below the default block, with Add model adjacent to the list.
- [ ] Place the currently selected model’s provider, connection, model, reasoning, timeout, retention, credential, test, and delete configuration below the list.
- [ ] Remove the current two-column/sidebar-detail layout in favor of one clear vertical flow.
- [ ] Keep selected-model context visible while editing without creating nested scrolling traps.
- [ ] Use responsive disclosure/overflow patterns for advanced provider settings on compact devices.
- [ ] Ensure document generation resolves reasoning from the selected profile and records the effective effort in job/provenance data.
- [ ] Add profile migration, model capability, default assignment, save/test/delete, compact layout, generation-resolution, and UI smoke tests.

## Stage 12 — Measured responsiveness and rendering hardening

- [ ] Re-run the Stage 0 traces after each affected subsystem and attribute improvements to specific changes.
- [ ] Profile navigation hover latency across themes/material modes before changing visual effects.
- [ ] Audit large backdrop filters, shadows, paint areas, animated properties, and route transitions; reduce or isolate only effects shown to cause missed frames.
- [ ] Partition controller state so unrelated hover, menu, selection, progress, and drag changes do not rerender whole screens.
- [ ] Memoize expensive rows/projections with stable props and callbacks where profiling demonstrates repeated work.
- [ ] Batch or defer non-urgent refreshes and ensure no synchronous IPC occurs in hover, pointer-move, or drag-over handlers.
- [ ] Keep document and generation polling scoped to active jobs and prevent one-second polling from repainting static rows.
- [ ] Virtualize very large deck/document/guide/card collections only when measured thresholds justify it, preserving focus, screen-reader order, and drag semantics.
- [ ] Add development-only render counters/User Timing marks that tree-shake or disable in production.
- [ ] Add bounded performance regression scenarios for navigation hover, large-library selection, drag, document polling, guide opening, editor typing, and progress updates.
- [ ] Meet the recorded budgets on the project’s reference workstation and self-hosted runner where the metric is deterministic.
- [ ] Document remaining platform-specific bottlenecks rather than masking them with arbitrary animation delays.

## Stage 13 — Documentation, smoke coverage, and exact-head qualification

- [ ] Update `docs/architecture/ui.md` for the route, action-density, practice-session, editor-adapter, and performance boundaries.
- [ ] Update document/AI/study-guide/editor user guides and remove archive/Practice-destination instructions.
- [ ] Document citation behavior after source deletion and guide behavior after guide/document deletion.
- [ ] Document the IBM Plex Sans default, fallback, packaging, and PDF embedding behavior.
- [ ] Update the UI smoke manifest so every top-level route and every critical guide/editor/document flow is represented.
- [ ] Run focused unit/integration tests after every bounded stage commit.
- [ ] Run static ownership/policy qualification and `scripts/check-fast.sh` at each stable checkpoint.
- [ ] Run authoritative Linux/Tauri `scripts/check.sh`, including startup and native WebDriver smoke, on the final exact head.
- [ ] Run Android compile/package qualification and API-36 emulator smoke on the final exact head.
- [ ] Launch the staged standalone desktop artifact through `./run-app.sh`.
- [ ] Complete the human matrix for Light, Dark, Console, and Synthwave; wide/medium/compact; 80/100/140% scale; full/reduced/no motion; rich/simple materials; keyboard, screen reader, pointer, and touch alternatives.
- [ ] Manually verify full-screen/maximized desktop behavior, wide study-guide use, action overflow, selection, drag/drop hit targets, editor tables/checklists, PDF typography, and hover responsiveness.
- [ ] Record the final exact SHA and qualification evidence in the PR without modifying qualified source afterward.
- [ ] Keep the PR draft until automated gates are green and the human acceptance matrix is approved.
- [ ] Squash merge after approval and remove the development branch.

## Implementation sequencing notes

The stage order is deliberate:

- shared controls land before feature cleanup, preventing one-off icon/menu implementations;
- route and practice boundaries land before deleting the old Practice destination;
- document citation snapshots land before archive removal or metadata cleanup;
- the Study Guides workspace lands before the reader/export redesign;
- the editor adapter lands before adding schema features;
- baseline traces land before any performance claim.

When a stage exposes an unexpected domain dependency, update this ledger and the PR checklist before implementation. Do not silently fold unrelated architectural work into a stage.
