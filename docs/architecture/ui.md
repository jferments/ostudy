# UI architecture

One React presentation serves Linux and Android through capability-oriented platform adapters. [The comprehensive architecture](overview.md) covers the system; this reference owns current UI composition, interaction, state and styling. PR #96's completed migration is preserved in the [UI implementation record](../development/ui-overhaul-current-implementation.md) and [migration roadmap](../development/ui-overhaul-roadmap.md). The active post-migration aesthetic work is tracked separately in the [visual refresh roadmap](../development/ui-visual-refresh-roadmap.md).

## Responsibilities and dependency direction

```text
feature screen -> controller -> typed IPC -> native domain
       |              |
       v              +-> platform capability adapter
  oStudy UI facade -> owned shadcn source -> Base UI + Tailwind
```

Screens compose controllers, typed read models and semantic UI. Controllers own asynchronous domain reads/mutations, request identity and reconciliation. `src/ipc` owns direct native invocation; `src/platform` owns OS-facing frontend adapters. Shared `src/ui` must not import IPC, feature controllers or Tauri APIs.

This diagram is an ownership rule, not a claim that every feature is a pure view. General controllers are in `src/controllers`; focused library hooks/models are colocated with that feature. Refactor with behavior tests rather than arbitrary file-size targets.

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

Supported schema-v1 data normalizes through native code. Setup-only Practice does not masquerade as a running session. Restoration uses fresh transition identities; deleted targets fall back without inventing missing content. Legacy Cards/Templates checkpoint and Flashcards-workspace values are accepted only at their compatibility boundaries and normalize to current destinations before reaching live React route state. The current application-section vocabulary is therefore exactly Flashcards, Study Guides, Documents, Tutor, Analytics and Settings; the live Flashcards workspace is exactly Decks/Search. Selection remains transient. See [ADR 0050](../decisions/0050-versioned-ui-checkpoint-workflow-restoration.md).

## Flashcards library and interaction

The library separates organization, selection, pointer session feedback, reorder policy and view composition. An empty library shows one create action plus import; archived-only data remains accessible. Folder, Unsorted, collapse/expand and selection controls appear only for meaningful scopes.

Row activation, selection, menus, text selection and dragging are distinct intents. A pointer drag begins on non-interactive surfaces after its threshold. Hover, canceled or no-op drops do not mutate native state. Valid feedback identifies the destination and insertion boundary. Keyboard reorder and Move sheet alternatives remain.

Organization mutations use a library revision. Conflicts reload authoritative state instead of overwriting newer order. Selection survives relevant in-process navigation only for eligible unchanged rows; missing/stale identities are pruned. Empty decks cannot practice; mixed archived/active selection explains its applicable operations.

DeckDetailRoute owns the production deck header/actions, shared browser and source footer. Practice remains the only permanently exposed ordinary deck action; merge, export/save, delete, and archive/restore live in the shared overflow hierarchy. Merge opens a destination sheet, captures one operation identity and both deck revisions/counts, and retries an ambiguous native reply with the same request rather than issuing a second move. The old all-in-one DecksScreen and its exclusive styles are removed; active route/controller/transfer regressions cover their meaningful contracts. The same browser instance stays mounted when its header hides for editor mode; changing presentation must not discard an unsaved card.

## Authoring, Search and transfers

Deck detail owns Add flashcard/Bulk add with a fixed destination. Cross-library Search manages/query-scopes existing content, not a second authoring route. Common scope/sort controls remain visible; advanced filters open contextually and active chips expose them.

Source notes and generated card presentations remain distinct. Select visible/all matching resolve different identity sets. Bulk mutations account for forward/reverse cards sharing a note. Tags use stable IDs rather than display names.

The editor adapter exposes supported commands without distributing a Tiptap instance. Durable drafts and canonical commits have separate lifecycles. Editor and template chrome use shared controls; rich content, media/math, ProseMirror descendants and print styling remain specialized. See [rich-document editor](rich-document-editor.md).

Transfer UI presents native import/export/share operations. Anki completion is a post-commit receipt; refresh/navigation failure cannot permit duplicate importing. Unknown progress remains indeterminate. Sync UI keeps sender-selected categories, Documents/Guide and settings/key dependencies, credential availability and distinct USB/LAN flows. Appearance preferences remain device-local.

