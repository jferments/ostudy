# ADR 0048: Frontend state and style ownership

- **Status:** Proposed for implementation
- **Date:** 2026-08-25
- **Base:** `main` at `f088c401235419535a9fd36e76badbc004509255`
- **Implementation branch:** `agent/frontend-css-ownership-refactor`

## Context

ostudy already has a semantic token system, appearance provider, layout modes, shared UI primitives, shared workspace patterns, CSS Modules, and cascade layers. The remaining frontend debt is concentrated in two places:

1. `DeckLibraryWorkspace.tsx` still combines route composition, remote state, optimistic concurrency, deck/folder use cases, HTML drag-and-drop, touch/pointer movement, keyboard movement, accessibility announcements, view components, sheets, and pure organization transforms.
2. `src/styles/base/application.css` and the broad files under `src/styles/features/` still act as a second visual system beneath the modern modules. They duplicate generated themes, style application components globally, and require compatibility class names and cascade/specificity overrides.

This refactor completes the migration anticipated by ADR 0034. It changes ownership and topology while preserving the current interface, accessibility semantics, routes, IPC, domain behavior, themes, motion/material preferences, safe areas, Android IME behavior, and user settings.

## Decision

### Frontend dependency direction

```text
typed IPC/controllers
        ↓
feature controller hooks
        ↓
pure feature model + interaction adapters
        ↓
feature views
        ↓
shared patterns, primitives, and foundations
```

For the deck library:

```text
DeckLibraryWorkspace          route/detail composition only
DeckLibraryRoot               connects controller and view
useDeckLibraryController      loading, mutations, revisions, recovery, toasts
organizationModel             pure ordering and optimistic transformation
useDeckLibraryReorder         keyboard/HTML drag/pointer → LibraryMoveIntent
DeckLibraryView/components    presentation only; no IPC imports
```

All movement mechanisms produce one semantic `LibraryMoveIntent` before invoking mutation logic.

### Style ownership hierarchy

Every authored style has exactly one owner:

1. generated design tokens;
2. global reset/document/platform foundations;
3. shared primitive;
4. shared pattern;
5. feature/component CSS Module;
6. narrowly scoped third-party markup bridge; or
7. audited accessibility utility.

There is no generic miscellaneous application stylesheet.

### Global foundation

`src/main.tsx` continues importing exactly one global entry point, `src/ui/foundations/index.css`. The final foundation is:

```text
src/ui/foundations/
├── index.css
├── local-fonts.css
├── reset.css
├── document.css
├── platform.css
└── utilities.css
```

Generated themes are the only theme authority. Temporary compatibility aliases may exist only during the implementation sequence and must be removed before completion.

### Final cascade

```css
@layer reset, tokens, theme, base, components, patterns, features, utilities;
```

The legacy layer and global feature imports are deleted after their owners migrate.

### Responsive ownership

- `LayoutModeProvider` owns application-wide wide/medium/compact shell behavior.
- Component-local adaptations use container queries when they depend on allocated space.
- Viewport media queries are limited to true platform/document concerns such as safe areas, virtual keyboards, forced colors, reduced motion, and coarse pointers.

### Rich content exception

ProseMirror/Tiptap-generated descendants are styled through a wrapper-scoped rich-content CSS Module. A small number of `:global(.ProseMirror)` selectors are allowed only at that third-party boundary. Editor chrome and rendered educational prose have separate owners.

### Enforcement

Static policy dynamically scans authored CSS and rejects new global feature files, raw theme colors, arbitrary z-indexes/durations, direct artwork imports, feature theme branches, unexplained `!important`, and new compatibility aliases. A legacy-style ratchet permits legacy files only to shrink until the final zero-legacy assertion replaces it.

## Non-goals

- No visual redesign.
- No new CSS framework, preprocessor, CSS-in-JS system, utility framework, Storybook requirement, or global state library.
- No route, IPC, Rust, scheduling, card, document, study-guide, analytics, or settings semantic change.
- No change to theme names, appearance preference fields, or user-facing information architecture.
- No drag-only operation and no removal of keyboard/screen-reader alternatives.

## Implementation stages

The detailed, checked execution ledger lives in `docs/development/frontend-css-refactor-roadmap.md` and the PR body.

## Acceptance criteria

- **AC-01:** Deck-library organization policy is pure, immutable, and exhaustively tested.
- **AC-02:** Deck-library controller owns all IPC/controller mutations, revision recovery, optimistic state, toasts, and announcements.
- **AC-03:** Keyboard, HTML drag, pointer/touch, and fallback controls share one move intent and one policy.
- **AC-04:** Deck-library views are small, presentation-only modules with co-located styles and accessibility parity.
- **AC-05:** Generated tokens are the only theme authority and shell/platform foundations have one owner.
- **AC-06:** Shared controls, feedback, overlays, editor chrome, and rendered rich content no longer depend on global legacy selectors.
- **AC-07:** Every global feature stylesheet and `application.css` is deleted; the legacy layer and compatibility aliases are gone.
- **AC-08:** Static enforcement prevents legacy topology, raw visual values, hidden global ownership, and uncompiled style/test fixtures from returning.
- **AC-09:** Existing frontend, native Linux/Tauri, Android package, API-36 emulator, accessibility, and IPC behavior remains compatible.

## Consequences

- Feature changes become locally reviewable and do not depend on hidden import order.
- All themes and material/motion modes continue sharing one semantic contract.
- The migration is large but reviewable through bounded stages and a non-growth ratchet.
- Some feature files gain explicit CSS imports and component boundaries; this is intentional ownership rather than duplicated presentation.
