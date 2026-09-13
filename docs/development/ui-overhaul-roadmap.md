# shadcn/ui + Tailwind UI overhaul roadmap

Status: **completed historical roadmap for PR #96.** Phases 0–15 were implemented, exact-head automated qualification passed on the final PR head, and PR #96 was squash-merged to `main` as `5a4a4212e15c847db5bbf5daf1948e6ca6b3f860`. The phase descriptions below preserve the architectural scope and exit contracts; they are not an outstanding TODO list. [The implementation record](ui-overhaul-current-implementation.md) owns the precise #96 surface and retained-CSS exceptions. Post-merge aesthetic work is tracked by the [visual refresh roadmap](ui-visual-refresh-roadmap.md).

The original planning base was PR #94 at `279430d799b78add3a0885168e011de0b0d47ff8`. Implementation reconciled its final main merge `fd5659e77671cb814393024c1baa2e0cba9a59d7`. The historical inventory is not a frozen manifest of today's source.

## Target architecture

```text
feature/domain UI
  -> oStudy semantic UI facades and domain components
    -> owned shadcn source under src/ui/vendor/shadcn
      -> Base UI behavioral primitives + Tailwind CSS v4
        -> oStudy semantic design tokens and root capability state
          -> DOM/CSS in the Tauri WebView
```

Feature components may use Tailwind for local presentation, but not Base UI/vendor imports. Tokens, AppearanceProvider, platform capabilities, controllers and native domain policy remain authoritative. Source copies require reviewed upstream updates, not blind CLI overwrites.

## Cross-phase rules

1. Retain the exact known head/tree while working. Refresh after publication or a genuine concurrent change, not after every truncated response.
2. Migrate a bounded subsystem, publish it and qualify the concrete result before accumulating another independent candidate.
3. Keep legacy CSS Modules interoperable during migration; both old and new presentation consume the same semantic tokens.
4. Preserve public UI behavior before changing APIs. Do not mix scheduler/provider/data changes into renderer replacement.
5. Run the smallest relevant tests first. Repair their actual failure rather than restarting the project investigation.
6. Finish documentation, cleanup and source review before freezing a candidate for expensive Linux/Android qualification.
7. Never put shadcn/Base UI/Tailwind concepts into Rust persistence, IPC, provider policy, retrieval, scheduling or document processing.
8. Preserve accessible labels/focus, keyboard and touch behavior, enlarged text, coarse input, reduced motion/materials, forced colors, safe areas, Android IME and Back.
9. Judge the intended visual hierarchy, not only functional continuity. Keeping obsolete screen CSS over new controls is not the completed visual migration.

## Phase 0 — inventory and architecture

**Recorded.** Establish the architecture decision, PR #94-aware affected-surface inventory, staged roadmap, documentation index, acceptance criteria and non-goals. Its initial documentation-only child branch was subsequently reconciled with merged PR #94 for implementation; that historical exit condition did not prohibit the later source work requested in PR #96.

## Phase 1 — build foundation

**Implemented.** Tailwind/Vite integration, shadcn configuration and aliases, exact dependency/lock graph, small class-merge utility, source detection, licenses and initial boundary checks. No CDN/compiler is needed at runtime. Preflight is not imported; utilities are namespaced to avoid existing-class collisions.

**Acceptance:** frozen dependency installation and frontend/native builds; no accidental reset effects on unmigrated content. Build success is recorded for its actual tested SHA.

## Phase 2 — token and capability bridge

**Implemented.** Generate semantic background/foreground/card/popover/primary/secondary/muted/accent/destructive/border/input/ring and application state aliases from oStudy tokens. Preserve light/dark/console/synthwave parity and expose root input/motion/material/contrast capabilities. Existing short aliases do not change meaning, and no parallel dark provider is introduced.

**Acceptance:** compiled utilities resolve correctly under all themes/capabilities; manual visual contrast/focus remains required.

## Phase 3 — low-risk primitive proof

**Implemented for the selected proof set.** Button, Badge and Skeleton established stable oStudy facades over copied source, semantic styling, refs/forms/loading/disabled contracts and import ownership. Subsequent phases migrated Panel/Surface and Progress. Do not install unused candidates merely to fill a catalog.

**Acceptance:** representative production callers use shared components; old exclusive styles are removed; behavior and theme regressions are covered.

## Phase 4 — forms and Settings

**Implemented.** Shared TextField/Input, Textarea, Checkbox, Switch, RadioGroup, Slider, NativeSelect, Disclosure, Tabs and field grouping support the existing required control set. NativeSelect deliberately retains the OS picker; add a richer combobox only when a real workflow needs it.

