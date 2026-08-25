# Frontend CSS ownership inventory

This document began as the Stage 0 migration map for ADR 0048. Stage 8 is now implemented, so it records the **final ownership contract** and the retired global owners for auditability rather than describing active legacy files.

## Current ownership contract

Authored presentation ends in one of these explicit owners:

1. generated semantic/reference/component tokens;
2. global reset/document/platform foundations;
3. shared primitive;
4. shared pattern;
5. feature/component CSS Module;
6. narrowly scoped third-party markup bridge; or
7. audited accessibility utility.

`src/main.tsx` has exactly one global CSS import:

```text
src/main.tsx → ./ui/foundations/index.css
```

The authored global foundation is limited to:

```text
src/ui/foundations/
├── index.css
├── local-fonts.css
├── reset.css
├── document.css
├── platform.css
└── utilities.css
```

The final cascade is:

```css
@layer reset, tokens, theme, base, components, patterns, features, utilities;
```

There is no `src/styles` tree, `legacy` layer, global feature stylesheet, or temporary style-policy allowance.

## Retired global owners

| Retired owner | Historical role | Final owner |
| --- | --- | --- |
| `src/styles/base/application.css` | Duplicate themes, generic application controls, shell/navigation, modal/toast/editor/rich-content compatibility, and old responsive rules | Generated themes; global foundations; shell/navigation modules; shared primitives/patterns; editor/rich-content modules; feature modules |
| `src/styles/features/practice.css` | Practice setup, active review, confidence controls, completion layout | Practice setup/active/complete modules and shared controls/status owners |
| `src/styles/features/cards.css` | Card browser/search/selection, bulk actions, rows, tags/status presentation | Card-browser/authoring modules, shared controls, overlays, status owners |
| `src/styles/features/authoring.css` | Card editor, cloze/template-entry chrome and previews | Card-editor/template modules and shared rich editor/content owners |
| `src/styles/features/templates.css` | Template library/editor presentation | Template feature modules and shared form/surface owners |
| `src/styles/features/scheduling.css` | Scheduling controls, limits, badges, action panels | Scheduling feature modules plus shared controls/status owners |
| `src/styles/features/analytics.css` | Analytics filters, metrics, charts, grids, tables | Analytics modules plus shared Metrics patterns |
| `src/styles/features/assets.css` | Asset/document compatibility styles | Documents/assets feature modules and shared status/surface owners |
| `src/styles/features/ui-overhaul.css` | Cross-feature containment/navigation/layout compatibility | The component or feature that owns each affected layout |
| `src/styles/features/page-density.css` | Cross-feature compact spacing/breakpoint overrides | Workspace patterns and local feature/component modules |
| `src/screens/DecksScreen.stage12.css` | Global deck detail/card-browser compatibility imported by Flashcards | Deck/card-browser modules and shared patterns |
| `src/screens/cards/CardListing.css` | Card-listing/rich-content compatibility | Card-browser and `RichContent.module.css` ownership |

All of these retired paths are forbidden by the Stage 8 zero-legacy verifier.

## Foundation ownership

- `reset.css`: box sizing, inherited control fonts, disabled/cursor semantics, focus-visible baseline.
- `document.css`: `html`/`body`/`#root` sizing, canvas/text/font defaults, document-wide native-control defaults.
- `platform.css`: safe-area variables, Android IME displacement, true platform media behavior, forced-colors and motion/material platform contracts.
- `utilities.css`: audited accessibility utilities only, such as visually hidden text.
- `index.css`: final layer ordering, generated-token imports, and narrow semantic aliases required by shared presentation.

Feature layout helpers do not belong in the global foundation.

## Shared and feature owners

- `Button`, `InlineNotice`, `EmptyState`, `Panel`, `Surface`, `FormField`, status/badge primitives: reusable controls and feedback.
- Workspace/screen/control/overlay patterns: reusable layout and interaction presentation.
- Overlay modules: dialog, sheet, backdrop, menu, and toast ownership.
- `RichTextEditor.module.css`: editor toolbar/editable chrome.
- `RichContent.module.css`: rendered educational headings, prose, lists, tables, code, links, images, indentation, and scoped third-party markup bridge.
- Feature CSS Modules: presentation whose meaning belongs to one feature/screen/component.

Some migrated views preserve semantic class **keys** through `ownedClassNames(...)`. Those strings are not global CSS classes: `scripts/verify-owned-class-usage.mjs` parses production TSX and proves each key resolves through an imported CSS Module/shared owner before rendering.

## Zero-legacy enforcement

`scripts/style-ownership-baseline.json` is in `zero` mode with:

- an empty `legacyFiles` object;
- exactly one allowed global TypeScript CSS import;
- an empty `policyAllowances` object.

`scripts/verify-style-ownership.mjs` additionally rejects:

- any CSS under `src/styles/base` or `src/styles/features`;
- authored global CSS outside the foundation;
- any `legacy` layer/import;
- an incorrect final cascade order;
- raw authored colors, arbitrary numeric stacking values, or literal motion durations outside generated token sources.

`scripts/verify-owned-class-usage.mjs` separately rejects raw/unowned production class semantics.

The device-sync QR quiet zone is intentionally fixed white for scan reliability, but the authored module uses `var(--ostudy-color-neutral-0)`. `verify-device-sync-qr-ui.mjs` independently requires the reference token source for `color.neutral.0` to remain exactly `#ffffff`.
