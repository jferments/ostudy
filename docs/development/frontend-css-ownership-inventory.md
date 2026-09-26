# Frontend CSS ownership inventory

This document began as the Stage 0 migration map for [ADR 0048](../decisions/0048-frontend-state-and-style-ownership.md). Its retired global-owner inventory remains historical evidence. The later [ADR 0051](../decisions/0051-shadcn-tailwind-presentation-architecture.md) and [current UI architecture](../architecture/ui.md) supersede the CSS-Modules-first ordinary presentation choice; state and ownership boundaries remain explicit.

## Current ownership contract

Authored presentation has an explicit owner: generated semantic/reference/component tokens; global reset/document/platform foundations; shared primitives/patterns; complete `tw:` utilities through oStudy-owned presentation; registered remaining CSS Modules; or specialized content/platform/vendor-markup styling. New feature styling must not recreate a global stylesheet or an unowned second presentation system.

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

The cascade remains:

```css
@layer reset, tokens, theme, base, components, patterns, features, utilities;
```

The foundation imports Tailwind theme and prefixed utilities without Preflight or a second reset. Utilities currently compile into the `components` layer to retain the established override ordering; the presence of an outer layer named `utilities` is not a reason to move them silently. Locally packaged math vendor styles/fonts are separate explicit imports.

There is no `src/styles` tree, `legacy` layer, global feature stylesheet, or temporary style-policy allowance. That zero-legacy claim concerns the retired global stylesheet system, not the absence of all CSS Modules.

## Retired global owners

This table records where responsibilities went during the earlier CSS ownership migration. Its references to feature modules are historical dispositions, not instructions to use CSS Modules for all new presentation after the Tailwind migration.

| Retired owner | Historical role | Ownership established by the earlier migration |
| --- | --- | --- |
| `src/styles/base/application.css` | Duplicate themes, application controls, shell/navigation, overlays/editor/rich-content compatibility | Generated themes; foundations; shared primitives/patterns; explicit editor/content/feature owners |
| `src/styles/features/practice.css` | Practice setup, review, confidence controls and completion | Practice setup/active/complete owners and shared controls/status |
| `src/styles/features/cards.css` | Browser/search/selection, bulk actions, rows and tags | Card-browser/authoring owners, shared controls and overlays |
| `src/styles/features/authoring.css` | Editor, cloze/template-entry chrome and previews | Card-editor/template owners and shared rich content |
| `src/styles/features/templates.css` | Template library/editor presentation | Template owners and shared forms/surfaces |
| `src/styles/features/scheduling.css` | Scheduling controls, limits, badges and panels | Scheduling owners and shared controls/status |
| `src/styles/features/analytics.css` | Filters, metrics, charts and grids | Analytics owners and shared metrics patterns |
| `src/styles/features/assets.css` | Asset/document compatibility styles | Document/asset owners and shared status/surfaces |
| `src/styles/features/ui-overhaul.css` | Cross-feature containment/navigation/layout | The component or feature owning each layout |
| `src/styles/features/page-density.css` | Cross-feature compact-spacing overrides | Workspace patterns and explicit local owners |
| `src/screens/DecksScreen.stage12.css` | Global deck-detail/card-browser compatibility | Deck/card-browser owners and shared patterns |
| `src/screens/cards/CardListing.css` | Card-listing/rich-content compatibility | Card-browser and `RichContent.module.css` ownership |

These retired paths remain forbidden by the zero-legacy verifier.

## Foundation ownership

- `reset.css`: box sizing, inherited control fonts, disabled/cursor semantics, focus-visible baseline.
- `document.css`: root sizing, canvas/text/font defaults, document-wide native-control defaults.
- `platform.css`: safe areas, Android IME displacement, platform media behavior, forced colors and motion/material contracts.
- `utilities.css`: audited accessibility utilities, not a feature-layout dumping ground.
- `index.css`: layer order, Tailwind integration, generated tokens, vendor math styles and narrow semantic aliases.

## Shared and feature owners

Shared Button, InlineNotice, EmptyState, Panel, Surface, field/status primitives and workspace patterns own reusable control and layout semantics. Base UI is consumed through the oStudy vendor/adaptor boundary, not directly by feature code. OverlayProvider owns the application stack and Back policy; the shared Base UI adapters own browser modality/focus behavior.

Ordinary migrated presentation uses complete `tw:` utility names and semantic tokens. Feature presentation maps can expose that owned vocabulary without becoming global CSS classes. Registered remaining CSS Modules retain their specific rationale and consumers; specialized rich-content, ProseMirror descendants, math/media geometry and platform constraints need not be forced into utilities merely for migration bookkeeping.

Some earlier views use `ownedClassNames(...)` to resolve semantic keys through imported owners. That mechanism and newer utility-based presentation maps must be interpreted from the actual source, not confused with raw global classes. Use the maintained style/owned-class/UI-boundary verifiers together; no one verifier proves all presentation behavior.

## Zero-legacy enforcement

`scripts/style-ownership-baseline.json` remains in `zero` mode with an empty `legacyFiles` object, one allowed global TypeScript CSS import and an empty `policyAllowances` object.

`scripts/verify-style-ownership.mjs` guards retired global paths, authored global CSS outside the foundation, legacy imports/layers, cascade order, and token ownership. `scripts/verify-owned-class-usage.mjs` separately checks owned production class semantics. The current presentation-policy checks also cover vendor confinement and retained CSS rationale. Follow their actual registered contracts rather than inventing a new exception for convenience.

The device-sync QR quiet zone uses the fixed white reference token for scan reliability rather than a theme-dependent surface. `verify-device-sync-qr-ui.mjs` protects that token contract. Scan behavior, focus, Back, touch, enlarged text, reduced motion and platform geometry still require their own behavior/visual qualification; static CSS ownership is not runtime acceptance.
