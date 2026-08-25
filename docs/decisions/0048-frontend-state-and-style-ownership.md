# ADR 0048: Frontend state and style ownership

- **Status:** Accepted
- **Date:** 2026-08-25
- **Original base:** `main` at `f088c401235419535a9fd36e76badbc004509255`
- **Primary implementation:** PR #75, completed by Stage 8 in PR #76

## Context

ostudy already had semantic tokens, an appearance provider, layout modes, shared UI primitives, shared workspace patterns, CSS Modules, and cascade layers. The remaining frontend debt was concentrated in two areas:

1. `DeckLibraryWorkspace.tsx` combined route composition, remote state, optimistic concurrency, deck/folder use cases, drag/touch/keyboard movement, accessibility announcements, views, and mutation policy.
2. `src/styles/base/application.css`, the global feature stylesheets under `src/styles/features/`, `DecksScreen.stage12.css`, and `CardListing.css` formed a second global visual system beneath the module-based UI.

The refactor changes ownership and topology while preserving interface behavior, accessibility semantics, routes, IPC, domain behavior, themes, motion/material preferences, safe areas, Android IME behavior, and persisted settings.

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

`src/main.tsx` imports exactly one global entry point:

```text
src/ui/foundations/index.css
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

Generated token CSS remains generated source, not feature ownership. Shared and feature presentation uses CSS Modules.

### Final cascade

```css
@layer reset, tokens, theme, base, components, patterns, features, utilities;
```

There is no `legacy` layer and no global feature stylesheet import.

### Responsive ownership

- `LayoutModeProvider` owns application-wide wide/medium/compact shell behavior.
- Component-local adaptations use container queries when they depend on allocated space.
- Viewport media queries are limited to true platform/document concerns such as safe areas, virtual keyboards, forced colors, reduced motion, and coarse pointers.

### Rich content exception

ProseMirror/Tiptap-generated descendants are styled through a wrapper-scoped rich-content CSS Module. `:global(...)` is allowed only at a documented third-party markup boundary; editor chrome and rendered educational content remain separate owners.

### Enforcement

`scripts/style-ownership-baseline.json` is in `zero` mode. `scripts/verify-style-ownership.mjs` requires:

- no `src/styles/base` or `src/styles/features` CSS;
- exactly one global TypeScript CSS import (`src/main.tsx` → `./ui/foundations/index.css`);
- the exact final cascade layer order;
- no authored global CSS outside the foundation;
- no temporary style-policy allowances;
- no raw colors, arbitrary numeric stacking values, or literal motion durations in authored CSS outside generated token sources.

`scripts/verify-owned-class-usage.mjs` separately parses production TSX and verifies that semantic class keys resolve through explicit CSS Module owners; raw production class literals outside audited global utilities are rejected.

The fixed white QR quiet zone is represented by generated reference token `color.neutral.0`, whose source value is mechanically required to remain `#ffffff` by the device-sync QR contract.

### Standalone desktop qualification artifact

Native WebDriver acceptance and `run-app.sh` execute the same staged standalone Tauri executable. Cargo's mutable `target/debug/ostudy` path is not an acceptance artifact. See `docs/development/standalone-desktop-artifact.md`.

## Non-goals

- No visual redesign.
- No new CSS framework, preprocessor, CSS-in-JS system, utility framework, Storybook requirement, or global state library.
- No route, IPC, Rust, scheduling, card, document, study-guide, analytics, backup, or persisted-settings semantic change.
- No change to theme names or user-facing information architecture.
- No drag-only operation and no removal of keyboard/screen-reader alternatives.

## Acceptance criteria

- **AC-01:** Deck-library organization policy is pure, immutable, and tested.
- **AC-02:** Deck-library controller owns IPC mutations, revision recovery, optimistic state, toasts, and announcements.
- **AC-03:** Keyboard, HTML drag, pointer/touch, and fallback controls share one move intent and policy.
- **AC-04:** Deck-library views are presentation-oriented modules with co-located styles and accessibility parity.
- **AC-05:** Generated tokens are the theme authority and shell/platform foundations have one owner.
- **AC-06:** Shared controls, feedback, overlays, editor chrome, and rendered rich content do not depend on global legacy selectors.
- **AC-07:** Global feature stylesheets and `application.css` are deleted; the legacy layer and temporary compatibility attributes are gone.
- **AC-08:** Static enforcement prevents legacy topology, hidden global ownership, raw visual values, and unowned production class semantics from returning.
- **AC-09:** Existing frontend, native Linux/Tauri, Android package/API-36 emulator, accessibility, and IPC behavior remain compatible.

Final automated and human acceptance evidence is recorded on PR #76 against its exact final head; changing this document after that run would create a new, unqualified head.

## Consequences

- Feature changes are locally reviewable rather than depending on hidden global import order.
- All themes and material/motion modes share one semantic contract.
- Shared primitives/patterns own reusable behavior while feature modules own feature presentation.
- Static checks make reintroduction of the retired global CSS topology a build failure.