## Study Guides, Documents and Tutor

StudyGuidesScreen owns bounded paginated summaries; opening one guide hydrates its reader. StudyGuideDialog is internally named a dialog but uses InlineWorkspace with contents/article, learner notes/overrides, practice and PDF export.

Generated revisions differ from learner-owned notes/overrides. Generation belongs to the retained document's action; its single active control becomes Stop. Internal revision badges and redundant regeneration controls are not ordinary reader UI. Conflicts remain visible.

DocumentsSection retains the actual unavailable-capability fallback. DocumentsScreen displays application-lifetime controller/native state; a route unmount cannot cancel durable jobs. Parsing, OCR and provider/storage policy remain native/domain-owned. Document, image-set and collection flashcard workflows expose learner-authored per-run guidance through the same semantic field, but React owns only the draft: the native request captures the normalized instructions, binds them to request identity, and keeps the current run immutable. A document-collection workspace reads StudyTarget status locally and presents one compact Create/Update study materials action plus separate Flashcards/Study Guide states. A ready collection Flashcards state opens its ordinary deck. Guided collection flashcards remain a selection over eligible, already-grounded document cards rather than a React-side rewrite or second authoring pipeline. Ambiguous responses retain the same durable operation identity and captured instructions; a later refresh failure is reported separately from a committed bundle. Collection-guide reading remains the 104-E guide-viewer boundary rather than a second ad-hoc reader.

Tutor keeps final PR #94 multi-source selection, safe Markdown, source-optional conversation, private question criteria and hidden-answer/citation safeguards. Questions use Send; do not restore Quiz me. The Documents, guide reader, Analytics, active Practice and Tutor layouts use the same scaffold, actions, fields, metrics and feedback while their controllers and rich-content models remain independent. The inactive Practice face is inert, and Tutor submission remains composition-aware.

## Overlay lifecycle

Base UI owns browser focus containment/restoration, keyboard interaction, popup positioning, modality and scroll locking through the shared Dialog/Sheet/Menu/Tooltip implementations. OverlayProvider owns application parent/depth registration and native Back dispatch; it is not a duplicate DOM focus trap.

Every Dialog and Sheet, including confirmations and MetadataDialog, shares one structure: a header with the title, optional description and close button; a body that scrolls with standard padding and spacing; and an `actions` footer that stays visible while the body scrolls, with a rule above it while content continues below. `onSubmit` makes the body and actions one form, so a submit action in the footer submits it without navigating. Widths come from four sizes (small, medium, large, workspace). On compact screens a dialog fills the screen, while sheets and confirmations rise from the bottom edge at their content height; both keep safe-area and IME padding.

Only the applicable top layer dismisses. Busy/non-dismissible state, alert-dialog pointer behavior, compact safe-area/IME layout and explicit initial focus remain contracts. Menus unmount before an action opens a dialog. Feature workflows import the shared Dialog/Sheet owners directly; the retired compatibility Modal wrapper is not another lifecycle. Features use semantic confirmation/overflow APIs rather than ordinary `window.confirm` or handwritten backdrops.

An inline workspace stays nonmodal. Browser beforeunload/native active-document shutdown protection remain distinct from an interactive React confirmation, which cannot guarantee completion during every shutdown path.

## Styles, tokens and typography

`src/ui/foundations/index.css` is the single global entry. It defines cascade layers, root sizing, typography, token foundations, platform insets and accessibility capabilities. Tailwind compiles namespaced `tw:` utilities through the Vite plugin without Preflight or a second reset.

All ordinary feature presentation uses Tailwind and shared oStudy components. Utilities occupy the components layer below retained specialized geometry/content overrides; no global important flag is used. Partial separators clear their other border edges explicitly. Do not use global stage sheets or important flags to hide precedence defects.

Reference/semantic/component token sources generate committed artifacts. Themes change values, not feature selectors. Reference tokens hold theme-independent primitives; each theme file owns its palette (including six subject colors with soft variants, which also feed the chart roles), radius scale, shadow levels and primary-button glow, and may alias its own roles (`radius.button` is a pill except in Console). Surfaces are opaque in every theme, without blur or translucency. Text uses the token type scale (caption, label, small, body, body-large, title-small, section, title, display) with bundled line heights; stock Tailwind text sizes are disabled. Headings are semibold with -0.01em tracking in the theme's heading family (the interface font, or IBM Plex Mono in Console); card content keeps its own font. Control height, touch target and icon sizes are rem-based so the interface scale reaches them.

