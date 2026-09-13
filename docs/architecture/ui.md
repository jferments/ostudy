# UI architecture

One React presentation serves Linux and Android through capability-oriented platform adapters. [The comprehensive architecture](overview.md) covers the system; this reference owns current UI composition, interaction, state and styling. [The UI implementation record](../development/ui-overhaul-current-implementation.md) and [roadmap](../development/ui-overhaul-roadmap.md) record the implemented Phase 0–15 ownership and distinguish source completion from automated and human acceptance.

## Responsibilities and dependency direction

```text
feature screen -> controller -> typed IPC -> native domain
       |              |
       v              +-> platform capability adapter
  oStudy UI facade -> owned shadcn source -> Base UI + Tailwind
```

Screens compose controllers, typed read models and semantic UI. Controllers own asynchronous domain reads/mutations, request identity and reconciliation. `src/ipc` owns direct native invocation; `src/platform` owns OS-facing frontend adapters. Shared `src/ui` must not import IPC, feature controllers or Tauri APIs.

This diagram is an ownership rule, not a claim that every old feature is already a pure view. General controllers are in `src/controllers`; focused library hooks/models are colocated with that feature. Refactor with behavior tests rather than arbitrary file-size targets.

Rust owns durable content, validation, transactions, schedules, history, processing and provider policy. Efficient UI projections do not make React's layout the data model. No screen performs SQL or contacts an AI provider directly.

## Root composition and state

`main.tsx` composes providers and imports one foundation stylesheet. `App.tsx` renders lazy feature roots inside AppFrame/AppContent. `useAppController` owns navigation, active workflows, dirty guards, Practice launch/exit and checkpoint integration. TutorProvider shares top-level/contextual Tutor state; DocumentJobsProvider keeps document jobs alive across navigation.

Destination state and an active Practice workflow are separate. Contextual launch is a semantic request, not a tab selection. Consuming a one-shot payload must not unmount the session. Exit guards run before same-destination short-circuiting.

Independent reads use independent request lanes. An obsolete success/error must not replace current state. Durable mutation completion has different semantics: a committed operation remains successful when a subsequent refresh or navigation fails. Reconciliation is reported separately and supported retry identities are preserved.

## Current destinations

| Area | Contract |
| --- | --- |
| Main navigation | Flashcards, Study Guides, Documents, Tutor, Analytics, Settings |
| Compact navigation | First five in the bottom bar; Settings gear in the upper-right app bar; no More menu |
| Flashcards | Decks and Search/Filter; compact Search is the same workspace |
| Templates | Card templates tab in Settings; lazy activation and retained editor state |
| Transfers | Contextual deck import/export; library recovery and device sync in Backup/Sync |
| Practice | Internal contextual session, not a standing navigation destination |
| Study Guide reader | Inline workspace preserving navigation, not a modal |
| Tutor | Shared top-level/contextual controller; general chat does not require source material or embeddings |

Selecting Flashcards/Decks returns to the library root. All decks returns from deck detail. Selecting Study Guides returns to its library. Unsaved content retains save/discard protection. The native OS owns desktop window chrome; the frontend does not reintroduce a duplicate maximize/window manager.

## Restoration and compatibility

The schema-v2 device-local checkpoint separates destination from a real resumable-session hint and logical return target. Native SQLite owns session progress; the checkpoint is not a serialized React store or a content backup.

Supported schema-v1 data normalizes through native code. Setup-only Practice does not masquerade as a running session. Restoration uses fresh transition identities; deleted targets fall back without inventing missing content. Legacy route values are compatibility readers, not obsolete visible tabs. Selection remains transient. See [ADR 0050](../decisions/0050-versioned-ui-checkpoint-workflow-restoration.md).

## Flashcards library and interaction

The library separates organization, selection, pointer session feedback, reorder policy and view composition. An empty library shows one create action plus import; archived-only data remains accessible. Folder, Unsorted, collapse/expand and selection controls appear only for meaningful scopes.

Row activation, selection, menus, text selection and dragging are distinct intents. A pointer drag begins on non-interactive surfaces after its threshold. Hover, canceled or no-op drops do not mutate native state. Valid feedback identifies the destination and insertion boundary. Keyboard reorder and Move sheet alternatives remain.

Organization mutations use a library revision. Conflicts reload authoritative state instead of overwriting newer order. Selection survives relevant in-process navigation only for eligible unchanged rows; missing/stale identities are pruned. Empty decks cannot practice; mixed archived/active selection explains its applicable operations.

DeckDetailRoute owns the production deck header/actions, shared browser and source footer. The old all-in-one DecksScreen and its exclusive styles are removed; active route/controller/transfer regressions cover their meaningful contracts. The same browser instance stays mounted when its header hides for editor mode; changing presentation must not discard an unsaved card.

## Authoring, Search and transfers

Deck detail owns Add flashcard/Bulk add with a fixed destination. Cross-library Search manages/query-scopes existing content, not a second authoring route. Common scope/sort controls remain visible; advanced filters open contextually and active chips expose them.

Source notes and generated card presentations remain distinct. Select visible/all matching resolve different identity sets. Bulk mutations account for forward/reverse cards sharing a note. Tags use stable IDs rather than display names.

The editor adapter exposes supported commands without distributing a Tiptap instance. Durable drafts and canonical commits have separate lifecycles. Editor and template chrome use shared controls; rich content, media/math, ProseMirror descendants and print styling remain specialized. See [rich-document editor](rich-document-editor.md).

Transfer UI presents native import/export/share operations. Anki completion is a post-commit receipt; refresh/navigation failure cannot permit duplicate importing. Unknown progress remains indeterminate. Sync UI keeps sender-selected categories, Documents/Guide and settings/key dependencies, credential availability and distinct USB/LAN flows. Appearance preferences remain device-local.