**Product scope:** Appearance, Review/Scheduling, AI providers/default task models, Diagnostics, Backup/Storage, DeviceSync, and Template management. Preserve credential ownership, probe/request identity, dirty/discard/delete handling, installed fonts, scope dependencies and safe native operations.

**Acceptance:** migrated live owners use shared controls/Tailwind rather than ad-hoc native controls or old generic button classes. FormField keeps label/description/error associations. Test keyboard, disabled states, live-region errors, provider assignments, sync category dependencies and compact/IME behavior. Template rich content remains editor-owned.

## Phase 5 — overlays and adaptive interaction

**Implemented.** Dialog, confirmation/alert, Sheet, menu/overflow and tooltip behavior use Base UI behind oStudy adapters, with one application Back/parent-depth policy. Compact overflow actions become a sheet; desktop menus use popover positioning. Native selectors do not need a second popup implementation.

**Acceptance:** Escape, outside pointer, nested dialogs, initial/final focus, scroll locking, busy/non-dismissible state, safe areas and Android Back/IME. InlineWorkspace remains nonmodal. Guided tutorial spotlight geometry is custom but its ordinary actions use shared controls. New context-menu/combobox widgets are added only for actual consumers, not as unused implementations.

## Phase 6 — feedback, status and transient UI

**Implemented.** Shared Base UI notifications, notices, progress, skeletons, badges/chips, empty states and error/retry/cancel presentation. The existing notification API and durable operation boundaries remain application-owned. Absolute Undo expiration remains authoritative even when a toast is hovered or its action is busy.

**Acceptance:** one status vocabulary; successful commits cannot become retryable failures merely because a following refresh/navigation fails. Unknown totals do not produce invented progress. Reduced motion and status/alert semantics remain correct. Specialized inline rich progress keeps its content contract.

## Phase 7 — shell and navigation

**Implemented.** AppFrame, labeled Sidebar, compact app bar/bottom navigation, PageHeader/PageScaffold, common toolbars and responsive action groups. The visual system uses inset content surfaces, clearer hierarchy and semantic active-state treatment.

**Acceptance:** main destinations and checkpoint/dirty-guard contracts remain stable. Tutor stays first-class; contextual Tutor does not destroy its origin. Compact navigation stays shallow with a separate Settings action. Fixed/sticky chrome handles enlarged text, safe areas and IME. No vendor router/mobile detector replaces the existing controllers.

## Phase 8 — Flashcards, search, bulk and transfer

**Implemented.** Deck/folder library, archived/unsorted presentation, deck detail, shared card browser, Search/Filter, tags, selection actions, bulk entry/tagging/move-copy/delete/schedule/results, transfer/import receipts, and selected-deck Practice setup.

**Keep application-owned:** FTS/query semantics, note-versus-presentation identities, visible/all-matching selection, folder mutation revisions, drag geometry, scheduler requests, native transport, import validation and managed assets. Retain keyboard alternatives to dragging. Anki completion is a receipt, never a second import preview/submit.

**Acceptance:** explicit scheduled/extra mode, stale setup snapshots rejected, duplicate starts prevented, source/card editor state survives layout changes, selection and bulk operations retain identity. Ordinary feature controls use shared components; pointer geometry and the QR quiet zone retain documented CSS. The old unmounted DecksScreen and its exclusive styles/tests were retired in Phase 15 after transferring their meaningful coverage to the live routes, transfer dialog and controllers.

## Phase 9 — card editor chrome

**Implemented.** The migration covers rich editor toolbar, menus, formatting selectors, popovers, media/math dialogs, tags, metadata, template authoring tools, save/delete status and responsive panels.

Retain Tiptap/ProseMirror schemas, extensions, content validation/serialization, cloze semantics, asset ownership and citation safety. The template-management form's earlier migration does not imply the rich editor itself is complete.

**Exit:** generic editor chrome uses shared components; no vendor types enter durable editor data; formatting, keyboard, focus, drafts, media/math and native Back tests pass. Specialized content-editable CSS is allowed.

## Phase 10 — Documents and Study Guides

**Implemented.** Convert document upload/import/search, library cards, metadata, progress/status/action presentation and source panels. Convert guide library/reader toolbar, sections/disclosures, source/citations, export/practice/contextual Tutor actions, note/override controls and responsive reader layout.

Retain ingestion/OCR, document/guide revisions, source provenance, managed files, learning plans, generation checkpoints and application-lifetime jobs. Learner notes and overrides remain distinct from generated base revisions.