The interface scale sets the root font size, so every rem-based size follows it while media queries (which resolve rem against the initial size) stay put. Layout breakpoints are defined once (`breakpoint.medium` 760px and `breakpoint.wide` 1180px in the reference tokens) and generate both the `viewportQueries` LayoutModeProvider uses and Tailwind's `compact`, `medium` and `wide` variants; CSS Modules reach the variants through `@reference` and a nested `@variant`, so JavaScript and CSS agree at every width. Stacking uses one layer scale, lowest first: behind, base (the application frame, which keeps every in-app layer below portals), selected, focus, raised, sticky, chrome, floating, drag, then the body-level dialog band (nested dialogs add 2 per depth), popover (menus and tooltips), toast and effects. Numeric z-index values are rejected by the UI theme test. The Tailwind adapter maps semantic roles to oStudy variables without overwriting old short aliases or adding a separate dark provider. Utility names are complete source literals, not interpolated fragments.

Stable oStudy facades wrap owned shadcn source in `src/ui/vendor/shadcn`; only that directory imports Base UI. Feature code may use Tailwind but not vendor types/APIs. Current primitives include forms, tabs, Card composition, Item list objects, ButtonGroup/Toolbar actions, badges/chips, progress, notifications, menus and dialogs. Customized copies need reviewed upstream refreshes; they do not update automatically.

Icons come from one set. `Icon` renders Lucide glyphs vendored as path data on Lucide's 24-unit grid with a 1.9 stroke, behind application names such as `cards`, `study-guide` or `stop`; features pass a name instead of drawing their own inline SVG, and a missing glyph is added to `Icon` from Lucide.

### Stock-first visual rule

`components.json` selects shadcn's Base UI Nova family. For ordinary presentation, the default is to use the owned Nova component's sizing, spacing, hierarchy and standard variants rather than recreating the pre-PR #96 appearance. oStudy remains the authority for semantic colors, Light/Dark/Console/Synthwave themes, fonts, accessibility capabilities and product meaning.

A shared application abstraction should exist because it carries product semantics, cross-platform behavior or reusable interaction policy—not merely because it groups Tailwind classes. Prefer Card for bounded information, ListRow for the rows of a list, Item for a standalone list object, Field/FormField for labels/help/errors, Alert/notice for status, and Toolbar/ButtonGroup for related actions. `Surface` remains a compatibility/product-state boundary while generic consumers migrate away from elevation/material vocabulary.

Card is the one card surface: the theme's large radius (20 px, 4 px in Console), a hairline border and the level-1 shadow, drawn flat on phones, inside another card and inside dialogs or popovers. Its `--card-gutter` custom property (20 px, 16 px on phones, 12 px for small cards) pads the slots, and a screen's own padding classes replace it. The compositions over it keep separate jobs: Panel adds a title, description and actions; Surface adds product state (tone, selection, hover for clickable surfaces, an inset well); StatTile adds a label, a value, an optional StatDelta pill whose arrow and hidden word carry the direction, a hint and a small visual, with StatGrid for rows of tiles. Screens apply Card instead of repeating its radius, border or shadow.

Headings come from two components on the type scale. PageHeader holds the page's `h1` at the title size (34 px, 28 px on phones) with an optional eyebrow, a 16 px description and actions that sit beside the title and wrap below it when the row is narrow; a ResponsiveActionGroup passed as the actions collapses secondary ones into an overflow menu on phones. SectionHeading titles a card or a part of a page at 19 px, or at the 24 px section size for a section that groups several cards, with an optional eyebrow, description and actions. Its level follows the outline rather than the size: `h2` under the page title, `h3` inside a dialog or another section. Panel draws its title with SectionHeading, so a titled card and a titled page section read the same.

SegmentedControl chooses one of two to five short options; its option type accepts no fewer and no more, and longer lists belong in a select or a menu. It is a radio group: Tab enters at the chosen option, arrow keys move and choose, Home and End jump to the ends, and the focus ring follows the visible pill. Each option's button is the whole touch target (`max(44px, 2.75rem)`, more on phones) while its pill sits 4 px inside, as in the mockups. Options that cannot share a row move whole to a second row instead of clipping or squeezing their labels. A null value leaves every option unchecked for a choice made beside the control, such as the custom dates toggle next to the Analytics range presets.

