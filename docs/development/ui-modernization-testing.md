# UI modernization implementation and validation

This document is the implementation ledger for phases U0–U9. It complements ADR 0034, its partial supersession by ADR 0039, and `docs/architecture/ui.md`.

## Invariants

- Desktop, tablet, and mobile use the same semantic components and domain workflows.
- Android safe-area, input-method, lifecycle, restoration, capability, file, share, and Back behavior remain owned by the platform boundaries introduced in PR #45.
- Feature code does not select raw theme colors, shadows, blur values, z-indexes, animation durations, or artwork file paths.
- Theme changes never alter information architecture or accessibility semantics.
- Reduced motion removes spatial motion, card flips, parallax, reorder interpolation, and particle celebrations.
- Simple materials and reduced transparency replace translucent surfaces with opaque equivalents.
- Dragging is never the only way to move or reorder content.
- Decorative artwork settings do not affect user-authored card images or educational document content.
- The shell resolves compact, medium, and wide layout modes centrally; feature panels adapt to their own available width rather than inventing contradictory viewport breakpoints.
- Flashcards nested navigation is explicit route state, never inferred from DOM selectors.

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

- Add ambient canvas, responsive brand components, compact app bar, labeled medium navigation, wide navigation, and route-layer transitions.
- Preserve PR #45 mobile navigation, safe-area, input-method, and Back behavior.
- Resolve compact, medium, and wide layout modes once at the shared appearance/shell boundary.

### U5 — Flashcards workspace

- Rename the user-facing Decks section to Flashcards.
- Use route-backed Decks, Templates, Search, and Import/Export roots.
- Keep card authoring deck-scoped instead of maintaining a duplicate Create Cards destination.
- Extend device-local checkpoint restoration with stable workspace plus root/deck destination state.
- Normalize legacy Create Cards checkpoints to the Decks root.

### U6 — folders and ordering

- Add one-level deck folders, transactional membership and ordering, complete-library backup support, one compact deck presentation, drag enhancement, and menu/keyboard alternatives.
- Keep compact visual rows compatible with full keyboard and coarse-pointer targets.

### U7 — workspace refinement

- Simplify Search through progressive disclosure and active-filter chips.
- Separate template library/editor presentation.
- Put **Bulk add** and **Add flashcard** in the open deck's card-table toolbar.
- Group import, export, backup, restore, and platform sharing by task without decorative filler surfaces.
- Use compact `Q:`/`A:` previews in constrained card-list containers.

### U8 — practice and analytics

- Add visual card-stack depth, progress semantics, semantic micro-feedback, one-shot completion celebration, reusable metric presentation, and session analytics.

### U9 — documents, settings, help, and tutorial

- Migrate Documents and Appearance to shared patterns.
- Add contextual help and route-aware tutorial modules with stable anchors.

## Required local gates

Run from the repository root:

```bash
./scripts/check-changed.sh
./scripts/check-static.sh
./scripts/check-fast.sh
./scripts/check.sh
```

When Android tooling is installed, the authoritative full gate also performs the Android compile/package and emulator qualification described in `testing-and-ci.md`.

## Manual matrix

Validate Light, Dark, Console, and Synthwave at wide, medium, and compact widths. At minimum, exercise:

- app shell and navigation, including visible labels in the medium layout;
- the four Flashcards workspace roots, root reactivation, nested Decks state, and Back behavior;
- folder creation, movement, reordering, keyboard alternatives, and persistence;
- the single compact deck presentation with touch/coarse-pointer targets;
- Search filters, selection mode, and mobile filter sheet;
- deck-scoped **Add flashcard** and **Bulk add**, including durable-draft discard behavior;
- two-line `Q:`/`A:` card previews at compact widths;
- card and template durable drafts with the software keyboard visible;
- document empty, processing, success, error, retry, archive, and delete states;
- practice reveal, confidence commit, progress, card stack, completion, and restoration;
- artwork enabled and disabled;
- full, reduced, and disabled motion;
- rich and simple materials;
- 140% interface scale;
- keyboard-only operation and visible focus;
- Android safe areas, input-method displacement, overlay-first Back, process restoration, and incoming share flows.

Representative responsive widths include 390 × 844 phone, 600 × 960 small tablet, 768 × 1024 tablet, 1024 × 768 tablet landscape, 1280 × 800 compact desktop/tablet landscape, and 1440 × 900 desktop. Device-specific e-ink verification remains a manual complement to these deterministic width checks.

## Evidence expectations

Pull-request notes must distinguish commands actually run from manual checks still required. A screenshot or green CI state is not evidence that every theme, platform, input mode, accessibility preference, or physical tablet was exercised.
