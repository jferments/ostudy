# ADR 0051: Adopt shadcn/ui and Tailwind behind oStudy UI ownership

Status: accepted for staged migration. Phase 0 records the architecture only; it does not install or migrate runtime UI code.

## Context

oStudy already has a substantial application-owned UI system: semantic design tokens, multiple themes, appearance/capability state, platform safe-area and reduced-motion rules, shared primitives, shared overlays, responsive patterns, and visual/performance regression infrastructure. At the same time, many generic controls and interaction patterns remain hand-built or styled through a broad set of CSS Modules and legacy shared classes. That makes visual polish expensive, repeats common control behavior, and leaves accessibility and cross-platform interaction work in application code that does not need to own it.

PR #94 expands the surface further. It introduces the Tutor workspace and contextual Tutor entry points, new conversation/memory/material-preparation controls, and broader AI provider/model configuration. These are presentation-heavy areas with selects, checkboxes, text fields, text areas, dialogs, progress states, menus, disclosures, and responsive behavior. They should participate in the same migration rather than becoming a second UI generation that must be refactored later.

The goal is therefore not to replace oStudy's design system with a third-party theme. The goal is to use mature open-source presentation and interaction primitives while keeping oStudy's domain semantics, visual identity, platform behavior, and change boundaries authoritative.

## Decision

### 1. Tailwind CSS v4 becomes the standard styling language for ordinary application UI

New and migrated generic component styling should use Tailwind utilities. Feature/domain components may use Tailwind directly for local layout and presentation; oStudy will not wrap every flex/grid/spacing utility in a pseudo-layout framework.

Ordinary hard-coded palette colors are not the design API. Tailwind semantic utilities must resolve through oStudy-owned theme variables. Dynamic utility-name construction is prohibited; state/variant maps must contain complete class names so they remain statically discoverable.

### 2. shadcn/ui is source material, not an application dependency boundary

Generated/owned shadcn component source will live under an explicit vendor implementation directory such as `src/ui/vendor/shadcn/`. The initial component family should use Base UI behavior unless a concrete component/platform limitation justifies another implementation.

Feature code must not import Base UI or vendor shadcn modules directly. Application code consumes stable oStudy UI APIs under `src/ui/primitives`, `src/ui/overlays`, `src/ui/layout`, `src/ui/patterns`, and related semantic owners.

This yields the dependency direction:

```text
features/screens
    -> oStudy UI facade and domain components
        -> owned shadcn implementation
            -> Base UI + Tailwind
                -> browser DOM/CSS in the Tauri WebView
```

### 3. oStudy design tokens remain the source of truth

The existing token sources under `src/ui/tokens/source/` and `src/ui/tokens/themes/` remain authoritative. `scripts/generate-ui-tokens.mjs` will be extended in a later phase to emit Tailwind/shadcn semantic adapters in addition to the existing CSS and TypeScript artifacts.

Names such as `background`, `foreground`, `primary`, `muted`, `border`, `ring`, `destructive`, and component-state colors are compatibility aliases. They must ultimately resolve to `--ostudy-*` semantic variables rather than introduce an independent shadcn palette.

Light, dark, console, and synthwave remain oStudy themes. A third-party component default must not become a second theme source.

### 4. AppearanceProvider and platform capability contracts remain authoritative

`AppearanceProvider` continues to own theme, motion, material, artwork, celebration, tutorial, contrast, pointer, hover, scale, and font state through root attributes and variables. Tailwind variants may consume these attributes, but should not replace them with a simpler `class="dark"`-only model.

The safe-area, Android IME, touch-target, reduced-transparency, reduced-motion, and forced-color contracts in the current foundation/platform CSS remain part of the platform layer. A component migration is not permission to weaken these behaviors.

### 5. Preserve semantic oStudy component APIs where they carry product meaning

Existing abstractions such as `Button`, `FormField`, application confirmation flows, feedback state, and adaptive layout/overlay behavior should keep application-level contracts even when their renderer changes.

For example, callers should continue to express destructive intent, validation/help relationships, or practice-grade semantics rather than importing a vendor variant directly. Mechanical renderer replacement should be separable from later API cleanup.

### 6. Generic interaction behavior should move to mature primitives

Dialogs, alert/confirmation dialogs, menus, popovers, tooltips, selects/comboboxes, checkboxes, switches, tabs, drawers/sheets, and related focus/keyboard/portal behavior should increasingly rely on Base UI/shadcn implementations.

Application-specific policies remain outside those primitives: navigation meaning, native command ownership, provider consent, Tutor hint privacy, durable mutation identity, scheduling semantics, and document/card domain rules are unchanged.

### 7. Responsive presentation may adapt without forking product workflows

The same application workflow may render a centered dialog on a wide/fine-pointer layout and a drawer or sheet on a compact/coarse-pointer layout. These choices should use existing capability/layout state rather than user-agent branching.

A reusable oStudy abstraction such as `AdaptiveDialog` or `AdaptiveActionMenu` may select the presentation primitive while preserving one workflow contract.

### 8. CSS Modules are a migration source, not a permanent requirement

Most generic component and screen chrome should migrate to Tailwind. The end state is not required to have zero CSS files. Dedicated semantic CSS remains appropriate for rich Tiptap/document rendering, imported content, math/media rendering, print behavior, complex descendant selectors, platform/browser compatibility, forced-color rules, and other cases where utilities make ownership less clear.

No new blanket rule should force specialized content CSS into unreadable utility strings.

### 9. Framework replacement must remain bounded

The vendor boundary and oStudy semantic facades are deliberate exit points. Upstream shadcn refreshes are reviewed as source merges, not blindly regenerated over local behavior. If oStudy later replaces shadcn/Base UI, most changes should remain inside `src/ui` rather than propagate through controllers, IPC, native services, or domain state.

## Explicit non-goals

This migration does not redesign Tauri IPC, Rust services, persistence, FSRS scheduling, document processing, the knowledge/Tutor architecture, provider ownership, Chart.js analytics, or Tiptap's content schema. It does not make UI components responsible for native commands or provider calls. It does not require a single identical layout across desktop and mobile.

Phase 0 specifically does not add Tailwind, shadcn, Base UI, CVA, icon packages, or any other runtime/build dependency. Exact dependency versions, licenses, platform compatibility, and lockfile changes belong to Phase 1 and must be reviewed there.

## Consequences

The migration can proceed incrementally because current oStudy facades can change renderer before their call sites change. Theme migration can also proceed incrementally because legacy CSS and Tailwind utilities can consume the same semantic token values during the transition.

The cost is that oStudy intentionally maintains a thin component ownership layer instead of importing vendor components everywhere. That is desirable coupling: product semantics depend on oStudy APIs while generic interaction behavior can evolve behind them.

The migration will temporarily run CSS Modules and Tailwind side-by-side. Phase-specific inventories and verification rules must prevent that transitional state from becoming permanent or allowing a third styling vocabulary to grow.

See `../development/ui-overhaul-phase-0-inventory.md` for the source assessment and `../development/ui-overhaul-roadmap.md` for the staged implementation sequence.