# ADR 0051: Adopt shadcn/ui and Tailwind behind oStudy UI ownership

Status: **accepted and implemented.** This decision was originally approved for a staged migration; PR #96 completed that migration and was squash-merged as `5a4a4212e15c847db5bbf5daf1948e6ca6b3f860`. The Phase 0/Phase 1 wording below records how the decision was staged, not the repository's present dependency state. Current styling rules live in [UI architecture](../architecture/ui.md), while post-migration aesthetic work is tracked in the [visual refresh roadmap](../development/ui-visual-refresh-roadmap.md).

## Context

oStudy already had a substantial application-owned UI system: semantic design tokens, multiple themes, appearance/capability state, platform safe-area and reduced-motion rules, shared primitives, shared overlays, responsive patterns, and visual/performance regression infrastructure. At the same time, many generic controls and interaction patterns remained hand-built or styled through a broad set of CSS Modules and legacy shared classes. That made visual polish expensive, repeated common control behavior, and left accessibility and cross-platform interaction work in application code that did not need to own it.

PR #94 expanded the surface further. It introduced the Tutor workspace and contextual Tutor entry points, new conversation/memory/material-preparation controls, and broader AI provider/model configuration. These are presentation-heavy areas with selects, checkboxes, text fields, text areas, dialogs, progress states, menus, disclosures, and responsive behavior. They needed to participate in the same migration rather than become a second UI generation.

The goal was therefore not to replace oStudy's design system with a third-party theme. The goal was to use mature open-source presentation and interaction primitives while keeping oStudy's domain semantics, visual identity, platform behavior, and change boundaries authoritative.

## Decision

### 1. Tailwind CSS v4 becomes the standard styling language for ordinary application UI

New and migrated generic component styling should use Tailwind utilities. Feature/domain components may use Tailwind directly for local layout and presentation; oStudy will not wrap every flex/grid/spacing utility in a pseudo-layout framework.

Ordinary hard-coded palette colors are not the design API. Tailwind semantic utilities must resolve through oStudy-owned theme variables. Dynamic utility-name construction is prohibited; state/variant maps must contain complete class names so they remain statically discoverable.

### 2. shadcn/ui is source material, not an application dependency boundary

Generated/owned shadcn component source lives under `src/ui/vendor/shadcn/`. The component family uses Base UI behavior unless a concrete component/platform limitation justifies another implementation.

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

The token sources under `src/ui/tokens/source/` and `src/ui/tokens/themes/` remain authoritative. `scripts/generate-ui-tokens.mjs` emits Tailwind/shadcn semantic adapters in addition to the application's CSS/token artifacts.

Names such as `background`, `foreground`, `primary`, `muted`, `border`, `ring`, `destructive`, and component-state colors are compatibility aliases. They resolve to `--ostudy-*` semantic variables rather than introduce an independent shadcn palette.

Light, dark, console, and synthwave remain oStudy themes. A third-party component default must not become a second theme source.

### 4. AppearanceProvider and platform capability contracts remain authoritative

`AppearanceProvider` continues to own theme, motion, material, artwork, celebration, tutorial, contrast, pointer, hover, scale, and font state through root attributes and variables. Tailwind variants may consume these attributes, but should not replace them with a simpler `class="dark"`-only model.

Safe-area, Android IME, touch-target, reduced-transparency, reduced-motion, and forced-color contracts remain part of the platform layer. A component migration is not permission to weaken these behaviors.

### 5. Preserve semantic oStudy component APIs where they carry product meaning

Abstractions such as `Button`, `FormField`, application confirmation flows, feedback state, and adaptive layout/overlay behavior keep application-level contracts even when their renderer changes.

For example, callers express destructive intent, validation/help relationships, or practice-grade semantics rather than importing a vendor variant directly. Renderer replacement remains separable from domain API changes.

A later post-migration refinement may thin a facade that only preserved legacy appearance, but it must not remove product semantics, cross-platform behavior or accessibility policy. Ordinary visual composition follows the stock-first rule documented in [UI architecture](../architecture/ui.md).

### 6. Generic interaction behavior should move to mature primitives

Dialogs, alert/confirmation dialogs, menus, popovers, tooltips, selects/comboboxes, checkboxes, switches, tabs, drawers/sheets, and related focus/keyboard/portal behavior should rely on Base UI/shadcn implementations where they fit the application's contract.

Application-specific policies remain outside those primitives: navigation meaning, native command ownership, provider consent, Tutor hint privacy, durable mutation identity, scheduling semantics, and document/card domain rules are unchanged.

### 7. Responsive presentation may adapt without forking product workflows

The same application workflow may render a centered dialog on a wide/fine-pointer layout and a drawer or sheet on a compact/coarse-pointer layout. These choices use existing capability/layout state rather than user-agent branching.

Reusable oStudy abstractions may select the presentation primitive while preserving one workflow contract.

### 8. CSS Modules are not a permanent requirement for ordinary chrome

Most generic component and screen chrome uses Tailwind. The end state is not required to have zero CSS files. Dedicated semantic CSS remains appropriate for rich Tiptap/document rendering, imported content, math/media rendering, print behavior, complex descendant selectors, platform/browser compatibility, forced-color rules, measured drag geometry and other cases where utilities make ownership less clear.

No blanket rule should force specialized content CSS into unreadable utility strings. The retained specialized modules are explicitly registered and verified.

### 9. Framework replacement must remain bounded

The vendor boundary and oStudy semantic facades are deliberate exit points. Upstream shadcn refreshes are reviewed as source merges, not blindly regenerated over local behavior. If oStudy later replaces shadcn/Base UI, most changes should remain inside `src/ui` rather than propagate through controllers, IPC, native services, or domain state.

## Explicit non-goals

This migration does not redesign Tauri IPC, Rust services, persistence, FSRS scheduling, document processing, the knowledge/Tutor architecture, provider ownership, Chart.js analytics, or Tiptap's content schema. It does not make UI components responsible for native commands or provider calls. It does not require a single identical layout across desktop and mobile.

At decision time, Phase 0 intentionally added no runtime/build dependency; exact versions, licenses, platform compatibility, and lockfile changes were deferred to Phase 1. Those later stages are now completed historical work.

## Consequences

The migration proceeded incrementally because existing oStudy facades could change renderer before their call sites changed. Theme migration likewise worked incrementally because legacy CSS and Tailwind utilities consumed the same semantic token values during transition.

The repository intentionally maintains a thin component ownership layer instead of importing vendor components everywhere. That is desirable coupling: product semantics depend on oStudy APIs while generic interaction behavior can evolve behind them.

The temporary broad CSS-Module/Tailwind coexistence has ended for ordinary presentation. A checked manifest now retains only specialized CSS owners with explicit rationales. Post-migration visual cleanup should simplify generic facades and adopt stock shadcn composition where doing so does not erase application semantics.

See the historical [Phase 0 source assessment](../development/ui-overhaul-phase-0-inventory.md), completed [migration roadmap](../development/ui-overhaul-roadmap.md), current [UI architecture](../architecture/ui.md), and active [visual refresh roadmap](../development/ui-visual-refresh-roadmap.md).