ListRow is one row of a list: an optional leading tile, a title, meta lines and trailing status, figures or actions, which wrap below the text when the row is narrow. ListRowMedia draws the mockups' square tile with an icon or a short format label such as PDF. ListRows puts a hairline between rows and, inside a card, runs them to the card's edges while each row indents by the card's gutter. Comfortable rows (16 px titles) suit a few rich items, such as study guides and a collection's sources; dense rows (14 px titles, tighter padding) suit long lists and editing, as on the Workbench mockup, and today carry the Analytics session and review lists. With `onActivate`, the tile and text become one button that opens the row, so the row's own actions stay separate buttons beside it instead of nesting inside it. Titles and meta wrap rather than truncate. Item remains the standalone list object, such as the model panel on Documents.

Fine-pointer desktop controls follow Nova's compact geometry. Coarse-pointer capabilities expand interactive targets to the application touch minimum without creating a second mobile component tree. Theme differences change semantic values and optional effects, not the information architecture.

Interface/card font and scale preferences remain independent. Current React appearance state consumes `motionPreference`; the native settings reader alone translates historical `animationsEnabled`/`reduceMotion` rows before IPC. Packaged IBM Plex Sans has a system fallback; a fresh source build may provision uncached fonts, but the installed app needs no runtime font service. Explicit user selections remain authoritative; native PDF export has its own print/font contract.

## Responsive and accessible presentation

Wide/medium use a labeled sidebar and inset content; compact uses five bottom destinations and Settings in the app bar. Layout follows viewport and capabilities, not OS-name conditionals. Bounded reading measures, intrinsic wrapping and contained table scroll prevent oversized pages or unreadably tiny controls.

Preserve labels, focus, announcements, topmost dismissal, keyboard alternatives, touch targets, enlarged text and safe-area/IME insets. Motion and artwork preferences are distinct. Reduced motion removes nonessential animation without hiding progress. Surfaces are flat and opaque in every theme, so nothing depends on blur or translucency; the stored material preference is kept for settings compatibility but is no longer shown or styled. Educational images and functional status/icons are not decorative artwork.

## Help, tutorials and feedback

HelpProvider uses one registry and shared Sheet presentation; labels/help triggers remain associated without nesting buttons in labels. Tutorials default off and require an explicit user action. Their spotlight target geometry remains custom, while ordinary actions and text use shared UI. Disabling tutorials closes active guidance.

Tutorial progress is versioned device-local state, not a portable study session. Semantic feedback follows completed actions; domain success never depends on animation/haptics or a toast. Base UI renders notifications while the app retains action identity, absolute Undo deadlines and post-commit recovery semantics.

## Testing and cleanup

Test active controllers, route/dirty guards, overlays/Back, library empty/populated/archive states, drag/keyboard movement, authoring identity, imports and sync recovery. CSS compilation discovers the entire production frontend and checks emitted utilities. The AST-based ownership guard rejects raw generic controls outside the vendor layer, with only disabled sanitized Markdown task markers exempt. Native WebDriver/Android tests establish integration not proven by mocks.

Retain dedicated pointer geometry, QR quiet-zone, guided spotlight, inline rich progress, native root-height and document/editor/print CSS with explicit ownership. The 16 remaining Modules are registered with rationales in `scripts/ui/presentation-policy.mjs`; the verifier rejects missing, dead or unregistered owners. Do not delete real versioned compatibility readers or unavailable-capability fallbacks merely to reduce source counts.

The committed Stage-0 screenshot matrix is historical provenance, not the current visual specification. During an intentional visual redesign, capture an exact-head **candidate** into ignored `.artifacts/` output, review it separately from behavior gates, and only establish a new committed baseline after human acceptance. Never overwrite the historical matrix to make an intentional difference disappear.

See [testing and CI](../development/testing-and-ci.md), [local handoff](../development/local-testing-and-handoff.md), [visual refresh roadmap](../development/ui-visual-refresh-roadmap.md) and [cleanup audit](../development/documentation-and-cleanup-audit.md).