## Study Guides, Documents and Tutor

StudyGuidesScreen owns bounded paginated summaries; opening one guide hydrates its reader. StudyGuideDialog is internally named a dialog but uses InlineWorkspace with contents/article, learner notes/overrides, practice and PDF export.

Generated revisions differ from learner-owned notes/overrides. Generation belongs to the retained document's action; its single active control becomes Stop. Internal revision badges and redundant regeneration controls are not ordinary reader UI. Conflicts remain visible.

DocumentsSection retains the actual unavailable-capability fallback. DocumentsScreen displays application-lifetime controller/native state; a route unmount cannot cancel durable jobs. Parsing, OCR and provider/storage policy remain native/domain-owned.

Tutor keeps final PR #94 multi-source selection, safe Markdown, source-optional conversation, private question criteria and hidden-answer/citation safeguards. Questions use Send; do not restore Quiz me. The Documents, guide reader, Analytics, active Practice and Tutor layouts now use the same scaffold, actions, fields, metrics and feedback. Their controllers and rich-content models remain independent. The inactive Practice face is inert, and Tutor submission remains composition-aware.

## Overlay lifecycle

Base UI owns browser focus containment/restoration, keyboard interaction, popup positioning, modality and scroll locking through the shared Dialog/Sheet/Menu/Tooltip implementations. OverlayProvider owns application parent/depth registration and native Back dispatch; it is not a duplicate DOM focus trap.

Only the applicable top layer dismisses. Busy/non-dismissible state, alert-dialog pointer behavior, compact safe-area/IME layout and explicit initial focus remain contracts. Menus unmount before an action opens a dialog. Features use semantic confirmation/overflow APIs rather than ordinary window.confirm or handwritten backdrops.

An inline workspace stays nonmodal. Browser beforeunload/native active-document shutdown protection remain distinct from an interactive React confirmation, which cannot guarantee completion during every shutdown path.

## Styles, tokens and typography

`src/ui/foundations/index.css` is the single global entry. It defines cascade layers, root sizing, typography, token foundations, platform insets and accessibility capabilities. Tailwind compiles namespaced `tw:` utilities through the Vite plugin without Preflight or a second reset.

All ordinary feature presentation uses Tailwind and shared oStudy components. Utilities intentionally occupy the components layer below retained specialized geometry/content overrides; the final precedence audit does not introduce a global important flag. Partial separators clear their other border edges explicitly. Do not use global stage sheets or important flags to hide precedence defects.

Reference/semantic/component token sources generate committed artifacts. Themes change values, not feature selectors. The Tailwind adapter maps semantic roles to oStudy variables without overwriting old short aliases or adding a separate dark provider. Utility names are complete source literals, not interpolated fragments.

Stable oStudy facades wrap owned shadcn source in `src/ui/vendor/shadcn`; only that directory imports Base UI. Feature code may use Tailwind but not vendor types/APIs. Current primitives include forms, tabs, cards/panels, badges/chips, progress, notifications, menus and dialogs. Customized copies need reviewed upstream refreshes; they do not update automatically.

Interface/card font and scale preferences remain independent. Packaged IBM Plex Sans has a system fallback; a fresh source build may provision uncached fonts, but the installed app needs no runtime font service. Explicit user selections remain authoritative; native PDF export has its own print/font contract.

## Responsive and accessible presentation

Wide/medium use a labeled sidebar and inset content; compact uses five bottom destinations and Settings in the app bar. Layout follows viewport and capabilities, not OS-name conditionals. Bounded reading measures, intrinsic wrapping and contained table scroll prevent oversized pages or unreadably tiny controls.

Preserve labels, focus, announcements, topmost dismissal, keyboard alternatives, touch targets, enlarged text and safe-area/IME insets. Motion/material/artwork preferences are distinct. Reduced motion removes nonessential animation without hiding progress; simplified materials preserve hierarchy without blur. Educational images and functional status/icons are not decorative artwork.

## Help, tutorials and feedback

HelpProvider uses one registry and shared Sheet presentation; labels/help triggers remain associated without nesting buttons in labels. Tutorials default off and require an explicit user action. Their spotlight target geometry remains custom, while ordinary actions and text use shared UI. Disabling tutorials closes active guidance.

Tutorial progress is versioned device-local state, not a portable study session. Semantic feedback follows completed actions; domain success never depends on animation/haptics or a toast. Base UI renders notifications while the app retains action identity, absolute Undo deadlines and post-commit recovery semantics.

## Testing and cleanup

Test active controllers, route/dirty guards, overlays/Back, library empty/populated/archive states, drag/keyboard movement, authoring identity, imports and sync recovery. CSS compilation discovers the entire production frontend and checks emitted utilities. The AST-based ownership guard rejects raw generic controls outside the vendor layer, with only disabled sanitized Markdown task markers exempt. Native WebDriver/Android tests establish integration not proven by mocks.

Retain dedicated pointer geometry, QR quiet-zone, guided spotlight, inline rich progress, native root-height and document/editor/print CSS with explicit ownership. The 16 remaining Modules are registered with rationales in `scripts/ui/presentation-policy.mjs`; the verifier rejects missing, dead or unregistered owners. Legacy class maps and the duplicate deck harness are removed. Their creation, archive/restore, Undo and transfer contracts are tested on the live implementation. Do not delete real versioned compatibility readers or unavailable-capability fallbacks.

Historical screenshot/performance records are provenance, not current acceptance. Evaluate the intended new appearance and behavioral invariants separately; no speedup is established merely by adopting a UI library. See [testing and CI](../development/testing-and-ci.md), [local handoff](../development/local-testing-and-handoff.md) and [cleanup audit](../development/documentation-and-cleanup-audit.md).
