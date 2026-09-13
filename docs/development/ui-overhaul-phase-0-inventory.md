# UI overhaul Phase 0 affected-surface inventory

Status: Phase 0 planning record. No runtime UI implementation is changed by this assessment.

Assessment base: PR #94 (`agent/semantic-ai-tutor`) at `279430d799b78add3a0885168e011de0b0d47ff8`.

This inventory deliberately uses PR #94 rather than `main` as the product snapshot. PR #94 adds the top-level/contextual Tutor experience and materially expands AI provider/model configuration, so both are first-class migration surfaces.

## Acceptance criteria

| ID | Criterion |
| --- | --- |
| AC-01 | The assessment branch is created directly from the reviewed PR #94 head, and the stacked PR targets `agent/semantic-ai-tutor` rather than `main`. |
| AC-02 | The current UI ownership layers, styling systems, component facades, platform behavior, and verification infrastructure are identified before implementation begins. |
| AC-03 | The affected-surface inventory includes the Tutor and AI provider/model UI introduced or expanded by PR #94. |
| AC-04 | Every major frontend area receives a migration disposition: preserve, wrap/replace renderer, migrate styling, or retain specialized CSS. |
| AC-05 | A staged roadmap defines ordering, dependencies, rollback boundaries, and validation expectations without requiring a big-bang rewrite. |
| AC-06 | Durable architecture records keep oStudy tokens, platform contracts, and semantic UI facades authoritative over third-party defaults. |
| AC-07 | Phase 0 changes documentation only: no dependency, lockfile, build, Tauri, native, database, IPC, or runtime UI changes. |

## Non-goals for Phase 0

This phase does not install Tailwind, shadcn/ui, Base UI, CVA, icons, or animation packages. It does not convert any component or CSS Module. It does not alter Tutor behavior, provider configuration, document processing, scheduling, navigation semantics, persistence, or native platform behavior. It does not refresh visual baselines because no rendered output is intentionally changed.

## Executive assessment

The codebase is better prepared for this migration than its amount of CSS might suggest. The strongest existing architecture should be retained:

- `src/ui/tokens/` is already an application-owned semantic token pipeline with source JSON, generated CSS/TypeScript, and four theme contracts.
- `src/ui/appearance/AppearanceProvider.tsx` already projects theme, motion, material, contrast, pointer/hover capability, scale, and font state into root attributes/variables.
- `src/ui/foundations/platform.css` already owns safe areas, Android IME behavior, reduced motion/transparency, forced colors, and compact touch-target policy.
- `src/ui/primitives/`, `src/ui/overlays/`, `src/ui/patterns/`, and `src/ui/layout/` provide a real seam between product code and generic presentation behavior.
- Existing style/token/owned-class verification scripts and UI visual/performance baseline machinery provide enforcement points for the migration.

The migration should therefore replace implementation beneath existing semantic seams where practical, while using Tailwind directly for feature-local layout/presentation. It should not erase `src/ui` and replace it with `src/components/ui` imports spread across the product.

The largest sources of migration work are different:

1. generic interaction behavior still implemented by oStudy or raw HTML;
2. a broad CSS Module surface across screens/components;
3. shared legacy class ownership still used by some settings/UI code;
4. large feature components where visual markup and workflow logic are interleaved;
5. new PR #94 Tutor/provider surfaces that would otherwise immediately become migration debt.

## Current styling and presentation layers

