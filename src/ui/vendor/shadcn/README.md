# Owned shadcn components

Upstream comparison baseline: shadcn-ui/ui commit `2b3e6d4f8d9161fe5c19340dc383aade392012dd` (2026-09-12).
License: MIT; the distributed notice is [SHADCN-MIT.txt](../../../../public/licenses/SHADCN-MIT.txt).

This directory is oStudy's owned shadcn implementation boundary. Features import application facades from `src/ui`, not Base UI, copied vendor modules or vendor prop types. Base UI remains the generic interaction layer where it adds behavior; ordinary structural composition stays React/DOM plus Tailwind.

`components.json` selects the `base-nova` family. The visual-refresh rule is **stock first**: follow Nova's ordinary component vocabulary, geometry, spacing and hierarchy unless oStudy has a concrete product, accessibility or platform reason to differ. oStudy remains the authority for semantic colors, four themes, capability state, safe areas, coarse-pointer touch targets, reduced motion and domain behavior.

## Current component vocabulary

Owned source includes Button, Badge, Skeleton, Input, NativeSelect, form controls, Tabs, Card, Item, ButtonGroup, Dialog/Sheet, DropdownMenu, Tooltip, Toast/notification, Alert, Progress, Empty, Toolbar and Sidebar composition.

Card exposes title/description/action/content/footer slots instead of requiring feature-local header recipes, and owns the one card surface and its gutter; Panel, Surface and StatTile compose it. Item is the preferred generic list-object presentation. ButtonGroup is the preferred visual grouping for related adjacent controls. Application-specific wrappers should encapsulate semantic intent or platform behavior rather than preserve legacy styling.

## Deliberate adaptations

- Registry style-map placeholders are expanded into literal namespaced `tw:` utilities so Tailwind can discover them statically.
- Semantic shadcn roles resolve through oStudy tokens; no parallel palette or `dark` provider exists.
- Button follows Nova's stock `default`, `outline`, `secondary`, `ghost`, `destructive` and `link` variants. The public oStudy facade keeps application semantic names and loading/write-lock behavior without growing additional vendor variants.
- Fine-pointer controls use Nova-like compact dimensions. Coarse-pointer variants retain the application touch-target minimum.
- Segmented control options keep the application touch target at every pointer size: each option's button is the target and the pill it draws sits inside it, so the mockups' proportions stay visual only.
- NativeSelect deliberately retains the operating-system picker, options/optgroups, multiple selection and numeric listbox semantics.
- Checkbox, Switch, RadioGroup, Slider, Tabs and Toolbar rely on Base UI interaction behavior rather than custom event imitation.
- Disclosure content remains mounted when required to preserve drafts. FormField keeps contextual help outside the input label and owns label/description/error association.
- Sidebar is intentionally non-collapsing. `LayoutModeProvider` owns responsive behavior, so there is no second cookie, mobile detector or vendor navigation hotkey.
- Modal behavior uses Base UI while oStudy owns application overlay registration, native Back, compact/fullscreen policy, busy dismissal and explicit focus policy. The modal composes one header, scrolling body and pinned action footer (optionally one form) for every dialog, sheet and confirmation. Inline workspaces remain nonmodal.
- Notification rendering does not own durable mutation success, retry identity or Undo expiry.
- Rich editor/document/Markdown descendants, drag geometry, Practice card transforms, QR quiet zones and other registered specialized CSS owners remain outside generic component styling.
- No Preflight is imported. Border recipes that require a visible border therefore specify border style explicitly.

## Updating and qualification

These are customized source copies, not a runtime registry dependency. Compare a pinned upstream revision, review behavior and dependencies, then port relevant changes deliberately. Do not blindly overwrite the directory with the shadcn CLI.

Runtime rendering needs no CLI, component registry, CDN, hosted font or external provider. Use focused component tests, theme/token/presentation policy checks and CSS compilation before native qualification. Human visual review remains separate from behavioral automation.
