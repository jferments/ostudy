# UI modernization implementation and validation

This document is the implementation ledger for phases U0–U9. It complements ADR 0034 and `docs/architecture/ui.md`.

## Invariants

- Desktop, tablet, and mobile use the same semantic components and domain workflows.
- Android safe-area, input-method, lifecycle, restoration, capability, file, share, and Back behavior remain owned by the platform boundaries introduced in PR #45.
- Feature code does not select raw theme colors, shadows, blur values, z-indexes, animation durations, or artwork file paths.
- Theme changes never alter information architecture or accessibility semantics.
- Reduced motion removes spatial motion, card flips, parallax, reorder interpolation, and particle celebrations.
- Simple materials and reduced transparency replace translucent surfaces with opaque equivalents.
- Dragging is never the only way to move or reorder content.
- Decorative artwork settings do not affect user-authored card images or educational document content.

## Phase ledger

### U0 — audit and decisions

- Record the visual-system, workspace, folder, help, artwork, and feedback decisions.
- Inventory legacy global style ownership.
- Add this validation ledger.

### U1 — tokens and appearance

- Add reference, component, and complete semantic theme token files.
- Generate CSS and typed token names deterministically.
- Resolve appearance from settings, media preferences, and platform capabilities.
- Migrate motion, material, artwork, and celebration settings.

### U2 — primitives and materials

- Add surfaces, layout primitives, progress, badges, chips, skeletons, presence, route transitions, ambient backdrop, noise, and card-stack material.
- Ensure full, reduced, and no-motion behavior.

### U3 — icons, artwork, help, and patterns

- Add an app-owned SVG icon registry.
- Add semantic artwork slots and fallbacks.
- Add tooltip, mobile help sheet, field help, page, command-bar, filter, split-pane, and adaptive-sheet patterns.

### U4 — shell

- Add ambient canvas, responsive brand components, compact app bar, icon navigation, and route-layer transitions.
- Preserve PR #45 mobile navigation, safe-area, input-method, and Back behavior.

### U5 — Flashcards workspace

- Rename the user-facing Decks section to Flashcards.
- Add route-backed Decks, Templates, Search, Import/Export, and Create Cards destinations.
- Extend device-local checkpoint restoration with stable workspace state.

### U6 — folders and ordering

- Add one-level deck folders, transactional membership and ordering, complete-library backup support, compact/comfortable views, drag enhancement, and menu/keyboard alternatives.

### U7 — workspace refinement

- Simplify Search through progressive disclosure and active-filter chips.
- Separate template library/editor presentation.
- Make single and bulk card creation first-class workspace modes.
- Group import, export, backup, restore, and platform sharing by task.

### U8 — practice and analytics

- Add visual card-stack depth, progress semantics, semantic micro-feedback, one-shot completion celebration, reusable metric presentation, and session analytics.

### U9 — documents, settings, help, and tutorial

- Migrate Documents and Appearance to shared patterns.
- Add contextual help and route-aware tutorial modules with stable anchors.

## Required local gates

Run from the repository root:

```bash
pnpm install --frozen-lockfile
pnpm check:static
pnpm check:fast
pnpm check
```

When Android tooling is installed:

```bash
pnpm android:doctor:emulator
pnpm android:test
```

## Manual matrix

Validate Light, Dark, Console, and Synthwave at wide, medium, and compact widths. At minimum, exercise:

- app shell and navigation;
- Flashcards workspace tabs and Back behavior;
- folder creation, movement, reordering, keyboard/menu alternatives, and persistence;
- compact and comfortable deck density;
- Search filters, selection mode, and mobile filter sheet;
- card and template durable drafts with the software keyboard visible;
- document empty, processing, success, error, retry, archive, and delete states;
- practice reveal, confidence commit, progress, card stack, completion, and restoration;
- artwork enabled and disabled;
- full, reduced, and disabled motion;
- rich and simple materials;
- 140% interface scale;
- keyboard-only operation and visible focus;
- Android safe areas, input-method displacement, overlay-first Back, process restoration, and incoming share flows.

## Evidence expectations

Pull-request notes must distinguish commands actually run from manual checks still required. A screenshot or green CI state is not evidence that every theme, platform, input mode, or accessibility preference was exercised.
