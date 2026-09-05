# UI architecture

The UI is one shared application across Linux and Android, with capability-oriented platform integration and responsive presentation. The [comprehensive architecture](overview.md) explains the full system; this reference owns frontend composition, interaction, state, styling, and current workspace contracts.

## Responsibilities and dependency direction

Screens compose controllers, typed read models, and shared UI. Controllers coordinate asynchronous domain reads/mutations, workflow identity, and reconciliation. `src/ipc/` owns direct native command invocation. `src/platform/` owns OS-facing frontend adapters. Shared `src/ui/` owns reusable presentation and must not import IPC, feature controllers, or Tauri APIs.

```text
feature screen → controller → typed IPC → native domain
       │              │
       └── shared UI  └── platform capability adapter
```

The diagram shows call/import responsibility, not an assertion that every screen is already a pure view. General controllers are in `src/controllers/`; focused deck-library hooks/models are colocated with that feature. Some document and shell orchestration remains concentrated. Refactor by responsibility with behavior tests, not by inventing a new ownership layer or imposing arbitrary file sizes.

Rust owns durable content, validation, transactions, schedules, history, source processing, and provider policy. View-specific projections can be efficient without making React's layout the data model. No screen performs SQL or contacts a provider directly.

## Root composition and state

`src/main.tsx` composes the application providers and imports one global foundation stylesheet. `src/App.tsx` renders lazily selected feature roots. `useAppController` owns the shell's navigation and active-workflow state, dirty guards, practice launch/exit, and checkpoint integration.

Visible destination state and an active Practice workflow are separate. A contextual launch is a typed semantic request, not a UI tab selection. Consuming the one-shot launch payload must not unmount the active session. Exit guards run before same-destination navigation can short-circuit.

Independent reads use independent request lanes. An obsolete success or error must not replace current state. Durable mutation completion is not subject to the same stale-read cancellation rule. A committed native operation stays successful when a following list refresh fails; controllers report reconciliation separately and preserve supported retry identity.

## Current destinations

| Area | Current contract |
| --- | --- |
| Main navigation | Flashcards, Study Guides, Documents, Analytics, Settings |
| Compact navigation | First four directly in the bottom bar; Settings gear in the upper-right app bar; no More menu |
| Flashcards | Decks and Search/Filter; compact Search label refers to the same workspace |
| Templates | Full normal Card templates tab in Settings; lazy activation and retained editor state |
| Transfers | Contextual deck import/export; whole-library recovery and sync in Backup/Sync |
| Practice | Internal contextual session, not standing navigation |
| Study Guide reader | Inline workspace preserving main navigation, not a modal |

Selecting Flashcards or Decks returns to the library root, while an open deck's All decks action does the same locally. Selecting Study Guides returns to its library. Unsaved content still requires save/discard protection.

The native OS supplies desktop window chrome. The previous frontend maximize/full-screen adapter and duplicate in-app controls are not part of current ownership. Compact width changes responsive layout, not a replacement window manager.

## Restoration and compatibility

The schema-v2 device-local checkpoint records a destination separately from an optional actual resumable-session hint and logical return target. Native SQLite owns session progress and schedules; the checkpoint is not a serialized React store or a backup of content.

Supported schema-v1 input normalizes through native code. A real old session can recover; setup-only Practice state does not masquerade as a running session. Restored transitions use new request/transition identities. A removed deck can fall back to the library without inventing missing content.

Legacy Templates/Flashcards route values remain narrow compatibility readers. They do not require obsolete visible tabs. Selection is transient and not part of the checkpoint. See [ADR 0050](../decisions/0050-versioned-ui-checkpoint-workflow-restoration.md).

## Flashcards library and interaction

The library separates organization, selection, drag-session feedback, reorder policy, and view composition. Empty state is conditional: no active/archived decks means one create-deck action and a still-available import affordance. Archived-only content remains accessible. Folders use a `+`; Unsorted appears only when populated. Collapse/expand and select/deselect appear only when their scopes are meaningful.

Row activation, selection, menus, text selection, and dragging are different intents. A pointer drag begins only from a non-interactive surface after its activation threshold. Native state is not mutated for hover, scrolling, cancellation, invalid targets, or a no-op drop. Valid drop feedback shows a semantic destination and insertion boundary.

The same organization operations support keyboard reorder controls and the selected Move sheet. Dragging is not the only way to organize material. Optimistic organization commits against a library revision; conflict reloads authoritative data rather than silently overriding a newer order.

Selection survives relevant in-process navigation only for unchanged eligible entries. Lifecycle/revision changes prune stale selections. An empty active deck can still be managed but cannot start practice. Mixed active/archived selections explain which operations apply to which subset.

Deck detail composes its own native controller, header actions, card browser, and contextual transfer dialog; it does not require mounting the old all-in-one library component. Shared styling that remains imported by the detail route is live even when its filename reflects an earlier owner.

## Authoring and Search

Deck detail owns Add flashcard and Bulk add with a fixed destination. Cross-library Search is a management/query workflow, not a second card-creation screen. Current common query/scope/sort controls stay visible; advanced filters open contextually and active chips expose the scope.

CardNote editing and CardPresentation listing/study remain distinct. Select visible and select all matching resolve different identity sets. Note-level bulk actions account for forward/reverse presentations sharing one source. Stable tag IDs, not display text, define current links from Analytics into Search.