| Surface | Current owner | Phase-0 disposition |
| --- | --- | --- |
| Design tokens | `src/ui/tokens/source`, `src/ui/tokens/themes`, `scripts/generate-ui-tokens.mjs` | **Preserve and extend.** Add Tailwind/shadcn adapters later; do not replace token authority. |
| Global foundations | `src/ui/foundations/` | **Preserve selectively.** Integrate Tailwind entry point later; keep document/platform/accessibility CSS where semantic CSS is clearer. |
| Appearance/capabilities | `src/ui/appearance/` | **Preserve.** Tailwind consumes root data attributes rather than replacing them. |
| Generic primitives | `src/ui/primitives/` | **Replace renderer incrementally.** Keep stable oStudy APIs when they express product semantics. |
| Overlays | `src/ui/overlays/` | **High-priority renderer migration.** Move generic focus/keyboard/portal behavior to mature primitives while preserving oStudy overlay policy. |
| Layout/patterns | `src/ui/layout/`, `src/ui/patterns/` | **Review case-by-case.** Retain semantic reusable patterns; do not wrap every Tailwind layout utility. |
| Icons | `src/ui/icons/` | **Review in Phase 1/3.** Preserve icon names/domain meaning; renderer/library can change behind facade where useful. |
| Feedback/help/tutorials | `src/ui/feedback/`, `src/ui/help/`, `src/ui/tutorials/` | **Preserve behavior, migrate chrome.** These carry application policy beyond visual styling. |
| Materials/motion/artwork | `src/ui/materials/`, `src/ui/motion/`, `src/ui/artwork/` | **Preserve semantics.** Tailwind may replace ordinary layout/style rules; capability/reduced-motion contracts remain. |
| Feature CSS Modules | components/screens/editor subtrees | **Migrate progressively.** Ordinary chrome/layout to Tailwind; specialized rich-content/platform CSS may remain. |
| Legacy shared classes | `src/ui/styles/classOwnership.ts` and owned class maps | **Transitional debt.** Migrate consumers and then retire only after verified unused. |

## Shared primitive inventory

These are the first renderer seams because many feature call sites can remain unchanged while implementation moves underneath them.

| Current oStudy primitive | Likely target | Notes |
| --- | --- | --- |
| `Button` | shadcn Button/Base UI or native button through shadcn-owned source | Preserve intent/size semantics initially; separate renderer migration from API cleanup. |
| `Badge`, `Chip` | Badge/toggle-like owned primitives | Keep semantic status/tag meaning outside vendor code. |
| `FormField` | oStudy wrapper around shadcn input/control primitives | Preserve label, description, error, help-topic, ARIA relationships. |
| `Progress` | shadcn/Base UI Progress | Preserve operation semantics and accessibility announcements. |
| `SegmentedControl` | Toggle Group/Tabs depending semantics | Do not force one visual primitive onto navigation and value selection indiscriminately. |
| `Skeleton` | Tailwind/shadcn Skeleton | Low-risk proof component. |
| `Surface`, `Panel` | Tailwind/shadcn Card-like composition | Preserve semantic surface hierarchy rather than importing Card throughout features. |
| `Tabs` | Base UI/shadcn Tabs | Preserve hidden/lazy behavior tested by existing contracts. |
| `Tooltip` | Base UI/shadcn Tooltip | High value for keyboard/focus/positioning behavior. |
| `VisuallyHidden` | Base UI utility or retained local helper | Keep whichever form produces the smallest stable accessibility dependency. |
| `InlineNotice`, status styles | Alert/callout-style oStudy primitive | Semantic success/warning/error intent remains application-owned. |

## Overlay inventory

Current overlay infrastructure includes `Dialog`, `Menu`, `MetadataDialog`, `OverflowMenu`, `OverlayProvider`, confirmation context, and supporting overlay CSS/tests.

Migration direction:

- use mature Dialog/AlertDialog/DropdownMenu/Popover/Tooltip/Drawer/Sheet behavior;
- preserve one application confirmation API so durable workflows do not know which vendor primitive renders it;
- preserve overlay stacking/back-navigation policy and contextual mobile behavior;
- introduce adaptive presentation only at the oStudy layer, e.g. a desktop Dialog and compact/coarse-pointer Drawer sharing one workflow contract;
- remove custom generic focus traps/backdrops/positioning only after callers and tests prove equivalent behavior;
- do not replace native/platform navigation interception with component-library state.

Overlay migration is high risk because a visual success can still regress focus restoration, Escape/Back, scroll locking, nested overlays, IME behavior, or compact layouts.

## Application shell and navigation

Affected files include `src/App.tsx`, `src/AppShell.module.css`, `src/AppStatus.module.css`, `src/components/NavigationSidebar.*`, `src/components/MobileAppBar.*`, `src/components/Brand.*`, and shared layout/material components.

PR #94 adds Tutor as a first-class section in `App.tsx`, `NavigationSidebar`, `MobileAppBar`, checkpoints, WebDriver contracts, and Android smoke coverage. The migration must preserve the resulting navigation semantics:

- top-level Tutor remains a real section after PR #94;
- contextual Tutor entry points must return to the originating card/guide state rather than being treated as ordinary route replacement;
- compact navigation, Settings placement, Android Back, saved UI checkpoints, and lazy feature mounting remain behavioral contracts;
- shell visual hierarchy can move to Tailwind/shadcn, but controller/section ownership does not move into a component library.

## Flashcards: library, folders, deck detail, transfer

Primary affected areas:

- `src/screens/DecksScreen.*` and `src/screens/flashcards/DeckLibraryWorkspace.*`;
- `src/screens/flashcards/deck-library/` including deck/folder rows, archived sections, sheets, practice setup, selection and drag feedback;
- `src/screens/flashcards/DeckDetailRoute.*`;
- `src/screens/flashcards/DeckTransferDialog.*` and `AnkiImportReport.*`.

Migration opportunities include buttons, menus, checkboxes, sheets/dialogs, badges, tooltips, search inputs, selection chrome, status/progress states, and surface/layout styling.

Keep custom:

- folder/deck organization and selection state;
- pointer drag/reorder session models and feedback semantics;
- archive/delete/undo rules;
- deck import/export and Anki result semantics;
- route/checkpoint compatibility.

Drag/reorder geometry should be migrated late within the Flashcards phase because generic component wrappers can alter DOM nesting, hit targets, pointer capture, and measurement assumptions.

## Search/filter and card browser

Affected areas include `src/screens/cards/CardBrowserWorkspace.*`, `CardFilters.*`, bulk-entry/tag/delete/move-copy/schedule/result modals, `TagInput.*`, and selection toolbar/patterns.

High-value shadcn/Base UI candidates:

- inputs, checkbox/radio/select controls;
- Command/Combobox for searchable selectors where keyboard semantics match product needs;
- dialogs/alert dialogs for bulk operations;
- dropdown/context menus for secondary actions;
- badges/chips for tags and active filters;
- table/list chrome and empty/loading states.

Keep search query state, FTS semantics, saved-filter rules, duplicate detection, bulk mutation identity, and scheduling actions outside vendor components.

## Card editor and rich content

Affected areas include `CardEditorScreenBase.tsx`, `CardEditorScreen.module.css`, `src/screens/card-editor/`, `src/editor/RichTextEditor.module.css`, `RichContent.module.css`, and Tiptap renderer/editor implementations.

This is a mixed migration:

**Move to shared/Tailwind UI:** toolbar buttons, dropdowns, popovers, dialogs, tabs, metadata controls, template selectors, tags, media/math authoring controls, save/delete/secondary actions, notices and loading states.

**Retain application-owned rich-content behavior:** Tiptap schema/extensions, serialization, validated content model, cloze semantics, citation links, editor pair registry, asset/media ownership, math representation, and imported-content safety.

**Likely retain dedicated CSS:** ProseMirror/Tiptap descendant selectors, rendered document typography, embedded media/math rules, browser content-editable workarounds, and print-like content layout when utilities reduce clarity.

Large editor components should be split along presentation/workflow seams where that reduces coupling; Phase 0 does not authorize unrelated editor-domain refactors.

## Documents

Affected areas include `DocumentsScreen.*`, `DocumentsSection.*`, document progress/development-state CSS, source citation UI, and document-related dialogs/actions.

Move ordinary chrome to the new design system: upload/import actions, search/filter controls, document rows/cards, metadata/status badges, progress/skeletons, empty/error states, action menus, confirmation dialogs, and responsive surfaces.

Keep document ingestion, OCR/rendering, AI generation, source handles, progress authority, recovery semantics, managed-file behavior, and native provider calls untouched. The document screen should continue receiving product state rather than making presentation components aware of native pipelines.

## Study Guides

Affected areas include `StudyGuidesScreen.*`, `StudyGuideDialog.*`, route/navigation helpers, study-guide reader chrome, source/citation UI, and contextual Tutor entry points added by PR #94.

Likely migration targets: library surfaces, filters/actions, dialogs/sheets, reader toolbar, disclosures, source panels, badges, buttons and empty/loading states.

Keep guide revision/overlay semantics, source provenance, export behavior, practice navigation, and return-to-library behavior application-owned. Rich guide body rendering may retain semantic content CSS.

## Tutor and contextual tutoring — PR #94 child-branch requirement

PR #94 introduces `src/screens/tutor/` and makes it part of the UI overhaul baseline rather than later follow-up work. Reviewed surfaces include at least:

- `TutorProvider` / `TutorContext` state ownership;
- `TutorWorkspace` and `TutorWorkspace.module.css`;
- `TutorEntryButton` contextual entry;
- `TutorMemoryControls` and its CSS Module;
- `MaterialPreparation`;
- source citation dialog and loading/error states;
- top-level navigation integration in `App.tsx`, `NavigationSidebar`, and `MobileAppBar`;
- contextual hint presentation that must preserve the active practice state.

`TutorWorkspace` currently combines oStudy `Button`/`Dialog` with raw `select`, checkbox `input`, `textarea`, `details/summary`, transcript/message markup, status/error notices, and source actions. That makes it an excellent later consumer of the unified system but a poor early proving ground: its privacy, focus, Back/return, question-answer mode, pending-operation, and preparation states are behaviorally dense.

Migration direction:

- Select/Combobox for saved conversations and material scope;
- Checkbox/Switch where semantics fit conversation retrieval/memory settings;
- Textarea/input primitives for the composer;
- Accordion/Collapsible for optional material/memory controls where accessible behavior matches current disclosure semantics;
- Dialog/Sheet for sources and contextual presentation;
- Skeleton/Progress/Alert for preparation and turn states;
- domain-owned `TutorMessage`, `TutorComposer`, `TutorCitation`, `TutorContextIndicator`, and quiz/feedback components styled with Tailwind but not reduced to generic Cards.

Do not move semantic retrieval, source resolution, privacy/consent, hint-leakage policy, provider calls, persisted conversation state, retry identity, or quiz evaluation into React visual components.

## AI provider/model Settings — materially expanded by PR #94

`src/screens/settings/AiLlmSettings.tsx`, `AiLlmSettings.module.css`, `AiModelsLayout.module.css`, `Settings.module.css`, model-default controls, controllers, and related tests are a major migration target.

The reviewed PR #94 version contains numerous raw buttons, selects, URL/password/text inputs, checkboxes, radio buttons, details/summary disclosures, a custom API-key visibility icon, and legacy `ownedClassNames`/shared-class styling. This is one of the best early vertical slices after the primitive/token foundation because it exercises almost every generic form control without requiring drag geometry or rich document rendering.

Migration direction:

- oStudy `FormField` + Input/TextField for URLs, names and keys;
- Select for provider/auth/timeout/model choices;
- Checkbox/Switch and RadioGroup for enablement, persistence and provider privacy flags;
- shared `IconButton`/Tooltip for API-key visibility;
- Alert/InlineNotice and Progress/Skeleton for errors/connect/model loading;
- AlertDialog/confirmation facade for provider deletion/discard;
- Card/Surface/layout patterns for provider list/detail panes;
- responsive two-pane-to-stacked layout using application layout/capability rules.

Do not let shadcn form components read credentials, choose defaults, call providers, or own dirty/discard logic. Existing controller/native secret boundaries remain authoritative.

## General Settings

Affected areas include `SettingsScreen.*` and `src/screens/settings/` for appearance, default AI models, review scheduling, device sync, diagnostics, storage/backup, and shared settings controls.

Settings should be the first large product area migrated after basic primitives because it reveals inconsistent form spacing, labels, validation, toggles, groups, tabs and responsive layouts early. `FormField` and settings-section patterns should be stabilized here before they spread to Documents/Tutor/Editor.

## Practice

Affected areas include `PracticeScreen.tsx`, `src/screens/practice/ActivePractice.*`, `PracticeComplete.*`, `Practice.module.css`, practice setup sheets, result modals, and contextual Tutor hint entry from PR #94.

Move ordinary chrome: buttons, progress, menus, dialogs, surfaces, toolbar/layout, completion status, loading/error states.

Keep domain-specific grade controls and flashcard presentation semantics application-owned. The 0-3 grading experience is not just four generic colored buttons. Preserve keyboard shortcuts, reveal state, interval-preview meaning, session snapshots, scheduler mutation identity, exit guard, and contextual Tutor return state.

Practice should migrate after overlay/form patterns are stable because compact/coarse-pointer and keyboard behavior are unusually important.

## Analytics

Affected areas include `AnalyticsScreen.*`, `src/screens/analytics/AnalyticsFilters.tsx`, progress chart wrappers and statistics/metric patterns.

