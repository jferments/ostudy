# Frontend CSS ownership inventory

This inventory is the Stage 0 migration map for ADR 0048. It describes the current global owners at the PR #74 baseline and the only acceptable destinations for their selectors. `scripts/verify-style-ownership.mjs` records the exact baseline selector sets and prevents new legacy selectors or new global stylesheet imports while the migration is in progress.

## Ownership rules

Authored styles end in one of six places: generated semantic tokens, global foundations, shared primitives, shared patterns, feature/component CSS Modules, or a narrowly scoped third-party markup bridge. The only intentional global utility is the audited accessibility utility in `src/ui/foundations/utilities.css`. No new selector may be added to the legacy files during migration.

| Current owner | Baseline role | Final owner |
| --- | --- | --- |
| `src/styles/base/application.css` | Duplicate hard-coded themes, reset/document defaults, shell/navigation, generic buttons/forms/notices/panels, modal/toast/editor/rich-content rules, and old responsive rules | Generated themes; `src/ui/foundations/{reset,document,platform,utilities}.css`; `AppShell.module.css`; navigation modules; shared primitives/patterns; rich-content/editor modules; individual features |
| `src/styles/features/practice.css` | Practice setup, active review, confidence controls, completion layout | Practice setup CSS Module, `ActivePractice.module.css`, `PracticeComplete.module.css`, shared controls |
| `src/styles/features/cards.css` | Card browser/search/selection, bulk actions, card rows, tag/status presentation | Card browser/authoring CSS Modules, `CardFilters.module.css`, shared status/form/overlay owners |
| `src/styles/features/authoring.css` | Card editor, cloze/template-entry chrome, preview and rich field editing | Card-editor feature modules plus shared rich editor/content owners |
| `src/styles/features/templates.css` | Template library/editor presentation | Templates feature CSS Module plus shared form/surface owners |
| `src/styles/features/scheduling.css` | Scheduling controls, limits, status/action panels | Scheduling feature CSS Module plus shared form/status owners |
| `src/styles/features/analytics.css` | Analytics filters, metric cards, chart/grid/table layout | Analytics feature CSS Module plus `ui/patterns/Metrics.module.css` |
| `src/styles/features/assets.css` | Asset/document list details | Documents/assets CSS Modules and shared status/surface owners |
| `src/styles/features/ui-overhaul.css` | Cross-feature containment fixes, navigation/layout compatibility, deck/card/practice density fixes | The component/feature that owns each affected layout; no compatibility stylesheet remains |
| `src/styles/features/page-density.css` | Cross-feature compact spacing and breakpoint overrides | Workspace patterns and the feature/component modules whose spacing is being adjusted |
| `src/screens/DecksScreen.stage12.css` | Legacy Decks detail/editor global styles imported by Flashcards workspace | Deck detail/editor CSS Module and rich-content/editor modules |
| `src/screens/cards/CardListing.css` | Rendered card listing/prose compatibility rules | `RichContent.module.css` and card-browser module |

## Global foundation map

- `reset.css`: box sizing, zero margins, inherited control fonts, disabled/cursor semantics, focus-visible reset contract.
- `document.css`: `html`/`body`/`#root` sizing, canvas/text/font defaults, headings, prose defaults, keyboard markup, native form defaults that are intentionally document-wide.
- `platform.css`: safe-area variables, Android IME displacement, compact-layout platform hooks, forced-colors and platform-specific behavior.
- `utilities.css`: audited accessibility-only utilities such as visually hidden text. Feature layout helpers do not belong here.

## Shared-owner map

- `Button`, `InlineNotice`, `EmptyState`, `Panel`, `Surface`, `FormField`, status/badge primitives: generic controls and feedback.
- `WorkspacePatterns`: screen frame, page header, header actions, compact/wrap layout patterns.
- Overlay modules: dialog/sheet/backdrop/menu/toast ownership; feature code does not define global modal classes.
- `RichTextEditor.module.css`: editor toolbar and editable chrome.
- `RichContent.module.css`: rendered educational prose, headings, lists, tables, code, links, images, indentation, and the scoped ProseMirror bridge.

## Ratchet and completion criteria

During Stages 0–7 the baseline is in `ratchet` mode: legacy files may shrink or disappear, but no new legacy selector or global CSS import may appear, and dynamically discovered authored CSS may not add raw colors, numeric stacking values, or arbitrary motion durations beyond the explicit migration allowances.

Stage 8 changes the baseline to `zero`. In zero mode:

1. `src/styles/base/*.css` and `src/styles/features/*.css` must not exist.
2. TypeScript may globally import only `src/ui/foundations/index.css` from `src/main.tsx`.
3. the `legacy` cascade layer/imports must not exist.
4. temporary policy allowances must be empty.