**Exit:** ordinary chrome is migrated; source/citation and job cancellation semantics remain; reading/draft state survives responsive and overlay transitions; no accidental restoration of redundant regeneration/revision controls.

## Phase 11 — Analytics

**Implemented.** Convert filters, date/deck controls, metric surfaces, tabs, chart containers, responsive layout, loading and empty states. Keep Chart.js and existing aggregation/difficulty-series semantics; adopting a UI library does not justify changing chart engines.

**Exit:** accessible filters/data alternatives and contained responsive charts; visualization configuration stays independent of vendor UI.

## Phase 12 — active Practice and contextual hints

**Implemented.** Convert the active study shell, question/answer surface chrome, reveal/progress/status UI, grading presentation, action menus, completion/results and contextual hint presentation. Phase 8 setup is already migrated but is not the whole active study experience.

Retain session snapshots, submission identity, scheduler semantics, shortcuts, intervals, reveal state, exit guards and grading meaning. Grade controls remain domain components over shared Button internals.

**Exit:** opening/closing Tutor does not reset the card/session or reveal hidden answers; keyboard, touch, native Back/IME and session restart paths pass.

## Phase 13 — Tutor workspace

**Implemented.** Convert transcript/messages, composer, conversation/material selectors, multi-source context and memory controls, preparation states, citations/source views, conversational question feedback and retry/cancel presentation. Use the final PR #94 implementation, not the obsolete Quiz me design.

Keep retrieval, source revisions, provider calls, consent/privacy, persistence, private question criteria and operation identity outside rendering. General chat remains source-optional; saved conversations remain readable offline.

**Exit:** contextual and top-level Tutor share the visual vocabulary; hint restrictions and focus/Back/return behavior survive; no source preview or hidden-answer exposure through UI.

## Phase 14 — remaining ordinary CSS migration

**Implemented.** Classify each remaining CSS Module: ordinary styling, reusable pattern, specialized content/platform geometry, animation or obsolete rule. Move ordinary styling to semantic Tailwind; move shared behavior to its actual owner; retain dedicated CSS when descendants, pseudo-elements, document/print or native geometry are clearer there.

**Exit:** no unexplained generic modules or parallel form/button vocabulary remains. Remove a module only when all imports/callers are migrated and tests cover the replacement. Do not blindly convert selectors or arbitrary computed geometry.

## Phase 15 — cleanup and stabilization

**Completed in PR #96.** The legacy class maps and duplicate deck harness were retired, 16 specialized Modules retained checked owners, current documentation was reconciled, and exact-head Linux/Android automated qualification passed before merge. Human visual/device review remained intentionally distinct from source/automation completion, and the later visual refresh is free to intentionally change that appearance.

Remove obsolete renderers, exclusive tests/styles and legacy shared class maps after their last production consumer migrates. Replace old regression harnesses with equivalent live-route coverage before deleting them. Remove temporary aliases and review final utility-layer placement. Strengthen semantic-color/native-control/vendor/dynamic-class boundaries without banning legitimate content/platform CSS.

**Exit achieved:** vendor replacement primarily affects `src/ui`, not business logic. Current docs and actual source agree on the #96 architecture. Required platform qualification passed on its frozen final head; subjective visual acceptance was explicitly separate. No unsupported measured speedup or defect-rate claim was made.

## Validation and visual acceptance

The migration used this validation order:

```text
focused behavior tests
  -> migration-specific policy/CSS compilation
    -> static
      -> fast
        -> targeted native tests for the changed boundary
          -> frozen full Linux/Android qualification
            -> human theme/layout/device acceptance
```

The same resource discipline remains current: use maintained wrappers from [the command runbook](../agents/COMMAND-RUNBOOK.md), do not rerun terminal suites for metadata-only updates, and never treat a displayed command or ancestor result as current-head qualification.

The committed Stage-0 visual evidence predates #96 and remains historical provenance. A later aesthetic redesign should capture a separate exact-head candidate and only establish a new baseline after intentional review.

## Branch sequence and rollback

PR #96 is merged and no longer an active development branch. Independent work starts from authoritative `main` on a new short-lived branch/PR; the active post-migration visual work is described by [ui-visual-refresh-roadmap.md](ui-visual-refresh-roadmap.md). Never force-push or discard stacked work merely to simplify history.

Each bounded renderer increment is reversible through a normal source revert. Tokens and semantic facades stay stable, so reverting presentation does not require reverting unrelated Tutor/provider/domain work or modifying user data. Publication, qualification, manual acceptance and merge remain distinct facts.