Move filters, selectors, cards/surfaces, tabs, tooltip chrome, empty/loading states, date/deck scope UI and responsive layout to the shared system. Keep Chart.js and analytics query/domain semantics unchanged. Chart canvas configuration and data-series logic are not Tailwind/shadcn responsibilities.

## Device sync, backup/import, diagnostics and utility workflows

Affected UI is spread across Settings, `UsbSyncCoordinator`, transfer dialogs, incoming-share coordination, progress/status components, and import/report views.

Generic controls and dialogs should migrate, but Tauri/native capability branching, USB/network transport, QR scanning, file pickers, encryption, backup scope, operation identity and recovery remain behind existing platform/IPC/controller boundaries.

These flows need explicit compact/Android validation because component-library changes can affect permission prompts, focus after external native activities, IME state and Back behavior even when native code is untouched.

## Feedback, help, tutorials and artwork

`Toast`, experience/feedback providers, help registry/provider, tour provider, artwork/material/motion components should keep their semantic APIs. Rendering and positioning can adopt Tailwind/shadcn primitives once overlay/feedback architecture is stable.

A mature visual redesign should continue using Jimothy/artwork selectively rather than turning every shadcn empty state into stock monochrome UI. Celebration, haptic/audio/visual feedback must remain optional effects that cannot determine durable operation success.

## Testing and verification affected surfaces

The migration must account for more than component unit tests. Existing relevant infrastructure includes:

- `scripts/verify-style-ownership.mjs` and matching tests;
- `scripts/verify-owned-class-usage.mjs`;
- `scripts/verify-ui-policy.mjs`;
- `scripts/verify-ui-token-usage.mjs`;
- `scripts/generate-ui-tokens.mjs` and token-currentness checks;
- route smoke contracts and native WebDriver smoke tests;
- Android instrumentation smoke tests;
- UI visual baseline scripts/fixtures;
- UI performance baseline scripts/fixtures;
- semantic role/name assertions in React Testing Library.

Planned enforcement additions during implementation:

1. UI dependency-boundary check: feature code cannot import Base UI or `src/ui/vendor/shadcn` directly.
2. Native-control inventory/check: new raw generic buttons/selects/inputs outside approved low-level owners are rejected or explicitly allowlisted.
3. Tailwind semantic-color check: product UI does not accumulate arbitrary palette colors that bypass oStudy tokens.
4. Dynamic-class check: prohibit runtime-constructed Tailwind utility names that production scanning cannot discover.
5. Legacy CSS cutoff: after a subsystem migrates, prevent new CSS Modules/shared legacy classes from being added to that subsystem.

These checks should enter static/fast qualification as soon as their owning architecture exists, so violations are found before expensive platform gates.

## Risk classification

### Low risk

Skeletons, badges, separators, simple buttons, static surfaces, straightforward progress presentation, simple settings rows.

### Medium risk

Inputs/selects, Tabs, tooltips, ordinary menus, responsive settings layout, empty/error states, shell styling, card/document list presentation.

### High risk

Dialogs/drawers and nested overlays; Android Back/IME/safe-area interactions; deck drag/reorder DOM geometry; Tiptap/content-editable integration; active Practice and contextual Tutor hints; Tutor transcript/source/memory state; destructive workflows; any change that alters semantic selectors consumed by WebDriver or Android instrumentation.

High-risk areas should be migrated only after the corresponding low-level primitive has focused cross-input tests.

## Recommended migration order derived from the inventory

1. Build/Tailwind integration with no product conversion.
2. Token bridge and semantic variants.
3. Low-risk primitive renderer proof.
4. Forms plus AI/general Settings, including PR #94 provider/model controls.
5. Overlay system and adaptive desktop/mobile presentation.
6. Feedback/status/loading primitives.
7. Shell/navigation.
8. Flashcard library/search/filter and bulk workflows.
9. Card editor chrome while retaining rich-content ownership.
10. Documents and Study Guides.
11. Analytics.
12. Practice and contextual hint integration.
13. Tutor workspace/contextual Tutor UI.
14. Remaining CSS Module conversion.
15. Legacy ownership cleanup and stronger enforcement.

The detailed exit criteria and validation gates for these stages are in `ui-overhaul-roadmap.md`.