The editor adapter exposes supported commands/capabilities without passing a Tiptap instance into feature code. Durable drafts and canonical commits have separate lifecycles. Full rich storage capability does not imply every current feature UI exposes every formatting tool: the guide reader's note/override entry is currently plain text backed by rich storage. See [rich-document editor](rich-document-editor.md).

## Study Guides and Documents

StudyGuidesScreen is the bounded paginated library owner. It handles search/sort/open and contextual deletion with useStudyGuidesController. Opening one guide hydrates that reader rather than every summary row. The active reader remains named StudyGuideDialog internally but renders an InlineWorkspace with its own close action, contents/article layout, notes/overrides, linked-card practice, and PDF export.

Internal revision badges and standing regeneration controls are not ordinary reader UI. Generation belongs to the retained document's action. The initial generation action changes into an explicit stop control while its task is active. A revisioned generated base is distinct from learner-owned notes and section overrides; conflicts remain visible rather than silently lost.

DocumentsSection gates the native document capability and still uses DocumentsDevelopmentStub as a real unavailable-capability fallback. That component is not dead simply because a configured Linux installation does not show it. DocumentsScreen coordinates current task UI over native document services; do not treat it as authority for parser, provider, or storage policy.

## Overlay lifecycle

OverlayProvider owns the dialog/sheet/menu stack, including parent/depth ordering. Only the top layer consumes Escape, platform Back, backdrop/outside dismissal, and focus restoration. Modal primitives establish accessible title/description relations, initial focus, forward/reverse Tab containment, inert/hidden background, and document-scroll locking until the final modal closes.

Busy/non-dismissible behavior is explicit. Compact overlay layouts account for dynamic viewport, safe areas, and keyboard insets. Features use the shared Dialog/Sheet/Menu/confirmation/overflow primitives, including the narrow compatibility Modal wrapper; they do not hand-write ordinary `window.confirm()`, focus traps, or another dialog stack.

An inline workspace is not forced into that modal policy merely because it is large. Browser `beforeunload` and native active-document close protection remain different mechanisms: a React overlay cannot guarantee a completed decision during all shutdown paths.

## Styles, tokens, and typography

The only global CSS entry imported by main is `src/ui/foundations/index.css`. It establishes cascade layers, token foundations, base typography, document/root sizing, platform safe-area variables, accessibility utilities, and reduced-motion behavior. New feature/component styling uses CSS Modules, not global stage-named sheets or import-order overrides.

Reference, semantic, and component token sources generate the committed theme artifacts. Themes change values, not feature selectors. Use semantic surface/text/border/emphasis/status roles and named z-index layers; do not hard-code theme-specific colors or arbitrary stacking numbers in feature code.

Interface and card font/scale preferences are independent. IBM Plex Sans is a locally provisioned packaged default with a system fallback. An installed app does not request runtime web fonts; a fresh source build may need to provision uncached assets. Explicit user font choices remain authoritative. Native PDF export owns its own print/font contract.

## Responsive and accessible presentation

Layout responds to available viewport/container space and input/accessibility capabilities. Wide layouts use a full sidebar and useful multi-column content. Medium remains a labeled-sidebar mode with denser secondary layout. Compact uses four direct bottom destinations, an app-bar Settings action, stacked content, and appropriate sheets.

Keep reading measures bounded where unlimited lines would be difficult to scan. Tables can scroll within their own region rather than causing document-level overflow or tiny text. Preserve visible focus, semantic labels, initial/error announcements, topmost dismissal, keyboard alternatives, and touch-sized targets. Respect enlarged text and safe-area/IME insets for fixed/sticky actions.

Motion/materials/artwork preferences are separate. Reduced motion removes nonessential spatial effects without removing state changes or progress. Simplified materials preserve hierarchy without blur. Educational images and functional status/icons are not decorative artwork and must not disappear with that toggle.

## Help, tutorials, and feedback

HelpProvider uses one topic registry for short explanations and detailed touch-friendly help. Tutorial opt-in defaults off and a module starts only through an explicit action. An active tour may observe its target for positioning, but route entry or target appearance never authorizes auto-start. Disabling tutorials closes active guidance.

Tutorial progress is versioned device-local presentation state, not a portable personal-library session. Semantic feedback events notify the experience layer after meaningful actions; domain controllers never depend on an effect completing or a device offering haptics/audio.

## Testing and cleanup

Test active owners: controllers/request lanes, route reset/dirty guards, overlays/focus/Back, library empty/populated/archive states, pointer and keyboard movement, guide listing/reader behavior, and contextual transfer success/reconciliation. Native WebDriver and Android instrumentation cover integration that mocks cannot establish.

Historical screenshot/performance records remain provenance, not current UI requirements. Remove unused alternate components with their exclusively owned styles/tests while preserving active behavior coverage. Keep versioned routes/checkpoints and actual capability fallbacks. [The cleanup audit](../development/documentation-and-cleanup-audit.md) records the distinction.

Structural enforcement includes IPC, route/smoke, style ownership, token usage, platform and UI policy scripts. None replaces actual compact-width, native, keyboard, touch, and representative manual acceptance. Use [Testing and CI](../development/testing-and-ci.md) and [local handoff](../development/local-testing-and-handoff.md).
